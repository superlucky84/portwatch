//! 멈추기 실행기. 코어가 만든 계획을 실행하고 다시 수집해 결과를 정한다(docs/DESIGN.md "멈추기").

use std::time::Duration;

use portwatch_core::parse;
use portwatch_core::stop::{
    admin_script, admin_shell_command, observe, report_for, validate, Observed, Outcome,
    StopMethod, StopReport, SIGKILL, SIGTERM, USER_BREW_ENV,
};
use portwatch_core::{Entry, Snapshot};

use crate::{Environment, Runner};

pub const OSASCRIPT: &str = "/usr/bin/osascript";
const ENV: &str = "/usr/bin/env";
/// 다시 수집하는 간격과 횟수(최대 5초). 사라져 보여도 2초까지는 다시 뜨는지 본다.
const POLL: Duration = Duration::from_millis(500);
const POLLS: u32 = 10;
const SETTLE_POLLS: u32 = 4;

/// 신호 보내기. `target`이 음수면 프로세스 그룹이다.
pub trait Signaller: Sync {
    fn send(&self, target: i32, signal: i32) -> Result<(), String>;
}

extern "C" {
    fn kill(pid: i32, sig: i32) -> i32;
}

/// 시스템 호출로 신호를 보낸다. 셸을 거치지 않는다.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemSignaller;

impl Signaller for SystemSignaller {
    fn send(&self, target: i32, signal: i32) -> Result<(), String> {
        if matches!(target, -1..=1) {
            return Err("PID 0·1에는 신호를 보내지 않습니다.".to_owned());
        }
        // SAFETY: kill은 정수 두 개만 받고, 대상은 위에서 0·1·-1을 막았다.
        if unsafe { kill(target, signal) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error().to_string())
        }
    }
}

/// 멈추기에 쓰는 실행 도구.
pub struct StopTools<'a> {
    /// `ps`·brew·docker. 멈추기 명령은 끝날 때까지 기다려야 해서 제한 시간이 수집보다 길다.
    pub runner: &'a dyn Runner,
    /// osascript. 사용자가 비밀번호를 넣는 동안 기다린다.
    pub admin: &'a dyn Runner,
    pub signaller: &'a dyn Signaller,
    pub env: &'a Environment,
}

fn failed(message: impl Into<String>, command: Option<String>) -> StopReport {
    let mut report = StopReport::new(Outcome::Failed, message);
    report.command = command;
    report
}

fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("")
}

/// 실행 직전에 최상위 프로세스의 PID·시작 시각·명령줄이 스냅샷과 같은지 본다(PID 재사용 방지).
pub fn same_process(runner: &dyn Runner, entry: &Entry) -> bool {
    let Some(top) = entry.processes.first() else {
        return false;
    };
    let pid = top.pid.to_string();
    let args = [
        "-ww",
        "-o",
        "pid=,ppid=,pgid=,uid=,tdev=,user=,lstart=,command=",
        "-p",
        pid.as_str(),
    ];
    let Ok(output) = runner.run(crate::PS.0, &args) else {
        return false;
    };
    let (rows, _) = parse::ps_rows(&output.stdout);
    rows.iter().any(|row| {
        row.pid == top.pid && row.started_at == top.started_at && row.command == top.command
    })
}

/// 계획을 실행한다. 결과 판정 전의 실패·취소는 바로 돌려준다.
fn execute(tools: &StopTools, method: &StopMethod, force: bool) -> Result<(), StopReport> {
    match method {
        StopMethod::Signal { pid, group } => {
            let pid = *pid as i32;
            let target = if *group { -pid } else { pid };
            let signal = if force { SIGKILL } else { SIGTERM };
            let name = if force { "KILL" } else { "TERM" };
            tools.signaller.send(target, signal).map_err(|error| {
                failed(
                    format!("신호를 보내지 못했습니다: {error}"),
                    Some(format!("kill -{name} {target}")),
                )
            })
        }
        StopMethod::BrewServices {
            brew,
            formula,
            domain,
        } if domain == "system" => {
            let command = admin_shell_command(method, &tools.env.home)
                .ok_or_else(|| failed("관리자 명령을 만들 수 없습니다.", None))?;
            let script = admin_script(&command)
                .ok_or_else(|| failed("관리자 명령을 만들 수 없습니다.", None))?;
            let manual = Some(format!("sudo {brew} services stop {formula}"));
            match tools.admin.run(OSASCRIPT, &["-e", script.as_str()]) {
                Ok(output) if output.success => Ok(()),
                // 비밀번호 창에서 취소하면 osascript가 -128 오류로 끝난다.
                Ok(output) if output.stderr.contains("-128") => {
                    Err(StopReport::new(Outcome::Cancelled, "취소됨"))
                }
                Ok(output) => Err(failed(
                    format!("멈추지 못했습니다. {}", first_line(&output.stderr)),
                    manual,
                )),
                Err(error) => Err(failed(format!("멈추지 못했습니다. {error}"), manual)),
            }
        }
        StopMethod::BrewServices { brew, formula, .. } => {
            let home = format!("HOME={}", tools.env.home);
            let user = format!("USER={}", tools.env.user);
            let logname = format!("LOGNAME={}", tools.env.user);
            let mut args = vec![home.as_str(), user.as_str(), logname.as_str()];
            args.extend(USER_BREW_ENV);
            args.extend([brew.as_str(), "services", "stop", formula.as_str()]);
            run_checked(
                tools.runner,
                ENV,
                &args,
                format!("{brew} services stop {formula}"),
            )
        }
        StopMethod::DockerStop {
            docker,
            socket,
            container_id,
        } => run_checked(
            tools.runner,
            docker,
            &["--host", socket, "stop", container_id],
            format!("{docker} --host {socket} stop {container_id}"),
        ),
    }
}

fn run_checked(
    runner: &dyn Runner,
    program: &str,
    args: &[&str],
    manual: String,
) -> Result<(), StopReport> {
    match runner.run(program, args) {
        Ok(output) if output.success => Ok(()),
        Ok(output) => Err(failed(
            format!("멈추지 못했습니다. {}", first_line(&output.stderr))
                .trim()
                .to_owned(),
            Some(manual),
        )),
        Err(error) => Err(failed(format!("멈추지 못했습니다. {error}"), Some(manual))),
    }
}

/// `before`의 항목 `id`를 멈춘다. `force`는 신호 계획에만 SIGKILL을 보낸다.
/// `recollect`는 다시 수집한 스냅샷을 돌려주고, `pause`는 다시 수집하기 전에 기다린다.
pub fn stop_entry(
    tools: &StopTools,
    before: &Snapshot,
    id: &str,
    force: bool,
    recollect: &mut dyn FnMut() -> Snapshot,
    pause: &dyn Fn(Duration),
) -> StopReport {
    let Some(entry) = before.entries.iter().find(|entry| entry.id == id) else {
        return StopReport::new(Outcome::Changed, "목록이 바뀌었습니다. 다시 확인해 주세요.");
    };
    let Some(plan) = &entry.stop_plan else {
        let reason = entry
            .protection
            .as_ref()
            .map_or("멈추는 방법이 없는 항목입니다.", |p| {
                p.reason.as_str()
            });
        return failed(reason, None);
    };
    if force && !matches!(plan.method, StopMethod::Signal { .. }) {
        return failed("강제 종료는 신호로 멈추는 항목에만 쓸 수 있습니다.", None);
    }
    if let Err(error) = validate(&plan.method) {
        return failed(error, None);
    }
    if !same_process(tools.runner, entry) {
        return StopReport::new(
            Outcome::Changed,
            "프로세스가 바뀌어 멈추지 않았습니다. 목록을 다시 확인해 주세요.",
        );
    }
    if let Err(report) = execute(tools, &plan.method, force) {
        return report;
    }

    let mut after = before.clone();
    let mut observed = Observed::StillRunning;
    for poll in 1..=POLLS {
        pause(POLL);
        after = recollect();
        observed = observe(before, id, &after);
        if observed != Observed::StillRunning && poll >= SETTLE_POLLS {
            break;
        }
    }
    report_for(observed, &after, entry)
}

/// 실제 명령으로 멈춘다. 다시 수집할 때마다 그 스냅샷을 `on_snapshot`에 넘긴다(화면 갱신용).
pub fn stop_on_this_mac(
    before: &Snapshot,
    id: &str,
    force: bool,
    security_names: Vec<String>,
    on_snapshot: &mut dyn FnMut(&Snapshot),
) -> StopReport {
    if !cfg!(target_os = "macos") {
        return failed("macOS가 아니어서 멈추지 않았습니다.", None);
    }
    let runner = crate::SystemRunner {
        timeout: Duration::from_secs(30),
    };
    let admin = crate::SystemRunner {
        timeout: Duration::from_secs(120),
    };
    let env = Environment::current();
    let tools = StopTools {
        runner: &runner,
        admin: &admin,
        signaller: &SystemSignaller,
        env: &env,
    };
    let mut recollect = || {
        let snapshot = crate::collect_snapshot_using(security_names.clone());
        on_snapshot(&snapshot);
        snapshot
    };
    stop_entry(
        &tools,
        before,
        id,
        force,
        &mut recollect,
        &std::thread::sleep,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::env;
    use crate::CommandOutput;
    use portwatch_core::stop::{plan, DockerHint, PlanHints};
    use portwatch_core::{Category, Listener, Manager, ManagerKind, ProcessInfo};
    use std::sync::Mutex;

    const STARTED: &str = "Wed Nov 18 09:11:00 2026";

    /// 명령을 기록하고 정해진 답을 돌려주는 실행기.
    struct Scripted {
        calls: Mutex<Vec<Vec<String>>>,
        ps: String,
        reply: CommandOutput,
    }

    impl Scripted {
        fn new(ps: &str, reply: CommandOutput) -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                ps: ps.to_owned(),
                reply,
            }
        }
        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl Runner for Scripted {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
            let mut call = vec![program.to_owned()];
            call.extend(args.iter().map(|a| (*a).to_owned()));
            self.calls.lock().unwrap().push(call);
            if program == "/bin/ps" {
                return Ok(CommandOutput {
                    success: true,
                    stdout: self.ps.clone(),
                    stderr: String::new(),
                });
            }
            Ok(self.reply.clone())
        }
    }

    #[derive(Default)]
    struct Recorder(Mutex<Vec<(i32, i32)>>);

    impl Signaller for Recorder {
        fn send(&self, target: i32, signal: i32) -> Result<(), String> {
            self.0.lock().unwrap().push((target, signal));
            Ok(())
        }
    }

    fn ok() -> CommandOutput {
        CommandOutput {
            success: true,
            ..CommandOutput::default()
        }
    }

    fn ps_line(pid: u32, command: &str) -> String {
        format!("{pid:>5}  2001  {pid}   501 ttys001  dev      {STARTED} {command}\n")
    }

    fn entry(kind: ManagerKind, hints: PlanHints) -> Entry {
        let mut entry = Entry {
            id: "terminal:2100:x".into(),
            key: "project:/Users/dev/work/shop-web".into(),
            title: "shop-web".into(),
            category: Category::DevServer,
            manager: Manager {
                kind,
                domain: None,
                label: None,
                formula: None,
                container: None,
                app_name: None,
                runs_as_root: false,
            },
            listeners: vec![Listener {
                port: 5173,
                address: "*".into(),
                family: "tcp4".into(),
                pid: 2100,
            }],
            processes: vec![ProcessInfo {
                pid: 2100,
                ppid: 2001,
                pgid: 2100,
                uid: 501,
                user: "dev".into(),
                tty: "ttys001".into(),
                started_at: STARTED.into(),
                command: "npm run dev".into(),
            }],
            project: None,
            domains: Vec::new(),
            protection: None,
            stop_plan: None,
            hidden: false,
            pinned: false,
        };
        if kind == ManagerKind::LaunchdBrew {
            entry.manager.formula = Some("nginx".into());
        }
        entry.stop_plan = plan(&entry, &hints);
        entry
    }

    fn brew_entry(domain: &str) -> Entry {
        let mut entry = Entry {
            manager: Manager {
                domain: Some(domain.into()),
                formula: Some("nginx".into()),
                ..entry(ManagerKind::LaunchdBrew, PlanHints::default()).manager
            },
            ..entry(ManagerKind::LaunchdBrew, PlanHints::default())
        };
        entry.stop_plan = plan(
            &entry,
            &PlanHints {
                service_executable: Some("/opt/homebrew/opt/nginx/bin/nginx".into()),
                docker: None,
            },
        );
        entry
    }

    fn snapshot(entries: Vec<Entry>) -> Snapshot {
        Snapshot {
            taken_at_ms: 0,
            collect_ms: 0,
            entries,
            warnings: Vec::new(),
        }
    }

    fn run(
        runner: &Scripted,
        admin: &Scripted,
        signaller: &Recorder,
        target: &Entry,
        force: bool,
        afters: Vec<Snapshot>,
    ) -> (StopReport, usize) {
        let env = env();
        let tools = StopTools {
            runner,
            admin,
            signaller,
            env: &env,
        };
        let before = snapshot(vec![target.clone()]);
        let mut afters = afters.into_iter();
        let mut last = snapshot(vec![]);
        let mut polls = 0;
        let mut recollect = || {
            polls += 1;
            if let Some(next) = afters.next() {
                last = next;
            }
            last.clone()
        };
        let report = stop_entry(&tools, &before, &target.id, force, &mut recollect, &|_| {});
        (report, polls)
    }

    #[test]
    fn terminal_group_gets_sigterm_and_waits_before_reporting_stopped() {
        let target = entry(ManagerKind::Terminal, PlanHints::default());
        let runner = Scripted::new(&ps_line(2100, "npm run dev"), ok());
        let signaller = Recorder::default();
        let (report, polls) = run(
            &runner,
            &runner,
            &signaller,
            &target,
            false,
            vec![snapshot(vec![])],
        );
        assert_eq!(report.outcome, Outcome::Stopped);
        assert_eq!(polls, 4, "사라져도 2초까지는 다시 본다");
        assert_eq!(*signaller.0.lock().unwrap(), [(-2100, SIGTERM)]);
        assert_eq!(
            runner.calls()[0],
            [
                "/bin/ps",
                "-ww",
                "-o",
                "pid=,ppid=,pgid=,uid=,tdev=,user=,lstart=,command=",
                "-p",
                "2100"
            ]
        );
    }

    #[test]
    fn a_different_process_on_the_same_pid_is_not_signalled() {
        let target = entry(ManagerKind::Terminal, PlanHints::default());
        let runner = Scripted::new(&ps_line(2100, "node other.js"), ok());
        let signaller = Recorder::default();
        let (report, polls) = run(&runner, &runner, &signaller, &target, false, vec![]);
        assert_eq!(report.outcome, Outcome::Changed);
        assert_eq!(polls, 0);
        assert!(signaller.0.lock().unwrap().is_empty());
    }

    #[test]
    fn still_running_offers_force_and_force_sends_sigkill() {
        let target = entry(ManagerKind::Terminal, PlanHints::default());
        let runner = Scripted::new(&ps_line(2100, "npm run dev"), ok());
        let signaller = Recorder::default();
        let (report, polls) = run(
            &runner,
            &runner,
            &signaller,
            &target,
            false,
            vec![snapshot(vec![target.clone()])],
        );
        assert_eq!(report.outcome, Outcome::StillRunning);
        assert!(report.can_force);
        assert_eq!(polls, 10, "최대 5초");

        let (report, _) = run(
            &runner,
            &runner,
            &signaller,
            &target,
            true,
            vec![snapshot(vec![])],
        );
        assert_eq!(report.outcome, Outcome::Stopped);
        assert_eq!(signaller.0.lock().unwrap()[1], (-2100, SIGKILL));
    }

    #[test]
    fn user_brew_service_runs_brew_with_home_and_no_auto_update() {
        let target = brew_entry("user");
        let runner = Scripted::new(&ps_line(2100, "npm run dev"), ok());
        let signaller = Recorder::default();
        let (report, _) = run(
            &runner,
            &runner,
            &signaller,
            &target,
            false,
            vec![snapshot(vec![])],
        );
        assert_eq!(report.outcome, Outcome::Stopped);
        assert_eq!(
            runner.calls()[1],
            [
                "/usr/bin/env",
                "HOME=/Users/dev",
                "USER=dev",
                "LOGNAME=dev",
                "HOMEBREW_NO_AUTO_UPDATE=1",
                "HOMEBREW_NO_ANALYTICS=1",
                "HOMEBREW_NO_ENV_HINTS=1",
                "/opt/homebrew/bin/brew",
                "services",
                "stop",
                "nginx"
            ]
        );
        assert!(
            signaller.0.lock().unwrap().is_empty(),
            "launchd 서비스는 kill 하지 않는다"
        );
    }

    #[test]
    fn a_process_that_already_ended_is_not_signalled() {
        let target = entry(ManagerKind::Terminal, PlanHints::default());
        let runner = Scripted::new("", ok());
        let signaller = Recorder::default();
        let (report, _) = run(&runner, &runner, &signaller, &target, false, vec![]);
        assert_eq!(report.outcome, Outcome::Changed);
        assert!(signaller.0.lock().unwrap().is_empty());
    }

    #[test]
    fn missing_brew_is_reported_with_the_manual_command() {
        let target = brew_entry("user");
        let runner = Scripted::new(
            &ps_line(2100, "npm run dev"),
            CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "env: /opt/homebrew/bin/brew: No such file or directory\n".into(),
            },
        );
        let signaller = Recorder::default();
        let (report, _) = run(&runner, &runner, &signaller, &target, false, vec![]);
        assert_eq!(report.outcome, Outcome::Failed);
        assert_eq!(
            report.message,
            "멈추지 못했습니다. env: /opt/homebrew/bin/brew: No such file or directory"
        );
        assert_eq!(
            report.command.as_deref(),
            Some("/opt/homebrew/bin/brew services stop nginx")
        );
    }

    #[test]
    fn root_brew_service_uses_the_password_dialog_and_reports_cancel() {
        let target = brew_entry("system");
        let runner = Scripted::new(&ps_line(2100, "npm run dev"), ok());
        let cancelled = Scripted::new(
            "",
            CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "execution error: User canceled. (-128)\n".into(),
            },
        );
        let signaller = Recorder::default();
        let (report, polls) = run(&runner, &cancelled, &signaller, &target, false, vec![]);
        assert_eq!(report.outcome, Outcome::Cancelled);
        assert_eq!(report.message, "취소됨");
        assert_eq!(polls, 0);
        assert_eq!(
            cancelled.calls()[0],
            [
                "/usr/bin/osascript",
                "-e",
                "do shell script \"/usr/bin/env HOME=/var/root HOMEBREW_NO_AUTO_UPDATE=1 \
                 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 \
                 HOMEBREW_CACHE='/Users/dev/Library/Caches/Homebrew' /opt/homebrew/bin/brew services \
                 stop nginx\" with administrator privileges"
            ]
        );

        let broken = Scripted::new(
            "",
            CommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "Error: permission denied\n".into(),
            },
        );
        let (report, _) = run(&runner, &broken, &signaller, &target, false, vec![]);
        assert_eq!(report.outcome, Outcome::Failed);
        assert_eq!(
            report.command.as_deref(),
            Some("sudo /opt/homebrew/bin/brew services stop nginx")
        );

        let (report, _) = run(&runner, &broken, &signaller, &target, true, vec![]);
        assert_eq!(
            report.outcome,
            Outcome::Failed,
            "brew 서비스는 강제 종료하지 않는다"
        );
    }

    #[test]
    fn docker_container_is_stopped_by_id_on_the_desktop_socket() {
        let mut target = entry(ManagerKind::Docker, PlanHints::default());
        target.manager.container = Some("shop-db".into());
        target.stop_plan = plan(
            &target,
            &PlanHints {
                service_executable: None,
                docker: Some(DockerHint {
                    cli: "/Applications/Docker.app/Contents/Resources/bin/docker".into(),
                    socket: "unix:///Users/dev/.docker/run/docker.sock".into(),
                    container_id: "5d1c0e2f3a4b".into(),
                }),
            },
        );
        let runner = Scripted::new(&ps_line(2100, "npm run dev"), ok());
        let signaller = Recorder::default();
        let (report, _) = run(
            &runner,
            &runner,
            &signaller,
            &target,
            false,
            vec![snapshot(vec![])],
        );
        assert_eq!(report.outcome, Outcome::Stopped);
        assert_eq!(
            runner.calls()[1],
            [
                "/Applications/Docker.app/Contents/Resources/bin/docker",
                "--host",
                "unix:///Users/dev/.docker/run/docker.sock",
                "stop",
                "5d1c0e2f3a4b"
            ]
        );
    }

    #[test]
    fn entries_without_a_plan_are_not_touched() {
        let target = entry(ManagerKind::Pm2, PlanHints::default());
        let runner = Scripted::new("", ok());
        let signaller = Recorder::default();
        let (report, _) = run(&runner, &runner, &signaller, &target, false, vec![]);
        assert_eq!(report.outcome, Outcome::Failed);
        assert!(runner.calls().is_empty());
    }

    /// 실제 프로세스 그룹: ps로 다시 확인하고 그룹 전체에 SIGTERM을 보낸다.
    #[test]
    fn real_process_group_is_rechecked_and_terminated() {
        use std::os::unix::process::{CommandExt, ExitStatusExt};

        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "sleep 30 & sleep 30; wait"])
            .process_group(0)
            .spawn()
            .expect("시험 프로세스");
        let pid = child.id();
        std::thread::sleep(Duration::from_millis(200));

        let runner = crate::SystemRunner::default();
        let ps = runner
            .run(
                crate::PS.0,
                &[
                    "-ww",
                    "-o",
                    "pid=,ppid=,pgid=,uid=,tdev=,user=,lstart=,command=",
                    "-p",
                    &pid.to_string(),
                ],
            )
            .unwrap();
        let (rows, _) = parse::ps_rows(&ps.stdout);
        let row = rows.first().expect("ps 행");
        let mut target = entry(ManagerKind::Terminal, PlanHints::default());
        target.processes[0] = ProcessInfo {
            pid,
            ppid: row.ppid,
            pgid: row.pgid,
            uid: row.uid,
            user: row.user.clone(),
            tty: row.tty.clone(),
            started_at: row.started_at.clone(),
            command: row.command.clone(),
        };
        assert!(same_process(&runner, &target));
        let mut stale = target.clone();
        stale.processes[0].started_at = "Mon Jan  1 00:00:00 2024".into();
        assert!(
            !same_process(&runner, &stale),
            "시작 시각이 다르면 다른 프로세스"
        );

        SystemSignaller
            .send(-(row.pgid as i32), SIGTERM)
            .expect("그룹에 신호");
        let status = child.wait().unwrap();
        assert_eq!(status.signal(), Some(SIGTERM));
        assert!(SystemSignaller.send(1, SIGTERM).is_err());
    }
}

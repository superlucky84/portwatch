//! portwatch 수집기.
//!
//! macOS 명령을 실행해 원문을 모으고, 코어로 스냅샷을 만든다. 명령 실행([`Runner`])과
//! 파일 확인([`Files`])을 바꿔 끼울 수 있어서 합성 자료로 수집 흐름 전체를 테스트한다.

pub mod raw;
pub mod report;
pub mod stop;
mod system;

use std::collections::HashMap;

use portwatch_core::parse;
use portwatch_core::project::{candidate_dirs, dir_name, package_name, PROJECT_MARKERS};
use portwatch_core::resolve::{
    build_snapshot, plan_lookups, Facts, ServiceTarget, DEFAULT_SECURITY_NAMES,
};
use portwatch_core::{Project, Snapshot};

pub use system::{RealFiles, SystemRunner};

pub const NETSTAT: (&str, &[&str]) = ("/usr/sbin/netstat", &["-anv", "-p", "tcp"]);
pub const PS: (&str, &[&str]) = (
    "/bin/ps",
    &[
        "-ww",
        "-axo",
        "pid=,ppid=,pgid=,uid=,tty=,user=,lstart=,command=",
    ],
);
pub const LAUNCHCTL: &str = "/bin/launchctl";
pub const LSOF: &str = "/usr/sbin/lsof";

/// 명령 실행 결과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

/// 명령 실행기. 실행 자체를 못 하거나 시간을 넘기면 `Err`.
pub trait Runner: Sync {
    fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String>;
}

/// 프로젝트 판별용 파일 확인.
pub trait Files {
    fn exists(&self, path: &str) -> bool;
    fn read(&self, path: &str) -> Option<String>;
}

/// 수집하는 사용자와 시각.
#[derive(Debug, Clone)]
pub struct Environment {
    pub home: String,
    pub user: String,
    pub uid: u32,
    pub self_pid: u32,
    pub now_ms: u64,
    pub security_names: Vec<String>,
}

impl Environment {
    /// 현재 프로세스 기준 환경.
    pub fn current() -> Self {
        system::current_environment()
    }

    pub fn default_security_names() -> Vec<String> {
        DEFAULT_SECURITY_NAMES
            .iter()
            .map(|s| (*s).to_owned())
            .collect()
    }
}

/// 첫 수집에서 읽은 원문. `--raw` 출력에도 쓴다.
#[derive(Debug, Clone)]
pub struct FirstPass {
    pub netstat: Result<CommandOutput, String>,
    pub ps: Result<CommandOutput, String>,
    pub system: Result<CommandOutput, String>,
    pub gui: Result<CommandOutput, String>,
}

/// 네 명령을 함께 실행한다.
pub fn first_pass(runner: &dyn Runner, env: &Environment) -> FirstPass {
    let gui = format!("gui/{}", env.uid);
    std::thread::scope(|scope| {
        let netstat = scope.spawn(|| runner.run(NETSTAT.0, NETSTAT.1));
        let ps = scope.spawn(|| runner.run(PS.0, PS.1));
        let system = scope.spawn(|| runner.run(LAUNCHCTL, &["print", "system"]));
        let gui = scope.spawn(|| runner.run(LAUNCHCTL, &["print", gui.as_str()]));
        let join = |handle: std::thread::ScopedJoinHandle<'_, Result<CommandOutput, String>>| {
            handle
                .join()
                .unwrap_or_else(|_| Err("명령 실행 스레드가 멈췄습니다.".to_owned()))
        };
        FirstPass {
            netstat: join(netstat),
            ps: join(ps),
            system: join(system),
            gui: join(gui),
        }
    })
}

/// 명령 결과의 표준 출력. 실패면 경고를 남기고 없음.
fn stdout_or_warn(
    result: &Result<CommandOutput, String>,
    what: &str,
    warnings: &mut Vec<String>,
) -> Option<String> {
    match result {
        Ok(output) if output.success => Some(output.stdout.clone()),
        Ok(output) => {
            let detail = output.stderr.lines().next().unwrap_or("").trim();
            warnings.push(
                format!("{what}을(를) 읽지 못했습니다. {detail}")
                    .trim()
                    .to_owned(),
            );
            None
        }
        Err(error) => {
            warnings.push(format!("{what}을(를) 읽지 못했습니다. {error}"));
            None
        }
    }
}

/// 첫 수집 결과로 사실을 만든다(추가 조회 전).
pub fn facts_from_first_pass(pass: &FirstPass, env: &Environment) -> Facts {
    let mut warnings = Vec::new();
    let listeners = match stdout_or_warn(&pass.netstat, "포트 목록(netstat)", &mut warnings) {
        Some(text) => parse::netstat_listeners(&text).unwrap_or_else(|error| {
            warnings.push(error);
            Vec::new()
        }),
        None => Vec::new(),
    };
    let processes = match stdout_or_warn(&pass.ps, "프로세스 목록(ps)", &mut warnings) {
        Some(text) => {
            let (rows, skipped) = parse::ps_rows(&text);
            if skipped > 0 {
                warnings.push(format!("ps 출력 {skipped}줄을 읽지 못했습니다."));
            }
            rows
        }
        None => Vec::new(),
    };
    let system_jobs = stdout_or_warn(&pass.system, "root 서비스 목록(launchctl)", &mut warnings)
        .map(|text| parse::launchctl_services(&text))
        .unwrap_or_default();
    let user_jobs = stdout_or_warn(&pass.gui, "사용자 서비스 목록(launchctl)", &mut warnings)
        .map(|text| parse::launchctl_services(&text))
        .unwrap_or_default();

    Facts {
        taken_at_ms: env.now_ms,
        listeners,
        processes,
        system_jobs,
        user_jobs,
        home: env.home.clone(),
        self_pid: env.self_pid,
        current_uid: env.uid,
        security_names: env.security_names.clone(),
        warnings,
        ..Facts::default()
    }
}

/// `launchctl print <대상>` 인수.
pub fn detail_args(target: &ServiceTarget, uid: u32) -> [String; 2] {
    ["print".to_owned(), target.launchctl_target(uid)]
}

/// `docker ps` 인수. 소켓을 Docker Desktop 것으로 고정해 원격 context로 나가지 않게 한다(docs/DESIGN.md "수집").
pub fn docker_ps_args(home: &str) -> Vec<String> {
    [
        "--host",
        portwatch_core::stop::docker_socket(home).as_str(),
        "ps",
        "--no-trunc",
        "--format",
        "{{json .}}",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect()
}

/// `lsof` 인수.
pub fn lsof_args(pids: &[u32]) -> Vec<String> {
    let list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    ["-a", "-p", list.as_str(), "-d", "cwd", "-Fn"]
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

fn run_owned(runner: &dyn Runner, program: &str, args: &[String]) -> Result<CommandOutput, String> {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    runner.run(program, &args)
}

/// 작업 폴더에서 프로젝트를 찾는다.
pub fn find_project(files: &dyn Files, cwd: &str, home: &str) -> Option<Project> {
    candidate_dirs(cwd, home).into_iter().find_map(|dir| {
        let has_marker = PROJECT_MARKERS
            .iter()
            .any(|marker| files.exists(&format!("{dir}/{marker}")));
        has_marker.then(|| Project {
            name: files
                .read(&format!("{dir}/package.json"))
                .and_then(|text| package_name(&text))
                .unwrap_or_else(|| dir_name(&dir)),
            path: dir.clone(),
        })
    })
}

/// 수집 전체를 실행해 사실을 만든다.
pub fn collect_facts(runner: &dyn Runner, files: &dyn Files, env: &Environment) -> Facts {
    let pass = first_pass(runner, env);
    let mut facts = facts_from_first_pass(&pass, env);
    let lookups = plan_lookups(&facts);

    let mut failed = Vec::new();
    for target in &lookups.services {
        match run_owned(runner, LAUNCHCTL, &detail_args(target, env.uid)) {
            Ok(output) if output.success => {
                facts
                    .service_details
                    .insert(target.clone(), parse::launchctl_detail(&output.stdout));
            }
            _ => failed.push(target.label.clone()),
        }
    }
    if !failed.is_empty() {
        facts.warnings.push(format!(
            "서비스 상세 정보를 읽지 못했습니다: {}",
            failed.join(", ")
        ));
    }

    if !lookups.cwd_pids.is_empty() {
        // lsof는 일부 PID가 이미 끝났으면 실패 코드를 돌려주지만 나머지 결과는 쓸 수 있다.
        match run_owned(runner, LSOF, &lsof_args(&lookups.cwd_pids)) {
            Ok(output) => facts.cwd = parse::lsof_cwd(&output.stdout),
            Err(error) => facts
                .warnings
                .push(format!("작업 폴더(lsof)를 읽지 못했습니다. {error}")),
        }
    }

    if let Some(docker) = &lookups.docker_cli {
        match run_owned(runner, docker, &docker_ps_args(&env.home)) {
            Ok(output) if output.success => {
                let (containers, skipped) = parse::docker_ps(&output.stdout);
                if skipped > 0 {
                    facts
                        .warnings
                        .push(format!("docker ps 출력 {skipped}줄을 읽지 못했습니다."));
                }
                facts.containers = containers;
            }
            result => {
                let detail = match result {
                    Ok(output) => output.stderr.lines().next().unwrap_or("").trim().to_owned(),
                    Err(error) => error,
                };
                facts.warnings.push(
                    format!("Docker 컨테이너 목록을 읽지 못했습니다. {detail}")
                        .trim()
                        .to_owned(),
                );
            }
        }
    }

    let mut projects: HashMap<String, Project> = HashMap::new();
    for cwd in facts.cwd.values() {
        if projects.contains_key(cwd) {
            continue;
        }
        if let Some(project) = find_project(files, cwd, &env.home) {
            projects.insert(cwd.clone(), project);
        }
    }
    facts.projects = projects;
    facts
}

/// 바꿔 끼운 실행기·파일로 스냅샷을 만든다. 걸린 시간을 `collect_ms`에 담는다.
pub fn collect_with(runner: &dyn Runner, files: &dyn Files, env: &Environment) -> Snapshot {
    let started = std::time::Instant::now();
    let mut snapshot = build_snapshot(&collect_facts(runner, files, env));
    snapshot.collect_ms = started.elapsed().as_millis() as u64;
    snapshot
}

/// 기본 보안 프로그램 목록으로 현재 상태의 스냅샷을 만든다.
pub fn collect_snapshot() -> Snapshot {
    collect_snapshot_using(Environment::default_security_names())
}

/// 주어진 보안 프로그램 목록으로 현재 상태의 스냅샷을 만든다. macOS가 아니면 경고만 담는다.
pub fn collect_snapshot_using(security_names: Vec<String>) -> Snapshot {
    let mut env = Environment::current();
    env.security_names = security_names;
    if !cfg!(target_os = "macos") {
        let mut snapshot = Snapshot::empty(env.now_ms);
        snapshot
            .warnings
            .push("macOS가 아니어서 수집하지 않았습니다.".to_owned());
        return snapshot;
    }
    collect_with(&SystemRunner::default(), &RealFiles, &env)
}

#[cfg(test)]
pub(crate) mod fake {
    use super::*;

    pub const DIR: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../portwatch-core/tests/fixtures/mac-15"
    );

    pub fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{DIR}/{name}")).expect("합성 자료 파일")
    }

    /// 합성 자료로 답하는 실행기. 받은 명령을 기록한다.
    #[derive(Default)]
    pub struct FakeRunner {
        pub calls: std::sync::Mutex<Vec<String>>,
        pub fail: Vec<&'static str>,
    }

    impl Runner for FakeRunner {
        fn run(&self, program: &str, args: &[&str]) -> Result<CommandOutput, String> {
            let call = format!("{program} {}", args.join(" "));
            self.calls.lock().unwrap().push(call.clone());
            if self.fail.iter().any(|needle| call.contains(needle)) {
                return Ok(CommandOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: "Could not find service".into(),
                });
            }
            let file = match (program, args) {
                ("/usr/sbin/netstat", _) => "netstat.txt".to_owned(),
                ("/bin/ps", _) => "ps.txt".to_owned(),
                ("/bin/launchctl", ["print", "system"]) => "launchctl-system.txt".to_owned(),
                ("/bin/launchctl", ["print", "gui/501"]) => "launchctl-gui.txt".to_owned(),
                ("/bin/launchctl", ["print", target]) => {
                    let (domain, label) = target.rsplit_once('/').unwrap();
                    let domain = if domain == "system" { "system" } else { "gui" };
                    format!("service-{domain}-{label}.txt")
                }
                ("/usr/sbin/lsof", _) => "lsof-cwd.txt".to_owned(),
                ("/Applications/Docker.app/Contents/Resources/bin/docker", _) => {
                    "docker-ps.txt".to_owned()
                }
                _ => return Err(format!("알 수 없는 명령: {call}")),
            };
            Ok(CommandOutput {
                success: true,
                stdout: fixture(&file),
                stderr: String::new(),
            })
        }
    }

    /// 경로 목록으로 답하는 파일 확인.
    pub struct FakeFiles(pub HashMap<&'static str, &'static str>);

    impl Files for FakeFiles {
        fn exists(&self, path: &str) -> bool {
            self.0.contains_key(path)
        }
        fn read(&self, path: &str) -> Option<String> {
            self.0.get(path).map(|text| (*text).to_owned())
        }
    }

    pub fn files() -> FakeFiles {
        FakeFiles(HashMap::from([
            (
                "/Users/dev/work/shop-web/package.json",
                r#"{"name":"shop-web"}"#,
            ),
            (
                "/Users/dev/work/api/package.json",
                r#"{"name":"api-server"}"#,
            ),
            ("/Users/dev/work/worker/.git", ""),
            ("/Users/dev/work/queue/package.json", r#"{"private":true}"#),
            ("/Users/dev/.git", "홈 폴더는 프로젝트로 보지 않는다"),
        ]))
    }

    pub fn env() -> Environment {
        Environment {
            home: "/Users/dev".into(),
            user: "dev".into(),
            uid: 501,
            self_pid: 3000,
            now_ms: 1_795_000_000_000,
            security_names: Environment::default_security_names(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fake::*;
    use super::*;
    use portwatch_core::{Category, ManagerKind};

    #[test]
    fn collects_the_fixture_mac_end_to_end() {
        let runner = FakeRunner::default();
        let snapshot = collect_with(&runner, &files(), &env());

        assert!(snapshot.warnings.is_empty(), "{:?}", snapshot.warnings);
        assert_eq!(snapshot.entries.len(), 19);
        let titles: Vec<&str> = snapshot.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            &titles[..10],
            [
                "api-server",
                "node",
                "worker",
                "queue",
                "shop-web",
                "shop-db",
                "Python",
                "mock-api",
                "redis",
                "nginx"
            ]
        );

        let nginx = snapshot
            .entries
            .iter()
            .find(|e| e.title == "nginx")
            .unwrap();
        assert_eq!(nginx.manager.kind, ManagerKind::LaunchdBrew);
        assert!(nginx.manager.runs_as_root);

        let python = snapshot
            .entries
            .iter()
            .find(|e| e.title == "Python")
            .unwrap();
        assert_eq!(python.project, None, "홈 폴더의 .git은 프로젝트가 아니다");
        assert_eq!(python.category, Category::DevServer);

        let calls = runner.calls.lock().unwrap();
        assert!(calls.contains(
            &"/usr/sbin/lsof -a -p 2100,2102,2200,2202,2300,2400,2500,2601,2700,2801,2802 -d cwd -Fn"
                .to_owned()
        ));
        assert!(calls.contains(&"/bin/launchctl print gui/501/homebrew.mxcl.redis".to_owned()));
        assert!(calls.contains(
            &"/Applications/Docker.app/Contents/Resources/bin/docker --host unix:///Users/dev/.docker/run/docker.sock ps --no-trunc --format {{json .}}"
                .to_owned()
        ));
        assert!(calls.contains(
            &"/bin/ps -ww -axo pid=,ppid=,pgid=,uid=,tty=,user=,lstart=,command=".to_owned()
        ));
    }

    #[test]
    fn failed_commands_become_warnings_not_invented_entries() {
        let runner = FakeRunner {
            fail: vec!["print gui/501", "sh.brew.nginx"],
            ..FakeRunner::default()
        };
        let snapshot = collect_with(&runner, &files(), &env());
        assert_eq!(
            snapshot.warnings,
            [
                "사용자 서비스 목록(launchctl)을(를) 읽지 못했습니다. Could not find service",
                "서비스 상세 정보를 읽지 못했습니다: sh.brew.nginx",
            ]
        );
        let nginx = snapshot
            .entries
            .iter()
            .find(|e| e.listeners.iter().any(|l| l.port == 8080))
            .unwrap();
        assert_eq!(
            nginx.manager.kind,
            ManagerKind::LaunchdOther,
            "상세가 없으면 brew로 단정하지 않는다"
        );
        let redis = snapshot
            .entries
            .iter()
            .find(|e| e.listeners.iter().any(|l| l.port == 6379))
            .unwrap();
        assert_eq!(
            redis.manager.kind,
            ManagerKind::Detached,
            "사용자 서비스 목록이 없으면 분리 실행으로 보인다"
        );
    }

    #[test]
    fn project_lookup_prefers_the_nearest_marker_and_package_name() {
        let files = FakeFiles(HashMap::from([
            ("/Users/dev/work/shop/package.json", r#"{"name":"shop"}"#),
            (
                "/Users/dev/work/shop/apps/web/package.json",
                r#"{"name":"@shop/web"}"#,
            ),
        ]));
        let project =
            find_project(&files, "/Users/dev/work/shop/apps/web/src", "/Users/dev").unwrap();
        assert_eq!(project.name, "@shop/web");
        assert_eq!(project.path, "/Users/dev/work/shop/apps/web");
        assert_eq!(find_project(&files, "/Users/dev/notes", "/Users/dev"), None);
    }
}

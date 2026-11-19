//! 멈추기 계획·값 검증·결과 판정. 실행은 수집기가 한다(docs/DESIGN.md "멈추기").

use serde::{Deserialize, Serialize};

use crate::snapshot::{Entry, ManagerKind, Snapshot};

/// 서비스 실행 파일 경로 앞부분과 그 brew. 다른 경로의 brew는 쓰지 않는다.
const BREWS: &[(&str, &str)] = &[
    ("/opt/homebrew/", "/opt/homebrew/bin/brew"),
    ("/usr/local/", "/usr/local/bin/brew"),
];
const DOCKER_CLI_SUFFIX: &str = "/Docker.app/Contents/Resources/bin/docker";
const DOCKER_SOCKET_SUFFIX: &str = "/.docker/run/docker.sock";

/// root brew가 사용자 홈에 root 소유 파일을 만들지 않고, 자동 업데이트·통계 전송을 하지 않게 하는 환경(DC-17).
const ADMIN_BREW_ENV: &str = "/usr/bin/env HOME=/var/root HOMEBREW_NO_AUTO_UPDATE=1 \
HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1";
/// 사용자 권한 brew에 주는 환경. HOME·USER는 실행기가 붙인다.
pub const USER_BREW_ENV: &[&str] = &[
    "HOMEBREW_NO_AUTO_UPDATE=1",
    "HOMEBREW_NO_ANALYTICS=1",
    "HOMEBREW_NO_ENV_HINTS=1",
];

pub const SIGTERM: i32 = 15;
pub const SIGKILL: i32 = 9;

/// 항목 하나를 멈추는 계획.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopPlan {
    pub method: StopMethod,
    /// 확인 창에 보여 줄 명령.
    pub command: String,
    /// 관리자 권한이 필요한지.
    pub admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
pub enum StopMethod {
    /// `group`이면 `-pid` 프로세스 그룹에, 아니면 그 PID에만 신호를 보낸다.
    Signal { pid: u32, group: bool },
    /// `domain`은 "system" 또는 "user".
    BrewServices {
        brew: String,
        formula: String,
        domain: String,
    },
    DockerStop {
        docker: String,
        socket: String,
        container_id: String,
    },
}

/// 판별기가 계획을 만들 때 쓰는 값. 스냅샷에 그대로 나오지 않는 정보다.
#[derive(Debug, Clone, Default)]
pub struct PlanHints {
    /// launchd 서비스의 실행 파일 경로.
    pub service_executable: Option<String>,
    pub docker: Option<DockerHint>,
}

#[derive(Debug, Clone)]
pub struct DockerHint {
    pub cli: String,
    pub socket: String,
    pub container_id: String,
}

/// Docker Desktop 소켓 주소.
pub fn docker_socket(home: &str) -> String {
    format!("unix://{home}{DOCKER_SOCKET_SUFFIX}")
}

/// 항목의 멈추기 계획. 보호 항목과 멈추는 방법이 없는 관리 주체는 없음.
pub fn plan(entry: &Entry, hints: &PlanHints) -> Option<StopPlan> {
    if entry.protection.is_some() {
        return None;
    }
    let manager = &entry.manager;
    let method = match manager.kind {
        ManagerKind::LaunchdBrew => {
            let executable = hints.service_executable.as_deref()?;
            let brew = BREWS
                .iter()
                .find(|(prefix, _)| executable.starts_with(prefix))
                .map(|(_, brew)| (*brew).to_owned())?;
            StopMethod::BrewServices {
                brew,
                formula: manager.formula.clone()?,
                domain: manager.domain.clone()?,
            }
        }
        ManagerKind::Docker => {
            let hint = hints.docker.as_ref()?;
            StopMethod::DockerStop {
                docker: hint.cli.clone(),
                socket: hint.socket.clone(),
                container_id: hint.container_id.clone(),
            }
        }
        ManagerKind::Terminal | ManagerKind::Detached | ManagerKind::App | ManagerKind::Unknown => {
            let top = entry.processes.first()?;
            // 그룹 리더가 아니면 같은 그룹에 셸이 있을 수 있어 그 PID에만 보낸다.
            StopMethod::Signal {
                pid: top.pid,
                group: top.pgid == top.pid,
            }
        }
        ManagerKind::LaunchdOther | ManagerKind::Pm2 | ManagerKind::LaunchdSocket => return None,
    };
    validate(&method).ok()?;
    let command = match &method {
        StopMethod::Signal { pid, group: true } => format!("kill -TERM -{pid}"),
        StopMethod::Signal { pid, group: false } => format!("kill -TERM {pid}"),
        StopMethod::BrewServices { formula, .. } => format!("brew services stop {formula}"),
        StopMethod::DockerStop { .. } => format!(
            "docker stop {}",
            manager.container.as_deref().unwrap_or("컨테이너")
        ),
    };
    let admin = matches!(&method, StopMethod::BrewServices { domain, .. } if domain == "system");
    Some(StopPlan {
        method,
        command,
        admin,
    })
}

fn is_formula(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 100
        && value.bytes().all(|b| {
            b.is_ascii_lowercase()
                || b.is_ascii_digit()
                || matches!(b, b'@' | b'+' | b'.' | b'_' | b'-')
        })
        && !value.starts_with(['-', '.'])
}

fn is_container_id(value: &str) -> bool {
    (12..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// 절대 경로이고 `..`·제어 문자·따옴표·백슬래시가 없는지.
fn is_plain_path(value: &str) -> bool {
    value.starts_with('/')
        && !value.split('/').any(|part| part == "..")
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '"' | '\'' | '\\' | '`' | '$'))
}

/// 실행 직전에 계획의 값을 다시 검증한다(docs/DESIGN.md "관리자 권한").
pub fn validate(method: &StopMethod) -> Result<(), String> {
    let ok = match method {
        StopMethod::Signal { pid, .. } => *pid > 1 && *pid <= i32::MAX as u32,
        StopMethod::BrewServices {
            brew,
            formula,
            domain,
        } => {
            BREWS.iter().any(|(_, known)| known == brew)
                && is_formula(formula)
                && matches!(domain.as_str(), "system" | "user")
        }
        StopMethod::DockerStop {
            docker,
            socket,
            container_id,
        } => {
            docker.ends_with(DOCKER_CLI_SUFFIX)
                && is_plain_path(docker)
                && socket
                    .strip_prefix("unix://")
                    .is_some_and(|path| is_plain_path(path) && path.ends_with(DOCKER_SOCKET_SUFFIX))
                && is_container_id(container_id)
        }
    };
    if ok {
        Ok(())
    } else {
        Err("멈추기 계획의 값이 올바르지 않아 실행하지 않았습니다.".to_owned())
    }
}

/// 관리자 권한으로 실행할 `/bin/sh` 명령. 검증한 값만 고정 틀에 넣는다. root brew 서비스만 해당한다.
pub fn admin_shell_command(method: &StopMethod) -> Option<String> {
    validate(method).ok()?;
    match method {
        StopMethod::BrewServices {
            brew,
            formula,
            domain,
        } if domain == "system" => Some(format!("{ADMIN_BREW_ENV} {brew} services stop {formula}")),
        _ => None,
    }
}

/// 관리자 비밀번호 창으로 명령 하나를 실행하는 AppleScript. 명령에는 검증한 값만 있어 따옴표가 없다.
pub fn admin_script(shell_command: &str) -> Option<String> {
    let safe = !shell_command
        .chars()
        .any(|c| c.is_control() || matches!(c, '"' | '\\'));
    safe.then(|| format!("do shell script \"{shell_command}\" with administrator privileges"))
}

/// 다시 수집한 결과로 본 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Observed {
    Stopped,
    Restarted,
    StillRunning,
}

/// 멈춘 뒤 스냅샷으로 결과를 정한다. 같은 id가 남았으면 아직 실행 중, 멈추기 전에 없던 id가 같은 key로 생겼으면 다시 실행됨.
pub fn observe(before: &Snapshot, id: &str, after: &Snapshot) -> Observed {
    let Some(target) = before.entries.iter().find(|entry| entry.id == id) else {
        return Observed::Stopped;
    };
    if after.entries.iter().any(|entry| entry.id == id) {
        return Observed::StillRunning;
    }
    let restarted = after.entries.iter().any(|entry| {
        entry.key == target.key && !before.entries.iter().any(|old| old.id == entry.id)
    });
    if restarted {
        Observed::Restarted
    } else {
        Observed::Stopped
    }
}

/// 화면에 돌려주는 결과(docs/DESIGN.md "화면").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    Stopped,
    Restarted,
    StillRunning,
    Cancelled,
    Changed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StopReport {
    pub outcome: Outcome,
    pub message: String,
    /// 실패했을 때 터미널에서 직접 실행할 명령.
    pub command: Option<String>,
    /// 강제 종료(SIGKILL)를 제안하는지.
    pub can_force: bool,
}

impl StopReport {
    pub fn new(outcome: Outcome, message: impl Into<String>) -> Self {
        Self {
            outcome,
            message: message.into(),
            command: None,
            can_force: false,
        }
    }
}

/// 관리 주체의 화면 이름. "다시 실행됨" 안내에 쓴다.
pub fn manager_name(kind: ManagerKind) -> &'static str {
    match kind {
        ManagerKind::LaunchdBrew => "brew 서비스",
        ManagerKind::LaunchdOther => "launchd 서비스",
        ManagerKind::LaunchdSocket => "launchd 소켓",
        ManagerKind::Docker => "Docker",
        ManagerKind::Pm2 => "pm2",
        ManagerKind::Terminal => "터미널",
        ManagerKind::Detached => "분리 실행",
        ManagerKind::App => "앱",
        ManagerKind::Unknown => "알 수 없음",
    }
}

/// 다시 수집해 본 상태를 화면 결과로 바꾼다.
pub fn report_for(observed: Observed, after: &Snapshot, before: &Entry) -> StopReport {
    match observed {
        Observed::Stopped => StopReport::new(Outcome::Stopped, "멈췄습니다."),
        Observed::Restarted => {
            let kind = after
                .entries
                .iter()
                .find(|entry| entry.key == before.key)
                .map_or(before.manager.kind, |entry| entry.manager.kind);
            StopReport::new(
                Outcome::Restarted,
                format!("다시 실행됨: {}", manager_name(kind)),
            )
        }
        Observed::StillRunning => {
            let mut report = StopReport::new(Outcome::StillRunning, "아직 실행 중입니다.");
            report.can_force = matches!(
                before.stop_plan.as_ref().map(|plan| &plan.method),
                Some(StopMethod::Signal { .. })
            );
            report
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Category, Listener, Manager, ProcessInfo, Protection};

    fn entry(kind: ManagerKind) -> Entry {
        Entry {
            id: "terminal:2100:Wed Nov 18 09:00:00 2026".into(),
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
                pid: 2102,
            }],
            processes: vec![ProcessInfo {
                pid: 2100,
                ppid: 2001,
                pgid: 2100,
                uid: 501,
                user: "dev".into(),
                tty: "ttys001".into(),
                started_at: "Wed Nov 18 09:00:00 2026".into(),
                command: "npm run dev".into(),
            }],
            project: None,
            domains: Vec::new(),
            protection: None,
            stop_plan: None,
            hidden: false,
            pinned: false,
        }
    }

    fn brew(domain: &str, executable: &str, formula: &str) -> (Entry, PlanHints) {
        let mut entry = entry(ManagerKind::LaunchdBrew);
        entry.manager.domain = Some(domain.into());
        entry.manager.formula = Some(formula.into());
        let hints = PlanHints {
            service_executable: Some(executable.into()),
            docker: None,
        };
        (entry, hints)
    }

    #[test]
    fn terminal_servers_get_a_group_signal_only_when_they_lead_the_group() {
        let leader = entry(ManagerKind::Terminal);
        let plan = plan(&leader, &PlanHints::default()).unwrap();
        assert_eq!(
            plan.method,
            StopMethod::Signal {
                pid: 2100,
                group: true
            }
        );
        assert_eq!(plan.command, "kill -TERM -2100");
        assert!(!plan.admin);

        let mut member = entry(ManagerKind::Detached);
        member.processes[0].pgid = 2001;
        let plan = super::plan(&member, &PlanHints::default()).unwrap();
        assert_eq!(plan.command, "kill -TERM 2100");
    }

    #[test]
    fn protected_and_unsupported_managers_have_no_plan() {
        let mut protected = entry(ManagerKind::Terminal);
        protected.protection = Some(Protection {
            reason: "portwatch 자신입니다.".into(),
        });
        assert_eq!(plan(&protected, &PlanHints::default()), None);
        for kind in [
            ManagerKind::LaunchdOther,
            ManagerKind::Pm2,
            ManagerKind::LaunchdSocket,
        ] {
            assert_eq!(plan(&entry(kind), &PlanHints::default()), None, "{kind:?}");
        }
        let mut pid_one = entry(ManagerKind::Unknown);
        pid_one.processes[0].pid = 1;
        assert_eq!(plan(&pid_one, &PlanHints::default()), None);
    }

    #[test]
    fn brew_services_use_the_fixed_brew_and_admin_only_for_system() {
        let (user, hints) = brew("user", "/opt/homebrew/opt/redis/bin/redis-server", "redis");
        let plan = plan(&user, &hints).unwrap();
        assert_eq!(plan.command, "brew services stop redis");
        assert!(!plan.admin);
        assert_eq!(admin_shell_command(&plan.method), None);

        let (root, hints) = brew("system", "/usr/local/opt/nginx/bin/nginx", "nginx");
        let plan = super::plan(&root, &hints).unwrap();
        assert!(plan.admin);
        assert_eq!(
            admin_shell_command(&plan.method).unwrap(),
            "/usr/bin/env HOME=/var/root HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 \
             HOMEBREW_NO_ENV_HINTS=1 /usr/local/bin/brew services stop nginx"
        );

        let (other, hints) = brew("system", "/Users/dev/bin/nginx", "nginx");
        assert_eq!(
            super::plan(&other, &hints),
            None,
            "brew 경로 밖이면 계획이 없다"
        );
    }

    #[test]
    fn injection_attempts_are_rejected() {
        for formula in [
            "nginx; rm -rf /",
            "nginx\"",
            "$(id)",
            "a b",
            "-x",
            "Nginx",
            "",
            "nginx`id`",
            "nginx\\",
        ] {
            let (root, hints) = brew("system", "/opt/homebrew/opt/x/bin/x", formula);
            assert_eq!(plan(&root, &hints), None, "{formula:?}");
            let method = StopMethod::BrewServices {
                brew: "/opt/homebrew/bin/brew".into(),
                formula: formula.into(),
                domain: "system".into(),
            };
            assert!(validate(&method).is_err(), "{formula:?}");
            assert_eq!(admin_shell_command(&method), None);
        }
        for formula in ["nginx", "php@8.3", "gtk+3", "python-tk_x.y"] {
            let method = StopMethod::BrewServices {
                brew: "/opt/homebrew/bin/brew".into(),
                formula: formula.into(),
                domain: "system".into(),
            };
            assert!(validate(&method).is_ok(), "{formula:?}");
        }
        let fake_brew = StopMethod::BrewServices {
            brew: "/tmp/brew".into(),
            formula: "nginx".into(),
            domain: "system".into(),
        };
        assert!(validate(&fake_brew).is_err());
        assert_eq!(admin_script("echo \"x\""), None);
        assert_eq!(
            admin_script("/usr/bin/true").unwrap(),
            "do shell script \"/usr/bin/true\" with administrator privileges"
        );
    }

    #[test]
    fn docker_plans_need_a_valid_cli_socket_and_id() {
        let mut container = entry(ManagerKind::Docker);
        container.manager.container = Some("shop-db".into());
        let good = DockerHint {
            cli: "/Applications/Docker.app/Contents/Resources/bin/docker".into(),
            socket: docker_socket("/Users/dev"),
            container_id: "a1b2c3d4e5f6".into(),
        };
        let hints = PlanHints {
            service_executable: None,
            docker: Some(good.clone()),
        };
        let plan = plan(&container, &hints).unwrap();
        assert_eq!(plan.command, "docker stop shop-db");
        assert_eq!(
            plan.method,
            StopMethod::DockerStop {
                docker: good.cli.clone(),
                socket: "unix:///Users/dev/.docker/run/docker.sock".into(),
                container_id: "a1b2c3d4e5f6".into(),
            }
        );

        for bad in [
            DockerHint {
                container_id: "shop-db".into(),
                ..good.clone()
            },
            DockerHint {
                container_id: "A1B2C3D4E5F6".into(),
                ..good.clone()
            },
            DockerHint {
                cli: "/tmp/docker".into(),
                ..good.clone()
            },
            DockerHint {
                socket: "tcp://example.test:2375".into(),
                ..good.clone()
            },
            DockerHint {
                socket: docker_socket("/Users/dev/../root"),
                ..good.clone()
            },
        ] {
            let hints = PlanHints {
                service_executable: None,
                docker: Some(bad.clone()),
            };
            assert_eq!(super::plan(&container, &hints), None, "{bad:?}");
        }
    }

    #[test]
    fn stop_plan_serializes_with_a_kind_tag() {
        let plan = plan(&entry(ManagerKind::Terminal), &PlanHints::default()).unwrap();
        let value = serde_json::to_value(&plan).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "method": { "kind": "signal", "pid": 2100, "group": true },
                "command": "kill -TERM -2100",
                "admin": false
            })
        );
        let docker = StopMethod::DockerStop {
            docker: "/d".into(),
            socket: "s".into(),
            container_id: "c".into(),
        };
        assert_eq!(
            serde_json::to_value(docker).unwrap()["containerId"],
            serde_json::json!("c")
        );
    }

    fn snapshot(entries: Vec<Entry>) -> Snapshot {
        Snapshot {
            taken_at_ms: 0,
            collect_ms: 0,
            entries,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn observe_tells_stopped_restarted_and_still_running_apart() {
        let target = entry(ManagerKind::Terminal);
        let mut sibling = entry(ManagerKind::Terminal);
        sibling.id = "terminal:2200:x".into();
        let before = snapshot(vec![target.clone(), sibling.clone()]);
        let id = target.id.as_str();

        assert_eq!(
            observe(&before, id, &snapshot(vec![target.clone()])),
            Observed::StillRunning
        );
        assert_eq!(
            observe(&before, id, &snapshot(vec![sibling.clone()])),
            Observed::Stopped,
            "같은 key라도 멈추기 전부터 있던 항목은 다시 실행된 것이 아니다"
        );
        let mut relaunched = target.clone();
        relaunched.id = "terminal:2300:y".into();
        relaunched.manager.kind = ManagerKind::LaunchdBrew;
        let after = snapshot(vec![relaunched]);
        assert_eq!(observe(&before, id, &after), Observed::Restarted);
        let report = report_for(Observed::Restarted, &after, &target);
        assert_eq!(report.outcome, Outcome::Restarted);
        assert_eq!(report.message, "다시 실행됨: brew 서비스");
    }

    #[test]
    fn only_signal_plans_offer_a_forced_stop() {
        let mut target = entry(ManagerKind::Terminal);
        target.stop_plan = plan(&target, &PlanHints::default());
        let report = report_for(Observed::StillRunning, &snapshot(vec![]), &target);
        assert!(report.can_force);

        let (mut root, hints) = brew("system", "/opt/homebrew/opt/nginx/bin/nginx", "nginx");
        root.stop_plan = plan(&root, &hints);
        let report = report_for(Observed::StillRunning, &snapshot(vec![]), &root);
        assert!(!report.can_force);
    }
}

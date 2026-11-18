//! 합성 macOS 출력(tests/fixtures/mac-15)으로 파서와 판별기를 함께 검증한다.
//! 시나리오 ID는 docs/MANUAL_TEST_CHECKLIST.md의 합성 자료 표(F-01~F-06)를 따른다.

use std::collections::HashMap;

use portwatch_core::parse;
use portwatch_core::resolve::{
    build_snapshot, plan_lookups, Facts, LaunchdDomain, ServiceTarget, DEFAULT_SECURITY_NAMES,
};
use portwatch_core::{Category, Entry, ManagerKind, Project, Snapshot};

const NETSTAT: &str = include_str!("fixtures/mac-15/netstat.txt");
const PS: &str = include_str!("fixtures/mac-15/ps.txt");
const SYSTEM: &str = include_str!("fixtures/mac-15/launchctl-system.txt");
const GUI: &str = include_str!("fixtures/mac-15/launchctl-gui.txt");
const NGINX: &str = include_str!("fixtures/mac-15/service-system-sh.brew.nginx.txt");
const REDIS: &str = include_str!("fixtures/mac-15/service-gui-homebrew.mxcl.redis.txt");
const AGENTD: &str = include_str!("fixtures/mac-15/service-system-com.vendor.agentd.txt");
const KDC: &str = include_str!("fixtures/mac-15/service-system-com.apple.Kerberos.kdc.txt");
const LSOF: &str = include_str!("fixtures/mac-15/lsof-cwd.txt");

fn target(domain: LaunchdDomain, label: &str) -> ServiceTarget {
    ServiceTarget {
        domain,
        label: label.to_owned(),
    }
}

fn first_pass() -> Facts {
    let (processes, skipped) = parse::ps_rows(PS);
    assert_eq!(skipped, 0, "합성 ps 출력은 모두 읽혀야 한다");
    Facts {
        taken_at_ms: 1_795_000_000_000,
        listeners: parse::netstat_listeners(NETSTAT).unwrap(),
        processes,
        system_jobs: parse::launchctl_services(SYSTEM),
        user_jobs: parse::launchctl_services(GUI),
        self_pid: 3000,
        current_uid: 501,
        security_names: DEFAULT_SECURITY_NAMES
            .iter()
            .map(|s| s.to_string())
            .collect(),
        ..Facts::default()
    }
}

fn snapshot() -> Snapshot {
    let mut facts = first_pass();
    facts.service_details = HashMap::from([
        (
            target(LaunchdDomain::System, "sh.brew.nginx"),
            parse::launchctl_detail(NGINX),
        ),
        (
            target(LaunchdDomain::User, "homebrew.mxcl.redis"),
            parse::launchctl_detail(REDIS),
        ),
        (
            target(LaunchdDomain::System, "com.vendor.agentd"),
            parse::launchctl_detail(AGENTD),
        ),
        (
            target(LaunchdDomain::System, "com.apple.Kerberos.kdc"),
            parse::launchctl_detail(KDC),
        ),
    ]);
    facts.cwd = parse::lsof_cwd(LSOF);
    let project = |name: &str, path: &str| Project {
        name: name.to_owned(),
        path: path.to_owned(),
    };
    facts.projects = HashMap::from([
        (
            "/Users/dev/work/shop-web".to_owned(),
            project("shop-web", "/Users/dev/work/shop-web"),
        ),
        (
            "/Users/dev/work/api".to_owned(),
            project("api-server", "/Users/dev/work/api"),
        ),
        (
            "/Users/dev/work/worker".to_owned(),
            project("worker", "/Users/dev/work/worker"),
        ),
        (
            "/Users/dev/work/queue".to_owned(),
            project("queue", "/Users/dev/work/queue"),
        ),
    ]);
    build_snapshot(&facts)
}

fn by_port(snapshot: &Snapshot, port: u16) -> &Entry {
    snapshot
        .entries
        .iter()
        .find(|entry| entry.listeners.iter().any(|l| l.port == port))
        .unwrap_or_else(|| panic!("{port}번 포트 항목이 없다"))
}

#[test]
fn lookups_ask_only_for_launchd_details_and_own_working_folders() {
    let lookups = plan_lookups(&first_pass());
    assert_eq!(
        lookups.services,
        vec![
            target(LaunchdDomain::System, "com.apple.Kerberos.kdc"),
            target(LaunchdDomain::System, "com.vendor.agentd"),
            target(LaunchdDomain::System, "sh.brew.nginx"),
            target(LaunchdDomain::User, "homebrew.mxcl.redis"),
        ]
    );
    assert_eq!(
        lookups.cwd_pids,
        vec![2100, 2102, 2200, 2202, 2300, 2400, 2500, 2601]
    );
}

#[test]
fn f01_root_brew_nginx_is_one_service_entry() {
    let snapshot = snapshot();
    let nginx = by_port(&snapshot, 8080);
    assert_eq!(nginx.title, "nginx");
    assert_eq!(nginx.category, Category::Service);
    assert_eq!(nginx.manager.kind, ManagerKind::LaunchdBrew);
    assert_eq!(nginx.manager.domain.as_deref(), Some("system"));
    assert_eq!(nginx.manager.label.as_deref(), Some("sh.brew.nginx"));
    assert_eq!(nginx.manager.formula.as_deref(), Some("nginx"));
    assert!(nginx.manager.runs_as_root);
    assert_eq!(
        nginx.processes.iter().map(|p| p.pid).collect::<Vec<_>>(),
        [1100, 1101]
    );
    assert_eq!(nginx.processes[1].user, "nobody");
    assert_eq!(nginx.project, None);
    assert_eq!(nginx.protection, None);
    assert_eq!(nginx.id, "launchd:1100:Wed Nov 18 09:00:00 2026");
}

#[test]
fn f02_user_brew_service_is_not_root() {
    let snapshot = snapshot();
    let redis = by_port(&snapshot, 6379);
    assert_eq!(redis.title, "redis");
    assert_eq!(redis.manager.kind, ManagerKind::LaunchdBrew);
    assert_eq!(redis.manager.domain.as_deref(), Some("user"));
    assert!(!redis.manager.runs_as_root);
    assert_eq!(redis.listeners.len(), 2, "127.0.0.1과 ::1을 각각 둔다");
    assert_eq!(
        redis.project, None,
        "Homebrew 폴더를 프로젝트로 잡지 않는다"
    );
    assert_eq!(redis.protection, None);
}

#[test]
fn f03_terminal_servers_are_grouped_by_project() {
    let snapshot = snapshot();

    let web = by_port(&snapshot, 5173);
    assert_eq!(web.title, "shop-web");
    assert_eq!(web.manager.kind, ManagerKind::Terminal);
    assert_eq!(web.category, Category::DevServer);
    assert_eq!(
        web.processes.iter().map(|p| p.pid).collect::<Vec<_>>(),
        [2100, 2101, 2102, 2103],
        "npm부터 그 아래 전부"
    );
    assert_eq!(web.processes[0].pgid, 2100);
    assert_eq!(web.listeners[0].family, "tcp46");

    let api = by_port(&snapshot, 3000);
    assert_eq!(api.title, "api-server");
    assert_eq!(
        api.processes[0].command, "npm start",
        "nodemon 위의 npm이 최상위"
    );

    let python = by_port(&snapshot, 8000);
    assert_eq!(
        python.manager.kind,
        ManagerKind::Terminal,
        "Python.app 경로여도 터미널이 먼저"
    );
    assert_eq!(python.title, "Python");
    assert_eq!(python.project, None);
}

#[test]
fn f04_noise_is_folded_and_protected() {
    let snapshot = snapshot();

    let launchd = by_port(&snapshot, 22);
    assert_eq!(launchd.manager.kind, ManagerKind::LaunchdSocket);
    assert_eq!(launchd.category, Category::System);
    assert_eq!(launchd.listeners[0].family, "tcp4,tcp6");
    assert_eq!(launchd.processes.len(), 1, "PID 1 아래 전체를 넣지 않는다");
    assert_eq!(
        launchd.protection.as_ref().unwrap().reason,
        "launchd가 직접 연 포트입니다."
    );

    let kdc = by_port(&snapshot, 88);
    assert_eq!(kdc.manager.kind, ManagerKind::LaunchdOther);
    assert_eq!(kdc.category, Category::System);

    let security = by_port(&snapshot, 16105);
    assert_eq!(security.category, Category::System);
    assert_eq!(security.title, "veraport");
    assert_eq!(
        security.protection.as_ref().unwrap().reason,
        "시스템·보안 프로그램입니다."
    );

    let vendor = by_port(&snapshot, 21300);
    assert_eq!(vendor.manager.kind, ManagerKind::LaunchdOther);
    assert_eq!(vendor.manager.domain.as_deref(), Some("system"));
    assert_eq!(vendor.category, Category::System);

    let tool = by_port(&snapshot, 53733);
    assert_eq!(tool.category, Category::DevTool);
    assert_eq!(tool.title, "assistant-cli");

    let player = by_port(&snapshot, 57621);
    assert_eq!(player.manager.kind, ManagerKind::App);
    assert_eq!(player.category, Category::App);
    assert_eq!(player.title, "Player");
}

#[test]
fn f06_detached_and_pm2_servers() {
    let snapshot = snapshot();

    let worker = by_port(&snapshot, 4000);
    assert_eq!(worker.manager.kind, ManagerKind::Detached);
    assert_eq!(worker.category, Category::DevServer);
    assert_eq!(worker.title, "worker");

    let queue = by_port(&snapshot, 4100);
    assert_eq!(queue.manager.kind, ManagerKind::Pm2);
    assert_eq!(queue.processes[0].pid, 2601, "pm2 데몬 바로 아래가 최상위");
    assert_eq!(queue.title, "queue");
}

#[test]
fn entries_are_sorted_by_category_then_port() {
    let snapshot = snapshot();
    let order: Vec<(Category, &str)> = snapshot
        .entries
        .iter()
        .map(|entry| (entry.category, entry.title.as_str()))
        .collect();
    assert_eq!(
        order,
        [
            (Category::DevServer, "api-server"),
            (Category::DevServer, "worker"),
            (Category::DevServer, "queue"),
            (Category::DevServer, "shop-web"),
            (Category::DevServer, "Python"),
            (Category::Service, "redis"),
            (Category::Service, "nginx"),
            (Category::DevTool, "assistant-cli"),
            (Category::App, "Player"),
            (Category::System, "launchd"),
            (Category::System, "com.apple.Kerberos.kdc"),
            (Category::System, "veraport"),
            (Category::System, "com.vendor.agentd"),
        ]
    );
}

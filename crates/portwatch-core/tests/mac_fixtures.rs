//! 합성 macOS 출력(tests/fixtures/mac-15)으로 파서와 판별기를 함께 검증한다.
//! 시나리오 ID는 docs/MANUAL_TEST_CHECKLIST.md의 합성 자료 표(F-01~F-07)를 따른다.
//! 멈추기 계획(stopPlan, Phase 3)과 주소(domains, Phase 4)도 같은 자료로 확인한다.

use std::collections::HashMap;

use portwatch_core::nginx::{self, ConfigFiles};
use portwatch_core::resolve::{
    build_snapshot, plan_lookups, Facts, LaunchdDomain, ServiceTarget, DEFAULT_SECURITY_NAMES,
};
use portwatch_core::stop::StopMethod;
use portwatch_core::{hosts, parse};
use portwatch_core::{Category, DomainVia, Entry, ManagerKind, Project, Scheme, Snapshot};

const NETSTAT: &str = include_str!("fixtures/mac-15/netstat.txt");
const PS: &str = include_str!("fixtures/mac-15/ps.txt");
const SYSTEM: &str = include_str!("fixtures/mac-15/launchctl-system.txt");
const GUI: &str = include_str!("fixtures/mac-15/launchctl-gui.txt");
const NGINX: &str = include_str!("fixtures/mac-15/service-system-sh.brew.nginx.txt");
const REDIS: &str = include_str!("fixtures/mac-15/service-gui-homebrew.mxcl.redis.txt");
const AGENTD: &str = include_str!("fixtures/mac-15/service-system-com.vendor.agentd.txt");
const KDC: &str = include_str!("fixtures/mac-15/service-system-com.apple.Kerberos.kdc.txt");
const CROSSEX: &str = include_str!("fixtures/mac-15/service-gui-kr.example.crossex-service.txt");
const LSOF: &str = include_str!("fixtures/mac-15/lsof-cwd.txt");
const DOCKER: &str = include_str!("fixtures/mac-15/docker-ps.txt");
const DOCKER_CLI: &str = "/Applications/Docker.app/Contents/Resources/bin/docker";
const HOSTS: &str = include_str!("fixtures/mac-15/hosts.txt");
const NGINX_CONF: &str = include_str!("fixtures/mac-15/nginx/nginx.conf");
const NGINX_MIME: &str = include_str!("fixtures/mac-15/nginx/mime.types");
const NGINX_SHOP: &str = include_str!("fixtures/mac-15/nginx/servers/shop.conf");
const CONF: &str = "/opt/homebrew/etc/nginx/nginx.conf";

/// 수집기가 읽었을 nginx 설정(F-07).
fn nginx_files() -> ConfigFiles {
    let shop = "/opt/homebrew/etc/nginx/servers/shop.conf".to_owned();
    ConfigFiles {
        texts: HashMap::from([
            (CONF.to_owned(), NGINX_CONF.to_owned()),
            (
                "/opt/homebrew/etc/nginx/mime.types".to_owned(),
                NGINX_MIME.to_owned(),
            ),
            (shop.clone(), NGINX_SHOP.to_owned()),
        ]),
        globs: HashMap::from([("/opt/homebrew/etc/nginx/servers/*".to_owned(), vec![shop])]),
    }
}

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
        home: "/Users/dev".to_owned(),
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
        (
            target(LaunchdDomain::User, "kr.example.crossex-service"),
            parse::launchctl_detail(CROSSEX),
        ),
    ]);
    facts.cwd = parse::lsof_cwd(LSOF);
    let (containers, skipped) = parse::docker_ps(DOCKER);
    assert_eq!(skipped, 0);
    facts.containers = containers;
    facts.hosts = hosts::loopback_names(HOSTS);
    let (routes, warnings) = nginx::routes(CONF, &nginx_files());
    assert!(warnings.is_empty(), "{warnings:?}");
    facts.nginx_routes = routes;
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
            target(LaunchdDomain::User, "kr.example.crossex-service"),
        ]
    );
    assert_eq!(
        lookups.cwd_pids,
        vec![2100, 2102, 2200, 2202, 2300, 2400, 2500, 2601, 2700, 2801, 2802]
    );
    assert_eq!(lookups.docker_cli.as_deref(), Some(DOCKER_CLI));
    assert_eq!(lookups.nginx_confs, [Ok(CONF.to_owned())]);
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
    assert_eq!(nginx.key, "launchd:sh.brew.nginx");

    let plan = nginx.stop_plan.as_ref().unwrap();
    assert!(plan.admin, "root 서비스는 관리자 권한");
    assert_eq!(plan.command, "brew services stop nginx");
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
    let plan = redis.stop_plan.as_ref().unwrap();
    assert!(!plan.admin);
    assert_eq!(
        plan.method,
        StopMethod::BrewServices {
            brew: "/opt/homebrew/bin/brew".into(),
            formula: "redis".into(),
            domain: "user".into(),
        }
    );
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

    assert_eq!(
        web.stop_plan.as_ref().unwrap().command,
        "kill -TERM -2100",
        "npm 그룹 전체(감시 도구 포함)"
    );
    assert_eq!(api.stop_plan.as_ref().unwrap().command, "kill -TERM -2200");
}

/// 2026-11-19 사용자 Mac: 코딩 도구가 자기 셸에서 띄운 서버의 최상위가 도구가 되면, 멈출 때 도구까지 멈춘다.
#[test]
fn server_started_by_a_tool_shell_stops_at_that_shell() {
    let snapshot = snapshot();
    let server = by_port(&snapshot, 3100);
    assert_eq!(server.manager.kind, ManagerKind::Terminal);
    assert_eq!(
        server.processes.iter().map(|p| p.pid).collect::<Vec<_>>(),
        [2801, 2802],
        "도구(2800)는 서버와 다른 프로세스 그룹이라 넣지 않는다"
    );
    assert_eq!(
        server.stop_plan.as_ref().unwrap().command,
        "kill -TERM -2801"
    );
    assert_eq!(server.category, Category::DevServer);
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
    assert_eq!(
        tool.title, "assistant-cli",
        "도구는 프로젝트 이름이 아니라 도구 이름"
    );
    assert_eq!(tool.project.as_ref().unwrap().name, "api-server");
    assert_eq!(
        tool.key, "exe:assistant-cli",
        "같은 프로젝트의 개발 서버와 key가 겹치지 않는다"
    );
    assert_eq!(by_port(&snapshot, 3000).key, "project:/Users/dev/work/api");
    assert_eq!(by_port(&snapshot, 22).key, "launchd-socket");
    assert_eq!(by_port(&snapshot, 57621).key, "app:Player");

    let player = by_port(&snapshot, 57621);
    assert_eq!(player.manager.kind, ManagerKind::App);
    assert_eq!(player.category, Category::App);
    assert_eq!(player.title, "Player");

    for port in [22, 88, 16105, 21300, 34581] {
        assert_eq!(
            by_port(&snapshot, port).stop_plan,
            None,
            "{port}번은 보호 항목"
        );
    }
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
    assert_eq!(queue.stop_plan, None, "pm2는 멈추지 않는다(DC-22)");
    assert!(queue.protection.as_ref().unwrap().reason.contains("DC-22"));
    assert_eq!(
        worker.stop_plan.as_ref().unwrap().command,
        "kill -TERM -2500"
    );
}

#[test]
fn f05_docker_containers_are_entries_and_other_ports_stay_with_the_app() {
    let snapshot = snapshot();

    let db = by_port(&snapshot, 5432);
    assert_eq!(db.manager.kind, ManagerKind::Docker);
    assert_eq!(db.manager.container.as_deref(), Some("shop-db"));
    assert_eq!(db.title, "shop-db");
    assert_eq!(db.key, "docker:shop-db");
    assert_eq!(db.category, Category::DevServer);
    assert_eq!(db.listeners.len(), 1);
    assert_eq!(
        db.listeners[0].family, "tcp4,tcp6",
        "IPv4·IPv6를 한 listener로"
    );
    assert_eq!(
        db.id,
        "docker:5d1c0e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d"
    );
    assert_eq!(db.processes.len(), 1, "포트 전달 프로세스 하나만");
    assert_eq!(db.project, None);
    let plan = db.stop_plan.as_ref().unwrap();
    assert_eq!(plan.command, "docker stop shop-db");
    assert_eq!(
        plan.method,
        StopMethod::DockerStop {
            docker: DOCKER_CLI.into(),
            socket: "unix:///Users/dev/.docker/run/docker.sock".into(),
            container_id: "5d1c0e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6f7a8b9c0d".into(),
        }
    );

    let mock = by_port(&snapshot, 9000);
    assert_eq!(mock.title, "mock-api");
    assert_eq!(
        mock.listeners.iter().map(|l| l.port).collect::<Vec<_>>(),
        [9000, 9001],
        "포트 범위는 한 컨테이너"
    );
    assert_eq!(mock.category, Category::DevServer);

    let app = by_port(&snapshot, 6443);
    assert_eq!(app.manager.kind, ManagerKind::App);
    assert_eq!(app.title, "Docker");
    assert_eq!(
        app.listeners.iter().map(|l| l.port).collect::<Vec<_>>(),
        [6443],
        "컨테이너에 맞은 포트는 앱 항목에 남지 않는다"
    );
}

#[test]
fn without_docker_ps_the_ports_stay_with_the_docker_app() {
    let mut facts = first_pass();
    facts.containers.clear();
    let snapshot = build_snapshot(&facts);
    let app = by_port(&snapshot, 5432);
    assert_eq!(app.manager.kind, ManagerKind::App);
    assert_eq!(
        app.listeners.iter().map(|l| l.port).collect::<Vec<_>>(),
        [5432, 6443, 9000, 9001]
    );
}

#[test]
fn f07_nginx_paths_and_hosts_names_reach_the_upstream_servers() {
    let snapshot = snapshot();
    let nginx = by_port(&snapshot, 8080);
    let summary: Vec<(DomainVia, Scheme, String, Option<u16>, bool)> = nginx
        .domains
        .iter()
        .map(|d| (d.via, d.scheme, d.url(), d.upstream, d.in_hosts))
        .collect();
    assert_eq!(
        summary,
        [
            (
                DomainVia::Nginx,
                Scheme::Http,
                "http://localhost:8080/api".to_owned(),
                Some(3000),
                true
            ),
            (
                DomainVia::Nginx,
                Scheme::Https,
                "https://shop.example.test/".to_owned(),
                Some(5173),
                true
            ),
        ],
        "루프백이 아닌 대상(intranet)과 정적 location은 빠진다"
    );

    let api = by_port(&snapshot, 3000);
    assert_eq!(api.title, "api-server");
    assert_eq!(api.domains.len(), 1);
    assert_eq!(api.domains[0].via, DomainVia::Proxied);
    assert_eq!(api.domains[0].url(), "http://localhost:8080/api");

    let shop = by_port(&snapshot, 5173);
    assert_eq!(shop.title, "shop-web");
    assert_eq!(shop.domains[0].url(), "https://shop.example.test/");
    assert_eq!(shop.domains[0].tls, None, "인증서는 수집기가 채운다");

    let others: Vec<&str> = snapshot
        .entries
        .iter()
        .filter(|e| !e.domains.is_empty())
        .map(|e| e.title.as_str())
        .collect();
    assert_eq!(others, ["api-server", "shop-web", "nginx"]);
}

#[test]
fn nginx_routes_need_a_running_nginx_on_that_port() {
    let mut facts = first_pass();
    facts.hosts = hosts::loopback_names(HOSTS);
    facts.nginx_routes = nginx::routes(CONF, &nginx_files()).0;
    // nginx가 443을 열지 않은 상태: 설정에만 있는 경로는 붙이지 않는다.
    facts.listeners.retain(|l| l.port != 443);
    let snapshot = build_snapshot(&facts);
    assert!(snapshot
        .entries
        .iter()
        .flat_map(|e| &e.domains)
        .all(|d| d.port == 8080));
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
            (Category::DevServer, "node"),
            (Category::DevServer, "worker"),
            (Category::DevServer, "queue"),
            (Category::DevServer, "shop-web"),
            (Category::DevServer, "shop-db"),
            (Category::DevServer, "Python"),
            (Category::DevServer, "mock-api"),
            (Category::Service, "nginx"),
            (Category::Service, "redis"),
            (Category::DevTool, "sessiond"),
            (Category::DevTool, "assistant-cli"),
            (Category::App, "Docker"),
            (Category::App, "Player"),
            (Category::System, "launchd"),
            (Category::System, "com.apple.Kerberos.kdc"),
            (Category::System, "veraport"),
            (Category::System, "com.vendor.agentd"),
            (Category::System, "kr.example.crossex-service"),
        ]
    );
}

/// 2026-11-19 사용자 Mac 1차 결과에서 나온 경우를 합성 자료로 재현한다(DC-12).
#[test]
fn security_agent_with_spaces_in_its_path_is_system() {
    let snapshot = snapshot();
    let agent = by_port(&snapshot, 34581);
    assert_eq!(agent.manager.kind, ManagerKind::LaunchdOther);
    assert_eq!(agent.manager.domain.as_deref(), Some("user"));
    assert_eq!(
        agent.category,
        Category::System,
        "경로 전체·라벨로 보안 프로그램을 맞춘다"
    );
    assert_eq!(
        agent.processes.iter().map(|p| p.pid).collect::<Vec<_>>(),
        [610, 611]
    );
    assert_eq!(
        agent.protection.as_ref().unwrap().reason,
        "시스템·보안 프로그램입니다."
    );
}

#[test]
fn detached_daemon_outside_a_project_is_a_dev_tool() {
    let snapshot = snapshot();
    let daemon = by_port(&snapshot, 2269);
    assert_eq!(daemon.manager.kind, ManagerKind::Detached);
    assert_eq!(daemon.category, Category::DevTool);
    assert_eq!(daemon.title, "sessiond");
    assert_eq!(daemon.project, None);
}

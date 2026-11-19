//! Phase 6 Test Hardening: 합성 자료 경계 사례(docs/IMPLEMENT.md "Phase 6").
//! 실제 출력 형식(tests/fixtures/mac-15)을 따라 줄을 만들고, 파서와 판별기를 함께 확인한다.

use std::time::Instant;

use portwatch_core::parse;
use portwatch_core::resolve::{build_snapshot, plan_lookups, Facts, LaunchdDomain};
use portwatch_core::stop::StopMethod;
use portwatch_core::{Category, ManagerKind, Snapshot};

const HEADER: &str = "Active Internet connections (including servers)
Proto Recv-Q Send-Q  Local Address          Foreign Address        (state)          rxbytes      txbytes  rhiwat  shiwat    pid   epid state  options           gencnt    flags   flags1 usecnt rtncnt fltrs";

fn listen_line(proto: &str, local: &str, pid: u32) -> String {
    format!(
        "{proto:<5}      0      0  {local:<22} *.*                    LISTEN                 0            0  131072  131072 {pid:>6}      0 00100 00000006 0000000000001b02 00000000 00000900      1      0 000000"
    )
}

fn netstat(lines: &[String]) -> String {
    format!("{HEADER}\n{}\n", lines.join("\n"))
}

fn ps_line(
    pid: u32,
    ppid: u32,
    pgid: u32,
    uid: i64,
    tty: &str,
    user: &str,
    command: &str,
) -> String {
    format!("{pid:>5} {ppid:>5} {pgid:>5} {uid:>5} {tty:<8} {user:<8} Wed Nov 18 09:00:00 2026 {command}")
}

/// 터미널(zsh) 아래에서 띄운 프로세스 줄들. 첫 줄은 셸이다.
fn terminal(shell_pid: u32, children: &[(u32, &str)]) -> Vec<String> {
    let mut lines = vec![ps_line(
        shell_pid, 500, shell_pid, 501, "ttys001", "dev", "-zsh",
    )];
    for (pid, command) in children {
        lines.push(ps_line(
            *pid, shell_pid, *pid, 501, "ttys001", "dev", command,
        ));
    }
    lines
}

fn facts(netstat_text: &str, ps_text: &str) -> Facts {
    let (processes, skipped) = parse::ps_rows(ps_text);
    assert_eq!(skipped, 0);
    Facts {
        taken_at_ms: 1_795_000_000_000,
        listeners: parse::netstat_listeners(netstat_text).unwrap(),
        processes,
        home: "/Users/dev".into(),
        self_pid: 3000,
        current_uid: 501,
        ..Facts::default()
    }
}

fn snapshot(netstat_lines: &[String], ps_lines: &[String]) -> Snapshot {
    build_snapshot(&facts(&netstat(netstat_lines), &ps_lines.join("\n")))
}

#[test]
fn ipv6_only_and_specific_addresses_keep_their_address() {
    let text = netstat(&[
        listen_line("tcp6", "::1.3000", 2101),
        listen_line("tcp6", "*.3001", 2102),
        listen_line("tcp4", "192.168.0.10.3002", 2103),
        listen_line("tcp6", "fe80::1%lo0.3003", 2104),
        listen_line("tcp46", "*.3004", 2105),
    ]);
    let listeners = parse::netstat_listeners(&text).unwrap();
    let pairs: Vec<(&str, u16, &str)> = listeners
        .iter()
        .map(|l| (l.address.as_str(), l.port, l.family.as_str()))
        .collect();
    assert_eq!(
        pairs,
        [
            ("::1", 3000, "tcp6"),
            ("*", 3001, "tcp6"),
            ("192.168.0.10", 3002, "tcp4"),
            ("fe80::1%lo0", 3003, "tcp6"),
            ("*", 3004, "tcp46"),
        ]
    );
}

#[test]
fn ipv6_loopback_ephemeral_ports_count_as_local_tool_ports() {
    let snapshot = snapshot(
        &[listen_line("tcp6", "::1.53001", 2101)],
        &terminal(2100, &[(2101, "/usr/local/bin/helper --serve")]),
    );
    assert_eq!(snapshot.entries[0].category, Category::DevTool);
}

#[test]
fn several_processes_on_one_port_are_grouped_by_their_top_process() {
    // node cluster처럼 한 부모 아래 일꾼 여럿이 같은 포트를 연 경우(SO_REUSEPORT)와 서로 다른 터미널의 두 서버.
    // fork한 일꾼은 부모(2101)의 프로세스 그룹을 물려받는다.
    let mut ps = terminal(2100, &[(2101, "node cluster.js")]);
    for pid in [2102, 2103] {
        ps.push(ps_line(
            pid,
            2101,
            2101,
            501,
            "ttys001",
            "dev",
            "node cluster.js worker",
        ));
    }
    ps.extend(terminal(2200, &[(2201, "node other.js")]));
    let snapshot = snapshot(
        &[
            listen_line("tcp4", "*.8080", 2102),
            listen_line("tcp4", "*.8080", 2103),
            listen_line("tcp4", "*.9090", 2201),
        ],
        &ps,
    );
    assert_eq!(snapshot.entries.len(), 2, "{:#?}", snapshot.entries);
    let cluster = &snapshot.entries[0];
    assert_eq!(cluster.listeners.len(), 2);
    assert_eq!(cluster.processes[0].pid, 2101);
}

#[test]
fn long_spaced_and_korean_command_lines_stay_whole() {
    let long_arg = "x".repeat(4000);
    let command =
        format!("node /Users/dev/작업 폴더/서버 앱/index.js --name \"내 서버\" {long_arg}");
    let ps = terminal(2100, &[(2101, &command)]);
    let (rows, skipped) = parse::ps_rows(&ps.join("\n"));
    assert_eq!(skipped, 0);
    assert_eq!(rows[1].command, command);

    let snapshot = snapshot(&[listen_line("tcp4", "127.0.0.1.4000", 2101)], &ps);
    let entry = &snapshot.entries[0];
    assert_eq!(entry.title, "node");
    assert_eq!(entry.processes[0].command, command);
    assert!(entry.stop_plan.is_some());
}

#[test]
fn odd_process_names_do_not_break_parsing() {
    let ps = [
        ps_line(2100, 1, 2100, 501, "??", "dev", "(node)"),
        ps_line(2101, 1, 2101, 501, "??", "dev", "[dev server]"),
        ps_line(2102, 1, 2102, -2, "??", "nobody", "/opt/x/bin/srv"),
        ps_line(2103, 1, 2103, 501, "??", "dev", "-"),
    ];
    let (rows, skipped) = parse::ps_rows(&ps.join("\n"));
    assert_eq!(skipped, 0);
    assert_eq!(rows[0].command, "(node)");
    assert_eq!(rows[2].uid, 4_294_967_294);

    let snapshot = snapshot(
        &[
            listen_line("tcp4", "*.7000", 2100),
            listen_line("tcp4", "*.7001", 2101),
            listen_line("tcp4", "*.7003", 2103),
        ],
        &ps,
    );
    let titles: Vec<&str> = snapshot.entries.iter().map(|e| e.title.as_str()).collect();
    assert_eq!(titles, ["(node)", "[dev", "-"]);
}

#[test]
fn malformed_ps_lines_are_counted_not_guessed() {
    let text = [
        ps_line(2100, 1, 2100, 501, "??", "dev", "node a.js"),
        "  2101     1  2101   501 ??       dev      Wed Nov 18".to_owned(),
        "garbage".to_owned(),
        ps_line(2102, 1, 2102, 501, "??", "dev", "node b.js").replacen("2102", "21x2", 1),
    ]
    .join("\n");
    let (rows, skipped) = parse::ps_rows(&text);
    assert_eq!(rows.len(), 1);
    assert_eq!(skipped, 3);
}

#[test]
fn netstat_without_the_expected_columns_is_an_error() {
    assert!(parse::netstat_listeners("tcp4 0 0 *.80 *.* LISTEN").is_err());
    let no_pid = "Proto Recv-Q Send-Q  Local Address          Foreign Address        (state)\ntcp4 0 0 *.80 *.* LISTEN";
    assert_eq!(
        parse::netstat_listeners(no_pid).unwrap_err(),
        "netstat 헤더에 `pid` 열이 없습니다."
    );
    // 열 순서가 바뀌어도 이름으로 찾는다.
    let reordered =
        "Proto Local Address (state) pid\ntcp4 *.5000 LISTEN 77\ntcp4 *.5001 ESTABLISHED 78";
    let listeners = parse::netstat_listeners(reordered).unwrap();
    assert_eq!(listeners.len(), 1);
    assert_eq!((listeners[0].port, listeners[0].pid), (5000, 77));
}

#[test]
fn a_listener_whose_process_already_ended_is_kept_without_a_plan() {
    // netstat과 ps 사이에 프로세스가 끝난 경우: 포트는 보이지만 프로세스 정보가 없다.
    let snapshot = snapshot(
        &[listen_line("tcp4", "*.5555", 4242)],
        &terminal(2100, &[(2101, "node a.js")]),
    );
    let entry = &snapshot.entries[0];
    assert_eq!(entry.listeners[0].pid, 4242);
    assert_eq!(entry.manager.kind, ManagerKind::Unknown);
    assert!(entry.processes.is_empty() || entry.processes[0].pid == 4242);
    assert_eq!(
        entry.stop_plan, None,
        "확인할 프로세스가 없으면 멈추지 않는다"
    );
}

#[test]
fn intel_brew_services_use_the_usr_local_brew() {
    let ps = [ps_line(
        1300,
        1,
        1300,
        501,
        "??",
        "dev",
        "/usr/local/opt/redis/bin/redis-server 127.0.0.1:6379",
    )];
    let mut facts = facts(
        &netstat(&[listen_line("tcp4", "127.0.0.1.6379", 1300)]),
        &ps.join("\n"),
    );
    facts.user_jobs = parse::launchctl_services(
        "gui/501 = {\n\tservices = {\n\t\t    1300      - \thomebrew.mxcl.redis\n\t}\n}\n",
    );
    let lookups = plan_lookups(&facts);
    assert_eq!(lookups.services.len(), 1);
    assert_eq!(lookups.services[0].domain, LaunchdDomain::User);
    facts.service_details.insert(
        lookups.services[0].clone(),
        parse::launchctl_detail(
            "gui/501/homebrew.mxcl.redis = {\n\tpath = /Users/dev/Library/LaunchAgents/homebrew.mxcl.redis.plist\n\tstate = running\n\tprogram = /usr/local/opt/redis/bin/redis-server\n\tpid = 1300\n}\n",
        ),
    );
    let snapshot = build_snapshot(&facts);
    let redis = &snapshot.entries[0];
    assert_eq!(redis.manager.kind, ManagerKind::LaunchdBrew);
    assert_eq!(redis.manager.formula.as_deref(), Some("redis"));
    match &redis.stop_plan.as_ref().unwrap().method {
        StopMethod::BrewServices { brew, formula, .. } => {
            assert_eq!(brew, "/usr/local/bin/brew");
            assert_eq!(formula, "redis");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn python_from_a_framework_bundle_is_not_an_app() {
    // Xcode의 /usr/bin/python3와 python.org 설치본은 Python.app 번들 안의 실행 파일로 돈다.
    let xcode = "/Applications/Xcode.app/Contents/Developer/Library/Frameworks/Python3.framework/Versions/3.9/Resources/Python.app/Contents/MacOS/Python -m http.server 18081";
    let python_org = "/Library/Frameworks/Python.framework/Versions/3.12/Resources/Python.app/Contents/MacOS/Python manage.py runserver";
    let ps = [
        ps_line(2100, 1, 2100, 501, "??", "dev", xcode),
        ps_line(2200, 1, 2200, 501, "??", "dev", python_org),
    ];
    let snapshot = snapshot(
        &[
            listen_line("tcp4", "127.0.0.1.18081", 2100),
            listen_line("tcp4", "127.0.0.1.8000", 2200),
        ],
        &ps,
    );
    for entry in &snapshot.entries {
        assert_eq!(entry.manager.kind, ManagerKind::Detached, "{}", entry.title);
        assert_eq!(entry.title, "Python");
    }
}

#[test]
fn app_helpers_inside_frameworks_are_still_apps() {
    let helper = "/Applications/Browser.app/Contents/Frameworks/Browser Framework.framework/Versions/1/Helpers/Browser Helper.app/Contents/MacOS/Browser Helper --type=utility";
    let ps = [
        ps_line(
            700,
            1,
            700,
            501,
            "??",
            "dev",
            "/Applications/Browser.app/Contents/MacOS/Browser",
        ),
        ps_line(701, 700, 700, 501, "??", "dev", helper),
    ];
    let snapshot = snapshot(&[listen_line("tcp4", "127.0.0.1.9222", 701)], &ps);
    assert_eq!(snapshot.entries[0].manager.kind, ManagerKind::App);
    assert_eq!(snapshot.entries[0].title, "Browser");
}

/// 포트 수백 개 규모. 측정값은 docs/IMPLEMENT.md Phase 6 기록에 남긴다.
#[test]
fn hundreds_of_ports_resolve_quickly() {
    let mut netstat_lines = Vec::new();
    let mut ps = Vec::new();
    for i in 0..300u32 {
        let shell = 10_000 + i * 10;
        ps.extend(terminal(
            shell,
            &[(shell + 1, "npm run dev"), (shell + 2, "node server.js")],
        ));
        netstat_lines.push(listen_line("tcp4", &format!("*.{}", 20_000 + i), shell + 2));
        netstat_lines.push(listen_line("tcp6", &format!("*.{}", 20_000 + i), shell + 2));
        netstat_lines.push(listen_line(
            "tcp4",
            &format!("127.0.0.1.{}", 50_000 + i),
            shell + 2,
        ));
    }
    for i in 0..1_000u32 {
        ps.push(ps_line(
            100_000 + i,
            1,
            100_000 + i,
            501,
            "??",
            "dev",
            "/usr/libexec/idle",
        ));
    }
    let netstat_text = netstat(&netstat_lines);
    let ps_text = ps.join("\n");

    let started = Instant::now();
    let snapshot = build_snapshot(&facts(&netstat_text, &ps_text));
    let elapsed = started.elapsed();
    println!(
        "항목 {}개, 판별 {} ms",
        snapshot.entries.len(),
        elapsed.as_millis()
    );
    assert_eq!(snapshot.entries.len(), 300);
    assert!(snapshot.entries.iter().all(|e| e.listeners.len() == 2));
    // 디버그 빌드 기준의 넉넉한 회귀 한도. 갱신 주기(최소 2초)보다 훨씬 작아야 한다.
    assert!(elapsed.as_millis() < 1_000, "{elapsed:?}");
}

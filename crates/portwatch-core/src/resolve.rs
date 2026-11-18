//! 판별기: 수집한 사실로 스냅샷을 만든다. docs/DESIGN.md의 "판별 순서"·"분류"·"보호 이유"를 따른다.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::parse::{LaunchdJob, ProcessRow, ServiceDetail};
use crate::snapshot::{
    Category, Entry, Listener, Manager, ManagerKind, ProcessInfo, Project, Protection, Snapshot,
};

/// launchd 도메인.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LaunchdDomain {
    System,
    User,
}

impl LaunchdDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
        }
    }
}

/// 상세 정보를 읽을 launchd 서비스.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ServiceTarget {
    pub domain: LaunchdDomain,
    pub label: String,
}

impl ServiceTarget {
    /// `launchctl print`에 넘기는 대상 이름.
    pub fn launchctl_target(&self, uid: u32) -> String {
        match self.domain {
            LaunchdDomain::System => format!("system/{}", self.label),
            LaunchdDomain::User => format!("gui/{uid}/{}", self.label),
        }
    }
}

/// 한 번의 수집에서 모은 사실.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    pub taken_at_ms: u64,
    pub listeners: Vec<Listener>,
    pub processes: Vec<ProcessRow>,
    pub system_jobs: Vec<LaunchdJob>,
    pub user_jobs: Vec<LaunchdJob>,
    pub service_details: HashMap<ServiceTarget, ServiceDetail>,
    /// PID → 작업 폴더.
    pub cwd: HashMap<u32, String>,
    /// 작업 폴더 → 프로젝트. 수집기가 파일을 확인해 채운다.
    pub projects: HashMap<String, Project>,
    pub self_pid: u32,
    pub current_uid: u32,
    /// 보안 프로그램 이름 조각(소문자).
    pub security_names: Vec<String>,
    pub warnings: Vec<String>,
}

/// 첫 수집 결과로 정한 추가 조회 목록.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookups {
    pub services: Vec<ServiceTarget>,
    pub cwd_pids: Vec<u32>,
}

/// 기본 보안 프로그램 이름 조각. 널리 알려진 국내 금융·인증 보안 프로그램이다.
pub const DEFAULT_SECURITY_NAMES: &[&str] = &[
    "veraport",
    "crossex",
    "anysign",
    "nprotect",
    "touchen",
    "delfino",
    "ipinside",
    "magicline",
    "inisafe",
    "astx",
    "wizvera",
    "ksbiz",
];

const SHELLS: &[&str] = &[
    "zsh", "bash", "sh", "fish", "tcsh", "csh", "ksh", "dash", "nu", "pwsh", "xonsh",
];
const SYSTEM_PATHS: &[&str] = &["/System/", "/usr/libexec/", "/usr/sbin/", "/sbin/"];
const MAX_TREE: usize = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Kind {
    LaunchdSocket,
    Launchd,
    Pm2,
    Terminal,
    App,
    Detached,
    Unknown,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Self::LaunchdSocket => "launchd-socket",
            Self::Launchd => "launchd",
            Self::Pm2 => "pm2",
            Self::Terminal => "terminal",
            Self::App => "app",
            Self::Detached => "detached",
            Self::Unknown => "unknown",
        }
    }
}

/// 같은 관리 주체와 최상위 프로세스에 속한 listener 묶음.
#[derive(Debug, Clone)]
struct Group {
    kind: Kind,
    top_pid: u32,
    service: Option<ServiceTarget>,
    app_name: Option<String>,
    /// 최상위가 앱 프로세스 자체인지.
    app_top: bool,
    listeners: Vec<Listener>,
}

struct Index<'a> {
    processes: HashMap<u32, &'a ProcessRow>,
    children: HashMap<u32, Vec<u32>>,
    jobs: HashMap<u32, (LaunchdDomain, &'a str)>,
}

impl<'a> Index<'a> {
    fn new(facts: &'a Facts) -> Self {
        let mut processes = HashMap::new();
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for row in &facts.processes {
            processes.insert(row.pid, row);
            if row.pid != row.ppid {
                children.entry(row.ppid).or_default().push(row.pid);
            }
        }
        let mut jobs = HashMap::new();
        for job in &facts.system_jobs {
            jobs.insert(job.pid, (LaunchdDomain::System, job.label.as_str()));
        }
        for job in &facts.user_jobs {
            jobs.entry(job.pid)
                .or_insert((LaunchdDomain::User, job.label.as_str()));
        }
        Self {
            processes,
            children,
            jobs,
        }
    }

    /// listener 프로세스에서 뿌리(부모가 1인 프로세스)까지.
    fn chain(&self, pid: u32) -> Vec<&'a ProcessRow> {
        let mut chain = Vec::new();
        let mut seen = HashSet::new();
        let mut current = pid;
        while let Some(row) = self.processes.get(&current) {
            if !seen.insert(current) || chain.len() >= 64 {
                break;
            }
            chain.push(*row);
            if row.ppid <= 1 {
                break;
            }
            current = row.ppid;
        }
        chain
    }

    /// 최상위와 그 아래 프로세스(너비 우선, 최대 50개).
    fn tree(&self, top: u32) -> Vec<&'a ProcessRow> {
        let mut rows = Vec::new();
        let mut queue = VecDeque::from([top]);
        let mut seen = HashSet::new();
        while let Some(pid) = queue.pop_front() {
            if rows.len() >= MAX_TREE || !seen.insert(pid) {
                continue;
            }
            if let Some(row) = self.processes.get(&pid) {
                rows.push(*row);
            }
            if let Some(children) = self.children.get(&pid) {
                let mut children = children.clone();
                children.sort_unstable();
                queue.extend(children);
            }
        }
        rows
    }
}

/// 명령줄의 첫 칸.
fn exe_path(row: &ProcessRow) -> &str {
    row.command.split_whitespace().next().unwrap_or("")
}

/// 실행 파일 이름. 로그인 셸의 앞 `-`를 뗀다.
fn exe_name(row: &ProcessRow) -> &str {
    let path = exe_path(row).trim_start_matches('-');
    path.rsplit('/').next().unwrap_or(path)
}

fn is_shell(row: &ProcessRow) -> bool {
    SHELLS.contains(&exe_name(row))
}

fn is_pm2_daemon(row: &ProcessRow) -> bool {
    row.command.contains("PM2") && row.command.contains("God Daemon")
}

fn is_app_process(row: &ProcessRow) -> bool {
    row.command.contains(".app/Contents/")
}

/// `.app/Contents/` 앞의 가장 바깥 앱 이름.
fn app_name(command: &str) -> Option<String> {
    let end = command.find(".app/Contents/")?;
    let name = command[..end].rsplit('/').next()?;
    (!name.is_empty()).then(|| name.to_owned())
}

fn is_system_path(path: &str) -> bool {
    SYSTEM_PATHS.iter().any(|prefix| path.starts_with(prefix))
}

fn is_loopback(address: &str) -> bool {
    address.starts_with("127.") || address == "::1" || address == "localhost"
}

/// brew 경로 아래 실행 파일이면 포뮬러 이름.
fn brew_formula(executable: &str) -> Option<String> {
    for prefix in ["/opt/homebrew/", "/usr/local/"] {
        let Some(rest) = executable.strip_prefix(prefix) else {
            continue;
        };
        for dir in ["opt/", "Cellar/"] {
            if let Some(name) = rest.strip_prefix(dir).and_then(|r| r.split('/').next()) {
                if !name.is_empty() {
                    return Some(name.to_owned());
                }
            }
        }
    }
    None
}

fn classify_listener(index: &Index, listener: &Listener) -> Group {
    let group = |kind, top_pid| Group {
        kind,
        top_pid,
        service: None,
        app_name: None,
        app_top: false,
        listeners: vec![listener.clone()],
    };

    if listener.pid == 1 {
        return group(Kind::LaunchdSocket, 1);
    }
    let chain = index.chain(listener.pid);
    let Some(root) = chain.last() else {
        return group(Kind::Unknown, listener.pid);
    };

    if root.ppid == 1 {
        if let Some((domain, label)) = index.jobs.get(&root.pid) {
            if !label.starts_with("application.") {
                let mut found = group(Kind::Launchd, root.pid);
                found.service = Some(ServiceTarget {
                    domain: *domain,
                    label: (*label).to_owned(),
                });
                return found;
            }
        }
    }

    if let Some(i) = chain.iter().position(|row| is_pm2_daemon(row)) {
        let top = if i > 0 { chain[i - 1] } else { chain[0] };
        return group(Kind::Pm2, top.pid);
    }

    if let Some(mut i) = chain.iter().rposition(|row| is_shell(row)) {
        while i > 0 && is_shell(chain[i - 1]) {
            i -= 1;
        }
        let top = if i > 0 { chain[i - 1] } else { chain[0] };
        return group(Kind::Terminal, top.pid);
    }

    if let Some(highest) = chain.iter().rposition(|row| is_app_process(row)) {
        let name = app_name(&chain[highest].command);
        let mut found = if is_app_process(chain[0]) {
            let mut found = group(Kind::App, chain[highest].pid);
            found.app_top = true;
            found
        } else {
            let lowest = chain
                .iter()
                .position(|row| is_app_process(row))
                .unwrap_or(highest);
            group(Kind::App, chain[lowest.saturating_sub(1)].pid)
        };
        found.app_name = name;
        return found;
    }

    if root.ppid == 1 {
        return group(Kind::Detached, root.pid);
    }
    group(Kind::Unknown, root.pid)
}

fn analyze(index: &Index, facts: &Facts) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for listener in &facts.listeners {
        let found = classify_listener(index, listener);
        match groups
            .iter_mut()
            .find(|g| g.kind == found.kind && g.top_pid == found.top_pid)
        {
            Some(existing) => existing.listeners.push(listener.clone()),
            None => groups.push(found),
        }
    }
    groups
}

/// 프로젝트를 표시할 묶음인지. launchd 서비스와 앱 자체는 작업 폴더가 프로젝트가 아니다
/// (예: brew 서비스의 작업 폴더 `/opt/homebrew`는 Homebrew 저장소다).
fn wants_project(group: &Group) -> bool {
    !matches!(group.kind, Kind::LaunchdSocket | Kind::Launchd) && !group.app_top
}

/// 첫 수집 결과로 추가 조회 대상을 정한다.
pub fn plan_lookups(facts: &Facts) -> Lookups {
    let index = Index::new(facts);
    let groups = analyze(&index, facts);

    let mut services: Vec<ServiceTarget> =
        groups.iter().filter_map(|g| g.service.clone()).collect();
    services.sort();
    services.dedup();

    let mut cwd_pids: Vec<u32> = groups
        .iter()
        .filter(|g| wants_project(g))
        .flat_map(|g| g.listeners.iter().map(|l| l.pid).chain([g.top_pid]))
        .filter(|pid| {
            index
                .processes
                .get(pid)
                .is_some_and(|row| row.uid == facts.current_uid)
        })
        .collect();
    cwd_pids.sort_unstable();
    cwd_pids.dedup();

    Lookups { services, cwd_pids }
}

/// 수집한 사실로 스냅샷을 만든다.
pub fn build_snapshot(facts: &Facts) -> Snapshot {
    let index = Index::new(facts);
    let mut entries: Vec<Entry> = analyze(&index, facts)
        .into_iter()
        .map(|group| make_entry(&index, facts, group))
        .collect();
    entries.sort_by_key(|entry| {
        (
            category_order(entry.category),
            entry.listeners.iter().map(|l| l.port).min().unwrap_or(0),
        )
    });
    Snapshot {
        taken_at_ms: facts.taken_at_ms,
        entries,
        warnings: facts.warnings.clone(),
    }
}

fn category_order(category: Category) -> u8 {
    match category {
        Category::DevServer => 0,
        Category::Service => 1,
        Category::DevTool => 2,
        Category::App => 3,
        Category::System => 4,
    }
}

fn make_entry(index: &Index, facts: &Facts, group: Group) -> Entry {
    let top = index.processes.get(&group.top_pid).copied();
    let detail = group
        .service
        .as_ref()
        .and_then(|target| facts.service_details.get(target));
    let manager = manager(&group, top, detail);

    let chains: Vec<&ProcessRow> = group
        .listeners
        .iter()
        .flat_map(|l| index.chain(l.pid))
        .collect();
    let security = chains.iter().any(|row| matches_security(facts, row));
    let executable = detail
        .and_then(ServiceDetail::executable)
        .map(str::to_owned)
        .or_else(|| top.map(|row| exe_path(row).to_owned()))
        .unwrap_or_default();
    let ephemeral = group
        .listeners
        .iter()
        .all(|l| is_loopback(&l.address) && l.port >= 49152);
    let category = category(&group, &manager, security, &executable, ephemeral);

    let project = wants_project(&group)
        .then(|| {
            group
                .listeners
                .iter()
                .map(|l| l.pid)
                .chain([group.top_pid])
                .find_map(|pid| facts.cwd.get(&pid).and_then(|cwd| facts.projects.get(cwd)))
                .cloned()
        })
        .flatten();
    let title = title(index, &group, &manager, project.as_ref());
    let protection = protection(facts, &group, &manager, category, top);

    // launchd(PID 1) 아래는 모든 프로세스라서 소켓 항목은 PID 1만 둔다.
    let tree = if group.kind == Kind::LaunchdSocket {
        top.into_iter().collect()
    } else {
        index.tree(group.top_pid)
    };
    let mut processes: Vec<ProcessInfo> = tree.into_iter().map(info).collect();
    if processes.is_empty() {
        processes = chains.iter().map(|row| info(row)).collect();
    }

    let mut listeners = group.listeners;
    listeners.sort_by_key(|l| (l.port, l.address.clone()));

    Entry {
        id: format!(
            "{}:{}:{}",
            group.kind.id(),
            group.top_pid,
            top.map(|row| row.started_at.as_str()).unwrap_or("")
        ),
        title,
        category,
        manager,
        listeners,
        processes,
        project,
        domains: Vec::new(),
        protection,
    }
}

fn info(row: &ProcessRow) -> ProcessInfo {
    ProcessInfo {
        pid: row.pid,
        ppid: row.ppid,
        pgid: row.pgid,
        uid: row.uid,
        user: row.user.clone(),
        tty: row.tty.clone(),
        started_at: row.started_at.clone(),
        command: row.command.clone(),
    }
}

fn manager(group: &Group, top: Option<&ProcessRow>, detail: Option<&ServiceDetail>) -> Manager {
    let formula = detail
        .and_then(ServiceDetail::executable)
        .and_then(brew_formula);
    let kind = match group.kind {
        Kind::LaunchdSocket => ManagerKind::LaunchdSocket,
        Kind::Launchd if formula.is_some() => ManagerKind::LaunchdBrew,
        Kind::Launchd => ManagerKind::LaunchdOther,
        Kind::Pm2 => ManagerKind::Pm2,
        Kind::Terminal => ManagerKind::Terminal,
        Kind::App => ManagerKind::App,
        Kind::Detached => ManagerKind::Detached,
        Kind::Unknown => ManagerKind::Unknown,
    };
    Manager {
        kind,
        domain: group
            .service
            .as_ref()
            .map(|target| target.domain.as_str().to_owned()),
        label: group.service.as_ref().map(|target| target.label.clone()),
        formula,
        container: None,
        app_name: group.app_name.clone(),
        runs_as_root: top.map_or(group.top_pid == 1, |row| row.uid == 0),
    }
}

fn matches_security(facts: &Facts, row: &ProcessRow) -> bool {
    let name = exe_name(row).to_lowercase();
    let app = app_name(&row.command).unwrap_or_default().to_lowercase();
    facts
        .security_names
        .iter()
        .any(|needle| name.contains(needle.as_str()) || app.contains(needle.as_str()))
}

fn category(
    group: &Group,
    manager: &Manager,
    security: bool,
    executable: &str,
    ephemeral: bool,
) -> Category {
    if group.kind == Kind::LaunchdSocket || security {
        return Category::System;
    }
    let user_side = |ephemeral: bool| {
        if ephemeral {
            Category::DevTool
        } else {
            Category::DevServer
        }
    };
    match manager.kind {
        ManagerKind::LaunchdBrew => Category::Service,
        ManagerKind::LaunchdOther => {
            let apple = manager
                .label
                .as_deref()
                .is_some_and(|label| label.starts_with("com.apple."));
            if manager.domain.as_deref() == Some("system") || apple || is_system_path(executable) {
                Category::System
            } else if ephemeral {
                Category::DevTool
            } else {
                Category::Service
            }
        }
        ManagerKind::App if group.app_top => {
            if is_system_path(executable) {
                Category::System
            } else {
                Category::App
            }
        }
        ManagerKind::Detached | ManagerKind::Unknown if is_system_path(executable) => {
            Category::System
        }
        _ => user_side(ephemeral),
    }
}

fn title(index: &Index, group: &Group, manager: &Manager, project: Option<&Project>) -> String {
    let fallback = || {
        group
            .listeners
            .first()
            .and_then(|l| index.processes.get(&l.pid))
            .or_else(|| index.processes.get(&group.top_pid))
            .map(|row| exe_name(row).to_owned())
            .unwrap_or_else(|| format!("PID {}", group.top_pid))
    };
    match manager.kind {
        ManagerKind::LaunchdBrew => manager.formula.clone().unwrap_or_else(fallback),
        ManagerKind::LaunchdOther => manager.label.clone().unwrap_or_else(fallback),
        ManagerKind::LaunchdSocket => "launchd".to_owned(),
        ManagerKind::App if group.app_top => manager.app_name.clone().unwrap_or_else(fallback),
        _ => project.map(|p| p.name.clone()).unwrap_or_else(fallback),
    }
}

fn protection(
    facts: &Facts,
    group: &Group,
    manager: &Manager,
    category: Category,
    top: Option<&ProcessRow>,
) -> Option<Protection> {
    let reason = if group.kind == Kind::LaunchdSocket {
        "launchd가 직접 연 포트입니다."
    } else if category == Category::System {
        "시스템·보안 프로그램입니다."
    } else if group.top_pid == facts.self_pid {
        "portwatch 자신입니다."
    } else if top.is_some_and(|row| row.uid != 0 && row.uid != facts.current_uid) {
        "다른 사용자의 프로세스입니다."
    } else if top.is_some_and(|row| row.uid == 0) && manager.kind != ManagerKind::LaunchdBrew {
        "root로 실행 중인 프로세스입니다."
    } else if manager.kind == ManagerKind::LaunchdOther {
        "brew가 아닌 launchd 서비스는 아직 멈추지 않습니다(DC-13)."
    } else {
        return None;
    };
    Some(Protection {
        reason: reason.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(pid: u32, ppid: u32, uid: u32, command: &str) -> ProcessRow {
        ProcessRow {
            pid,
            ppid,
            pgid: pid,
            uid,
            tty: "??".into(),
            user: if uid == 0 {
                "root".into()
            } else {
                "dev".into()
            },
            started_at: "Wed Nov 18 09:00:00 2026".into(),
            command: command.into(),
        }
    }

    fn listen(pid: u32, port: u16, address: &str) -> Listener {
        Listener {
            port,
            address: address.into(),
            family: "tcp4".into(),
            pid,
        }
    }

    #[test]
    fn brew_formula_reads_opt_and_cellar_paths() {
        assert_eq!(
            brew_formula("/opt/homebrew/opt/nginx/bin/nginx").as_deref(),
            Some("nginx")
        );
        assert_eq!(
            brew_formula("/usr/local/Cellar/redis/7.2.4/bin/redis-server").as_deref(),
            Some("redis")
        );
        assert_eq!(brew_formula("/usr/local/bin/node"), None);
        assert_eq!(brew_formula("/Library/Vendor/agentd"), None);
    }

    #[test]
    fn app_name_uses_the_outermost_bundle() {
        assert_eq!(
            app_name("/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper")
                .as_deref(),
            Some("Visual Studio Code")
        );
    }

    #[test]
    fn shell_inside_a_script_does_not_move_the_top() {
        let facts = Facts {
            listeners: vec![listen(13, 5173, "*")],
            processes: vec![
                row(
                    10,
                    1,
                    501,
                    "/System/Applications/Utilities/Terminal.app/Contents/MacOS/Terminal",
                ),
                row(11, 10, 501, "-zsh"),
                row(12, 11, 501, "npm run dev"),
                row(14, 12, 501, "sh -c vite"),
                row(
                    13,
                    14,
                    501,
                    "node /Users/dev/work/web/node_modules/.bin/vite",
                ),
            ],
            user_jobs: vec![LaunchdJob {
                pid: 10,
                label: "application.com.apple.Terminal.1.2".into(),
            }],
            current_uid: 501,
            ..Facts::default()
        };
        let snapshot = build_snapshot(&facts);
        let entry = &snapshot.entries[0];
        assert_eq!(entry.manager.kind, ManagerKind::Terminal);
        assert_eq!(entry.processes[0].pid, 12);
        assert_eq!(entry.category, Category::DevServer);
        assert_eq!(entry.title, "node");
        assert_eq!(entry.protection, None);
    }

    #[test]
    fn self_and_other_users_are_protected() {
        let facts = Facts {
            listeners: vec![listen(20, 7000, "*"), listen(30, 7001, "*")],
            processes: vec![
                row(20, 1, 501, "/usr/local/bin/thing"),
                row(30, 1, 502, "/usr/local/bin/other"),
            ],
            self_pid: 20,
            current_uid: 501,
            ..Facts::default()
        };
        let snapshot = build_snapshot(&facts);
        assert_eq!(
            snapshot.entries[0].protection.as_ref().unwrap().reason,
            "portwatch 자신입니다."
        );
        assert_eq!(
            snapshot.entries[1].protection.as_ref().unwrap().reason,
            "다른 사용자의 프로세스입니다."
        );
    }

    #[test]
    fn missing_process_rows_still_produce_an_unknown_entry() {
        let facts = Facts {
            listeners: vec![listen(99, 9999, "*")],
            ..Facts::default()
        };
        let entry = &build_snapshot(&facts).entries[0];
        assert_eq!(entry.manager.kind, ManagerKind::Unknown);
        assert_eq!(entry.title, "PID 99");
        assert!(entry.processes.is_empty());
    }
}

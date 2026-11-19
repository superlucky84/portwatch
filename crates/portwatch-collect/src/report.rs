//! `portwatch-probe` 기본 출력: 분류별 표.

use portwatch_core::privacy::sanitize;
use portwatch_core::{Category, Entry, ManagerKind, Snapshot};

fn category_name(category: Category) -> &'static str {
    match category {
        Category::DevServer => "개발 서버",
        Category::Service => "서비스",
        Category::DevTool => "개발 도구",
        Category::App => "앱",
        Category::System => "시스템·보안",
    }
}

fn manager_name(entry: &Entry) -> String {
    let manager = &entry.manager;
    let mut name = match manager.kind {
        ManagerKind::LaunchdBrew => "brew 서비스".to_owned(),
        ManagerKind::LaunchdOther => "launchd 서비스".to_owned(),
        ManagerKind::LaunchdSocket => "launchd 소켓".to_owned(),
        ManagerKind::Docker => "Docker".to_owned(),
        ManagerKind::Pm2 => "pm2".to_owned(),
        ManagerKind::Terminal => "터미널".to_owned(),
        ManagerKind::Detached => "분리 실행".to_owned(),
        ManagerKind::App if top_is_app(entry) => "앱".to_owned(),
        ManagerKind::App => format!("{}에서 실행", manager.app_name.as_deref().unwrap_or("앱")),
        ManagerKind::Unknown => "알 수 없음".to_owned(),
    };
    if let Some(domain) = &manager.domain {
        name.push_str(&format!("({domain})"));
    }
    if manager.runs_as_root {
        name.push_str(" root");
    }
    name
}

fn ports(entry: &Entry) -> String {
    let mut ports: Vec<String> = entry
        .listeners
        .iter()
        .map(|l| {
            let scope = if l.address.starts_with("127.") || l.address == "::1" {
                "로컬"
            } else {
                "외부"
            };
            format!(":{} {scope}", l.port)
        })
        .collect();
    ports.dedup();
    ports.join(", ")
}

fn shorten(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short.push('…');
    short
}

/// 최상위 프로세스가 앱 번들 자체인지.
fn top_is_app(entry: &Entry) -> bool {
    entry
        .processes
        .first()
        .is_some_and(|top| top.command.contains(".app/Contents/"))
}

fn user_label(name: &str, current: &str) -> String {
    if !current.is_empty() && name == current {
        "현재 사용자".to_owned()
    } else {
        name.to_owned()
    }
}

/// 분류별 표. 홈 경로를 `~`로 가리고 현재 사용자 이름은 "현재 사용자"로 쓴다.
/// 명령줄 속 단어는 바꾸지 않는다(사용자 이름과 같은 단어가 명령에 있을 수 있다).
pub fn render_table(snapshot: &Snapshot, home: &str, user: &str) -> String {
    let hide = |text: &str| sanitize(text, home, "");
    let mut lines = vec![format!(
        "portwatch 탐지 결과: 항목 {}개, 경고 {}개, 수집 {} ms",
        snapshot.entries.len(),
        snapshot.warnings.len(),
        snapshot.collect_ms
    )];
    for warning in &snapshot.warnings {
        lines.push(format!("  경고: {}", hide(warning)));
    }

    let mut current: Option<Category> = None;
    for entry in &snapshot.entries {
        if current != Some(entry.category) {
            current = Some(entry.category);
            lines.push(format!("\n[{}]", category_name(entry.category)));
        }
        lines.push(format!(
            "  {}  {}  ({})",
            entry.title,
            ports(entry),
            manager_name(entry)
        ));
        if let Some(label) = &entry.manager.label {
            lines.push(format!("      라벨: {label}"));
        }
        if let Some(project) = &entry.project {
            lines.push(format!("      프로젝트: {}", hide(&project.path)));
        }
        if let Some(top) = entry.processes.first() {
            lines.push(format!(
                "      최상위: pid {} {}  {}",
                top.pid,
                user_label(&top.user, user),
                shorten(&hide(&top.command), 70)
            ));
            if entry.processes.len() > 1 {
                lines.push(format!(
                    "      하위 프로세스: {}개",
                    entry.processes.len() - 1
                ));
            }
        }
        if let Some(protection) = &entry.protection {
            lines.push(format!("      보호: {}", protection.reason));
        }
    }
    lines.push(String::new());
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collect_with;
    use crate::fake::*;

    #[test]
    fn table_groups_entries_and_hides_the_home_path() {
        let snapshot = collect_with(&FakeRunner::default(), &files(), &env());
        let table = render_table(&snapshot, "/Users/dev", "dev");
        println!("{table}");
        assert!(table.starts_with("portwatch 탐지 결과: 항목 15개, 경고 0개, 수집 "));
        assert!(table.contains("[개발 서버]\n  api-server  :3000 로컬  (터미널)"));
        assert!(table.contains("  nginx  :8080 외부  (brew 서비스(system) root)"));
        assert!(table.contains("      프로젝트: ~/work/api"));
        assert!(table.contains("최상위: pid 2100 현재 사용자  npm run dev"));
        assert!(table.contains("  veraport  :16105 로컬  (앱)"));
        assert!(table.contains("      최상위: pid 1 root  /sbin/launchd\n      보호:"));
        assert!(table.contains("      보호: 시스템·보안 프로그램입니다."));
        assert!(!table.contains("/Users/dev"));
    }
}

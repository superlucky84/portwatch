//! 숨기기·고정 적용과 메뉴바 숫자 계산.

use crate::resolve::sort_entries;
use crate::settings::Settings;
use crate::snapshot::{Category, Snapshot};

/// 수집한 그대로의 스냅샷에 설정을 적용한 사본을 만든다. 고정한 항목은 개발 서버로 보인다(보호는 그대로).
pub fn apply(raw: &Snapshot, settings: &Settings) -> Snapshot {
    let mut snapshot = raw.clone();
    for entry in &mut snapshot.entries {
        entry.hidden = settings.hidden.contains(&entry.key);
        entry.pinned = settings.pinned.contains(&entry.key);
        if entry.pinned {
            entry.category = Category::DevServer;
        }
    }
    sort_entries(&mut snapshot.entries);
    snapshot
}

/// 메뉴바 아이콘 옆 숫자: 숨기지 않은 개발 서버·서비스 수.
pub fn badge_count(snapshot: &Snapshot) -> usize {
    snapshot
        .entries
        .iter()
        .filter(|entry| {
            !entry.hidden && matches!(entry.category, Category::DevServer | Category::Service)
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Entry, Listener, Manager, ManagerKind};

    fn entry(key: &str, category: Category, port: u16) -> Entry {
        Entry {
            id: key.into(),
            key: key.into(),
            title: key.into(),
            category,
            manager: Manager {
                kind: ManagerKind::Terminal,
                domain: None,
                label: None,
                formula: None,
                container: None,
                app_name: None,
                runs_as_root: false,
            },
            listeners: vec![Listener {
                port,
                address: "*".into(),
                family: "tcp4".into(),
                pid: 1000 + u32::from(port),
            }],
            processes: Vec::new(),
            project: None,
            domains: Vec::new(),
            protection: None,
            hidden: false,
            pinned: false,
        }
    }

    #[test]
    fn hidden_entries_leave_the_badge_and_pinned_entries_become_dev_servers() {
        let raw = Snapshot {
            taken_at_ms: 1,
            collect_ms: 0,
            entries: vec![
                entry("project:a", Category::DevServer, 3000),
                entry("launchd:svc", Category::Service, 8080),
                entry("exe:ccmux", Category::DevTool, 2269),
            ],
            warnings: Vec::new(),
        };
        assert_eq!(badge_count(&raw), 2);

        let mut settings = Settings::default();
        settings.set_hidden("launchd:svc", true);
        settings.set_pinned("exe:ccmux", true);
        let applied = apply(&raw, &settings);

        assert_eq!(
            badge_count(&applied),
            2,
            "숨긴 서비스는 빠지고 고정한 도구가 들어온다"
        );
        let keys: Vec<&str> = applied.entries.iter().map(|e| e.key.as_str()).collect();
        assert_eq!(
            keys,
            ["exe:ccmux", "project:a", "launchd:svc"],
            "고정 후 다시 정렬"
        );
        assert!(applied.entries[0].pinned);
        assert!(applied.entries[2].hidden);

        let unpinned = apply(&raw, &Settings::default());
        assert_eq!(
            unpinned.entries[2].category,
            Category::DevTool,
            "원본은 그대로"
        );
    }
}

//! 스냅샷 계약. docs/DESIGN.md의 "스냅샷 계약" 절과 같은 모양을 유지한다.
//!
//! 멈추기 계획 요약(stopPlan)은 Phase 3에서 추가한다.

use serde::{Deserialize, Serialize};

/// 수집 한 번의 결과.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// 수집 시각(Unix epoch 기준 밀리초).
    pub taken_at_ms: u64,
    pub entries: Vec<Entry>,
    /// 실패한 명령이나 읽지 못한 정보. 화면에 그대로 보여준다.
    pub warnings: Vec<String>,
}

impl Snapshot {
    /// 항목과 경고가 없는 스냅샷.
    pub fn empty(taken_at_ms: u64) -> Self {
        Self {
            taken_at_ms,
            entries: Vec::new(),
            warnings: Vec::new(),
        }
    }
}

/// 포트를 연 서버 하나. 프로세스 트리의 최상위 프로세스를 기준으로 묶는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    /// 관리 주체, 최상위 프로세스 PID, 시작 시각으로 만든 값.
    pub id: String,
    pub title: String,
    pub category: Category,
    pub manager: Manager,
    pub listeners: Vec<Listener>,
    /// 최상위 프로세스부터 차례로.
    pub processes: Vec<ProcessInfo>,
    pub project: Option<Project>,
    pub domains: Vec<Domain>,
    /// 멈추기를 막는 경우 그 이유.
    pub protection: Option<Protection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    DevServer,
    Service,
    DevTool,
    App,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manager {
    pub kind: ManagerKind,
    /// launchd 도메인: "system" 또는 "user".
    pub domain: Option<String>,
    /// launchd 라벨(예: sh.brew.nginx).
    pub label: Option<String>,
    /// brew 포뮬러 이름(예: nginx).
    pub formula: Option<String>,
    /// Docker 컨테이너 이름.
    pub container: Option<String>,
    /// 앱에서 실행한 경우 그 앱 이름.
    pub app_name: Option<String>,
    /// 최상위 프로세스가 root로 실행되는지.
    pub runs_as_root: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagerKind {
    LaunchdBrew,
    LaunchdOther,
    Docker,
    Pm2,
    Terminal,
    Detached,
    App,
    LaunchdSocket,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Listener {
    pub port: u16,
    /// netstat에 나온 주소 부분(예: "127.0.0.1", "*", "::1").
    pub address: String,
    /// "tcp4", "tcp6", "tcp46".
    pub family: String,
    pub pid: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessInfo {
    pub pid: u32,
    pub ppid: u32,
    pub user: String,
    /// ps가 보여준 시작 시각 원문. PID 재사용을 가려내는 데 쓴다.
    pub started_at: String,
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Domain {
    pub name: String,
    pub via: DomainVia,
    pub port: u16,
    pub tls: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DomainVia {
    Hosts,
    Nginx,
    Direct,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Protection {
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_snapshot_uses_contract_field_names() {
        let value = serde_json::to_value(Snapshot::empty(1_700_000_000_000)).unwrap();
        assert_eq!(
            value,
            json!({ "takenAtMs": 1_700_000_000_000u64, "entries": [], "warnings": [] })
        );
    }

    #[test]
    fn entry_serializes_kinds_in_kebab_case() {
        let entry = Entry {
            id: "launchd-brew:100:Wed Nov 18 09:00:00 2026".into(),
            title: "nginx".into(),
            category: Category::Service,
            manager: Manager {
                kind: ManagerKind::LaunchdBrew,
                domain: Some("system".into()),
                label: Some("sh.brew.nginx".into()),
                formula: Some("nginx".into()),
                container: None,
                app_name: None,
                runs_as_root: true,
            },
            listeners: vec![Listener {
                port: 8080,
                address: "*".into(),
                family: "tcp4".into(),
                pid: 101,
            }],
            processes: Vec::new(),
            project: None,
            domains: vec![Domain {
                name: "shop.example.test".into(),
                via: DomainVia::Nginx,
                port: 443,
                tls: true,
            }],
            protection: None,
        };

        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value["category"], "service");
        assert_eq!(value["manager"]["kind"], "launchd-brew");
        assert_eq!(value["manager"]["runsAsRoot"], true);
        assert_eq!(value["domains"][0]["via"], "nginx");

        let back: Entry = serde_json::from_value(value).unwrap();
        assert_eq!(back, entry);
    }
}

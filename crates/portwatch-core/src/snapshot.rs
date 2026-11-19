//! 스냅샷 계약. docs/DESIGN.md의 "스냅샷 계약" 절과 같은 모양을 유지한다.

use serde::{Deserialize, Serialize};

use crate::stop::StopPlan;

/// 수집 한 번의 결과.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// 수집 시각(Unix epoch 기준 밀리초).
    pub taken_at_ms: u64,
    /// 수집에 걸린 시간(밀리초).
    #[serde(default)]
    pub collect_ms: u64,
    pub entries: Vec<Entry>,
    /// 실패한 명령이나 읽지 못한 정보. 화면에 그대로 보여준다.
    pub warnings: Vec<String>,
}

impl Snapshot {
    /// 항목과 경고가 없는 스냅샷.
    pub fn empty(taken_at_ms: u64) -> Self {
        Self {
            taken_at_ms,
            collect_ms: 0,
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
    /// 다시 실행해도 같은 항목 기준. 숨기기·고정에 쓴다.
    pub key: String,
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
    /// 멈추기 계획. 보호 항목이나 멈추는 방법이 없는 항목은 없음.
    #[serde(default)]
    pub stop_plan: Option<StopPlan>,
    /// 사용자가 숨긴 항목.
    #[serde(default)]
    pub hidden: bool,
    /// 사용자가 "개발 서버로 고정"한 항목.
    #[serde(default)]
    pub pinned: bool,
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
    /// 프로세스 그룹. 터미널 서버를 멈출 때 그룹 단위로 신호를 보낸다.
    pub pgid: u32,
    pub uid: u32,
    pub user: String,
    /// 터미널 이름. 터미널이 없으면 `??`.
    pub tty: String,
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

/// 이 항목에 닿는 주소 하나. docs/DESIGN.md "도메인 연결".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Domain {
    pub name: String,
    pub via: DomainVia,
    pub scheme: Scheme,
    /// 브라우저가 쓰는 포트.
    pub port: u16,
    /// nginx location 경로. 기본 `/`.
    pub path: String,
    /// nginx가 넘기는 루프백 포트. direct는 없음.
    pub upstream: Option<u16>,
    /// hosts가 이 이름을 루프백으로 가리키는지. `localhost`는 항상 참.
    pub in_hosts: bool,
    /// https일 때 읽은 인증서.
    pub tls: Option<Cert>,
}

impl Domain {
    /// 브라우저로 열 주소. 기본 포트(http 80, https 443)는 붙이지 않는다.
    pub fn url(&self) -> String {
        let default_port = match self.scheme {
            Scheme::Http => 80,
            Scheme::Https => 443,
        };
        let scheme = match self.scheme {
            Scheme::Http => "http",
            Scheme::Https => "https",
        };
        if self.port == default_port {
            format!("{scheme}://{}{}", self.name, self.path)
        } else {
            format!("{scheme}://{}:{}{}", self.name, self.port, self.path)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DomainVia {
    /// 이 항목이 nginx이고 upstream으로 넘긴다.
    Nginx,
    /// nginx를 거쳐 이 항목에 닿는다.
    Proxied,
    /// 이 항목이 80·443을 직접 연다.
    Direct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scheme {
    Http,
    Https,
}

/// 루프백 TLS 확인으로 읽은 인증서.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cert {
    pub subject: String,
    pub sans: Vec<String>,
    pub issuer: String,
    pub mkcert: bool,
    pub not_after_ms: Option<u64>,
    pub warnings: Vec<String>,
    /// 읽지 못한 이유. 있으면 나머지는 비어 있다.
    pub error: Option<String>,
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
            json!({ "takenAtMs": 1_700_000_000_000u64, "collectMs": 0, "entries": [], "warnings": [] })
        );
    }

    #[test]
    fn entry_serializes_kinds_in_kebab_case() {
        let entry = Entry {
            id: "launchd-brew:100:Wed Nov 18 09:00:00 2026".into(),
            key: "launchd:sh.brew.nginx".into(),
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
                scheme: Scheme::Https,
                port: 443,
                path: "/".into(),
                upstream: Some(3000),
                in_hosts: true,
                tls: None,
            }],
            protection: None,
            stop_plan: None,
            hidden: false,
            pinned: true,
        };

        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value["category"], "service");
        assert_eq!(value["manager"]["kind"], "launchd-brew");
        assert_eq!(value["manager"]["runsAsRoot"], true);
        assert_eq!(value["domains"][0]["via"], "nginx");
        assert_eq!(value["domains"][0]["scheme"], "https");
        assert_eq!(value["domains"][0]["inHosts"], true);
        assert_eq!(value["key"], "launchd:sh.brew.nginx");
        assert_eq!(value["pinned"], true);
        assert_eq!(value["stopPlan"], serde_json::Value::Null);

        let back: Entry = serde_json::from_value(value).unwrap();
        assert_eq!(back, entry);
    }

    #[test]
    fn domain_url_drops_the_default_port() {
        let mut domain = Domain {
            name: "shop.example.test".into(),
            via: DomainVia::Nginx,
            scheme: Scheme::Https,
            port: 443,
            path: "/".into(),
            upstream: Some(3000),
            in_hosts: true,
            tls: None,
        };
        assert_eq!(domain.url(), "https://shop.example.test/");
        domain.scheme = Scheme::Http;
        domain.port = 8080;
        domain.path = "/api".into();
        assert_eq!(domain.url(), "http://shop.example.test:8080/api");
    }
}

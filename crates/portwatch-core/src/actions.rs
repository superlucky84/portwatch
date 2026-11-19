//! 화면에서 요청한 동작의 입력 검증. 실행은 셸이 한다(docs/DESIGN.md "화면에 노출하는 명령").

use crate::snapshot::Snapshot;

/// 클립보드에 넣을 수 있는 최대 글자 수.
pub const MAX_COPY_CHARS: usize = 2_000;

/// `http(s)://localhost:<포트>` 또는 `127.0.0.1`·`[::1]` 주소만 허용한다. 경로·질의·사용자 정보는 받지 않는다.
pub fn validate_open_url(url: &str) -> Result<String, String> {
    let reject = || Err(format!("열 수 없는 주소입니다: {url}"));
    let rest = match url.split_once("://") {
        Some(("http" | "https", rest)) => rest,
        _ => return reject(),
    };
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    let (host, port) = match authority.rsplit_once(':') {
        Some(pair) => pair,
        None => return reject(),
    };
    if !matches!(host, "localhost" | "127.0.0.1" | "[::1]") {
        return reject();
    }
    let valid_port = !port.is_empty()
        && port.len() <= 5
        && port.bytes().all(|b| b.is_ascii_digit())
        && port.parse::<u32>().is_ok_and(|p| (1..=65_535).contains(&p));
    if !valid_port {
        return reject();
    }
    Ok(url.to_owned())
}

/// 루프백 주소이거나, 스냅샷의 주소 중 hosts가 루프백으로 가리키는 것의 URL만 허용한다(Phase 4).
/// hosts에 없는 이름은 바깥 DNS로 풀려 외부로 나갈 수 있어 열지 않는다.
pub fn validate_open_url_in(url: &str, snapshot: Option<&Snapshot>) -> Result<String, String> {
    if let Ok(url) = validate_open_url(url) {
        return Ok(url);
    }
    let known = snapshot.is_some_and(|snapshot| {
        snapshot
            .entries
            .iter()
            .flat_map(|e| &e.domains)
            .any(|d| d.in_hosts && d.url() == url)
    });
    let plain = url
        .bytes()
        .all(|b| b.is_ascii_graphic() && !b"\"'<>\\`".contains(&b));
    if known && plain {
        Ok(url.to_owned())
    } else {
        Err(format!("열 수 없는 주소입니다: {url}"))
    }
}

/// 클립보드에 넣을 글자. 너무 길거나 NUL이 있으면 거부한다.
pub fn validate_copy_text(text: &str) -> Result<&str, String> {
    if text.is_empty() || text.contains('\0') || text.chars().count() > MAX_COPY_CHARS {
        return Err("복사할 수 없는 내용입니다.".to_owned());
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_ports_can_be_opened() {
        for ok in [
            "http://localhost:5173",
            "http://localhost:5173/",
            "https://127.0.0.1:8443",
            "http://[::1]:3000",
        ] {
            assert_eq!(validate_open_url(ok).as_deref(), Ok(ok), "{ok}");
        }
        for bad in [
            "http://example.test:80",
            "file:///etc/hosts",
            "http://localhost",
            "http://localhost:0",
            "http://localhost:70000",
            "http://localhost:80/admin",
            "http://localhost:80?x=1",
            "http://user@localhost:80",
            "http://localhost:80@evil.test:80",
            "javascript:alert(1)",
            "http://localhost:+80",
        ] {
            assert!(validate_open_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn hosts_addresses_from_the_snapshot_can_be_opened() {
        let snapshot: Snapshot = serde_json::from_value(serde_json::json!({
            "takenAtMs": 0, "warnings": [],
            "entries": [{
                "id": "a", "key": "a", "title": "nginx", "category": "service",
                "manager": { "kind": "launchd-brew", "domain": "system", "label": null, "formula": null,
                             "container": null, "appName": null, "runsAsRoot": true },
                "listeners": [], "processes": [], "project": null, "protection": null,
                "domains": [
                    { "name": "shop.example.test", "via": "nginx", "scheme": "https", "port": 443,
                      "path": "/", "upstream": 5173, "inHosts": true, "tls": null },
                    { "name": "intranet.example.test", "via": "nginx", "scheme": "http", "port": 8080,
                      "path": "/", "upstream": 3000, "inHosts": false, "tls": null }
                ]
            }]
        }))
        .unwrap();
        assert_eq!(
            validate_open_url_in("https://shop.example.test/", Some(&snapshot)).as_deref(),
            Ok("https://shop.example.test/")
        );
        assert_eq!(
            validate_open_url_in("http://localhost:3000", None).as_deref(),
            Ok("http://localhost:3000")
        );
        for bad in [
            "http://intranet.example.test:8080/",
            "https://shop.example.test/admin",
            "https://other.example.test/",
        ] {
            assert!(validate_open_url_in(bad, Some(&snapshot)).is_err(), "{bad}");
        }
        assert!(validate_open_url_in("https://shop.example.test/", None).is_err());
    }

    #[test]
    fn copy_text_has_limits() {
        assert_eq!(
            validate_copy_text("http://localhost:3000"),
            Ok("http://localhost:3000")
        );
        assert!(validate_copy_text("").is_err());
        assert!(validate_copy_text("a\0b").is_err());
        assert!(validate_copy_text(&"가".repeat(MAX_COPY_CHARS + 1)).is_err());
        assert!(validate_copy_text(&"가".repeat(MAX_COPY_CHARS)).is_ok());
    }
}

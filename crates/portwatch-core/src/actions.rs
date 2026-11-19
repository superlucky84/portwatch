//! 화면에서 요청한 동작의 입력 검증. 실행은 셸이 한다(docs/DESIGN.md "화면에 노출하는 명령").

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

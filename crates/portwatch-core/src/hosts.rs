//! `/etc/hosts`에서 루프백을 가리키는 이름을 모은다. docs/DESIGN.md "도메인 연결" 1.

use std::collections::BTreeSet;
use std::net::IpAddr;

/// 루프백(`127.0.0.0/8`, `::1`)을 가리키는 이름(소문자). `localhost`·`broadcasthost`와 주석은 뺀다.
pub fn loopback_names(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("");
        let mut fields = line.split_whitespace();
        let Some(address) = fields.next() else {
            continue;
        };
        if !address.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()) {
            continue;
        }
        for name in fields {
            let name = name.trim_end_matches('.').to_ascii_lowercase();
            if !matches!(name.as_str(), "localhost" | "broadcasthost") && is_host_name(&name) {
                names.insert(name);
            }
        }
    }
    names
}

/// 글자·숫자·`-`·`.`로만 된 이름인지.
pub fn is_host_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_loopback_names() {
        let text = "\
##
# Host Database
127.0.0.1\tlocalhost
255.255.255.255\tbroadcasthost
::1             localhost
127.0.0.1 shop.example.test api.example.test # 개발
127.0.1.1   Admin.Example.Test.
10.0.0.5 intranet.example.test
::1 v6.example.test
#127.0.0.1 off.example.test
127.0.0.1 bad$name.test
";
        let names: Vec<String> = loopback_names(text).into_iter().collect();
        assert_eq!(
            names,
            [
                "admin.example.test",
                "api.example.test",
                "shop.example.test",
                "v6.example.test"
            ]
        );
    }
}

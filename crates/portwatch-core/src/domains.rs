//! 주소를 항목에 붙인다. docs/DESIGN.md "도메인 연결" 3.

use std::collections::{BTreeSet, HashSet};

use crate::nginx::Route;
use crate::snapshot::{Domain, DomainVia, Entry, Scheme};

/// nginx 경로와 hosts 이름을 항목의 `domains`에 채운다.
pub fn attach(entries: &mut [Entry], routes: &[Route], hosts: &BTreeSet<String>) {
    // 설정에만 있고 실제로 nginx가 열지 않은 포트의 경로는 쓰지 않는다.
    let served: HashSet<u16> = entries
        .iter()
        .filter(|entry| is_nginx(entry))
        .flat_map(|entry| entry.listeners.iter().map(|l| l.port))
        .collect();
    let routes: Vec<&Route> = routes.iter().filter(|r| served.contains(&r.port)).collect();

    for entry in entries.iter_mut() {
        let nginx = is_nginx(entry);
        let ports: HashSet<u16> = entry.listeners.iter().map(|l| l.port).collect();
        let mut domains = Vec::new();
        for route in &routes {
            let via = if nginx && ports.contains(&route.port) {
                DomainVia::Nginx
            } else if !nginx && ports.contains(&route.upstream) {
                DomainVia::Proxied
            } else {
                continue;
            };
            domains.push(Domain {
                name: route.name.clone(),
                via,
                scheme: if route.https {
                    Scheme::Https
                } else {
                    Scheme::Http
                },
                port: route.port,
                path: route.path.clone(),
                upstream: Some(route.upstream),
                in_hosts: route.name == "localhost" || hosts.contains(&route.name),
                tls: None,
            });
        }
        if !nginx {
            for (port, scheme) in [(80, Scheme::Http), (443, Scheme::Https)] {
                if !ports.contains(&port) {
                    continue;
                }
                domains.extend(hosts.iter().map(|name| Domain {
                    name: name.clone(),
                    via: DomainVia::Direct,
                    scheme,
                    port,
                    path: "/".to_owned(),
                    upstream: None,
                    in_hosts: true,
                    tls: None,
                }));
            }
        }
        domains.sort_by(|a, b| {
            (&a.name, a.port, &a.path, a.via, a.scheme, a.upstream)
                .cmp(&(&b.name, b.port, &b.path, b.via, b.scheme, b.upstream))
        });
        domains.dedup();
        entry.domains = domains;
    }
}

/// nginx master나 worker가 든 항목인지.
pub fn is_nginx(entry: &Entry) -> bool {
    entry
        .processes
        .iter()
        .any(|p| p.command.starts_with("nginx: "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(title: &str, ports: &[u16], command: &str) -> Entry {
        serde_json::from_value(json!({
            "id": title, "key": title, "title": title, "category": "dev-server",
            "manager": { "kind": "terminal", "domain": null, "label": null, "formula": null,
                         "container": null, "appName": null, "runsAsRoot": false },
            "listeners": ports.iter().map(|p| json!({ "port": p, "address": "*", "family": "tcp4", "pid": 10 })).collect::<Vec<_>>(),
            "processes": [{ "pid": 10, "ppid": 1, "pgid": 10, "uid": 501, "user": "dev", "tty": "ttys001",
                            "startedAt": "Wed Nov 18 09:00:00 2026", "command": command }],
            "project": null, "domains": [], "protection": null
        }))
        .unwrap()
    }

    #[test]
    fn servers_on_80_and_443_get_every_hosts_name_directly() {
        let hosts = BTreeSet::from(["b.example.test".to_owned(), "a.example.test".to_owned()]);
        let mut entries = [
            entry("web", &[443, 3000], "node server.js"),
            entry("other", &[8443], "node other.js"),
        ];
        attach(&mut entries, &[], &hosts);
        let urls: Vec<String> = entries[0].domains.iter().map(Domain::url).collect();
        assert_eq!(urls, ["https://a.example.test/", "https://b.example.test/"]);
        assert!(entries[0]
            .domains
            .iter()
            .all(|d| d.via == DomainVia::Direct));
        assert!(entries[1].domains.is_empty());
    }

    #[test]
    fn nginx_itself_does_not_get_direct_names() {
        let hosts = BTreeSet::from(["a.example.test".to_owned()]);
        let mut entries = [entry(
            "nginx",
            &[80],
            "nginx: master process /opt/homebrew/bin/nginx",
        )];
        attach(&mut entries, &[], &hosts);
        assert!(entries[0].domains.is_empty());
    }
}

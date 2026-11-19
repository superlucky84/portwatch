//! nginx 설정에서 "주소 → listen 포트 → 루프백 upstream 포트"를 읽는다. docs/DESIGN.md "도메인 연결" 2.
//!
//! 파일 읽기와 `include` 패턴 펼치기는 수집기가 하고, 여기서는 읽은 글자만 다룬다.

use std::collections::HashMap;

use crate::parse::ProcessRow;

/// nginx master 프로세스 명령줄의 앞부분.
pub const MASTER_PREFIX: &str = "nginx: master process ";
/// `include`를 따라가는 최대 깊이.
pub const MAX_DEPTH: usize = 8;
/// 읽는 설정 파일 최대 개수.
pub const MAX_FILES: usize = 64;

const BREW_CONFS: [(&str, &str); 2] = [
    ("/opt/homebrew/", "/opt/homebrew/etc/nginx/nginx.conf"),
    ("/usr/local/", "/usr/local/etc/nginx/nginx.conf"),
];

/// 설정에서 읽은 경로 하나.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Route {
    /// 브라우저가 쓰는 이름(소문자).
    pub name: String,
    pub port: u16,
    pub https: bool,
    /// location 경로.
    pub path: String,
    /// 넘기는 루프백 포트.
    pub upstream: u16,
}

/// 수집기가 읽은 설정 파일들.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigFiles {
    /// 절대 경로 → 내용.
    pub texts: HashMap<String, String>,
    /// `*`가 든 절대 경로 패턴 → 맞는 파일 경로(이름순).
    pub globs: HashMap<String, Vec<String>>,
}

/// 실행 중인 nginx master들의 설정 파일 경로. 위치를 알 수 없으면 `Err`에 경고 문장을 담는다.
pub fn conf_paths(processes: &[ProcessRow]) -> Vec<Result<String, String>> {
    let mut paths: Vec<Result<String, String>> = processes
        .iter()
        .filter_map(|row| row.command.strip_prefix(MASTER_PREFIX))
        .map(conf_path)
        .collect();
    paths.sort();
    paths.dedup();
    paths
}

/// master 명령줄(`<실행 파일> [인수…]`)에서 설정 파일 경로를 정한다.
fn conf_path(command: &str) -> Result<String, String> {
    let mut words = command.split_whitespace();
    let executable = words.next().unwrap_or("");
    let words: Vec<&str> = words.collect();
    if let Some(index) = words.iter().position(|w| *w == "-c") {
        return match words.get(index + 1) {
            Some(path) if path.starts_with('/') && !path.contains("..") => Ok((*path).to_owned()),
            _ => Err("nginx 설정 파일 위치(-c)를 읽지 못했습니다.".to_owned()),
        };
    }
    BREW_CONFS
        .iter()
        .find(|(prefix, _)| executable.starts_with(prefix))
        .map(|(_, conf)| (*conf).to_owned())
        .ok_or_else(|| format!("nginx 설정 파일 위치를 알 수 없습니다: {executable}"))
}

/// 설정 파일 폴더. `include`의 상대 경로 기준이다.
pub fn conf_dir(conf: &str) -> &str {
    conf.rsplit_once('/').map_or("/", |(dir, _)| dir)
}

/// 글자 안의 모든 `include` 인수를 절대 경로로 돌려준다.
pub fn include_paths(text: &str, conf_dir: &str) -> Vec<String> {
    let mut paths = Vec::new();
    collect_includes(&parse(text), conf_dir, &mut paths);
    paths
}

fn collect_includes(directives: &[Directive], conf_dir: &str, out: &mut Vec<String>) {
    for directive in directives {
        if directive.name == "include" {
            if let Some(arg) = directive.args.first() {
                out.push(absolute(arg, conf_dir));
            }
        }
        if let Some(block) = &directive.block {
            collect_includes(block, conf_dir, out);
        }
    }
}

fn absolute(path: &str, conf_dir: &str) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("{}/{path}", conf_dir.trim_end_matches('/'))
    }
}

/// 경로 패턴인지(마지막 조각의 `*`·`?`만 지원한다).
pub fn is_pattern(path: &str) -> bool {
    path.contains(['*', '?'])
}

/// 패턴 `dir/name*`을 (폴더, 이름 패턴)으로 나눈다. 폴더에 패턴이 있으면 없음.
pub fn split_pattern(pattern: &str) -> Option<(&str, &str)> {
    let (dir, name) = pattern.rsplit_once('/')?;
    (!is_pattern(dir)).then_some((if dir.is_empty() { "/" } else { dir }, name))
}

/// 파일 이름이 `*`·`?` 패턴과 맞는지. `.`으로 시작하는 이름은 패턴이 `.`으로 시작할 때만 맞는다.
pub fn name_matches(pattern: &str, name: &str) -> bool {
    if name.starts_with('.') && !pattern.starts_with('.') {
        return false;
    }
    fn go(p: &[u8], n: &[u8]) -> bool {
        match (p.first(), n.first()) {
            (None, None) => true,
            (Some(b'*'), _) => go(&p[1..], n) || (!n.is_empty() && go(p, &n[1..])),
            (Some(b'?'), Some(_)) => go(&p[1..], &n[1..]),
            (Some(a), Some(b)) if a == b => go(&p[1..], &n[1..]),
            _ => false,
        }
    }
    go(pattern.as_bytes(), name.as_bytes())
}

/// 설정 전체에서 루프백으로 넘기는 경로를 읽는다. 빠진 파일·깊이 초과는 경고로 돌려준다.
pub fn routes(conf: &str, files: &ConfigFiles) -> (Vec<Route>, Vec<String>) {
    let mut warnings = Vec::new();
    let Some(text) = files.texts.get(conf) else {
        return (
            Vec::new(),
            vec![format!("nginx 설정을 읽지 못했습니다: {conf}")],
        );
    };
    let tree = expand(parse(text), files, conf_dir(conf), 0, &mut warnings);
    let mut routes = Vec::new();
    for http in tree.iter().filter(|d| d.name == "http") {
        let block = http.block.as_deref().unwrap_or_default();
        let upstreams = upstreams(block);
        for server in block.iter().filter(|d| d.name == "server") {
            server_routes(
                server.block.as_deref().unwrap_or_default(),
                &upstreams,
                &mut routes,
            );
        }
    }
    routes.sort();
    routes.dedup();
    warnings.sort();
    warnings.dedup();
    (routes, warnings)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Directive {
    name: String,
    args: Vec<String>,
    block: Option<Vec<Directive>>,
}

/// `include`를 읽은 파일 내용으로 바꾼다.
fn expand(
    directives: Vec<Directive>,
    files: &ConfigFiles,
    conf_dir: &str,
    depth: usize,
    warnings: &mut Vec<String>,
) -> Vec<Directive> {
    let mut out = Vec::new();
    for mut directive in directives {
        if directive.name == "include" && directive.block.is_none() {
            let Some(arg) = directive.args.first() else {
                continue;
            };
            if depth >= MAX_DEPTH {
                warnings.push(format!("nginx include가 너무 깊어 건너뛰었습니다: {arg}"));
                continue;
            }
            let path = absolute(arg, conf_dir);
            let targets = if is_pattern(&path) {
                files.globs.get(&path).cloned().unwrap_or_default()
            } else {
                vec![path]
            };
            for target in targets {
                match files.texts.get(&target) {
                    Some(text) => {
                        out.extend(expand(parse(text), files, conf_dir, depth + 1, warnings))
                    }
                    None => warnings.push(format!("nginx 설정을 읽지 못했습니다: {target}")),
                }
            }
            continue;
        }
        if let Some(block) = directive.block.take() {
            directive.block = Some(expand(block, files, conf_dir, depth, warnings));
        }
        out.push(directive);
    }
    out
}

/// upstream 이름 → 루프백 server 포트.
fn upstreams(http: &[Directive]) -> HashMap<String, Vec<u16>> {
    let mut map = HashMap::new();
    for upstream in http.iter().filter(|d| d.name == "upstream") {
        let Some(name) = upstream.args.first() else {
            continue;
        };
        let ports: Vec<u16> = upstream
            .block
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|d| d.name == "server")
            .filter_map(|d| d.args.first())
            .filter_map(|address| loopback_port(address, 80))
            .collect();
        map.insert(name.to_ascii_lowercase(), ports);
    }
    map
}

fn server_routes(
    server: &[Directive],
    upstreams: &HashMap<String, Vec<u16>>,
    out: &mut Vec<Route>,
) {
    let mut listens: Vec<(u16, bool)> = server
        .iter()
        .filter(|d| d.name == "listen")
        .filter_map(|d| listen(&d.args))
        .collect();
    if listens.is_empty() && !server.iter().any(|d| d.name == "listen") {
        listens.push((80, false));
    }
    let mut names: Vec<String> = server
        .iter()
        .filter(|d| d.name == "server_name")
        .flat_map(|d| d.args.iter())
        .map(|name| name.trim_end_matches('.').to_ascii_lowercase())
        .filter(|name| crate::hosts::is_host_name(name))
        .collect();
    if names.is_empty() {
        names.push("localhost".to_owned());
    }
    let mut targets = Vec::new();
    locations(server, upstreams, &mut targets);
    for (port, https) in &listens {
        for name in &names {
            for (path, upstream) in &targets {
                out.push(Route {
                    name: name.clone(),
                    port: *port,
                    https: *https,
                    path: path.clone(),
                    upstream: *upstream,
                });
            }
        }
    }
}

/// location 경로와 넘기는 포트. 정규식·이름 location은 건너뛴다. 안쪽 location도 읽는다.
fn locations(
    block: &[Directive],
    upstreams: &HashMap<String, Vec<u16>>,
    out: &mut Vec<(String, u16)>,
) {
    for location in block.iter().filter(|d| d.name == "location") {
        let path = match location.args.as_slice() {
            [path] if path.starts_with('/') => path,
            [modifier, path]
                if matches!(modifier.as_str(), "=" | "^~") && path.starts_with('/') =>
            {
                path
            }
            _ => continue,
        };
        let inner = location.block.as_deref().unwrap_or_default();
        for target in inner
            .iter()
            .filter(|d| d.name == "proxy_pass")
            .filter_map(|d| d.args.first())
        {
            for port in proxy_ports(target, upstreams) {
                out.push((path.clone(), port));
            }
        }
        locations(inner, upstreams, out);
    }
}

/// `proxy_pass`의 대상이 루프백이면 그 포트, upstream 이름이면 그 루프백 포트들.
fn proxy_ports(target: &str, upstreams: &HashMap<String, Vec<u16>>) -> Vec<u16> {
    if target.contains('$') {
        return Vec::new();
    }
    let (rest, default_port) = if let Some(rest) = target.strip_prefix("http://") {
        (rest, 80)
    } else if let Some(rest) = target.strip_prefix("https://") {
        (rest, 443)
    } else {
        return Vec::new();
    };
    let authority = rest.split('/').next().unwrap_or("");
    if let Some(port) = loopback_port(authority, default_port) {
        return vec![port];
    }
    upstreams
        .get(&authority.to_ascii_lowercase())
        .cloned()
        .unwrap_or_default()
}

/// `host[:port]`의 host가 루프백이면 포트.
fn loopback_port(address: &str, default_port: u16) -> Option<u16> {
    let (host, port) = split_host_port(address)?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback());
    if !loopback {
        return None;
    }
    match port {
        Some(port) => port.parse().ok().filter(|p| *p > 0),
        None => Some(default_port),
    }
}

/// `host:port`, `[v6]:port`, `host`를 나눈다. `unix:`는 없음.
fn split_host_port(address: &str) -> Option<(&str, Option<&str>)> {
    if address.starts_with("unix:") || address.is_empty() {
        return None;
    }
    if let Some(rest) = address.strip_prefix('[') {
        let (host, after) = rest.split_once(']')?;
        return Some((host, after.strip_prefix(':')));
    }
    Some(match address.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (address, None),
    })
}

/// `listen` 인수 → (포트, ssl). 주소만 있으면 80, `unix:`·`quic`은 없음.
fn listen(args: &[String]) -> Option<(u16, bool)> {
    let first = args.first()?;
    if args.iter().any(|a| a == "quic") {
        return None;
    }
    let https = args.iter().skip(1).any(|a| a == "ssl");
    let port = if first.bytes().all(|b| b.is_ascii_digit()) {
        first.parse().ok()?
    } else {
        let (_, port) = split_host_port(first)?;
        match port {
            Some(port) => port.parse().ok()?,
            None => 80,
        }
    };
    (port > 0).then_some((port, https))
}

/// 설정 글자를 지시어 나무로 읽는다. 잘못된 괄호는 있는 데까지만 읽는다.
fn parse(text: &str) -> Vec<Directive> {
    let tokens = tokenize(text);
    let mut position = 0;
    parse_block(&tokens, &mut position)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Word(String),
    Semicolon,
    Open,
    Close,
}

fn parse_block(tokens: &[Token], position: &mut usize) -> Vec<Directive> {
    let mut directives = Vec::new();
    let mut words: Vec<String> = Vec::new();
    while let Some(token) = tokens.get(*position) {
        *position += 1;
        match token {
            Token::Word(word) => words.push(word.clone()),
            Token::Semicolon => {
                if let Some((name, args)) = words.split_first() {
                    directives.push(Directive {
                        name: name.clone(),
                        args: args.to_vec(),
                        block: None,
                    });
                }
                words.clear();
            }
            Token::Open => {
                let block = parse_block(tokens, position);
                if let Some((name, args)) = words.split_first() {
                    directives.push(Directive {
                        name: name.clone(),
                        args: args.to_vec(),
                        block: Some(block),
                    });
                }
                words.clear();
            }
            Token::Close => return directives,
        }
    }
    directives
}

fn tokenize(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            _ if c.is_whitespace() => {
                chars.next();
            }
            '#' => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            ';' => {
                chars.next();
                tokens.push(Token::Semicolon);
            }
            '{' => {
                chars.next();
                tokens.push(Token::Open);
            }
            '}' => {
                chars.next();
                tokens.push(Token::Close);
            }
            '"' | '\'' => {
                let quote = c;
                chars.next();
                let mut word = String::new();
                while let Some(c) = chars.next() {
                    match c {
                        '\\' => word.extend(chars.next()),
                        _ if c == quote => break,
                        _ => word.push(c),
                    }
                }
                tokens.push(Token::Word(word));
            }
            _ => {
                let mut word = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || matches!(c, ';' | '{' | '}') {
                        break;
                    }
                    word.push(c);
                    chars.next();
                }
                tokens.push(Token::Word(word));
            }
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn files(entries: &[(&str, &str)]) -> ConfigFiles {
        ConfigFiles {
            texts: entries
                .iter()
                .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                .collect(),
            globs: HashMap::new(),
        }
    }

    fn route(name: &str, port: u16, https: bool, path: &str, upstream: u16) -> Route {
        Route {
            name: name.into(),
            port,
            https,
            path: path.into(),
            upstream,
        }
    }

    #[test]
    fn conf_path_comes_from_dash_c_or_the_brew_prefix() {
        let row = |command: &str| ProcessRow {
            pid: 10,
            ppid: 1,
            pgid: 10,
            uid: 0,
            tty: "??".into(),
            user: "root".into(),
            started_at: "Wed Nov 18 09:00:00 2026".into(),
            command: command.into(),
        };
        let rows = [
            row("nginx: master process /opt/homebrew/opt/nginx/bin/nginx -g daemon off;"),
            row("nginx: worker process"),
            row("nginx: master process /usr/local/bin/nginx -c /usr/local/etc/nginx/dev.conf"),
            row("nginx: master process /srv/nginx/sbin/nginx"),
            row("/bin/zsh"),
        ];
        assert_eq!(
            conf_paths(&rows),
            [
                Ok("/opt/homebrew/etc/nginx/nginx.conf".to_owned()),
                Ok("/usr/local/etc/nginx/dev.conf".to_owned()),
                Err("nginx 설정 파일 위치를 알 수 없습니다: /srv/nginx/sbin/nginx".to_owned()),
            ]
        );
    }

    #[test]
    fn reads_paths_routed_to_loopback_ports() {
        // 사용자 Mac에서 본 모양: localhost 하나에 경로별로 루프백 포트로 넘긴다.
        let conf = r#"
worker_processes  1;
events { worker_connections 1024; }
http {
    include       mime.types;
    server {
        listen       8080;
        server_name  localhost;
        location /api {
            proxy_pass http://127.0.0.1:3000;   # 개발 서버
        }
        location = "/docs" { proxy_pass http://localhost:4000/; }
        location ~ \.php$ { proxy_pass http://127.0.0.1:9000; }
        location @fallback { proxy_pass http://127.0.0.1:9001; }
        location /outside { proxy_pass http://intranet.example.test:8081; }
        location /var { proxy_pass http://127.0.0.1:$port; }
        location / {
            root   html;
            location /nested/ { proxy_pass http://[::1]:5000; }
        }
    }
    include servers/*;
}
"#;
        let (routes, warnings) = routes(
            "/opt/homebrew/etc/nginx/nginx.conf",
            &files(&[
                ("/opt/homebrew/etc/nginx/nginx.conf", conf),
                (
                    "/opt/homebrew/etc/nginx/mime.types",
                    "types { text/html html; }",
                ),
            ]),
        );
        assert_eq!(
            routes,
            [
                route("localhost", 8080, false, "/api", 3000),
                route("localhost", 8080, false, "/docs", 4000),
                route("localhost", 8080, false, "/nested/", 5000),
            ]
        );
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn follows_includes_upstreams_and_ssl_listens() {
        let main = "http {\n  include servers/*;\n  include /etc/missing.conf;\n}\n";
        let shop = r#"
upstream shop_web {
    server 127.0.0.1:5173 weight=2;
    server 10.0.0.9:5173;
    server unix:/tmp/web.sock;
}
server {
    listen 443 ssl;
    listen [::]:443 ssl http2;
    listen 443 quic;
    server_name shop.example.test www.shop.example.test _ "~^(?<x>.+)\.test$" *.shop.example.test;
    location / { proxy_pass http://shop_web; }
}
"#;
        let api = "server { server_name API.example.test.; location / { proxy_pass https://localhost; } }";
        let mut config = files(&[
            ("/opt/homebrew/etc/nginx/nginx.conf", main),
            ("/opt/homebrew/etc/nginx/servers/shop.conf", shop),
            ("/opt/homebrew/etc/nginx/servers/api.conf", api),
        ]);
        config.globs.insert(
            "/opt/homebrew/etc/nginx/servers/*".into(),
            vec![
                "/opt/homebrew/etc/nginx/servers/api.conf".into(),
                "/opt/homebrew/etc/nginx/servers/shop.conf".into(),
            ],
        );
        let (routes, warnings) = routes("/opt/homebrew/etc/nginx/nginx.conf", &config);
        assert_eq!(
            routes,
            [
                route("api.example.test", 80, false, "/", 443),
                route("shop.example.test", 443, true, "/", 5173),
                route("www.shop.example.test", 443, true, "/", 5173),
            ]
        );
        assert_eq!(
            warnings,
            ["nginx 설정을 읽지 못했습니다: /etc/missing.conf"]
        );
        assert_eq!(
            include_paths(main, "/opt/homebrew/etc/nginx"),
            ["/opt/homebrew/etc/nginx/servers/*", "/etc/missing.conf"]
        );
    }

    #[test]
    fn include_loops_stop_at_the_depth_limit() {
        let (routes, warnings) = routes(
            "/etc/nginx/nginx.conf",
            &files(&[("/etc/nginx/nginx.conf", "include nginx.conf;")]),
        );
        assert!(routes.is_empty());
        assert_eq!(
            warnings,
            ["nginx include가 너무 깊어 건너뛰었습니다: nginx.conf"]
        );
    }

    #[test]
    fn patterns_match_only_the_last_path_part() {
        assert!(name_matches("*", "shop.conf"));
        assert!(name_matches("*.conf", "shop.conf"));
        assert!(!name_matches("*.conf", "shop.conf.bak"));
        assert!(!name_matches("*", ".hidden"));
        assert!(name_matches("site?.conf", "site1.conf"));
        assert_eq!(
            split_pattern("/opt/homebrew/etc/nginx/servers/*"),
            Some(("/opt/homebrew/etc/nginx/servers", "*"))
        );
        assert_eq!(split_pattern("/etc/*/x.conf"), None);
    }
}

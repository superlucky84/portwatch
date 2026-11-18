//! 프로젝트 판별에 쓰는 순수 규칙. 파일 확인은 수집기가 한다.

/// 프로젝트 폴더를 알려 주는 파일·폴더 이름. 가장 가까운 폴더가 프로젝트다.
pub const PROJECT_MARKERS: &[&str] = &[
    "package.json",
    ".git",
    "pyproject.toml",
    "Cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Gemfile",
    "composer.json",
];

/// 작업 폴더에서 위로 올라가며 확인할 폴더 목록. 홈 폴더와 `/`는 넣지 않는다.
pub fn candidate_dirs(cwd: &str, home: &str) -> Vec<String> {
    let home = home.trim_end_matches('/');
    let mut dirs = Vec::new();
    let mut current = cwd.trim_end_matches('/');
    while !current.is_empty() && current != home && current.starts_with('/') {
        dirs.push(current.to_owned());
        match current.rsplit_once('/') {
            Some((parent, _)) => current = parent,
            None => break,
        }
    }
    dirs
}

/// `package.json` 내용에서 `name`을 읽는다. 비었거나 읽을 수 없으면 없음.
pub fn package_name(package_json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(package_json).ok()?;
    let name = value.get("name")?.as_str()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// 폴더 경로의 마지막 이름.
pub fn dir_name(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn candidates_stop_below_home_and_root() {
        assert_eq!(
            candidate_dirs("/Users/dev/work/shop/apps/web", "/Users/dev"),
            [
                "/Users/dev/work/shop/apps/web",
                "/Users/dev/work/shop/apps",
                "/Users/dev/work/shop",
                "/Users/dev/work",
            ]
        );
        assert!(candidate_dirs("/Users/dev", "/Users/dev/").is_empty());
        assert_eq!(
            candidate_dirs("/opt/app", "/Users/dev"),
            ["/opt/app", "/opt"]
        );
    }

    #[test]
    fn package_name_ignores_missing_or_invalid_names() {
        assert_eq!(
            package_name(r#"{"name": "@shop/web"}"#).as_deref(),
            Some("@shop/web")
        );
        assert_eq!(package_name(r#"{"name": "  "}"#), None);
        assert_eq!(package_name(r#"{"version": "1.0.0"}"#), None);
        assert_eq!(package_name("not json"), None);
    }

    #[test]
    fn dir_name_is_the_last_segment() {
        assert_eq!(dir_name("/Users/dev/work/api/"), "api");
    }
}

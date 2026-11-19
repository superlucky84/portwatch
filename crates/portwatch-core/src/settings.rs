//! 사용자 설정의 모양과 범위. 파일 읽기·쓰기는 셸이 한다(docs/DESIGN.md "설정").

use serde::{Deserialize, Serialize};

use crate::resolve::DEFAULT_SECURITY_NAMES;

/// 창을 연 동안 고를 수 있는 갱신 주기(초).
pub const OPEN_CHOICES: &[u64] = &[2, 3, 5];
/// 창을 닫은 동안 고를 수 있는 갱신 주기(초).
pub const CLOSED_CHOICES: &[u64] = &[10, 15, 30, 60];

const DEFAULT_OPEN: u64 = 3;
const DEFAULT_CLOSED: u64 = 15;
const MAX_KEYS: usize = 500;
const MAX_NAMES: usize = 100;
const MAX_TEXT: usize = 200;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub refresh_open_secs: u64,
    pub refresh_closed_secs: u64,
    /// 숨긴 항목 key.
    pub hidden: Vec<String>,
    /// "개발 서버로 고정"한 항목 key.
    pub pinned: Vec<String>,
    /// 보안 프로그램 이름 조각. 없으면 기본 목록을 쓴다.
    pub security_names: Option<Vec<String>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            refresh_open_secs: DEFAULT_OPEN,
            refresh_closed_secs: DEFAULT_CLOSED,
            hidden: Vec::new(),
            pinned: Vec::new(),
            security_names: None,
        }
    }
}

/// 설정 화면에서 바꾸는 값. 넣지 않은 값은 그대로 둔다.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsUpdate {
    pub refresh_open_secs: Option<u64>,
    pub refresh_closed_secs: Option<u64>,
    /// 보안 프로그램 목록을 이 값으로 바꾼다.
    pub security_names: Option<Vec<String>>,
    /// 보안 프로그램 목록을 기본 목록으로 되돌린다.
    pub reset_security_names: bool,
}

fn clean_list(values: Vec<String>, max: usize, lowercase: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for value in values {
        let value = value.trim();
        let value = if lowercase {
            value.to_lowercase()
        } else {
            value.to_owned()
        };
        if value.is_empty() || value.chars().count() > MAX_TEXT || out.contains(&value) {
            continue;
        }
        out.push(value);
        if out.len() >= max {
            break;
        }
    }
    out
}

fn default_names() -> Vec<String> {
    DEFAULT_SECURITY_NAMES
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

impl Settings {
    /// 범위 밖 주기는 기본값으로, 목록은 빈 값·중복·지나치게 긴 값을 빼고 정리한다.
    pub fn normalized(mut self) -> Self {
        if !OPEN_CHOICES.contains(&self.refresh_open_secs) {
            self.refresh_open_secs = DEFAULT_OPEN;
        }
        if !CLOSED_CHOICES.contains(&self.refresh_closed_secs) {
            self.refresh_closed_secs = DEFAULT_CLOSED;
        }
        self.hidden = clean_list(self.hidden, MAX_KEYS, false);
        self.pinned = clean_list(self.pinned, MAX_KEYS, false);
        self.security_names = self
            .security_names
            .map(|names| clean_list(names, MAX_NAMES, true))
            .filter(|names| *names != default_names());
        self
    }

    /// 파일 내용에서 읽는다. 형식이 틀리면 오류를 돌려준다(셸이 기본값으로 시작하고 경고를 남긴다).
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str::<Settings>(text)
            .map(Settings::normalized)
            .map_err(|error| format!("설정 파일을 읽지 못했습니다: {error}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_owned())
    }

    /// 수집에 쓸 보안 프로그램 이름 조각.
    pub fn security_names(&self) -> Vec<String> {
        self.security_names.clone().unwrap_or_else(default_names)
    }

    pub fn set_hidden(&mut self, key: &str, hidden: bool) {
        toggle(&mut self.hidden, key, hidden);
    }

    pub fn set_pinned(&mut self, key: &str, pinned: bool) {
        toggle(&mut self.pinned, key, pinned);
    }

    /// 화면에서 바꾼 값을 적용한다. 범위 밖 주기는 거부하고 아무것도 바꾸지 않는다.
    pub fn apply_update(&mut self, update: SettingsUpdate) -> Result<(), String> {
        if let Some(open) = update.refresh_open_secs {
            if !OPEN_CHOICES.contains(&open) {
                return Err(format!(
                    "창을 연 동안의 갱신 주기로 {open}초는 고를 수 없습니다."
                ));
            }
        }
        if let Some(closed) = update.refresh_closed_secs {
            if !CLOSED_CHOICES.contains(&closed) {
                return Err(format!(
                    "창을 닫은 동안의 갱신 주기로 {closed}초는 고를 수 없습니다."
                ));
            }
        }
        if let Some(open) = update.refresh_open_secs {
            self.refresh_open_secs = open;
        }
        if let Some(closed) = update.refresh_closed_secs {
            self.refresh_closed_secs = closed;
        }
        if update.reset_security_names {
            self.security_names = None;
        } else if let Some(names) = update.security_names {
            self.security_names = Some(names);
        }
        *self = std::mem::take(self).normalized();
        Ok(())
    }
}

fn toggle(list: &mut Vec<String>, key: &str, on: bool) {
    let key = key.trim();
    if key.is_empty() {
        return;
    }
    list.retain(|existing| existing != key);
    if on && list.len() < MAX_KEYS {
        list.push(key.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_out_of_range_values_fall_back_to_defaults() {
        let settings =
            Settings::from_json(r#"{"refreshOpenSecs": 1, "hidden": ["a", "a", " "]}"#).unwrap();
        assert_eq!(settings.refresh_open_secs, 3);
        assert_eq!(settings.refresh_closed_secs, 15);
        assert_eq!(settings.hidden, ["a"]);
        assert!(Settings::from_json("not json").is_err());
    }

    #[test]
    fn security_names_equal_to_the_default_are_not_stored() {
        let mut settings = Settings::default();
        settings
            .apply_update(SettingsUpdate {
                security_names: Some(default_names()),
                ..SettingsUpdate::default()
            })
            .unwrap();
        assert_eq!(settings.security_names, None);

        settings
            .apply_update(SettingsUpdate {
                security_names: Some(vec![" MyGuard ".into(), "".into()]),
                ..SettingsUpdate::default()
            })
            .unwrap();
        assert_eq!(settings.security_names(), ["myguard"]);

        settings
            .apply_update(SettingsUpdate {
                reset_security_names: true,
                ..SettingsUpdate::default()
            })
            .unwrap();
        assert_eq!(settings.security_names(), default_names());
    }

    #[test]
    fn invalid_intervals_are_rejected_without_partial_changes() {
        let mut settings = Settings::default();
        let error = settings
            .apply_update(SettingsUpdate {
                refresh_open_secs: Some(5),
                refresh_closed_secs: Some(7),
                ..SettingsUpdate::default()
            })
            .unwrap_err();
        assert!(error.contains("7초"));
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn hide_and_pin_toggle_keys() {
        let mut settings = Settings::default();
        settings.set_hidden("exe:codex", true);
        settings.set_hidden("exe:codex", true);
        settings.set_pinned("launchd:sh.brew.nginx", true);
        assert_eq!(settings.hidden, ["exe:codex"]);
        settings.set_hidden("exe:codex", false);
        assert!(settings.hidden.is_empty());
        assert_eq!(settings.pinned, ["launchd:sh.brew.nginx"]);

        let round_trip = Settings::from_json(&settings.to_json()).unwrap();
        assert_eq!(round_trip, settings);
    }
}

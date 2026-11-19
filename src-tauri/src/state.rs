//! 앱 상태: 마지막 스냅샷, 설정과 그 파일, 창 열림 여부, 갱신 루프 깨우기.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use portwatch_core::prefs;
use portwatch_core::settings::Settings;
use portwatch_core::Snapshot;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub struct AppState {
    /// 수집한 그대로의 마지막 스냅샷. 설정은 보낼 때마다 새로 적용한다.
    raw: Mutex<Option<Snapshot>>,
    settings: Mutex<Settings>,
    settings_path: Option<PathBuf>,
    /// 설정 파일을 읽거나 쓰지 못했을 때의 경고. 스냅샷 경고에 붙인다.
    settings_warning: Mutex<Option<String>>,
    pub popover_open: AtomicBool,
    /// 창이 포커스를 잃어 마지막으로 닫힌 시각.
    pub hidden_by_blur_at: Mutex<Option<Instant>>,
    wake: Mutex<Option<Sender<()>>>,
}

impl AppState {
    /// 설정 파일을 읽는다. 파일이 없으면 기본값, 읽을 수 없으면 기본값과 경고.
    pub fn load(settings_path: Option<PathBuf>) -> Self {
        let (settings, warning) = match settings_path.as_deref().map(std::fs::read_to_string) {
            None => (Settings::default(), None),
            Some(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                (Settings::default(), None)
            }
            Some(Err(error)) => (
                Settings::default(),
                Some(format!("설정 파일을 읽지 못했습니다: {error}")),
            ),
            Some(Ok(text)) => match Settings::from_json(&text) {
                Ok(settings) => (settings, None),
                Err(error) => (Settings::default(), Some(error)),
            },
        };
        Self {
            raw: Mutex::new(None),
            settings: Mutex::new(settings),
            settings_path,
            settings_warning: Mutex::new(warning),
            popover_open: AtomicBool::new(false),
            hidden_by_blur_at: Mutex::new(None),
            wake: Mutex::new(None),
        }
    }

    pub fn settings(&self) -> Settings {
        lock(&self.settings).clone()
    }

    /// 설정을 바꾸고 파일에 쓴다. 바꾸는 함수가 실패하면 아무것도 바꾸지 않는다.
    /// 파일에 쓰지 못하면 바꾼 값은 유지하고 경고를 남긴다.
    pub fn update_settings(
        &self,
        change: impl FnOnce(&mut Settings) -> Result<(), String>,
    ) -> Result<Settings, String> {
        let mut settings = lock(&self.settings);
        let mut next = settings.clone();
        change(&mut next)?;
        *settings = next.clone();
        drop(settings);

        let warning = self
            .settings_path
            .as_deref()
            .and_then(|path| save(path, &next).err());
        *lock(&self.settings_warning) = warning;
        Ok(next)
    }

    pub fn set_raw(&self, snapshot: Snapshot) {
        *lock(&self.raw) = Some(snapshot);
    }

    /// 마지막 스냅샷에 설정을 적용한 사본. 아직 수집 전이면 없음.
    pub fn applied(&self) -> Option<Snapshot> {
        let raw = lock(&self.raw).clone()?;
        let mut snapshot = prefs::apply(&raw, &self.settings());
        if let Some(warning) = lock(&self.settings_warning).clone() {
            snapshot.warnings.push(warning);
        }
        Some(snapshot)
    }

    pub fn set_wake(&self, sender: Sender<()>) {
        *lock(&self.wake) = Some(sender);
    }

    /// 갱신 루프를 바로 깨운다.
    pub fn wake(&self) {
        if let Some(sender) = lock(&self.wake).as_ref() {
            let _ = sender.send(());
        }
    }
}

/// 임시 파일에 쓴 뒤 이름을 바꿔, 쓰다가 멈춰도 설정 파일이 깨지지 않게 한다.
fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    let fail = |error: std::io::Error| format!("설정을 저장하지 못했습니다: {error}");
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(fail)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, settings.to_json()).map_err(fail)?;
    std::fs::rename(&temp, path).map_err(fail)
}

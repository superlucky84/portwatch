//! 화면에 노출하는 고정 명령(docs/DESIGN.md "화면에 노출하는 명령"). 다른 명령은 실행하지 않는다.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;

use portwatch_core::actions::{validate_copy_text, validate_open_url};
use portwatch_core::resolve::DEFAULT_SECURITY_NAMES;
use portwatch_core::settings::{Settings, SettingsUpdate, CLOSED_CHOICES, OPEN_CHOICES};
use portwatch_core::Snapshot;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::refresh;
use crate::state::AppState;

const MAIN_WINDOW: &str = "main";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiState {
    snapshot: Option<Snapshot>,
    settings: Settings,
    launch_at_login: bool,
    open_choices: &'static [u64],
    closed_choices: &'static [u64],
    default_security_names: &'static [&'static str],
}

#[tauri::command]
pub fn get_state(app: AppHandle, state: State<'_, AppState>) -> UiState {
    UiState {
        snapshot: state.applied(),
        settings: state.settings(),
        launch_at_login: app.autolaunch().is_enabled().unwrap_or(false),
        open_choices: OPEN_CHOICES,
        closed_choices: CLOSED_CHOICES,
        default_security_names: DEFAULT_SECURITY_NAMES,
    }
}

#[tauri::command]
pub fn refresh_now(state: State<'_, AppState>) {
    state.wake();
}

#[tauri::command]
pub fn set_hidden(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    hidden: bool,
) -> Result<Settings, String> {
    let settings = state.update_settings(|settings| {
        settings.set_hidden(&key, hidden);
        Ok(())
    })?;
    refresh::publish(&app);
    Ok(settings)
}

#[tauri::command]
pub fn set_pinned(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
    pinned: bool,
) -> Result<Settings, String> {
    let settings = state.update_settings(|settings| {
        settings.set_pinned(&key, pinned);
        Ok(())
    })?;
    refresh::publish(&app);
    Ok(settings)
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    update: SettingsUpdate,
) -> Result<Settings, String> {
    let before = state.settings();
    let after = state.update_settings(|settings| settings.apply_update(update))?;
    if before.security_names != after.security_names
        || before.refresh_open_secs != after.refresh_open_secs
        || before.refresh_closed_secs != after.refresh_closed_secs
    {
        // 보안 목록은 다시 수집해야 반영되고, 주기는 다음 대기부터 적용된다.
        state.wake();
    } else {
        refresh::publish(&app);
    }
    Ok(after)
}

#[tauri::command]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> Result<bool, String> {
    let launcher = app.autolaunch();
    let result = if enabled {
        launcher.enable()
    } else {
        launcher.disable()
    };
    result.map_err(|error| format!("로그인 시 자동 실행을 바꾸지 못했습니다: {error}"))?;
    launcher
        .is_enabled()
        .map_err(|error| format!("자동 실행 상태를 읽지 못했습니다: {error}"))
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    let url = validate_open_url(&url)?;
    let status = Command::new("/usr/bin/open")
        .arg(&url)
        .stdin(Stdio::null())
        .status()
        .map_err(|error| format!("브라우저를 열지 못했습니다: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("브라우저를 열지 못했습니다.".to_owned())
    }
}

#[tauri::command]
pub fn copy_text(text: String) -> Result<(), String> {
    let text = validate_copy_text(&text)?;
    let fail = |error: std::io::Error| format!("클립보드에 넣지 못했습니다: {error}");
    let mut child = Command::new("/usr/bin/pbcopy")
        .env_clear()
        .env("LANG", "en_US.UTF-8")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(fail)?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(fail)?;
    }
    let status = child.wait().map_err(fail)?;
    if status.success() {
        Ok(())
    } else {
        Err("클립보드에 넣지 못했습니다.".to_owned())
    }
}

#[tauri::command]
pub fn hide_popover(app: AppHandle, state: State<'_, AppState>) {
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.hide();
    }
    state.popover_open.store(false, Ordering::Relaxed);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

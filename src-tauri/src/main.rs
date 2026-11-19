//! portwatch 메뉴바 셸.
//!
//! 화면에 노출하는 명령은 `commands` 모듈의 고정 명령뿐이다.
//! 임의 명령을 실행하는 플러그인은 넣지 않는다(docs/DESIGN.md "Tauri 구성").

mod commands;
mod refresh;
mod state;

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::state::AppState;

const MAIN_WINDOW: &str = "main";
const QUIT_MENU_ID: &str = "quit";

/// 아이콘을 눌러 창이 포커스를 잃고 닫힌 직후의 같은 클릭을 "닫기"로 보는 시간.
const BLUR_CLICK_GRACE: Duration = Duration::from_millis(300);

fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let state = app.state::<AppState>();

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        state.popover_open.store(false, Ordering::Relaxed);
        return;
    }

    let just_hidden = state
        .hidden_by_blur_at
        .lock()
        .map(|guard| guard.is_some_and(|at| at.elapsed() < BLUR_CLICK_GRACE))
        .unwrap_or(false);
    if just_hidden {
        return;
    }

    let _ = window.move_window(Position::TrayBottomCenter);
    let _ = window.show();
    let _ = window.set_focus();
    state.popover_open.store(true, Ordering::Relaxed);
    // 창을 열면 바로 한 번 수집하고 짧은 주기로 바꾼다.
    state.wake();
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::refresh_now,
            commands::set_hidden,
            commands::set_pinned,
            commands::update_settings,
            commands::set_launch_at_login,
            commands::open_url,
            commands::copy_text,
            commands::hide_popover,
            commands::quit_app,
        ])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let settings_path = app
                .path()
                .app_config_dir()
                .ok()
                .map(|dir| dir.join("settings.json"));
            app.manage(AppState::load(settings_path));

            let quit = MenuItem::with_id(app, QUIT_MENU_ID, "portwatch 종료", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;

            TrayIconBuilder::with_id(refresh::TRAY_ID)
                .icon(icon)
                .icon_as_template(true)
                .tooltip("portwatch")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| {
                    if event.id() == QUIT_MENU_ID {
                        app.exit(0);
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_popover(tray.app_handle());
                    }
                })
                .build(app)?;

            refresh::start(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW {
                return;
            }
            if let WindowEvent::Focused(false) = event {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                    let state = window.state::<AppState>();
                    state.popover_open.store(false, Ordering::Relaxed);
                    let mut at = state
                        .hidden_by_blur_at
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *at = Some(Instant::now());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("portwatch를 실행하지 못했습니다");
}

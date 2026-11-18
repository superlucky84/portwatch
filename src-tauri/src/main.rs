//! portwatch 메뉴바 셸.
//!
//! 화면에 노출하는 명령은 아래 `invoke_handler`에 적은 고정 명령뿐이다.
//! 임의 명령을 실행하는 플러그인은 넣지 않는다(docs/DESIGN.md "Tauri 구성").

use std::sync::Mutex;
use std::time::{Duration, Instant};

use portwatch_core::Snapshot;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_positioner::{Position, WindowExt};

const MAIN_WINDOW: &str = "main";
const QUIT_MENU_ID: &str = "quit";

/// 아이콘을 눌러 창이 포커스를 잃고 닫힌 직후의 같은 클릭을 "닫기"로 보는 시간.
const BLUR_CLICK_GRACE: Duration = Duration::from_millis(300);

/// 창이 포커스를 잃어 마지막으로 닫힌 시각.
#[derive(Default)]
struct PopoverState {
    hidden_by_blur_at: Mutex<Option<Instant>>,
}

#[tauri::command]
fn get_snapshot() -> Snapshot {
    portwatch_collect::collect_snapshot()
}

#[tauri::command]
fn quit_app(app: AppHandle) {
    app.exit(0);
}

fn toggle_popover(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };

    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }

    let state = app.state::<PopoverState>();
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
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .manage(PopoverState::default())
        .invoke_handler(tauri::generate_handler![get_snapshot, quit_app])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let quit = MenuItem::with_id(app, QUIT_MENU_ID, "portwatch 종료", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&quit])?;
            let icon = Image::from_bytes(include_bytes!("../icons/tray-template.png"))?;

            TrayIconBuilder::with_id("portwatch-tray")
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
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != MAIN_WINDOW {
                return;
            }
            if let WindowEvent::Focused(false) = event {
                if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                    if let Ok(mut at) = window.state::<PopoverState>().hidden_by_blur_at.lock() {
                        *at = Some(Instant::now());
                    }
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("portwatch를 실행하지 못했습니다");
}

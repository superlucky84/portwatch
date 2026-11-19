//! 갱신 루프: 한 스레드에서 주기적으로 수집하고 아이콘 숫자와 화면을 갱신한다(docs/DESIGN.md "갱신 루프").

use std::sync::atomic::Ordering;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::Duration;

use portwatch_core::prefs;
use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

pub const TRAY_ID: &str = "portwatch-tray";
pub const SNAPSHOT_EVENT: &str = "snapshot";

/// 갱신 스레드를 띄운다. 수집이 겹치지 않도록 이 스레드에서만 수집한다.
pub fn start(app: AppHandle) {
    let (sender, receiver) = mpsc::channel();
    app.state::<AppState>().set_wake(sender);
    std::thread::Builder::new()
        .name("portwatch-refresh".to_owned())
        .spawn(move || run(app, receiver))
        .expect("갱신 스레드를 만들지 못했습니다");
}

fn run(app: AppHandle, wake: Receiver<()>) {
    loop {
        let state = app.state::<AppState>();
        let names = state.settings().security_names();
        state.set_raw(portwatch_collect::collect_snapshot_using(names));
        publish(&app);

        let settings = state.settings();
        let seconds = if state.popover_open.load(Ordering::Relaxed) {
            settings.refresh_open_secs
        } else {
            settings.refresh_closed_secs
        };
        match wake.recv_timeout(Duration::from_secs(seconds)) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => while wake.try_recv().is_ok() {},
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// 마지막 스냅샷에 설정을 적용해 아이콘 숫자를 바꾸고 화면에 보낸다.
pub fn publish(app: &AppHandle) {
    let Some(snapshot) = app.state::<AppState>().applied() else {
        return;
    };
    let count = prefs::badge_count(&snapshot);
    if let Some(tray) = app.tray_by_id(TRAY_ID) {
        let title = (count > 0).then(|| count.to_string());
        let _ = tray.set_title(title);
    }
    let _ = app.emit(SNAPSHOT_EVENT, &snapshot);
}

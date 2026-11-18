//! portwatch 수집기와 실행기.
//!
//! macOS 명령을 실행해 원문을 모으고, 코어로 스냅샷을 만든다.
//! Phase 0에서는 빈 스냅샷만 돌려준다. 실제 수집은 Phase 1에서 구현한다.

use std::time::{SystemTime, UNIX_EPOCH};

use portwatch_core::Snapshot;

/// 현재 상태의 스냅샷을 만든다.
pub fn collect_snapshot() -> Snapshot {
    let mut snapshot = Snapshot::empty(now_ms());
    if !cfg!(target_os = "macos") {
        snapshot
            .warnings
            .push("macOS가 아니어서 수집하지 않았습니다.".to_owned());
    }
    snapshot
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_has_a_timestamp_and_no_entries_yet() {
        let snapshot = collect_snapshot();
        assert!(snapshot.taken_at_ms > 0);
        assert!(snapshot.entries.is_empty());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn non_macos_reports_a_warning() {
        assert_eq!(collect_snapshot().warnings.len(), 1);
    }
}

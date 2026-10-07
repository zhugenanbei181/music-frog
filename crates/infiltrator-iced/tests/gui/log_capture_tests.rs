//! test-intent: behavior
use super::activate;
use crate::state::AppState;
use futures_util::StreamExt;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::parity::FeatureId;
use tokio::runtime::Builder;

#[test]
fn actual_follow_capture_sequence_locks_before_continued_receipts_and_retains_all_35_records() {
    let (mut state, _) = AppState::new();
    state.shell.demo = true;
    let task = activate(&mut state, FeatureId::LogsScrollLock);
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).unwrap();
            while let Some(action) = stream.next().await {
                if let Action::Output(message) = action {
                    let _ = state.update(message);
                }
            }
        });
    assert_eq!(state.diag.log_search.source_count(), 35);
    assert_eq!(state.diag.log_search.matched_count(), 35);
    assert!(!state.diag.log_search.follow.should_follow());
    assert_eq!(
        state.diag.log_search.follow.label_key(),
        "logs_scroll_resume"
    );
    assert!(state.diag.log_search.source_current());
    assert!(state.diag.connections.is_none());
    assert!(state.runtime.proxies.is_empty());
}

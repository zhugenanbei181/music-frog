//! test-intent: behavior
use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::runtime::ConfigSnapshot;

fn config(mode: &str) -> ConfigSnapshot {
    ConfigSnapshot {
        mode: mode.into(),
        ..ConfigSnapshot::default()
    }
}

#[test]
fn config_reads_started_before_or_during_mode_write_cannot_restore_the_old_mode() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(config("rule")),
        generation,
    ));
    let before = state.runtime.mode_actions.read_epoch();
    let request = state.runtime.mode_actions.begin(ProxyMode::Global).unwrap();
    let during = state.runtime.mode_actions.read_epoch();
    let _ = state.update(Message::RuntimeConfigReadFinished {
        result: Ok(config("direct")),
        generation,
        mode_epoch: during,
    });
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("rule"));
    assert_eq!(state.runtime.mode_actions.pending, Some(request));
    let _ = state.update(Message::ProxyModeFinished {
        request,
        result: Ok(ProxyMode::Global),
    });
    for mode_epoch in [before, during] {
        let _ = state.update(Message::RuntimeConfigReadFinished {
            result: Ok(config("rule")),
            generation,
            mode_epoch,
        });
        assert_eq!(state.runtime.proxy_mode.as_deref(), Some("global"));
        assert_eq!(
            state.runtime.mode_actions.observed.current,
            Some(ProxyMode::Global)
        );
    }
    let mode_epoch = state.runtime.mode_actions.read_epoch();
    let _ = state.update(Message::RuntimeConfigReadFinished {
        result: Ok(config("direct")),
        generation,
        mode_epoch,
    });
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("direct"));
    let _ = state.update(Message::SetProxyMode("direct".into()));
    assert!(state.runtime.mode_actions.failure.is_none());
}

#[test]
fn failed_config_read_disables_retry_until_live_mode_facts_recover_and_keeps_write_reason() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(config("rule")),
        generation,
    ));
    let request = state.runtime.mode_actions.begin(ProxyMode::Global).unwrap();
    let write_failure = Failure::new(ErrorCode::Network, "mode write failed", true);
    let _ = state.update(Message::ProxyModeFinished {
        request,
        result: Err(write_failure.clone()),
    });
    assert_eq!(
        state.runtime.mode_actions.retry_target(),
        Some(ProxyMode::Global)
    );
    let read_failure = Failure::new(ErrorCode::Authentication, "config read denied", false);
    let _ = state.update(Message::RuntimeConfigFetched(
        Err(read_failure.clone()),
        generation,
    ));
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("rule"));
    assert_eq!(state.runtime.mode_actions.retry_target(), None);
    assert_eq!(
        state.runtime.mode_actions.failure,
        Some(write_failure.clone())
    );
    assert_eq!(
        state.runtime.mode_actions.observed.failure,
        Some(read_failure)
    );
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(config("rule")),
        generation,
    ));
    assert_eq!(
        state.runtime.mode_actions.retry_target(),
        Some(ProxyMode::Global)
    );
    assert_eq!(state.runtime.mode_actions.failure, Some(write_failure));
    let _ = state.update(Message::DismissProxyModeFailure);
    assert!(state.runtime.mode_actions.failure.is_none());
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("rule"));
}

#[test]
fn mismatched_mode_receipt_preserves_actual_mode_and_exposes_retry_instead_of_claiming_target() {
    let (mut state, _) = AppState::new();
    let generation = state.runtime.runtime_generation;
    let _ = state.update(Message::RuntimeConfigFetched(
        Ok(config("rule")),
        generation,
    ));
    let request = state.runtime.mode_actions.begin(ProxyMode::Global).unwrap();
    let _ = state.update(Message::ProxyModeFinished {
        request,
        result: Ok(ProxyMode::Direct),
    });
    assert_eq!(state.runtime.proxy_mode.as_deref(), Some("direct"));
    assert_eq!(
        state.runtime.mode_actions.failure.as_ref().unwrap().code,
        ErrorCode::InvalidState
    );
    assert_eq!(
        state.runtime.mode_actions.retry_target(),
        Some(ProxyMode::Global)
    );
    assert!(state.runtime.mode_actions.pending.is_none());
    assert!(state.shell.error_msg.is_none());
}

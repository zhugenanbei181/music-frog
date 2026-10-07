//! test-intent: behavior
//! Inspection consumes the shared fold without running probes or commands.
use crate::test_support::demo_env;
use infiltrator_application::speedtest_detail_projection::{listing, project_details};
use infiltrator_contract::capability::Availability;
use infiltrator_contract::shortcuts::KeyModifiers;
use infiltrator_contract::speedtest::SpeedtestSnapshot;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_shared::locales::{Lang, Localizer};

#[test]
fn speedtest_detail_preserves_shared_facts_and_closes_without_effects() {
    let (mut state, _) = AppState::demo(&demo_env(Route::Runtime));
    state.diag.speedtest = SpeedtestSnapshot::demo_fixture();
    let original = state.diag.speedtest.clone();
    assert_eq!(state.update(Message::OpenSpeedtestDetail).units(), 0);
    assert!(state.diag.speedtest_detail_open);
    let details = project_details(&state.diag.speedtest);
    assert_eq!(details.rows.len(), original.node_results.len());
    assert_eq!(
        details.rows[0].node_name,
        original.sorted_by_latency()[0].node_name
    );
    drop(state.view());
    assert_eq!(state.update(Message::CloseSpeedtestDetail).units(), 0);
    assert!(!state.diag.speedtest_detail_open);
    let _ = state.update(Message::OpenSpeedtestDetail);
    assert_eq!(
        state
            .update(Message::KeyboardChord {
                key: "Escape".into(),
                modifiers: KeyModifiers::default()
            })
            .units(),
        0
    );
    assert!(!state.diag.speedtest_detail_open);
    assert_eq!(state.diag.speedtest, original);
    let _ = state.update(Message::OpenSpeedtestDetail);
    let _ = state.update(Message::Navigate(Route::Profiles));
    assert!(!state.diag.speedtest_detail_open);
    assert_eq!(state.diag.speedtest, original);
}

#[test]
fn speedtest_detail_keeps_unsupported_failure_and_empty_distinct() {
    let mut env = demo_env(Route::Runtime);
    env.lang = "en-US".into();
    env.window_size = (720.0, 480.0);
    let (mut state, _) = AppState::demo(&env);
    state.diag.speedtest = SpeedtestSnapshot::default();
    let text = |state: &AppState| {
        listing(&project_details(&state.diag.speedtest), &|key| {
            Lang(&state.shell.lang).tr(key).into_owned()
        })
    };
    let empty = text(&state);
    state.diag.speedtest.failure = Some("probe refused".into());
    let failure = text(&state);
    state.diag.speedtest.availability = Some(Availability::Unsupported {
        reason: "missing host engine".into(),
    });
    let unsupported = text(&state);
    assert_ne!(unsupported, failure);
    assert_ne!(unsupported, empty);
    assert!(unsupported.contains("missing host engine"));
    assert_eq!(state.update(Message::OpenSpeedtestDetail).units(), 0);
    assert!(state.diag.speedtest_detail_open);
    drop(state.view());
    assert_eq!(state.update(Message::CloseSpeedtestDetail).units(), 0);
    assert!(!state.diag.speedtest_detail_open);
}

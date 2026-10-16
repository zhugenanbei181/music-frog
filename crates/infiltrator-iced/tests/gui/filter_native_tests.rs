//! test-intent: behavior
//! Real field and action widgets feed TEA and the injected typed transaction.
use infiltrator_composition::mihomo_configuration::profiles_for_mihomo;
use crate::state::AppState;
use crate::test_mounts::command_harness::{
    attach_product_application_with_accepted_owner_snapshot, core_test_command_application,
};
use crate::test_mounts::execute_at_observed_scope;
use crate::test_mounts::identified_core_application::identified_core_application;
use crate::test_mounts::native_widgets;
use crate::types::message::Message;
use crate::view::profile_filter::filter_pane;
use crate::view_root::interaction_regions::InteractionRegion;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::core_capabilities::{CoreOperation, ProfileOperation};
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::subscription_filter_form::{FilterField, FilterObservation};
use infiltrator_test_support::filter_capture_store::{
    FILTER_POLICY, FILTER_PROFILE, FilterCaptureStore,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::Ordering};
use tokio::runtime::Builder;

fn composed() -> (AppState, Arc<FilterCaptureStore>) {
    let store = Arc::new(FilterCaptureStore::default());
    let runtime = tokio_application_runtime().unwrap();
    let core = identified_core_application();
    core.install_command_handler(Arc::new(
        core_test_command_application([CoreOperation::Profile(ProfileOperation::ApplyFeature)])
            .with_profile(
                profiles_for_mihomo(store.clone()),
            ),
    ));
    let observed = core.clone();
    let reply = Arc::new(Mutex::new(None));
    let output = reply.clone();
    runtime.block_on(Box::pin(async move {
        *output.lock().unwrap() = Some(
            execute_at_observed_scope(
                &observed,
                CommandIntent::LoadProfileOptions {
                    profile: Some(FILTER_PROFILE.into()),
                },
            )
            .await
            .into_output()
            .unwrap()
            .into_profile_options()
            .unwrap(),
        );
    }));
    let options = reply.lock().unwrap().take().unwrap();
    let (mut state, _) = AppState::new();
    attach_product_application_with_accepted_owner_snapshot(&mut state, core);
    state.shell.demo = true;
    state.editor.editor_path = Some(PathBuf::from(format!("/isolated/{FILTER_PROFILE}.yaml")));
    state.editor.filter_editor.observe_profile(
        FILTER_PROFILE,
        Ok(FilterObservation {
            workspace_identity: options.workspace_identity,
            filter: options.filter,
        }),
    );
    (state, store)
}
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("actual terminal");
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn native(state: &AppState, region: InteractionRegion, replace: Option<&str>) -> Vec<Message> {
    native_widgets::native(filter_pane(state), region.id(), replace)
}
fn feed(state: &mut AppState, messages: Vec<Message>) {
    assert!(
        !messages.is_empty(),
        "actual native input publishes messages"
    );
    for message in messages {
        let _ = state.update(message);
    }
}
#[test]
fn native_filter_fields_and_discard_preserve_source_and_never_write() {
    for (region, field, value) in [
        (InteractionRegion::FilterInclude, FilterField::Include, "HK"),
        (InteractionRegion::FilterExclude, FilterField::Exclude, "US"),
        (
            InteractionRegion::FilterProtocols,
            FilterField::Protocols,
            "trojan",
        ),
        (
            InteractionRegion::FilterRenames,
            FilterField::Renames,
            "public=>node",
        ),
        (
            InteractionRegion::FilterAdvanced,
            FilterField::Advanced,
            FILTER_POLICY,
        ),
    ] {
        let (mut state, store) = composed();
        let original = store.observed();
        let base = state.editor.filter_editor.draft.clone();
        let messages = native(&state, region, Some(value));
        feed(&mut state, messages);
        assert_eq!(field.value(&state.editor.filter_editor.draft), value);
        assert!(state.editor.filter_editor.dirty());
        let discard = native(&state, InteractionRegion::FilterDiscard, None);
        feed(&mut state, discard);
        assert_eq!(state.editor.filter_editor.draft, base);
        assert_eq!(store.observed(), original);
        assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    }
}
#[test]
fn native_filter_save_denial_retry_returns_actual_report_and_preserves_draft() {
    let (mut state, store) = composed();
    let messages = native(&state, InteractionRegion::FilterInclude, Some("HK"));
    feed(&mut state, messages);
    let messages = native(
        &state,
        InteractionRegion::FilterAdvanced,
        Some(FILTER_POLICY),
    );
    feed(&mut state, messages);
    store.deny_write.store(true, Ordering::SeqCst);
    let original = store.observed();
    let mut save = native(&state, InteractionRegion::FilterSave, None);
    assert_eq!(save.len(), 1);
    let task = state.update(save.remove(0));
    let token = state.editor.filter_editor.pending().unwrap().token;
    assert!(
        native(&state, InteractionRegion::FilterDiscard, None).is_empty(),
        "pending cancellation cannot pretend the write was cancelled"
    );
    let message = terminal(task);
    let _ = state.update(message);
    assert_eq!(
        state.editor.filter_editor.failure().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(store.observed(), original);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(
        state.editor.filter_editor.draft.advanced_policy.as_deref(),
        Some(FILTER_POLICY)
    );
    store.deny_write.store(false, Ordering::SeqCst);
    let mut save = native(&state, InteractionRegion::FilterSave, None);
    assert_eq!(save.len(), 1);
    let task = state.update(save.remove(0));
    assert!(state.editor.filter_editor.pending().unwrap().token > token);
    let message = terminal(task);
    let _ = state.update(message);
    let report = state.editor.filter_editor.report().unwrap();
    assert_eq!(
        (report.total_input, report.passed, report.excluded_by_server),
        (3, 1, 1)
    );
    assert!(state.editor.filter_editor.applied());
    assert!(state.editor.filter_editor.failure().is_none());
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    let observed = store.observed();
    let document = observed.document.utf8_text().unwrap();
    assert!(document.contains("HK-public"));
    assert!(!document.contains("HK-private"));
}

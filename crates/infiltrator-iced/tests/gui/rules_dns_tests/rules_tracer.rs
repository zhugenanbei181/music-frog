//! Native TEA flows execute the shared command owner and consume its actual reader.
//! test-intent: behavior
use super::*;
use crate::test_mounts::command_harness::{recording_application, rejecting_application};
use crate::types::app::ConfirmAction;
use crate::types::app::Route;
use crate::types::rule_trace::RuleTraceAction;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::configuration_application::ConfigurationApplication;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::rule_list_application::RuleListApplication;
use infiltrator_application::rule_trace_fixtures::{
    NAMED_TRACE_DOCUMENT, RuleTraceStore, TRACE_DOCUMENT,
};
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;

pub(crate) fn setup() -> (AppState, ApplicationSurfaceReader, Arc<RuleTraceStore>) {
    let store = Arc::new(RuleTraceStore::default());
    let owner = RuleTracerApplication::new();
    owner.set_override_port(store.clone());
    let configuration = ConfigurationApplication::new(store.clone());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_rule_tracer(owner.clone())
            .with_rule_list(RuleListApplication::new(store.clone())),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(application.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_rule_tracer(owner)
    .with_profiles(ProfileApplication::new(store.clone()))
    .with_configuration(configuration);
    let (mut state, _) = AppState::new();
    state.commands = Some(application);
    state.shell.demo = false;
    let _ = state.update(Message::UpdateTracerSourceIp("192.0.2.1".into()));
    publish(&mut state, &reader);
    (state, reader, store)
}
pub(crate) fn publish(state: &mut AppState, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = state.surface.revision() + 1;
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
fn complete(state: &mut AppState, task: Task<Message>) {
    let message = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual rule command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("rule command terminal");
            };
            assert!(stream.next().await.is_none());
            message
        });
    let _ = state.update(message);
}
pub(crate) fn run(state: &mut AppState, reader: &ApplicationSurfaceReader) {
    let task = state.update(Message::RunRulesTracer);
    complete(state, task);
    publish(state, reader);
}

#[test]
fn tracer_tab_close_preserves_report_and_permission_guide_does_not_commit() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::SetRulesTab(RulesTab::Tracer));
    let _ = state.update(Message::UpdateRulesTracerInput("google.com".into()));
    run(&mut state, &reader);
    let report = state.editor.rule_trace.snapshot.report.clone();
    let _ = state.update(Message::SetRulesTab(RulesTab::List));
    let _ = state.update(Message::SetRulesTab(RulesTab::Tracer));
    assert_eq!(state.editor.rule_trace.snapshot.report, report);
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    let _ = state.update(Message::UpdateTracerOverrideTarget("DIRECT".into()));
    let _ = state.update(Message::ApplyTracerRuleOverride { rule_index: 2 });
    let _ = state.update(Message::UpdateRulesTracerInput("blocked.test".into()));
    let _ = state.update(Message::UpdateTracerSourceIp("192.0.2.2".into()));
    assert_eq!(state.editor.rules_tracer_input, "google.com");
    assert_eq!(state.editor.rules_tracer_src_ip, "192.0.2.1");
    assert_eq!(state.update(Message::RunRulesTracer).units(), 0);
    store.deny_write.store(true, Ordering::SeqCst);
    let task = state.update(Message::ConfirmAction);
    complete(&mut state, task);
    assert_eq!(
        state
            .editor
            .rule_trace
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    let _ = state.update(Message::RuleTrace(RuleTraceAction::Settings));
    assert_eq!(state.shell.current_route, Route::Settings);
    assert!(state.shell.confirmation.is_none());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.content(), TRACE_DOCUMENT);
}
#[test]
fn test_rules_tracer_gui_flow() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateRulesTracerInput("mail.google.com".into()));
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 0);
    let task = state.update(Message::RunRulesTracer);
    let operation = state.editor.rule_trace.pending.expect("pending command");
    assert_eq!(state.update(Message::RunRulesTracer).units(), 0);
    let _ = state.update(Message::UpdateRulesTracerInput("ignored.example".into()));
    assert_eq!(state.editor.rules_tracer_input, "mail.google.com");
    complete(&mut state, task);
    assert_eq!(state.editor.rule_trace.requested, Some(operation));
    assert!(state.editor.rules_tracer_chain.is_none());
    publish(&mut state, &reader);
    let chain = state.editor.rules_tracer_chain.as_ref().unwrap();
    assert_eq!(chain.hit_rule_index, Some(2));
    assert_eq!(chain.matched_rule_raw, "DOMAIN-SUFFIX,google.com,PROXY");
    assert_eq!(chain.target_proxy, "PROXY");
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    for _ in 0..5 {
        publish(&mut state, &reader);
    }
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    assert_eq!(
        state
            .editor
            .rule_hit_audit
            .audit
            .as_ref()
            .unwrap()
            .trace_count,
        1
    );
    assert_eq!(store.content(), TRACE_DOCUMENT);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
#[test]
fn test_rules_tracer_source_ip_sandbox_flow() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateRulesTracerInput("example.org".into()));
    run(&mut state, &reader);
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "REJECT"
    );
    let _ = state.update(Message::UpdateTracerSourceIp("10.1.2.3".into()));
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "REJECT"
    );
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 1);
    run(&mut state, &reader);
    let chain = state.editor.rules_tracer_chain.as_ref().unwrap();
    assert_eq!(chain.matched_rule_type, "SRC-IP-CIDR");
    assert_eq!(chain.target_proxy, "DIRECT");
    assert_eq!(
        state
            .editor
            .rule_trace
            .snapshot
            .report
            .as_ref()
            .unwrap()
            .simulated_context
            .src_ip
            .as_deref(),
        Some("10.1.2.3")
    );
    let previous = state.editor.rules_tracer_chain.clone();
    let _ = state.update(Message::UpdateTracerSourceIp(String::new()));
    run(&mut state, &reader);
    assert_eq!(state.editor.rules_tracer_chain, previous);
    assert_eq!(
        state
            .editor
            .rule_trace
            .snapshot
            .failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 3);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
}
#[test]
fn test_rules_tracer_reverse_apply_override_dual_surface_flow() {
    let (mut state, reader, store) = setup();
    let _ = state.update(Message::UpdateRulesTracerInput("google.com".into()));
    run(&mut state, &reader);
    assert!(state.editor.rules_tracer_can_reverse_apply);
    assert_eq!(
        state.editor.rules_tracer_suggested_target.as_deref(),
        Some("DIRECT")
    );
    let _ = state.update(Message::UpdateTracerOverrideTarget("DIRECT".into()));
    let _ = state.update(Message::ApplyTracerRuleOverride { rule_index: 2 });
    assert!(matches!(
        state.shell.confirmation,
        Some(ConfirmAction::TracerOverride(_))
    ));
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::CancelConfirmation);
    assert!(state.shell.confirmation.is_none());
    assert_eq!(store.content(), TRACE_DOCUMENT);
    let _ = state.update(Message::ApplyTracerRuleOverride { rule_index: 2 });
    store.deny_write.store(true, Ordering::SeqCst);
    let task = state.update(Message::ConfirmAction);
    assert_eq!(state.update(Message::ConfirmAction).units(), 0);
    let _ = state.update(Message::CancelConfirmation);
    assert!(state.shell.confirmation.is_some());
    complete(&mut state, task);
    assert_eq!(
        state
            .editor
            .rule_trace
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert!(state.shell.confirmation.is_some());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(store.content(), TRACE_DOCUMENT);
    store.deny_write.store(false, Ordering::SeqCst);
    let task = state.update(Message::ConfirmAction);
    complete(&mut state, task);
    assert!(state.shell.confirmation.is_none());
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert!(store.content().contains("DOMAIN-SUFFIX,google.com,DIRECT"));
    publish(&mut state, &reader);
    assert!(!state.editor.rules_tracer_can_reverse_apply);
    run(&mut state, &reader);
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "DIRECT"
    );
    let _ = state.update(Message::UpdateTracerOverrideTarget("REJECT".into()));
    let _ = state.update(Message::ApplyTracerRuleOverride { rule_index: 2 });
    let changed = format!("{}# another owner edited this document\n", store.content());
    store.replace(&changed);
    let task = state.update(Message::ConfirmAction);
    complete(&mut state, task);
    assert_eq!(
        state
            .editor
            .rule_trace
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::NotReady
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(store.content(), changed);
    let (rejected, _) = rejecting_application();
    state.commands = Some(rejected);
    let task = state.update(Message::ConfirmAction);
    complete(&mut state, task);
    assert_eq!(
        state
            .editor
            .rule_trace
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
}

#[test]
fn named_rule_descent_confirmation_cancel_permission_retry_and_leaf_commit() {
    let (mut state, reader, store) = setup();
    store.replace(NAMED_TRACE_DOCUMENT);
    publish(&mut state, &reader);
    let _ = state.update(Message::SetRulesTab(RulesTab::Tracer));
    let _ = state.update(Message::UpdateRulesTracerInput("google.com:443".into()));
    run(&mut state, &reader);
    let chain = state.editor.rules_tracer_chain.as_ref().unwrap();
    assert_eq!(chain.hit_rule_table.as_deref(), Some("secure"));
    assert_eq!(chain.rule_path.len(), 3);
    assert_eq!(chain.target_proxy, "PROXY");
    let _ = state.update(Message::UpdateTracerOverrideTarget("DIRECT".into()));
    let _ = state.update(Message::ApplyTracerRuleOverride { rule_index: 0 });
    let Some(ConfirmAction::TracerOverride(request)) = &state.shell.confirmation else {
        panic!("native confirmation");
    };
    assert_eq!(request.rule_table.as_deref(), Some("secure"));
    assert_eq!(request.rule_index, 0);
    let _ = state.update(Message::CancelConfirmation);
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::ApplyTracerRuleOverride { rule_index: 0 });
    store.deny_write.store(true, Ordering::SeqCst);
    let task = state.update(Message::ConfirmAction);
    complete(&mut state, task);
    assert_eq!(
        state
            .editor
            .rule_trace
            .override_failure
            .as_ref()
            .unwrap()
            .code,
        ErrorCode::Permission
    );
    assert_eq!(store.content(), NAMED_TRACE_DOCUMENT);
    assert!(state.shell.confirmation.is_some());
    store.deny_write.store(false, Ordering::SeqCst);
    let task = state.update(Message::ConfirmAction);
    complete(&mut state, task);
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    assert_eq!(
        store.content(),
        NAMED_TRACE_DOCUMENT.replace(
            "'DOMAIN-SUFFIX,google.com,PROXY'",
            "'DOMAIN-SUFFIX,google.com,DIRECT'"
        )
    );
    publish(&mut state, &reader);
    assert!(!state.editor.rules_tracer_can_reverse_apply);
    run(&mut state, &reader);
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "DIRECT"
    );
}

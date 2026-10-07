//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_leak_application::{DnsLeakApplication, default_echo_sources};
use infiltrator_application::dns_leak_fixtures::{EchoMode, IsolatedEcho};
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::dns_leak::DnsLeakOperation;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::parity::FeatureId;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::PageStatus;
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use tokio::runtime::Builder;
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual echo probe command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("probe terminal")
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn publish(state: &mut AppState, reader: &ApplicationSurfaceReader) {
    let mut snapshot = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(reader.read())
        .unwrap();
    snapshot.revision = state.surface.revision() + 1;
    assert!(state.apply_shared_surface_snapshot(snapshot));
}
#[test]
fn actual_dns_probe_reports_divergence_and_preserves_facts_on_permission_failure_then_retry_with_configuration_unavailable()
 {
    let echo = Arc::new(IsolatedEcho::default());
    let leak = DnsLeakApplication::new(Some(echo.clone()), default_echo_sources());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_leak(leak.clone()),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(application.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_dns_leak(leak.clone());
    let (mut state, _) = AppState::demo(&demo_env(Route::Dns));
    state.commands = Some(application);
    publish(&mut state, &reader);
    assert!(matches!(
        state.surface.latest().unwrap().pages.dns.status,
        PageStatus::Unavailable { .. }
    ));
    let task = state.update(Message::RunDnsLeakProbe);
    let token = state.diag.dns_leak_action.pending.unwrap();
    assert_eq!(state.update(Message::RunDnsLeakProbe).units(), 0);
    let _ = state.update(Message::DnsLeakCommandFinished {
        token: token + 1,
        result: Ok(()),
    });
    assert_eq!(state.diag.dns_leak_action.pending, Some(token));
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(state.editor.dns_leak.conclusion().is_divergent());
    assert_eq!(state.editor.dns_leak.observations.len(), 2);
    let previous = state.editor.dns_leak.observations.clone();
    let mut tree = Tree::new(state.view().as_widget());
    echo.set_mode(EchoMode::Denied);
    let task = state.update(Message::RunDnsLeakProbe);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.dns_leak_action.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert!(state.diag.dns_leak_action.can_retry());
    assert_eq!(state.editor.dns_leak.observations, previous);
    assert!(matches!(
        state.editor.dns_leak.operation,
        DnsLeakOperation::Failed { .. }
    ));
    tree.diff(state.view().as_widget());
    echo.set_mode(EchoMode::Consistent);
    let task = state.update(Message::RetryDnsLeakProbe);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(state.editor.dns_leak.conclusion().is_consistent());
    assert!(state.diag.dns_leak_action.failure.is_none());
    echo.set_mode(EchoMode::AllSourcesFailed);
    let task = state.update(Message::RunDnsLeakProbe);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(state.editor.dns_leak.failed_count(), 2);
    assert!(!state.editor.dns_leak.conclusion().is_consistent());
    assert_eq!(echo.requests().len(), 4);
}
#[test]
fn unsupported_probe_keeps_its_typed_refusal_visible_without_observations_or_network() {
    let leak = DnsLeakApplication::unconfigured();
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_leak(leak.clone()),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(application.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_dns_leak(leak);
    let (mut state, _) = AppState::demo(&demo_env(Route::Dns));
    state.commands = Some(application);
    let task = state.update(Message::RunDnsLeakProbe);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.dns_leak_action.failure.as_ref().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(state.editor.dns_leak.observations.is_empty());
    assert!(
        state
            .editor
            .dns_leak
            .status
            .reason()
            .unwrap()
            .contains("no DNS leak echo prober")
    );
    assert!(!state.diag.dns_leak_action.can_retry());
    let tree = Tree::new(state.view().as_widget());
    assert!(!tree.children.is_empty());
}

#[test]
fn capture_activation_executes_the_actual_command_and_publishes_two_real_echo_observations() {
    let mut env = demo_env(Route::Dns);
    env.scenario = Some(FeatureId::DnsLeakAlert);
    let (mut state, task) = AppState::demo(&env);
    assert!(state.diag.dns_leak_action.pending.is_some());
    let _ = state.update(terminal(task));
    assert!(state.diag.dns_leak_action.failure.is_none());
    assert!(state.editor.dns_leak.conclusion().is_divergent());
    assert_eq!(state.editor.dns_leak.observations.len(), 2);
    let tree = Tree::new(state.view().as_widget());
    assert!(!tree.children.is_empty());
}

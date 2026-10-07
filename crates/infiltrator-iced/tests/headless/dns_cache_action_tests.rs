//! test-intent: behavior
use crate::command_harness::recording_application;
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::{Action, task::into_stream};
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::dns_cache_application::DnsCacheApplication;
use infiltrator_application::dns_cache_fixtures::{CachePortMode, IsolatedCaches};
use infiltrator_application::dns_cache_projection::project_cache;
use infiltrator_application::surface_reader::ApplicationSurfaceReader;
use infiltrator_contract::command_catalogue::CommandTarget;
use infiltrator_contract::dns_cache::DnsFlushOutcome;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_ports::surface::SurfaceReader;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tokio::runtime::Builder;
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual DNS clearing command");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("actual cache terminal result");
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
fn setup(owner: DnsCacheApplication) -> (AppState, ApplicationSurfaceReader) {
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        CommandApplication::new().with_dns_cache(owner.clone()),
    ));
    let reader = ApplicationSurfaceReader::new(
        Arc::new(application.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_dns_cache(owner);
    let (mut state, _) = AppState::demo(&demo_env(Route::Dns));
    state.commands = Some(application);
    publish(&mut state, &reader);
    (state, reader)
}
#[test]
fn actual_cache_confirmation_cancel_and_navigation_touch_neither_target_and_permission_failure_retries_on_the_modal()
 {
    let ports = Arc::new(IsolatedCaches::default());
    let owner = DnsCacheApplication::new(Some(ports.clone()), Some(ports.clone()));
    let (mut state, reader) = setup(owner);
    let original = ports.contents();
    assert!(
        state.surface.latest().unwrap().pages.dns.data.is_none(),
        "missing configuration cannot fake successful DNS data"
    );
    assert_eq!(state.update(Message::FlushFakeIpCache).units(), 0);
    assert!(state.diag.dns_cache_actions.open);
    let mut tree = Tree::new(state.view().as_widget());
    assert_eq!(state.update(Message::CancelDnsCacheFlush).units(), 0);
    assert!(!state.diag.dns_cache_actions.open);
    assert_eq!(state.update(Message::ConfirmDnsCacheFlush).units(), 0);
    assert_eq!(ports.contents(), original);
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 0);
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 0);
    let _ = state.update(Message::FlushFakeIpCache);
    let _ = state.update(Message::Navigate(Route::Settings));
    assert!(!state.diag.dns_cache_actions.open);
    assert_eq!(state.update(Message::ConfirmDnsCacheFlush).units(), 0);
    let _ = state.update(Message::Navigate(Route::Dns));
    let _ = state.update(Message::FlushFakeIpCache);
    ports.set_system_mode(CachePortMode::Denied);
    let task = state.update(Message::ConfirmDnsCacheFlush);
    let token = state.diag.dns_cache_actions.pending.unwrap();
    assert_eq!(state.update(Message::ConfirmDnsCacheFlush).units(), 0);
    let _ = state.update(Message::CancelDnsCacheFlush);
    assert!(state.diag.dns_cache_actions.open);
    let _ = state.update(Message::DnsCacheCommandFinished {
        token: token + 1,
        result: Ok(()),
    });
    assert_eq!(state.diag.dns_cache_actions.pending, Some(token));
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.dns_cache_actions.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(state.diag.dns_cache_flush.fake_ip, DnsFlushOutcome::Flushed);
    assert!(
        matches!(state.diag.dns_cache_flush.os_cache, DnsFlushOutcome::Failed { ref failure } if failure.code == ErrorCode::Permission)
    );
    assert!(ports.contents().0.is_empty());
    assert_eq!(ports.contents().1, original.1);
    assert!(state.diag.dns_cache_actions.can_retry());
    assert!(
        project_cache(&state.diag.dns_cache_actions, "en-US")
            .status
            .contains("Allow system resolver cache access")
    );
    tree.diff(state.view().as_widget());
    let mut stale = state.surface.latest().unwrap().clone();
    stale.revision += 1;
    stale.dns_cache.revision = 0;
    stale.dns_cache.report_id = None;
    assert!(state.apply_shared_surface_snapshot(stale));
    assert_eq!(
        state.diag.dns_cache_actions.failure.as_ref().unwrap().code,
        ErrorCode::Permission
    );
    ports.set_system_mode(CachePortMode::Allowed);
    let task = state.update(Message::RetryDnsCacheFlush);
    let next = state.diag.dns_cache_actions.pending.unwrap();
    let _ = state.update(Message::DnsCacheCommandFinished {
        token,
        result: Ok(()),
    });
    assert_eq!(state.diag.dns_cache_actions.pending, Some(next));
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(state.diag.dns_cache_actions.failure.is_none());
    assert_eq!(
        state.diag.dns_cache_flush.os_cache,
        DnsFlushOutcome::Flushed
    );
    assert!(ports.contents().0.is_empty() && ports.contents().1.is_empty());
    assert_eq!(ports.fake_calls.load(Ordering::SeqCst), 2);
    assert_eq!(ports.system_calls.load(Ordering::SeqCst), 2);
    assert_eq!(state.update(Message::ConfirmDnsCacheFlush).units(), 0);
    let _ = state.update(Message::CancelDnsCacheFlush);
    assert_eq!(state.update(Message::RetryDnsCacheFlush).units(), 0);
}
#[test]
fn palette_clear_routes_to_real_confirmation_and_unsupported_targets_stay_visible_without_cache_mutation()
 {
    let ports = Arc::new(IsolatedCaches::default());
    ports.set_fake_mode(CachePortMode::Unsupported);
    ports.set_system_mode(CachePortMode::Unsupported);
    let owner = DnsCacheApplication::new(Some(ports.clone()), Some(ports.clone()));
    let (mut state, reader) = setup(owner);
    let _ = state.update(Message::Navigate(Route::Overview));
    let _ = state.update(Message::ExecuteCommand(CommandTarget::FlushDnsCache));
    assert_eq!(state.shell.current_route, Route::Dns);
    assert!(state.diag.dns_cache_actions.open);
    let before = ports.contents();
    let task = state.update(Message::ConfirmDnsCacheFlush);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.diag.dns_cache_actions.failure.as_ref().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(matches!(
        state.diag.dns_cache_flush.fake_ip,
        DnsFlushOutcome::Unsupported { .. }
    ));
    assert!(matches!(
        state.diag.dns_cache_flush.os_cache,
        DnsFlushOutcome::Unsupported { .. }
    ));
    assert_eq!(ports.contents(), before);
    assert!(!state.diag.dns_cache_actions.can_retry());
}

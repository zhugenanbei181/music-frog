//! test-intent: behavior
use crate::command_harness::{core_test_command_application, recording_application};
use crate::test_support::demo_env;
use futures_util::StreamExt;
use iced::Task;
use iced::advanced::widget::Tree;
use iced_runtime::{Action, task::into_stream};
use infiltrator_composition::mihomo_configuration::for_mihomo_store;
use infiltrator_composition::surface_reader::ProductSurfaceReader;
use infiltrator_contract::core_capabilities::{CoreOperation, DnsOperation};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_iced::state::AppState;
use infiltrator_iced::types::app::Route;
use infiltrator_iced::types::message::Message;
use infiltrator_ports::surface::SurfaceReader;
use infiltrator_test_support::dns_hosts_fixtures::{HostsCaptureStore, ORIGINAL_HOSTS};
use lurker_core_contract::error::ErrorCode;
use lurker_core_contract::error::Failure;
use lurker_core_contract::page::{PageData, PageStatus};
use std::sync::{Arc, atomic::Ordering};
use tokio::runtime::Builder;
fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut stream = into_stream(task).expect("actual Hosts command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("Hosts terminal response");
            };
            assert!(stream.next().await.is_none());
            message
        })
}
fn publish(state: &mut AppState, reader: &ProductSurfaceReader) {
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
fn actual_hosts_modal_edit_cancel_migration_failure_retry_and_reader_fences_preserve_profile_bytes()
{
    let store = Arc::new(HostsCaptureStore::default());
    let configuration = for_mihomo_store(store.clone());
    let (application, _) = recording_application();
    application.install_command_handler(Arc::new(
        core_test_command_application([CoreOperation::Dns(DnsOperation::ApplyConfiguration)])
            .with_configuration(configuration.clone()),
    ));
    let reader = ProductSurfaceReader::new(
        Arc::new(application.clone()),
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
    )
    .with_configuration(configuration);
    let (mut state, _) = AppState::new();
    state.shell.current_route = Route::Dns;
    state.attach_product_application(application);
    publish(&mut state, &reader);
    assert_eq!(
        state
            .editor
            .dns_hosts_editor
            .applied
            .as_ref()
            .unwrap()
            .entries
            .len(),
        2
    );
    let _ = state.update(Message::OpenDnsHostsEditor);
    let first = state.editor.dns_hosts_editor.rows[0].id;
    let _ = state.update(Message::EditDnsHostRow(first));
    let _ = state.update(Message::UpdateDnsHostsAddress("4.4.4.4".into()));
    let _ = state.update(Message::AddDnsHostRow);
    assert_eq!(state.editor.dns_hosts_editor.rows[0].id, first);
    assert_eq!(
        state.editor.dns_hosts_editor.rows[0].entry.address,
        "4.4.4.4"
    );
    assert_eq!(store.content(), ORIGINAL_HOSTS);
    let _ = state.update(Message::CancelDnsHostsEditor);
    assert!(!state.editor.dns_hosts_editor.is_open());
    assert_eq!(store.writes.load(Ordering::SeqCst), 0);
    assert_eq!(state.update(Message::SaveDnsHosts).units(), 0);
    let _ = state.update(Message::OpenDnsHostsEditor);
    let _ = state.update(Message::ImportLegacyDnsHosts);
    assert_eq!(state.editor.dns_hosts_editor.rows.len(), 3);
    assert!(state.editor.dns_hosts_editor.is_importing_legacy());
    let mut tree = Tree::new(state.view().as_widget());
    store.deny_save.store(true, Ordering::SeqCst);
    let task = state.update(Message::SaveDnsHosts);
    let token = state.editor.dns_hosts_editor.pending().unwrap().token;
    assert_eq!(state.update(Message::SaveDnsHosts).units(), 0);
    let _ = state.update(Message::CancelDnsHostsEditor);
    assert!(state.editor.dns_hosts_editor.is_open());
    assert!(state.editor.dns_hosts_editor.is_pending());
    let _ = state.update(Message::DnsHostsCommandFinished {
        token: token + 1,
        result: Ok(()),
    });
    assert!(state.editor.dns_hosts_editor.is_pending());
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state.editor.dns_hosts_editor.failure().unwrap().code,
        ErrorCode::Permission
    );
    assert_eq!(state.editor.dns_hosts_editor.rows.len(), 3);
    assert_eq!(
        state
            .editor
            .dns_hosts_editor
            .applied
            .as_ref()
            .unwrap()
            .entries
            .len(),
        2
    );
    assert_eq!(store.content(), ORIGINAL_HOSTS);
    tree.diff(state.view().as_widget());
    store.deny_save.store(false, Ordering::SeqCst);
    let task = state.update(Message::SaveDnsHosts);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(state.editor.dns_hosts_editor.failure().is_none());
    assert_eq!(
        state
            .editor
            .dns_hosts_editor
            .applied
            .as_ref()
            .unwrap()
            .entries
            .len(),
        3
    );
    assert!(
        state
            .editor
            .dns_hosts_editor
            .applied
            .as_ref()
            .unwrap()
            .legacy_entries
            .is_empty()
    );
    assert_eq!(store.writes.load(Ordering::SeqCst), 1);
    let row = state.editor.dns_hosts_editor.rows[0].id;
    let _ = state.update(Message::EditDnsHostRow(row));
    let _ = state.update(Message::UpdateDnsHostsAddress("7.7.7.7".into()));
    let _ = state.update(Message::AddDnsHostRow);
    let draft = state.editor.dns_hosts_editor.rows.clone();
    store.deny_read.store(true, Ordering::SeqCst);
    publish(&mut state, &reader);
    assert_eq!(state.editor.dns_hosts_editor.rows, draft);
    assert_eq!(
        state.editor.dns_hosts_editor.read_failure().unwrap().code,
        ErrorCode::Permission
    );
    assert!(!state.editor.dns_hosts_editor.can_apply());
    store.deny_read.store(false, Ordering::SeqCst);
    publish(&mut state, &reader);
    store.replace("hosts:\n  changed.test: 2.2.2.2\n");
    publish(&mut state, &reader);
    assert_eq!(state.editor.dns_hosts_editor.rows, draft);
    assert!(!state.editor.dns_hosts_editor.can_apply());
    let _ = state.update(Message::CancelDnsHostsEditor);
    publish(&mut state, &reader);
    let _ = state.update(Message::OpenDnsHostsEditor);
    let ids: Vec<_> = state
        .editor
        .dns_hosts_editor
        .rows
        .iter()
        .map(|row| row.id)
        .collect();
    for id in ids {
        let _ = state.update(Message::RemoveDnsHostRow(id));
    }
    let task = state.update(Message::SaveDnsHosts);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert!(
        state
            .editor
            .dns_hosts_editor
            .applied
            .as_ref()
            .unwrap()
            .entries
            .is_empty()
    );
    let _ = state.update(Message::UpdateDnsHostsAddress("6.6.6.6".into()));
    let _ = state.update(Message::UpdateDnsHostsDomain("restored.test".into()));
    let _ = state.update(Message::AddDnsHostRow);
    let task = state.update(Message::SaveDnsHosts);
    let _ = state.update(terminal(task));
    publish(&mut state, &reader);
    assert_eq!(
        state
            .editor
            .dns_hosts_editor
            .applied
            .as_ref()
            .unwrap()
            .entries[0]
            .domain,
        "restored.test"
    );
}
#[test]
fn unsupported_hosts_reader_stays_visible_and_uncommitted_row_input_survives_row_switch_and_cancel()
{
    let (mut state, _) = AppState::demo(&demo_env(Route::Dns));
    state.commands = None;
    state.editor.dns_hosts_editor.observe(&PageData {
        status: PageStatus::Unavailable {
            failure: Failure::new(
                ErrorCode::Unsupported,
                "configuration reader unavailable",
                false,
            ),
        },
        data: None,
    });
    let _ = state.update(Message::OpenDnsHostsEditor);
    assert_eq!(
        state.editor.dns_hosts_editor.read_failure().unwrap().code,
        ErrorCode::Unsupported
    );
    assert!(!state.editor.dns_hosts_editor.can_apply());
    let _ = state.update(Message::UpdateDnsHostsAddress("1.1.1.1".into()));
    let _ = state.update(Message::UpdateDnsHostsDomain("new.test".into()));
    let _ = state.update(Message::EditDnsHostRow(0));
    assert_eq!(state.editor.dns_hosts_editor.address(), "1.1.1.1");
    assert_eq!(state.editor.dns_hosts_editor.domain(), "new.test");
    let _ = state.update(Message::CancelDnsHostRowInput);
    assert!(state.editor.dns_hosts_editor.address().is_empty());
    let _ = state.update(Message::AddDnsHostRow);
    assert!(!state.editor.dns_hosts_editor.issues.is_empty());
    let _ = state.update(Message::CancelDnsHostsEditor);
    assert!(!state.editor.dns_hosts_editor.is_open());
    assert_eq!(state.update(Message::SaveDnsHosts).units(), 0);
}

//! The Iced adapter consumes the settings owner's typed Connections view
//! readback and emits typed intents without keeping a second preference state.
//! test-intent: behavior

use crate::state::AppState;
use crate::state::diagnostics::default_connection_view_projection;
use crate::test_mounts::command_harness::{
    RecordingHandler, attach_product_application_with_accepted_owner_snapshot,
    recording_application,
};
use crate::types::message::Message;
use futures_util::StreamExt;
use iced::Task;
use iced_runtime::Action;
use iced_runtime::task::into_stream;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::connection_view_preference::{
    ConnectionColumn, ConnectionColumnSet, ConnectionViewIntent, ConnectionViewMode,
    ConnectionViewPreference,
};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{SettingsPageSnapshot, SurfaceSnapshot};
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::page::PageData;
use std::sync::Arc;
use tokio::runtime::Builder;

fn snapshot(revision: u64, settings: PageData<SettingsPageSnapshot>) -> SurfaceSnapshot {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = revision;
    snapshot.core.revision = revision;
    snapshot.pages.settings = settings;
    snapshot
}

fn settings_snapshot(preference: ConnectionViewPreference) -> SettingsPageSnapshot {
    SettingsPageSnapshot {
        close_to_tray: None,
        notifications_enabled: None,
        language: "zh-CN".into(),
        theme: "system".into(),
        autostart: false,
        system_proxy: false,
        lan_sharing: None,
        lan_security: None,
        ipv6_routing: None,
        pac: Default::default(),
        tun_enabled: None,
        tun_stack: None,
        tun_auto_route: None,
        tun_strict_route: None,
        controller_port: None,
        log_level: None,
        core_channel: "stable".into(),
        mini_hud: Default::default(),
        refresh_current_profile_on_start: false,
        custom_accent_color: None,
        tray_icon_color_mode: Default::default(),
        sidebar_expansion: Default::default(),
        connection_view: preference,
    }
}

fn table_preference() -> ConnectionViewPreference {
    ConnectionViewPreference::new(
        ConnectionViewMode::Table,
        ConnectionColumnSet::new(vec![
            ConnectionColumn::DnsMode,
            ConnectionColumn::Host,
            ConnectionColumn::SourceIp,
        ])
        .expect("custom column order is valid"),
    )
    .expect("table preference is valid")
}

fn terminal(task: Task<Message>) -> Message {
    Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime")
        .block_on(async {
            let mut stream = into_stream(task).expect("typed connection view command task");
            let Some(Action::Output(message)) = stream.next().await else {
                panic!("typed connection view command must deliver a terminal message");
            };
            assert!(stream.next().await.is_none());
            message
        })
}

#[test]
fn cold_start_publishes_the_contract_product_default_without_fabricating_a_readback() {
    let (state, _) = AppState::new();

    assert_eq!(
        state.diag.connection_view,
        default_connection_view_projection()
    );
    assert_eq!(state.diag.connection_view.mode, ConnectionViewMode::List);
    assert_eq!(
        state
            .diag
            .connection_view
            .columns
            .iter()
            .map(|column| column.column)
            .collect::<Vec<_>>(),
        ConnectionColumn::DEFAULT_VISIBLE.to_vec()
    );
}

#[test]
fn published_settings_readback_reprojects_table_mode_and_the_exact_column_order() {
    let (mut state, _) = AppState::new();
    let preference = table_preference();

    assert!(state.apply_shared_surface_snapshot(snapshot(
        7,
        PageData::ready(settings_snapshot(preference.clone()))
    )));

    assert_eq!(state.diag.connection_view.mode, ConnectionViewMode::Table);
    assert_eq!(
        state
            .diag
            .connection_view
            .columns
            .iter()
            .map(|column| column.column)
            .collect::<Vec<_>>(),
        preference.visible_columns.as_slice().to_vec()
    );
    assert_eq!(
        state
            .diag
            .connection_view
            .columns
            .iter()
            .map(|column| column.semantic_id)
            .collect::<Vec<_>>(),
        [
            "connection.column.dns_mode",
            "connection.column.host",
            "connection.column.source_ip",
        ]
    );
}

#[test]
fn unavailable_settings_keep_the_last_published_projection_without_inventing_a_default() {
    let (mut state, _) = AppState::new();
    assert!(state.apply_shared_surface_snapshot(snapshot(
        8,
        PageData::ready(settings_snapshot(table_preference()))
    )));
    let published = state.diag.connection_view.clone();

    assert!(state.apply_shared_surface_snapshot(snapshot(
        9,
        PageData::unavailable(Failure::new(
            ErrorCode::NotReady,
            "settings owner unavailable",
            true,
        ))
    )));

    assert_eq!(state.diag.connection_view, published);
    assert_ne!(
        state.diag.connection_view,
        default_connection_view_projection()
    );
}

#[test]
fn connection_view_message_dispatches_only_the_typed_owner_command() {
    let (application, handler): (_, Arc<RecordingHandler>) = recording_application();
    let (mut state, _) = AppState::new();
    attach_product_application_with_accepted_owner_snapshot(&mut state, application);
    let before = state.diag.connection_view.clone();
    let intent = ConnectionViewIntent::SetViewMode {
        mode: ConnectionViewMode::Table,
    };

    assert!(matches!(
        terminal(state.update(Message::ApplyConnectionViewPreference {
            intent: intent.clone()
        })),
        Message::Noop
    ));

    assert_eq!(
        handler.0.lock().expect("handler calls").as_slice(),
        [CommandIntent::ApplyConnectionViewPreference {
            intent: intent.clone()
        }]
    );
    assert_eq!(state.diag.connection_view, before);
}

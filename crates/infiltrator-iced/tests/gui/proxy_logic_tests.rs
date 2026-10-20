//! Proxy-page logic tests: filtering, delay sorting, runtime connection
//! sort/filter state and the runtime proxy selector sync/apply flow.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::test_mounts::command_harness::{
    RecordingHandler, install_test_core_capabilities, recording_application,
};
use crate::test_mounts::outbound_fixtures::{
    OutboundFixture, OutboundFixtureInput, standard_members, terminal,
};
use crate::types::message::Message;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{CoreOperation, OutboundOperation};
use infiltrator_domain::connection_view::ConnectionSortKey;
use lurker_core_contract::outbound::preferences::OutboundFavoriteState;
use lurker_core_contract::outbound::preferences::OutboundLayout;
use lurker_core_contract::outbound::preferences::{
    OutboundHealthFilter, OutboundSortOrder, OutboundUiPreferenceIntent,
};
use std::sync::Arc;

fn product_state() -> (
    AppState,
    CoreApplication,
    Arc<RecordingHandler>,
    OutboundFixture,
) {
    let (application, handler) = recording_application();
    install_test_core_capabilities(
        &application,
        [
            CoreOperation::Outbound(OutboundOperation::List),
            CoreOperation::Outbound(OutboundOperation::Select),
            CoreOperation::Outbound(OutboundOperation::Probe),
        ],
    );
    let scope = application
        .snapshot()
        .scope()
        .expect("identified test owner");
    let fixture = OutboundFixture::new(OutboundFixtureInput {
        scope: &scope,
        revision: 41,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    let mut state = AppState::empty();
    fixture.attach_to(&mut state, &application);
    (state, application, handler, fixture)
}

fn visible_member_names(state: &AppState) -> Vec<String> {
    state
        .runtime
        .filtered_groups
        .iter()
        .flat_map(|(group_name, member_ids)| {
            let Some(group) = state
                .runtime
                .proxy_groups
                .iter()
                .find(|group| &group.name == group_name)
            else {
                return Vec::new();
            };
            member_ids
                .iter()
                .filter_map(|member_id| {
                    group
                        .proxies
                        .iter()
                        .find(|member| &member.name == member_id)
                        .map(|member| member.search_name().to_owned())
                })
                .collect()
        })
        .collect()
}

#[test]
fn test_proxy_filtering_and_sorting() {
    let (mut state, _, _, _) = product_state();
    let _ = state.update(Message::FilterProxies("Proxy".into()));
    assert_eq!(state.runtime.proxy_filter, "Proxy");
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members, &["proxy-b".to_owned(), "proxy-a".to_owned()]);
    assert_eq!(visible_member_names(&state), ["Proxy-B", "Proxy-A"]);
}

#[test]
fn test_runtime_auto_refresh_toggle() {
    let (mut state, _) = AppState::new();
    assert!(state.runtime.runtime_auto_refresh);

    let _ = state.update(Message::UpdateRuntimeAutoRefresh(false));
    assert!(!state.runtime.runtime_auto_refresh);

    let _ = state.update(Message::UpdateRuntimeAutoRefresh(true));
    assert!(state.runtime.runtime_auto_refresh);
}

#[test]
fn test_connections_page_sort_key_switch() {
    let (mut state, _) = AppState::new();
    assert_eq!(
        state.diag.connections_model.sort_key(),
        ConnectionSortKey::DownloadDesc
    );

    let _ = state.update(Message::UpdateRuntimeConnectionSort(
        ConnectionSortKey::UploadDesc,
    ));
    assert_eq!(
        state.diag.connections_model.sort_key(),
        ConnectionSortKey::UploadDesc
    );

    // The rate keys are part of the shared typed key, so the page model can
    // hold them and the persisted identifier round-trips exactly.
    let _ = state.update(Message::UpdateRuntimeConnectionSort(
        ConnectionSortKey::DownloadRateDesc,
    ));
    assert_eq!(
        state.diag.connections_model.sort_key(),
        ConnectionSortKey::DownloadRateDesc
    );
    assert_eq!(
        ConnectionSortKey::from_identifier(ConnectionSortKey::DownloadRateDesc.as_str()),
        ConnectionSortKey::DownloadRateDesc
    );
}

#[test]
fn test_profiles_filter_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateProfilesFilter("default".into()));
    assert_eq!(state.profile.profiles_model.filter(), "default");
}

#[test]
fn test_runtime_connection_filter_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateRuntimeConnectionFilter("api".into()));
    assert_eq!(state.runtime.runtime_connection_filter, "api");
}

#[test]
fn test_runtime_proxy_selector_sync_and_apply() {
    let (mut state, _, handler, fixture) = product_state();
    assert_eq!(state.runtime.runtime_selected_group, "GLOBAL");
    assert_eq!(state.runtime.runtime_selected_proxy, "proxy-a");
    let prepare = state
        .prepare_runtime_outbound_selection_message("GLOBAL", "proxy-b")
        .expect("selection is formed from the accepted inventory");
    assert!(matches!(
        prepare,
        Message::PrepareRuntimeOutboundSelection { .. }
    ));
    let _ = state.update(prepare);
    assert_eq!(state.runtime.runtime_selected_proxy, "proxy-b");
    let changed = OutboundFixture::new(OutboundFixtureInput {
        scope: &fixture.scope,
        revision: 42,
        members: &standard_members(),
        selected: Some("proxy-b"),
        favorites: &[],
        health_filter: OutboundHealthFilter::All,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundInventory(changed.inventory));
    let select = terminal(state.update(Message::ApplyRuntimeSelectedProxy));
    let result = terminal(state.update(select));
    let _ = state.update(result);
    assert_eq!(state.runtime.runtime_selected_group, "GLOBAL");
    assert_eq!(state.runtime.runtime_selected_proxy, "proxy-b");
    assert!(matches!(
        handler.0.lock().unwrap().as_slice(),
        [CommandIntent::SelectOutbound { request }]
            if request.outbound_id() == &fixture.ids["proxy-b"]
                && request.collection_id() == &fixture.group_id
                && request.expected_revision().value() == 41
    ));
}

#[test]
fn test_filter_alive_and_favorite_pinning() {
    let (mut state, _, handler, initial) = product_state();
    assert_eq!(state.runtime.filtered_groups.len(), 1);
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members.len(), standard_members().len());
    assert_eq!(members[0], "node-alive-2");
    assert_eq!(members[1], "proxy-b");
    assert_eq!(
        visible_member_names(&state),
        [
            "Node-Alive-2",
            "Proxy-B",
            "日本 Tokyo-01",
            "Proxy-A",
            "Node-Alive-1",
            "香港 IEPL-01",
            "Node-Dead",
        ]
    );

    let healthy = OutboundFixture::new(OutboundFixtureInput {
        scope: &initial.scope,
        revision: 42,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &[],
        health_filter: OutboundHealthFilter::HealthyOnly,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            healthy.preferences,
        ));
    let task = state.update(Message::ToggleFilterAlive(true));
    let _ = state.update(terminal(task));
    assert!(state.runtime.proxies_model.healthy_only());
    let alive_members = &state.runtime.filtered_groups[0].1;
    assert_eq!(alive_members.len(), standard_members().len() - 1);
    assert!(!alive_members.contains(&"node-dead".to_owned()));
    assert!(!visible_member_names(&state).contains(&"Node-Dead".to_owned()));

    let pinned = OutboundFixture::new(OutboundFixtureInput {
        scope: &initial.scope,
        revision: 43,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &["node-alive-1"],
        health_filter: OutboundHealthFilter::HealthyOnly,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::Grid,
    });
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            pinned.preferences,
        ));
    let task = state.update(Message::ToggleFavoriteOutbound(
        initial.ids["node-alive-1"].clone(),
    ));
    let _ = state.update(terminal(task));
    assert!(
        state
            .runtime
            .proxies_model
            .is_favorite(&initial.ids["node-alive-1"])
    );
    let fav_members = &state.runtime.filtered_groups[0].1;
    assert_eq!(fav_members[0], "node-alive-1");
    assert_eq!(fav_members[1], "node-alive-2");
    let visible_favorites = visible_member_names(&state);
    assert_eq!(visible_favorites[0], "Node-Alive-1");
    assert_eq!(visible_favorites[1], "Node-Alive-2");
    let applied_preferences = handler
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|intent| match intent {
            CommandIntent::ApplyOutboundUiPreference { request } => Some(request.intent().clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        applied_preferences,
        vec![
            OutboundUiPreferenceIntent::SetHealthFilter(OutboundHealthFilter::HealthyOnly),
            OutboundUiPreferenceIntent::SetFavorite {
                outbound_id: initial.ids["node-alive-1"].clone(),
                state: OutboundFavoriteState::Pinned,
            },
        ]
    );

    let compact = OutboundFixture::new(OutboundFixtureInput {
        scope: &initial.scope,
        revision: 44,
        members: &standard_members(),
        selected: Some("proxy-a"),
        favorites: &["node-alive-1"],
        health_filter: OutboundHealthFilter::HealthyOnly,
        sort_order: OutboundSortOrder::LatencyAscending,
        layout: OutboundLayout::CompactList,
    });
    handler
        .2
        .lock()
        .unwrap()
        .push_back(CommandOutput::OutboundUiPreferencesApplied(
            compact.preferences,
        ));
    let task = state.update(Message::ToggleProxyCompactView);
    let _ = state.update(terminal(task));
    assert_eq!(
        state.runtime.proxies_model.layout(),
        OutboundLayout::CompactList
    );
}

#[test]
fn test_pinyin_fuzzy_filter_in_gui() {
    let (mut state, _, _, _) = product_state();

    // Search with pinyin initials "xg" -> should match 香港 IEPL-01
    let _ = state.update(Message::FilterProxies("xg".into()));
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0], "hong-kong");
    assert_eq!(visible_member_names(&state), ["香港 IEPL-01"]);

    // Search with country code "jp" -> should match 日本 Tokyo-01
    let _ = state.update(Message::FilterProxies("jp".into()));
    let members = &state.runtime.filtered_groups[0].1;
    assert_eq!(members.len(), 1);
    assert_eq!(members[0], "japan");
    assert_eq!(visible_member_names(&state), ["日本 Tokyo-01"]);
}

#[test]
fn test_custom_node_modal_interactions() {
    let (mut state, _) = AppState::new();
    assert!(!state.runtime.is_adding_custom_node);

    let _ = state.update(Message::OpenAddCustomNodeModal(true));
    assert!(state.runtime.is_adding_custom_node);

    let _ = state.update(Message::UpdateNewNodeType("vless".into()));
    assert_eq!(state.runtime.new_node_type, "vless");

    let _ = state.update(Message::UpdateNewNodeName("US-Fast-01".into()));
    assert_eq!(state.runtime.new_node_name, "US-Fast-01");

    let _ = state.update(Message::UpdateNewNodeServer("1.2.3.4".into()));
    assert_eq!(state.runtime.new_node_server, "1.2.3.4");

    let _ = state.update(Message::UpdateNewNodePort("443".into()));
    assert_eq!(state.runtime.new_node_port, "443");

    let _ = state.update(Message::UpdateNewNodeCredential("a1b2c3d4-uuid".into()));
    assert_eq!(state.runtime.new_node_credential, "a1b2c3d4-uuid");

    let _ = state.update(Message::UpdateNewNodeCipher(
        "chacha20-ietf-poly1305".into(),
    ));
    assert_eq!(state.runtime.new_node_cipher, "chacha20-ietf-poly1305");

    let _ = state.update(Message::UpdateNewNodeTls(true));
    assert!(state.runtime.new_node_tls);

    let _ = state.update(Message::OpenAddCustomNodeModal(false));
    assert!(!state.runtime.is_adding_custom_node);
}

//! test-intent: behavior
//! The Iced surface consumes the shared page models: every page renders from
//! the shared owner and holds no second derivation of the page state.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::BadgeKind;
use crate::view::lan_sharing_card::lan_sharing_status;
use infiltrator_composition::capture_fixtures::list_document;
use infiltrator_contract::app_routing::AppRoutingMode;
use infiltrator_contract::lan::LanSharingSnapshot;
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
use infiltrator_contract::runtime_control::{RuntimeControlSnapshot, RuntimeControlStatus};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::{AppRoutingPageSnapshot, SurfaceSnapshot};
use infiltrator_domain::app_routing;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_presentation::dns_page_model::{DnsEditMode, DnsOperation, DnsPageTab};
use infiltrator_presentation::editable_draft::EditableDraft;
use infiltrator_presentation::filter_query::FilterQuery;
use infiltrator_presentation::operation_phase::OperationState;
use infiltrator_presentation::overview_page_model::OverviewRunState;
use infiltrator_presentation::profiles_page_model::ProfilesOperation;
use infiltrator_presentation::proxies_page_model::{PROXY_SORT_OPTIONS, proxy_protocol_chip};
use infiltrator_presentation::rules_page_model::RulesOperation;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::outbound::preferences::{OutboundHealthFilter, OutboundSortOrder};

#[test]
fn the_proxies_page_reads_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The typed query is the shared model's, not a surface-local string.
    let _ = state.update(Message::FilterProxies("HK".into()));
    assert_eq!(
        state.runtime.proxies_model.query(),
        &FilterQuery::Text("hk".into())
    );
    assert_eq!(state.runtime.proxies_model.search_text(), "hk");

    // A latency bound is a typed query, not a free-text string the surface
    // re-parses on its own.
    let _ = state.update(Message::FilterProxies("<120".into()));
    assert!(matches!(
        state.runtime.proxies_model.query(),
        FilterQuery::Latency(_)
    ));

    // The sort order and the health filter are the shared model's.
    state
        .runtime
        .proxies_model
        .select_sort(OutboundSortOrder::NameDescending);
    assert_eq!(
        state.runtime.proxies_model.sort_order(),
        OutboundSortOrder::NameDescending
    );
    state
        .runtime
        .proxies_model
        .select_health_filter(OutboundHealthFilter::HealthyOnly);
    assert!(state.runtime.proxies_model.healthy_only());

    // The shared sort option table is the one the surface renders.
    assert_eq!(PROXY_SORT_OPTIONS.len(), 4);
    assert_eq!(
        state.runtime.proxies_model.sort_index(),
        PROXY_SORT_OPTIONS
            .iter()
            .position(|option| *option == OutboundSortOrder::NameDescending)
            .unwrap()
    );
}

#[test]
fn the_protocol_chip_label_has_one_shared_implementation() {
    // The surface has no private copy of the chip label any more: the shared
    // owner answers for every surface.
    assert_eq!(proxy_protocol_chip("Shadowsocks"), "Shadowsocks");
    assert_eq!(proxy_protocol_chip("hy2"), "Hysteria2");
    assert_eq!(proxy_protocol_chip(""), "Proxy");
}

#[test]
fn the_settings_page_reads_the_shared_drafts() {
    let (mut state, _) = AppState::new();

    // The LAN sharing draft is the shared model's.
    let _ = state.update(Message::ToggleLanSharing(true));
    assert!(state.runtime.settings_model.lan_sharing().enabled);
    assert!(
        state.runtime.settings_model.lan_sharing_modified(),
        "an edited draft is a modification of the owner value"
    );

    // A draft that returns to the owner value shows the owner value again:
    // the surface has no dirty boolean next to it.
    let _ = state.update(Message::ToggleLanSharing(false));
    assert!(!state.runtime.settings_model.lan_sharing().enabled);

    // The LAN security draft is the shared model's too.
    let _ = state.update(Message::UpdateLanAllowedIps("10.0.0.0/8".into()));
    assert_eq!(
        state.runtime.settings_model.lan_security().allowed_ips,
        vec!["10.0.0.0/8".to_owned()]
    );
    assert!(state.runtime.settings_model.lan_security_modified());

    // The PAC draft is the shared model's.
    let _ = state.update(Message::TogglePacMode(true));
    assert!(state.runtime.settings_model.pac().enabled);
    assert!(state.runtime.settings_model.pac_modified());
}

#[test]
fn the_rules_page_reads_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The active tab and the active JSON section are the shared model's.
    let _ = state.update(Message::SetRulesTab(RulesTab::JsonEditors));
    assert_eq!(state.editor.rules_model.tab(), RulesTab::JsonEditors);
    let _ = state.update(Message::SetRulesJsonTab(RulesJsonSection::Sniffer));
    assert_eq!(
        state.editor.rules_model.json_section(),
        RulesJsonSection::Sniffer
    );

    // The JSON document buffer is a typed draft with its own baseline.
    state
        .editor
        .rules_model
        .observe_json_document(RulesJsonSection::Sniffer, "{}".to_owned());
    assert!(
        !state
            .editor
            .rules_model
            .json_modified(RulesJsonSection::Sniffer)
    );
    state
        .editor
        .rules_model
        .json_buffer_mut(RulesJsonSection::Sniffer)
        .edit("{\"a\":1}".to_owned());
    assert!(
        state
            .editor
            .rules_model
            .json_modified(RulesJsonSection::Sniffer)
    );
    assert!(matches!(
        state
            .editor
            .rules_model
            .json_buffer(RulesJsonSection::Sniffer),
        EditableDraft::Modified { .. }
    ));

    // Every operation phase is the shared phase machine.
    let phase = state
        .editor
        .rules_model
        .operation_mut(RulesOperation::SaveRuleProvidersJson);
    let token = phase.begin();
    assert!(matches!(phase.state(), OperationState::Running { .. }));
    assert!(phase.succeed(token));
}

#[test]
fn the_profiles_page_reads_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The list filter is the shared model's.
    state.profile.profiles_model.edit_filter("  HK  ");
    assert_eq!(state.profile.profiles_model.filter(), "HK");

    // Every operation phase is the shared phase machine, and the URL import
    // and the local file import share one phase.
    let phase = state
        .profile
        .profiles_model
        .operation_mut(ProfilesOperation::Import);
    let token = phase.begin();
    assert!(phase.is_running());
    assert!(phase.succeed(token));
    assert!(!state.profile.operation_running(ProfilesOperation::Import));
}

#[test]
fn the_dns_page_reads_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The partition and the edit mode are the shared model's.
    let _ = state.update(Message::SetDnsTab(DnsPageTab::Tun));
    assert_eq!(state.editor.dns_model.tab(), DnsPageTab::Tun);
    let _ = state.update(Message::SetAdvancedMode(DnsPageTab::Tun, DnsEditMode::Json));
    assert_eq!(state.editor.dns_model.edit_mode(), DnsEditMode::Json);

    // The four write/probe phases are the shared phase machines.
    let token = state
        .editor
        .dns_model
        .operation_mut(DnsOperation::SaveTun)
        .begin();
    assert!(state.editor.dns_operation_running(DnsOperation::SaveTun));
    assert!(
        state
            .editor
            .dns_model
            .operation_mut(DnsOperation::SaveTun)
            .succeed(token)
    );
    assert!(!state.editor.dns_operation_running(DnsOperation::SaveTun));
}

#[test]
fn the_app_routing_page_reads_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The scan is a phase, not a boolean, and an unobserved mode is `None`.
    assert!(state.shell.uwp_loopback.mode().is_none());
    assert!(!state.shell.uwp_loopback.scan_phase().is_running());
    let _ = state.update(Message::ScanUwpApps);
    assert!(state.shell.uwp_loopback.scan_phase().is_running());
}

#[test]
fn the_overview_page_reads_the_shared_page_model() {
    let (state, _) = AppState::new();

    // The run tri-state is the shared model's.
    assert_eq!(
        state.runtime.overview_model.run_state(),
        &OverviewRunState::Running
    );
}

#[test]
fn the_sync_page_reads_the_shared_page_model() {
    let (mut state, _) = AppState::new();

    // The WebDAV phases live in the profiles page model: the sync page reads
    // them from there instead of keeping a second copy.
    let phase = state
        .profile
        .profiles_model
        .operation_mut(ProfilesOperation::SyncWebDav);
    let token = phase.begin();
    assert!(phase.is_running());
    assert!(phase.succeed(token));

    // The sync page's own phases are the shared phase machines.
    let phase = state.profile.sync_model.sync_now_phase_mut();
    let token = phase.begin();
    assert!(phase.is_running());
    assert!(phase.succeed(token));
}

#[test]
fn the_lan_sharing_card_reads_the_owner_readback_and_says_unobserved_without_one() {
    let (mut state, _) = AppState::new();

    // Without an owner readback the card says "not observed" — it never
    // reports an unobserved sharing as "off".
    assert!(!state.runtime.settings_model.lan_sharing_observed());
    assert_eq!(state.runtime.settings_model.lan_sharing_revision(), None);
    let (copy, kind) = lan_sharing_status(false, false, &Lang("en-US"));
    assert_eq!(copy, "Not observed");
    assert_eq!(kind, BadgeKind::Neutral);

    // The owner's readback is one typed fact: the surface renders it whole,
    // with the revision the write can be fenced against.
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 1;
    snapshot.runtime_control = RuntimeControlSnapshot {
        status: RuntimeControlStatus::Ready,
        tun_enabled: Some(false),
        lan_sharing: Some(LanSharingSnapshot::new(12, true, 7891, "192.168.1.10")),
        ..Default::default()
    };
    assert!(state.apply_shared_surface_snapshot(snapshot));
    assert!(state.runtime.settings_model.lan_sharing_observed());
    assert_eq!(
        state.runtime.settings_model.lan_sharing_revision(),
        Some(12)
    );
    assert!(state.runtime.settings_model.lan_sharing().enabled);
    assert_eq!(state.runtime.settings_model.lan_sharing().mixed_port, 7891);
    assert_eq!(
        state.runtime.settings_model.lan_sharing().bind_address,
        "192.168.1.10"
    );
    let (copy, kind) = lan_sharing_status(true, true, &Lang("en-US"));
    assert_eq!(copy, "LAN Active");
    assert_eq!(kind, BadgeKind::Success);
}

#[test]
fn the_pac_write_carries_the_owner_bypass_lan_and_minify() {
    let (mut state, _) = AppState::new();

    // The owner's PAC readback is the draft: the LAN bypass and the minifier
    // are the values the owner applied, not surface defaults.
    let snapshot = PacSnapshot {
        state: PacServiceState::Running {
            url: "http://127.0.0.1:25211/proxy.pac".to_owned(),
        },
        script_bytes: 128,
        bypass_domains: vec!["example.com".to_owned()],
        bypass_lan: true,
        minify: true,
        revision: 4,
    };
    let _ = state.update(Message::PacApplied(Ok(snapshot)));

    let draft = state.runtime.settings_model.pac();
    assert!(draft.bypass_lan);
    assert!(draft.minify);

    // The request the write builds is that draft's own shared conversion, so
    // the write carries the owner's values.
    let request = draft.to_request();
    assert!(request.bypass_lan);
    assert!(request.minify);
    assert_eq!(request.bypass_domains, vec!["example.com".to_owned()]);
}

#[test]
fn the_app_routing_mode_says_not_observed_instead_of_claiming_a_default() {
    let (mut state, _) = AppState::new();

    // The owner published no routing mode: the page says so instead of
    // claiming the default the owner never reported.
    assert_eq!(state.shell.uwp_loopback.mode(), None);
    assert!(!state.shell.uwp_loopback.mode_observed());
    assert_eq!(
        state.shell.uwp_loopback.mode_copy_key(),
        "app_routing_mode_unobserved"
    );

    // The owner's page readback is the only source of the mode.
    state
        .shell
        .uwp_loopback
        .observe_page(AppRoutingPageSnapshot {
            mode: AppRoutingMode::ProxySelected,
            include_system: false,
            apps: Vec::new(),
            uwp_loopback: Default::default(),
        });
    assert_eq!(
        state.shell.uwp_loopback.mode(),
        Some(app_routing::AppRoutingMode::ProxySelected)
    );
    assert!(state.shell.uwp_loopback.mode_observed());
    assert_eq!(
        state.shell.uwp_loopback.mode_copy_key(),
        "app_routing_mode_selected_proxy"
    );
}

#[test]
fn the_rules_list_filters_through_the_shared_model() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..5)
        .map(|index| RuleEntry {
            rule: format!("DOMAIN-SUFFIX,site{index}.com,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(list_document(rules))));

    // The query is the shared model's typed filter, not a surface-local
    // string: the surface keeps no second copy of it.
    let _ = state.update(Message::FilterRules("site4".into()));
    assert_eq!(state.editor.rules_model.filter_text(), "site4");
    assert_eq!(state.editor.rules_filtered_count(), 1);
    assert_eq!(state.editor.rules_filtered_indices(), vec![4]);

    // The shared model answers the same question the surface renders from.
    assert_eq!(
        state
            .editor
            .rules_model
            .filtered_indices(&state.editor.rule_list.draft),
        vec![4]
    );
}

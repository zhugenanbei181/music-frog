//! High-fidelity verification tests for the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::test_mounts::rules_dns_tests::rules_tracer::{run, setup};
use crate::test_mounts::script_workbench_tests;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::types::rule_trace::RuleTraceAction;
use crate::types::script::ScriptAction;
use crate::view::virtual_list::VirtualListConfig;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::rule_condition::TrafficField;
use infiltrator_contract::rules_workspace::RulesTab;
use infiltrator_contract::surface_snapshot::ProxyGroupSnapshot;
use infiltrator_desktop::process_enumerator::{ExtendedProcessInfo, ProcessCategory};
use infiltrator_domain::app_routing::{AppRoutingMode, AppRoutingRule};
use std::sync::atomic::Ordering;

#[test]
fn test_advancement_1_live_rule_tracer_contract() {
    let (mut state, reader, store) = setup();

    // 1. Initial tracer state check
    assert_eq!(state.editor.rules_tab, RulesTab::List);
    assert_eq!(state.editor.rules_tracer_input, "");
    assert!(state.editor.rules_tracer_chain.is_none());

    // Switch to Tracer tab
    let _ = state.update(Message::SetRulesTab(RulesTab::Tracer));
    assert_eq!(state.editor.rules_tab, RulesTab::Tracer);

    store.replace("proxy-groups:\n  - name: ProxyGroup\n    type: select\n    proxies: [DIRECT]\n  - name: GameProxy\n    type: select\n    proxies: [DIRECT]\n  - name: FallbackProxy\n    type: select\n    proxies: [DIRECT]\nrules:\n  - DOMAIN-SUFFIX,google.com,ProxyGroup\n  - IP-CIDR,1.1.1.1/32,DIRECT,no-resolve\n  - PROCESS-NAME,steam.exe,GameProxy\n  - MATCH,FallbackProxy\n");

    // Scenario A: trace domain match
    let _ = state.update(Message::UpdateRulesTracerInput(
        "mail.google.com".to_string(),
    ));
    assert_eq!(state.editor.rules_tracer_input, "mail.google.com");
    run(&mut state, &reader);

    let chain0 = state
        .editor
        .rules_tracer_chain
        .clone()
        .expect("match expected");
    assert_eq!(chain0.hit_rule_index, Some(0));
    assert_eq!(
        chain0.matched_rule_raw,
        "DOMAIN-SUFFIX,google.com,ProxyGroup"
    );
    assert_eq!(chain0.target_proxy, "ProxyGroup");
    assert_eq!(chain0.nodes.len(), 5);
    assert!(!chain0.is_fallback);

    // Scenario B: trace IP match
    let _ = state.update(Message::UpdateRulesTracerInput("1.1.1.1".to_string()));
    run(&mut state, &reader);
    let chain1 = state
        .editor
        .rules_tracer_chain
        .clone()
        .expect("IP match expected");
    assert_eq!(chain1.hit_rule_index, Some(1));
    assert_eq!(
        chain1.matched_rule_raw,
        "IP-CIDR,1.1.1.1/32,DIRECT,no-resolve"
    );
    assert_eq!(chain1.target_proxy, "DIRECT");

    let _ = state.update(Message::RuleTrace(RuleTraceAction::Sandbox(
        TrafficField::ProcessName,
        "other.exe".into(),
    )));
    // Scenario C: trace fallback MATCH
    let _ = state.update(Message::UpdateRulesTracerInput(
        "unknown-domain.xyz".to_string(),
    ));
    run(&mut state, &reader);
    let chain_fb = state
        .editor
        .rules_tracer_chain
        .clone()
        .expect("fallback expected");
    assert_eq!(chain_fb.hit_rule_index, Some(3));
    assert_eq!(chain_fb.matched_rule_type, "MATCH");
    assert_eq!(chain_fb.target_proxy, "FallbackProxy");
    assert!(chain_fb.is_fallback);

    // Invalid input preserves the last real report and performs no source read.
    let _ = state.update(Message::UpdateRulesTracerInput(String::new()));
    assert_eq!(state.update(Message::RunRulesTracer).units(), 0);
    assert_eq!(state.editor.rules_tracer_chain.as_ref(), Some(&chain_fb));
    assert_eq!(
        state.editor.rule_trace.failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    assert_eq!(store.rule_loads.load(Ordering::SeqCst), 3);
}

#[test]
fn test_advancement_2_app_routing_grid_state_and_transitions() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert_eq!(state.app_routing.mode, AppRoutingMode::ProxyAll);
    assert!(state.app_routing.processes.is_empty());
    assert!(state.app_routing.custom_rules.is_empty());

    // Switch mode: Global -> Whitelist -> Blacklist
    let _ = state.update(Message::SetAppRoutingMode(AppRoutingMode::ProxySelected));
    assert_eq!(state.app_routing.mode, AppRoutingMode::ProxySelected);

    let _ = state.update(Message::SetAppRoutingMode(AppRoutingMode::BypassSelected));
    assert_eq!(state.app_routing.mode, AppRoutingMode::BypassSelected);

    // Mock loaded processes
    let sample_procs = vec![
        ExtendedProcessInfo {
            pid: 1001,
            ppid: None,
            name: "chrome".to_string(),
            display_name: "Google Chrome".to_string(),
            canonical_name: "chrome".to_string(),
            binary_path: Some("/usr/bin/chrome".to_string()),
            is_system: false,
            category: ProcessCategory::Browser,
            icon_hint: Some("browser".to_string()),
            memory_bytes: 512 * 1024 * 1024,
            total_memory_bytes: 512 * 1024 * 1024,
            child_pids: vec![],
        },
        ExtendedProcessInfo {
            pid: 2002,
            ppid: None,
            name: "code".to_string(),
            display_name: "Visual Studio Code".to_string(),
            canonical_name: "code".to_string(),
            binary_path: Some("/usr/bin/code".to_string()),
            is_system: false,
            category: ProcessCategory::Developer,
            icon_hint: Some("editor".to_string()),
            memory_bytes: 256 * 1024 * 1024,
            total_memory_bytes: 256 * 1024 * 1024,
            child_pids: vec![],
        },
    ];

    let _ = state.update(Message::AppRoutingProcessesLoaded(sample_procs));
    assert_eq!(state.app_routing.processes.len(), 2);
    assert_eq!(state.app_routing.processes[0].name, "chrome");
    assert_eq!(state.app_routing.processes[1].name, "code");

    // Assign custom rule to Chrome: Proxy -> Direct -> Block
    let _ = state.update(Message::SetAppRouteRule {
        process: "chrome".to_string(),
        rule: AppRoutingRule::Direct,
    });
    assert_eq!(
        state.app_routing.custom_rules.get("chrome"),
        Some(&AppRoutingRule::Direct)
    );

    // Verify AppRoutingRule::next() cycle
    assert_eq!(AppRoutingRule::Proxy.next(), AppRoutingRule::Direct);
    assert_eq!(AppRoutingRule::Direct.next(), AppRoutingRule::Block);
    assert_eq!(AppRoutingRule::Block.next(), AppRoutingRule::Proxy);

    // Filter query test
    let _ = state.update(Message::SetAppRoutingFilter("studio".to_string()));
    assert_eq!(state.app_routing.filter_query, "studio");

    // Route navigation test
    let _ = state.update(Message::Navigate(Route::AppRouting));
    assert_eq!(state.shell.current_route, Route::AppRouting);
}

#[test]
fn test_advancement_3_virtual_viewport_scrolling_engine() {
    // 50,000 rules list: row height 40px, viewport height 600px, overscan 5
    let total_rules = 50_000;
    let item_h = 40.0;
    let vp_h = 600.0;
    let cfg = VirtualListConfig::new(total_rules, item_h, vp_h).with_overscan(5);

    // Test A: Scroll top (0px)
    let vp0 = cfg.compute_viewport();
    assert_eq!(vp0.start_index, 0);
    // visible count = ceil(600 / 40) + 1 = 16. With overscan 5 = 21
    assert_eq!(vp0.end_index, 21);
    assert_eq!(vp0.top_spacer_height, 0.0);
    assert_eq!(vp0.bottom_spacer_height, (50_000 - 21) as f32 * 40.0);
    assert_eq!(vp0.total_content_height, 50_000.0 * 40.0);

    // Test B: Scrolled to 40,000px (item index 1000)
    let vp_mid = cfg.with_scroll_offset(40_000.0).compute_viewport();
    assert_eq!(vp_mid.start_index, 1000 - 5); // 995
    assert_eq!(vp_mid.end_index, 1000 + 16 + 5); // 1021
    assert_eq!(vp_mid.top_spacer_height, 995.0 * 40.0);
    assert_eq!(vp_mid.bottom_spacer_height, (50_000 - 1021) as f32 * 40.0);

    // Mathematical invariant: top_spacer + bottom_spacer + rendered_height == total_content_height
    let rendered_count = vp_mid.end_index - vp_mid.start_index;
    let rendered_height = rendered_count as f32 * item_h;
    assert_eq!(
        vp_mid.top_spacer_height + vp_mid.bottom_spacer_height + rendered_height,
        vp_mid.total_content_height
    );
}

#[test]
fn test_advancement_4_proxy_group_reordering_and_reset() {
    let (mut state, _) = AppState::new();

    state.runtime.filtered_groups = vec![
        ("PROXIES".to_string(), vec!["node1".to_string()]),
        ("STREAMING".to_string(), vec!["node2".to_string()]),
        ("GAMES".to_string(), vec!["node3".to_string()]),
        ("FALLBACK".to_string(), vec!["node4".to_string()]),
    ];

    state.runtime.proxy_groups = state
        .runtime
        .filtered_groups
        .iter()
        .map(|(name, _)| ProxyGroupSnapshot {
            name: name.clone(),
            group_type: "Selector".into(),
            classification: None,
            current: String::new(),
            expanded: true,
            proxies: vec![],
        })
        .collect();
    let observed = state.runtime.proxy_groups.clone();
    assert!(!state.runtime.group_order_open);
    let _ = state.update(Message::MoveProxyGroupUp("GAMES".into()));
    assert!(state.runtime.group_order_open);
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec!["PROXIES", "GAMES", "STREAMING", "FALLBACK"]
    );
    let _ = state.update(Message::MoveProxyGroupUp("GAMES".into()));
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec!["GAMES", "PROXIES", "STREAMING", "FALLBACK"]
    );
    let _ = state.update(Message::MoveProxyGroupDown("GAMES".into()));
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec!["PROXIES", "GAMES", "STREAMING", "FALLBACK"]
    );
    let _ = state.update(Message::ResetProxyGroupOrder);
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec!["FALLBACK", "GAMES", "PROXIES", "STREAMING"]
    );
    assert_eq!(
        state.runtime.proxy_groups, observed,
        "editing never reorders the observed product facts"
    );
    let _ = state.update(Message::CancelProxyGroupOrder);
    assert!(!state.runtime.group_order_open);
    assert_eq!(
        state.runtime.group_order_editor.draft,
        vec!["PROXIES", "STREAMING", "GAMES", "FALLBACK"]
    );
}

#[test]
fn test_advancement_5_mini_hud_mode_and_always_on_top() {
    let (mut state, _) = AppState::new();

    // Default state
    assert!(!state.shell.mini_hud_mode);
    assert!(!state.shell.always_on_top);

    // Toggle Mini HUD mode on
    let _ = state.update(Message::ToggleMiniHudMode);
    assert!(state.shell.mini_hud_mode);

    // Toggle always on top
    let _ = state.update(Message::SetAlwaysOnTop(true));
    assert!(state.shell.always_on_top);

    {
        // Render mini HUD view smoke check in inner scope to drop element
        let _view_element = state.view();
    }

    // Toggle Mini HUD mode off
    let _ = state.update(Message::ToggleMiniHudMode);
    assert!(!state.shell.mini_hud_mode);
}

#[test]
fn test_advancement_6_quickjs_script_sandbox_console_lifecycle() {
    let scripts = ScriptApplication::new();
    let mut state = script_workbench_tests::setup(
        scripts.clone(),
        ScriptExportApplication::without_host_port(),
    );

    // Switch to Editor -> Script pane
    let _ = state.update(Message::SetEditorPane(EditorPane::Script));
    assert_eq!(state.editor.editor_pane, EditorPane::Script);

    // Load a shared preset catalogue entry
    let _ = state.update(Message::Script(ScriptAction::SelectPreset(
        "auto-country-groups".to_string(),
    )));
    assert_eq!(
        state.editor.script_sandbox.selected_preset.as_deref(),
        Some("auto-country-groups")
    );
    assert!(
        state
            .editor
            .script_sandbox
            .script_code
            .contains("auto_country_groups")
    );

    // Provide test input YAML with nodes from different regions
    let test_yaml = "proxies:\n  - name: HK-01\n    type: ss\n    server: hk.example.com\n    port: 8388\n  - name: US-01\n    type: ss\n    server: us.example.com\n    port: 8388\n  - name: JP-01\n    type: ss\n    server: jp.example.com\n    port: 8388\n";
    let _ = state.update(Message::Script(ScriptAction::EditYaml(
        test_yaml.to_string(),
    )));

    // Run the sandbox test through the shared application projection
    let task = state.update(Message::Script(ScriptAction::Run));
    script_workbench_tests::complete(&mut state, task);

    // Invariants assertion
    let snapshot = state
        .editor
        .script_sandbox
        .snapshot
        .as_ref()
        .expect("Shared projection expected");
    assert!(snapshot.is_success());
    assert!(snapshot.execution_time_ms < 500); // Strict latency SLA
    assert!(
        snapshot
            .transformed_yaml
            .as_deref()
            .unwrap_or("")
            .contains("proxy-groups")
    );
    // The read model names only the directive that really matched.
    assert_eq!(snapshot.matched_directive_count(), 1);
    assert_eq!(snapshot.matched_directives[0].id, "auto_country_groups");
    assert_eq!(snapshot.hook_stage, "pre_merge");
    assert!(!snapshot.engine_kind.is_real_javascript());
    assert!(snapshot.is_success());
    // The same projection is published for the Bevy surface.
    assert_eq!(
        scripts
            .observation()
            .result
            .as_ref()
            .map(|result| &result.snapshot),
        Some(snapshot)
    );

    // Clear sandbox
    let task = state.update(Message::Script(ScriptAction::Clear));
    script_workbench_tests::complete(&mut state, task);
    assert!(state.editor.script_sandbox.snapshot.is_none());
    assert!(scripts.observation().result.is_none());
}

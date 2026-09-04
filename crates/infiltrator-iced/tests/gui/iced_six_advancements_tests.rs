//! High-fidelity verification tests for the 6 Iced Core Maturity Advancements.
//!
//! Complies strictly with docs/TEST_GOVERNANCE.md (Zero-Tautology Rule):
//! Every assertion validates concrete business contracts, state transitions,
//! exact string/integer values, and mathematical invariants.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::app_routing::{AppRouteRule, AppRoutingMode};
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::types::rules::RulesTab;
use crate::view::virtual_list::VirtualListConfig;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_desktop::process_enumerator::{ExtendedProcessInfo, ProcessCategory};

#[test]
fn test_advancement_1_live_rule_tracer_contract() {
    let (mut state, _) = AppState::new();

    // 1. Initial tracer state check
    assert_eq!(state.editor.rules_tab, RulesTab::RulesList);
    assert_eq!(state.editor.rules_tracer_input, "");
    assert!(state.editor.rules_tracer_result.is_none());

    // Switch to Tracer tab
    let _ = state.update(Message::SetRulesTab(RulesTab::Tracer));
    assert_eq!(state.editor.rules_tab, RulesTab::Tracer);

    // Populate real test rules into the runtime
    state.editor.rules = vec![
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,ProxyGroup".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,1.1.1.1/32,DIRECT".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "PROCESS-NAME,steam.exe,GameProxy".to_string(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,FallbackProxy".to_string(),
            enabled: true,
        },
    ];

    // Scenario A: trace domain match
    let _ = state.update(Message::UpdateRulesTracerInput("mail.google.com".to_string()));
    assert_eq!(state.editor.rules_tracer_input, "mail.google.com");
    let _ = state.update(Message::RunRulesTracer);

    let (idx0, rule0, target0) = state.editor.rules_tracer_result.clone().expect("match expected");
    assert_eq!(idx0, 0);
    assert_eq!(rule0, "DOMAIN-SUFFIX,google.com");
    assert_eq!(target0, "ProxyGroup");

    // Scenario B: trace IP match
    let _ = state.update(Message::UpdateRulesTracerInput("1.1.1.1".to_string()));
    let _ = state.update(Message::RunRulesTracer);
    let (idx1, rule1, target1) = state.editor.rules_tracer_result.clone().expect("IP match expected");
    assert_eq!(idx1, 1);
    assert_eq!(rule1, "IP-CIDR,1.1.1.1/32");
    assert_eq!(target1, "DIRECT");

    // Scenario C: trace fallback MATCH
    let _ = state.update(Message::UpdateRulesTracerInput("unknown-domain.xyz".to_string()));
    let _ = state.update(Message::RunRulesTracer);
    let (idx_fb, rule_fb, target_fb) = state.editor.rules_tracer_result.clone().expect("fallback expected");
    assert_eq!(idx_fb, 3);
    assert_eq!(rule_fb, "MATCH");
    assert_eq!(target_fb, "FallbackProxy");
}

#[test]
fn test_advancement_2_app_routing_grid_state_and_transitions() {
    let (mut state, _) = AppState::new();

    // Initial state
    assert_eq!(state.app_routing.mode, AppRoutingMode::Global);
    assert!(state.app_routing.processes.is_empty());
    assert!(state.app_routing.custom_rules.is_empty());

    // Switch mode: Global -> Whitelist -> Blacklist
    let _ = state.update(Message::SetAppRoutingMode(AppRoutingMode::Whitelist));
    assert_eq!(state.app_routing.mode, AppRoutingMode::Whitelist);

    let _ = state.update(Message::SetAppRoutingMode(AppRoutingMode::Blacklist));
    assert_eq!(state.app_routing.mode, AppRoutingMode::Blacklist);

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
        rule: AppRouteRule::Direct,
    });
    assert_eq!(
        state.app_routing.custom_rules.get("chrome"),
        Some(&AppRouteRule::Direct)
    );

    // Verify AppRouteRule::next() cycle
    assert_eq!(AppRouteRule::Proxy.next(), AppRouteRule::Direct);
    assert_eq!(AppRouteRule::Direct.next(), AppRouteRule::Block);
    assert_eq!(AppRouteRule::Block.next(), AppRouteRule::Proxy);

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

    // Initial order is empty (follows filtered_groups natural order)
    assert!(state.runtime.proxy_group_order.is_empty());

    // Move "GAMES" (idx 2) up -> should become idx 1 (before STREAMING)
    let _ = state.update(Message::MoveProxyGroupUp("GAMES".to_string()));
    assert_eq!(
        state.runtime.proxy_group_order,
        vec!["PROXIES", "GAMES", "STREAMING", "FALLBACK"]
    );

    // Move "GAMES" up again -> should become idx 0 (top priority)
    let _ = state.update(Message::MoveProxyGroupUp("GAMES".to_string()));
    assert_eq!(
        state.runtime.proxy_group_order,
        vec!["GAMES", "PROXIES", "STREAMING", "FALLBACK"]
    );

    // Move "GAMES" down -> should become idx 1
    let _ = state.update(Message::MoveProxyGroupDown("GAMES".to_string()));
    assert_eq!(
        state.runtime.proxy_group_order,
        vec!["PROXIES", "GAMES", "STREAMING", "FALLBACK"]
    );

    // Reset order
    let _ = state.update(Message::ResetProxyGroupOrder);
    assert!(state.runtime.proxy_group_order.is_empty());
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
    let (mut state, _) = AppState::new();

    // Switch to Editor -> Script pane
    let _ = state.update(Message::SetEditorPane(EditorPane::Script));
    assert_eq!(state.editor.editor_pane, EditorPane::Script);

    // Load country groups preset
    let _ = state.update(Message::SelectScriptPreset("country".to_string()));
    assert_eq!(
        state.editor.script_sandbox.selected_preset.as_deref(),
        Some("country")
    );
    assert!(state.editor.script_sandbox.script_code.contains("auto_country_groups"));

    // Provide test input YAML with nodes from different regions
    let test_yaml = "proxies:\n  - name: HK-01\n    type: ss\n    server: hk.example.com\n    port: 8388\n  - name: US-01\n    type: ss\n    server: us.example.com\n    port: 8388\n  - name: JP-01\n    type: ss\n    server: jp.example.com\n    port: 8388\n";
    let _ = state.update(Message::UpdateScriptSandboxInputYaml(test_yaml.to_string()));

    // Run the sandbox test
    let _ = state.update(Message::RunScriptSandboxTest);

    // Invariants assertion
    assert!(state.editor.script_sandbox.execution_error.is_none());
    let res = state.editor.script_sandbox.execution_result.as_ref().expect("Execution result expected");
    assert!(res.success);
    assert!(res.execution_time_ms < 500); // Strict latency SLA
    assert!(res.transformed_yaml.contains("proxy-groups"));

    // Clear sandbox
    let _ = state.update(Message::ClearScriptSandbox);
    assert!(state.editor.script_sandbox.execution_result.is_none());
    assert!(state.editor.script_sandbox.execution_error.is_none());
}

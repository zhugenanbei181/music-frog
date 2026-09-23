//! Rules & DNS/advanced-config tests: render cache + filtering, pagination
//! bounds, lazy JSON editors, form drafts, validation and heavy-sample smoke.
//! Mounted via `src/test_mounts.rs` (crate root).
//! test-intent: behavior

use crate::state::AppState;
use crate::types::dns::{AdvancedConfigsBundle, AdvancedEditMode, DnsTab};
use crate::types::editor::EditorLazyState;
use crate::types::message::Message;
use crate::types::rules::RuleBadgeKind;
use crate::types::runtime::RebuildFlowState;
use crate::view::components::BadgeKind;
use crate::view::rules::{display_rule_type, semantic_badge_kind};
use infiltrator_domain::rules::RuleEntry;

#[test]
fn test_rules_render_cache_and_filter() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::RulesLoaded(Ok(vec![
        RuleEntry {
            rule: "DOMAIN,example.com,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "IP-CIDR,10.0.0.0/8,REJECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,example.net,GLOBAL".into(),
            enabled: true,
        },
    ])));

    assert_eq!(state.editor.rules_render_cache.len(), 3);
    assert_eq!(state.editor.rules_filtered_indices.len(), 3);
    assert_eq!(state.editor.rules_render_cache[0].payload, "example.com");

    let _ = state.update(Message::FilterRules("example.net".into()));
    assert_eq!(state.editor.rules_page, 0);
    assert_eq!(state.editor.rules_filtered_indices.len(), 1);
}

#[test]
fn test_rules_filter_and_pagination_delegate_to_shared_reduction() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..5)
        .map(|index| RuleEntry {
            rule: format!("DOMAIN-SUFFIX,site{index}.com,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    assert_eq!(
        state.editor.rules_filtered_indices,
        infiltrator_domain::rules::view::filter_rule_indices(&state.editor.rules, "")
    );

    state.editor.rules_page_size = 2;
    let _ = state.update(Message::RulesSetPage(1));
    assert_eq!(
        infiltrator_domain::rules::view::page_bounds(1, 5, 2),
        (2, 4)
    );
    assert_eq!(state.editor.rules_page, 1);
    // DUAL-11-08: the visible rows are the shared window at the page's scroll
    // offset, not the whole page slice; the count is bounded by the viewport.
    assert_eq!(
        state.editor.rules_scroll_offset_px,
        infiltrator_domain::rules::view::rule_scroll_offset_for_index(2)
    );
    assert_eq!(
        state.editor.rules_page,
        crate::view::rules_window::rules_window_page(&state)
    );
    assert_eq!(
        state.diag.perf_snapshot.rules_visible_rows,
        crate::view::rules_window::rendered_rule_rows(&state)
    );
    assert!(
        state.diag.perf_snapshot.rules_visible_rows
            <= infiltrator_domain::rules::view::rendered_row_bound(state.editor.rules_viewport_px)
    );

    // Forward paging stops at the shared last page; a stale page clamps back.
    let _ = state.update(Message::RulesNextPage);
    assert_eq!(state.editor.rules_page, 2);
    let _ = state.update(Message::RulesNextPage);
    assert_eq!(state.editor.rules_page, 2);
    let _ = state.update(Message::RulesSetPage(99));
    assert_eq!(
        state.editor.rules_page,
        infiltrator_domain::rules::view::clamp_page(99, 5, 2)
    );

    let _ = state.update(Message::FilterRules("site4".into()));
    assert_eq!(state.editor.rules_page, 0);
    assert_eq!(state.editor.rules_filtered_indices.len(), 1);
}

#[test]
fn test_rules_virtual_window_renders_bounded_rows_for_50k_list() {
    use infiltrator_domain::rules::view;

    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..50_000)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    assert_eq!(state.editor.rules_render_cache.len(), 50_000);

    let viewport = view::RULE_DEFAULT_VIEWPORT_PX;
    let bound = view::rendered_row_bound(viewport);
    // The render list is a window, and a 50,000-entry list renders no more
    // rows than the bound derived from the viewport alone.
    assert!(
        crate::view::rules_window::rendered_rule_rows(&state) <= bound,
        "50k list must render a bounded window"
    );
    assert_eq!(
        state.diag.perf_snapshot.rules_visible_rows,
        crate::view::rules_window::rendered_rule_rows(&state),
        "the published perf fact is the real rendered-row count"
    );

    // Scrolling to the middle slides the window without growing it, and the
    // rendered band starts where the shared window says it does.
    let offset = view::rule_scroll_offset_for_index(25_000);
    let _ = state.update(Message::RulesListScrolled {
        offset_px: offset,
        viewport_px: viewport,
    });
    let window = crate::view::rules_window::rules_window(&state);
    assert_eq!(window.start + view::RULE_WINDOW_OVERSCAN, 25_000);
    assert!(window.contains(25_000));
    assert_eq!(
        crate::view::rules_window::rendered_rule_rows(&state),
        window.rendered_rows()
    );
    assert!(crate::view::rules_window::rendered_rule_rows(&state) <= bound);
    assert_eq!(
        crate::view::rules_window::rules_window_page(&state),
        25_000 / view::DEFAULT_RULE_PAGE_SIZE
    );
    let rendered = crate::view::rules_window::visible_rule_items(&state);
    assert!(
        rendered
            .iter()
            .all(|index| *index >= 25_000 - view::RULE_WINDOW_OVERSCAN)
    );
    assert!(rendered.iter().all(|index| *index < 25_000 + bound));

    // Paging is a scroll jump to the shared page's first row; the window
    // follows the offset through the same shared reduction.
    let (page_start, _) = view::page_bounds(3, 50_000, view::DEFAULT_RULE_PAGE_SIZE);
    let _ = state.update(Message::RulesSetPage(3));
    assert_eq!(
        state.editor.rules_scroll_offset_px,
        view::rule_scroll_offset_for_index(page_start)
    );
    assert_eq!(crate::view::rules_window::rules_window_page(&state), 3);
    assert!(crate::view::rules_window::rendered_rule_rows(&state) <= bound);

    // A measured viewport replaces the declared fallback and tightens the
    // bound; a short viewport still renders at least one row band.
    let _ = state.update(Message::RulesListScrolled {
        offset_px: 0.0,
        viewport_px: 112.0,
    });
    assert_eq!(state.editor.rules_viewport_px, 112.0);
    let small_bound = view::rendered_row_bound(112.0);
    assert!(small_bound < bound);
    assert!(crate::view::rules_window::rendered_rule_rows(&state) <= small_bound);
    assert!(crate::view::rules_window::rendered_rule_rows(&state) > 0);
}

#[test]
fn test_rules_workspace_partitions_delegate_to_shared_vocabulary() {
    use infiltrator_contract::rules_workspace::{RulesJsonSection, RulesTab};
    use infiltrator_shared::locales::{Lang, Localizer};

    let (mut state, _) = AppState::new();
    // The default partition is the shared default, not a surface literal.
    assert_eq!(state.editor.rules_tab, RulesTab::default());
    assert_eq!(state.editor.rules_json_tab, RulesJsonSection::default());
    assert_eq!(RulesTab::default(), RulesTab::List);
    assert_eq!(RulesJsonSection::default(), RulesJsonSection::RuleProviders);

    // Every shared partition is selectable through the shared enum and keeps
    // its shared identity.
    for tab in RulesTab::ALL {
        let _ = state.update(Message::SetRulesTab(tab));
        assert_eq!(state.editor.rules_tab, tab);
        assert_eq!(RulesTab::from_index(tab.index()), tab);
    }
    for section in RulesJsonSection::ALL {
        let _ = state.update(Message::SetRulesJsonTab(section));
        assert_eq!(state.editor.rules_json_tab, section);
        assert_eq!(RulesJsonSection::from_index(section.index()), section);
    }

    // Every shared label key resolves in both language tables: a partition
    // cannot exist with a label the Iced surface cannot render.
    for lang in [Lang("zh-CN"), Lang("en")] {
        for tab in RulesTab::ALL {
            assert_ne!(lang.tr(tab.i18n_key()).as_ref(), tab.i18n_key(), "{tab:?}");
        }
        for section in RulesJsonSection::ALL {
            assert_ne!(
                lang.tr(section.i18n_key()).as_ref(),
                section.i18n_key(),
                "{section:?}"
            );
            assert_ne!(
                lang.tr(section.save_i18n_key()).as_ref(),
                section.save_i18n_key(),
                "{section:?}"
            );
        }
    }

    // DUAL-11-14: the Geo database entry point both surfaces expose is the
    // shared command vocabulary (Iced drives the same gateway call the
    // application performs for the Bevy intent).
    assert_eq!(
        infiltrator_contract::command::CommandIntent::UpgradeGeoDatabases.kind(),
        infiltrator_contract::command::CommandKind::Runtime
    );
    assert_eq!(
        infiltrator_contract::command::CommandIntent::ApplyRulesJsonDocument {
            section: RulesJsonSection::Sniffer,
            json: "{}".to_owned(),
        }
        .kind(),
        infiltrator_contract::command::CommandKind::Profile
    );
}

#[test]
fn test_rules_pagination_bounds() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..450)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    assert_eq!(state.editor.rules_page_size, 200);

    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage);
    let _ = state.update(Message::RulesNextPage); // should clamp to last
    assert_eq!(state.editor.rules_page, 2);

    let _ = state.update(Message::RulesPrevPage);
    assert_eq!(state.editor.rules_page, 1);

    let _ = state.update(Message::RulesSetPage(99));
    assert_eq!(state.editor.rules_page, 2);
}

#[test]
fn test_rules_dns_lazy_editor_state() {
    let (mut state, _) = AppState::new();

    assert_eq!(
        state.editor.rule_providers_editor_state,
        EditorLazyState::Unloaded
    );
    state.editor.rule_providers_json_cache = "{\"a\":1}".into();
    let _ = state.update(Message::EnsureRuleProvidersEditorLoaded);
    assert_eq!(
        state.editor.rule_providers_editor_state,
        EditorLazyState::Loaded
    );
    assert_eq!(state.editor.rule_providers_json_content.text(), "{\"a\":1}");

    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Unloaded);
    state.editor.dns_json_cache = "{\"enable\":true}".into();
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.dns_json_content.text(), "{\"enable\":true}");
}

#[test]
fn test_rules_dns_large_sample_smoke() {
    let (mut state, _) = AppState::new();
    let rules: Vec<RuleEntry> = (0..3200)
        .map(|i| RuleEntry {
            rule: format!("DOMAIN,host-{i}.example,DIRECT"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    assert_eq!(state.editor.rules_render_cache.len(), 3200);

    let large_json = "a".repeat(1024 * 1024);
    state.editor.dns_json_cache = format!("{{\"dns\":\"{}\"}}", large_json);
    state.editor.fake_ip_json_cache = format!("{{\"fake\":\"{}\"}}", large_json);
    state.editor.tun_json_cache = format!("{{\"tun\":\"{}\"}}", large_json);
    let _ = state.update(Message::EnsureDnsEditorLoaded);
    let _ = state.update(Message::EnsureFakeIpEditorLoaded);
    let _ = state.update(Message::EnsureTunEditorLoaded);
    assert_eq!(state.editor.dns_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.fake_ip_editor_state, EditorLazyState::Loaded);
    assert_eq!(state.editor.tun_editor_state, EditorLazyState::Loaded);
}

#[test]
fn test_dns_form_dirty_and_json_sync() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateDnsFormNameserver(
        "1.1.1.1, 8.8.8.8".to_string(),
    ));
    assert!(state.editor.dns_form_dirty);
    let patch: infiltrator_domain::dns::DnsConfigPatch =
        serde_json::from_str(&state.editor.dns_json_cache).expect("dns patch json");
    assert_eq!(
        patch.nameserver,
        Some(vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()])
    );
}

#[test]
fn test_set_advanced_mode_updates_state() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::SetAdvancedMode(
        DnsTab::Dns,
        AdvancedEditMode::Json,
    ));
    assert_eq!(state.editor.dns_mode, AdvancedEditMode::Json);
}

#[test]
fn test_tun_form_invalid_mtu_blocks_save() {
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::UpdateTunFormMtu("abc".to_string()));
    let _ = state.update(Message::SaveTunConfig);
    assert!(!state.editor.is_saving_tun);
    assert!(matches!(
        state.runtime.rebuild_flow,
        RebuildFlowState::Failed { .. }
    ));
    assert!(
        state
            .editor
            .advanced_validation
            .tun
            .as_ref()
            .is_some_and(|msg| msg.to_ascii_lowercase().contains("mtu"))
    );
}

#[test]
fn test_advanced_bundle_load_applies_form_drafts() {
    let (mut state, _) = AppState::new();
    let bundle = AdvancedConfigsBundle {
        dns_json: "{}".to_string(),
        fake_ip_json: "{}".to_string(),
        tun_json: "{}".to_string(),
        dns: infiltrator_domain::dns::DnsConfig {
            enable: Some(true),
            nameserver: Some(vec!["https://dns.google/dns-query".to_string()]),
            enhanced_mode: Some("fake-ip".to_string()),
            ..Default::default()
        },
        fake_ip: infiltrator_domain::fake_ip::FakeIpConfig {
            fake_ip_range: Some("198.18.0.1/16".to_string()),
            store_fake_ip: Some(true),
            ..Default::default()
        },
        tun: infiltrator_domain::tun::TunConfig {
            enable: Some(true),
            stack: Some("gvisor".to_string()),
            mtu: Some(1500),
            ..Default::default()
        },
    };
    let _ = state.update(Message::AdvancedConfigsBundleLoaded(Ok(Box::new(bundle))));
    assert!(state.editor.dns_form.switches.enable);
    assert_eq!(
        state.editor.dns_form.nameserver,
        "https://dns.google/dns-query".to_string()
    );
    assert_eq!(
        state.editor.fake_ip_form.fake_ip_range,
        "198.18.0.1/16".to_string()
    );
    assert!(state.editor.fake_ip_form.store_fake_ip);
    assert!(state.editor.tun_form.enable);
    assert_eq!(state.editor.tun_form.stack, "gvisor".to_string());
    assert_eq!(state.editor.tun_form.mtu, "1500".to_string());
}

#[test]
fn test_rules_tracer_gui_flow() {
    let (mut state, _) = AppState::new();
    state.editor.rules = vec![
        RuleEntry {
            rule: "DOMAIN,special.com,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,节点选择".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,漏网之鱼".into(),
            enabled: true,
        },
    ];
    state.rebuild_rules_render_cache();

    // Input domain and run tracer
    let _ = state.update(Message::UpdateRulesTracerInput("mail.google.com".into()));
    assert_eq!(state.editor.rules_tracer_input, "mail.google.com");

    let _ = state.update(Message::RunRulesTracer);
    let chain = state.editor.rules_tracer_chain.as_ref().unwrap();
    assert_eq!(chain.hit_rule_index, Some(1));
    assert_eq!(chain.matched_rule_raw, "DOMAIN-SUFFIX,google.com");
    assert_eq!(chain.target_proxy, "节点选择");
}

#[test]
fn test_rules_tracer_source_ip_sandbox_flow() {
    let (mut state, _) = AppState::new();
    state.editor.rules = vec![
        RuleEntry {
            rule: "SRC-IP-CIDR,10.0.0.0/8,DIRECT".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,FALLBACK".into(),
            enabled: true,
        },
    ];
    state.rebuild_rules_render_cache();

    let _ = state.update(Message::UpdateRulesTracerInput("example.org".into()));
    let _ = state.update(Message::RunRulesTracer);
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "FALLBACK"
    );

    // Typing a sandbox source IP re-runs the trace: the Inbound stage reflects
    // it and the SRC-IP-CIDR rule now matches the simulated LAN device.
    let _ = state.update(Message::UpdateTracerSourceIp("10.1.2.3".into()));
    assert_eq!(state.editor.rules_tracer_src_ip, "10.1.2.3");
    let chain = state.editor.rules_tracer_chain.as_ref().unwrap();
    assert_eq!(chain.matched_rule_type, "SRC-IP-CIDR");
    assert_eq!(chain.target_proxy, "DIRECT");
    assert!(chain.nodes[0].detail.contains("10.1.2.3"));

    // Clearing the source IP returns to the query-only decision.
    let _ = state.update(Message::UpdateTracerSourceIp(String::new()));
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "FALLBACK"
    );
}

#[test]
fn test_rules_tracer_reverse_apply_override_dual_surface_flow() {
    let (mut state, _) = AppState::new();
    state.editor.rules = vec![
        RuleEntry {
            rule: "DOMAIN-SUFFIX,example.com,PROXY".into(),
            enabled: true,
        },
        RuleEntry {
            rule: "MATCH,DIRECT".into(),
            enabled: true,
        },
    ];
    state.rebuild_rules_render_cache();

    let _ = state.update(Message::UpdateRulesTracerInput("example.com".into()));
    let _ = state.update(Message::RunRulesTracer);
    // The shared decision chain drives the reverse-apply gate and the chooser
    // seed; neither is a UI-local guess.
    assert!(state.editor.rules_tracer_can_reverse_apply);
    assert_eq!(
        state.editor.rules_tracer_suggested_target.as_deref(),
        Some("DIRECT")
    );
    assert_eq!(state.editor.rules_tracer_override_target, "DIRECT");
    assert_eq!(
        state
            .editor
            .rules_tracer_chain
            .as_ref()
            .unwrap()
            .target_proxy,
        "PROXY"
    );

    // Consume the shared typed result: the matched rule is rewritten and the
    // trace replays against the new target.
    let applied = infiltrator_contract::rule_tracer::TracerRuleOverrideResult::applied(
        0,
        "DIRECT".to_owned(),
        "DOMAIN-SUFFIX,example.com,PROXY".to_owned(),
        "DOMAIN-SUFFIX,example.com,DIRECT".to_owned(),
    );
    let _ = state.update(Message::TracerRuleOverrideApplied(applied));
    assert_eq!(
        state.editor.rules[0].rule,
        "DOMAIN-SUFFIX,example.com,DIRECT"
    );
    let chain = state.editor.rules_tracer_chain.as_ref().unwrap();
    assert_eq!(chain.target_proxy, "DIRECT");
    assert_eq!(chain.matched_rule_type, "DOMAIN-SUFFIX");
    assert_eq!(chain.matched_payload, "example.com");

    // A rejected result never mutates the rule list nor fabricates success.
    let rejected = infiltrator_contract::rule_tracer::TracerRuleOverrideResult::rejected(
        infiltrator_contract::rule_tracer::TracerRuleOverrideStatus::ApplyFailed,
        0,
        "REJECT".to_owned(),
        "apply transaction rolled back",
    );
    let _ = state.update(Message::TracerRuleOverrideApplied(rejected));
    assert_eq!(
        state.editor.rules[0].rule,
        "DOMAIN-SUFFIX,example.com,DIRECT"
    );

    // Without a composed apply port the request queues one typed error task
    // and leaves the rule list untouched.
    state.editor.rules_tracer_override_target = "REJECT".to_owned();
    let units = state
        .update(Message::ApplyTracerRuleOverride { rule_index: 0 })
        .units();
    assert_eq!(units, 1);
    assert_eq!(
        state.editor.rules[0].rule,
        "DOMAIN-SUFFIX,example.com,DIRECT"
    );
}

#[test]
fn test_rules_edit_operations_delegate_to_shared_module() {
    let (mut state, _) = AppState::new();
    let entry = |rule: &str| RuleEntry {
        rule: rule.into(),
        enabled: true,
    };
    state.editor.rules = vec![
        entry("DOMAIN,a.com,DIRECT"),
        entry("DOMAIN,b.com,PROXY"),
        entry("MATCH,DIRECT"),
    ];
    state.rebuild_rules_render_cache();

    // DUAL-11-09: toggle flips the shared `RuleEntry.enabled` fact.
    let _ = state.update(Message::ToggleRuleEnabled(1));
    assert!(!state.editor.rules[1].enabled);
    assert!(state.editor.rules_dirty);
    // A stale index is a typed no-op.
    state.editor.rules_dirty = false;
    let _ = state.update(Message::ToggleRuleEnabled(9));
    assert!(!state.editor.rules_dirty);

    // DUAL-11-10: reorder uses the shared swap reduction and edge guards.
    let _ = state.update(Message::MoveRuleUp(2));
    assert_eq!(state.editor.rules[1].rule, "MATCH,DIRECT");
    let _ = state.update(Message::MoveRuleDown(0));
    assert_eq!(state.editor.rules[0].rule, "MATCH,DIRECT");
    assert_eq!(state.editor.rules[1].rule, "DOMAIN,a.com,DIRECT");
    let _ = state.update(Message::MoveRuleUp(0));
    assert_eq!(state.editor.rules[0].rule, "MATCH,DIRECT");

    // DUAL-11-12: presets prepend exactly the shared list, in order.
    state.editor.new_rule_target = "Game-Proxy".into();
    let before = state.editor.rules.len();
    let _ = state.update(Message::ApplyGameRoutingPresets);
    let presets = infiltrator_domain::rules::game_routing_presets("Game-Proxy");
    assert_eq!(state.editor.rules.len(), before + presets.len());
    assert_eq!(state.editor.rules[0].rule, presets[0].rule);
    assert_eq!(state.editor.rules[presets.len()].rule, "MATCH,DIRECT");

    // DUAL-11-11: the wizard and the shared builder agree on the type list and
    // reject an empty payload before any async save is scheduled.
    assert_eq!(
        infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES.len(),
        12
    );
    state.editor.is_adding_rule = false;
    let rule = infiltrator_domain::rules::edit::build_custom_rule(
        &infiltrator_contract::rule_edit::RuleDraft {
            rule_type: "AND".into(),
            payload: "(DOMAIN,a.com),(DST-PORT,443)".into(),
            target: "AI".into(),
        },
    )
    .expect("shared builder");
    assert_eq!(rule.rule, "AND((DOMAIN,a.com),(DST-PORT,443),AI)");
}

#[test]
fn test_rules_type_matrix_and_logical_builder_delegate_to_shared() {
    let (mut state, _) = AppState::new();
    let types = [
        "DOMAIN",
        "DOMAIN-SUFFIX",
        "DOMAIN-KEYWORD",
        "DOMAIN-REGEX",
        "GEOSITE",
        "IP-CIDR",
        "IP-CIDR6",
        "IP-SUFFIX",
        "IP-ASN",
        "GEOIP",
        "SRC-GEOIP",
        "SRC-IP-CIDR",
        "SRC-IP-ASN",
        "DST-PORT",
        "SRC-PORT",
        "IN-PORT",
        "IN-TYPE",
        "IN-NAME",
        "IN-USER",
        "PROCESS-PATH",
        "PROCESS-PATH-REGEX",
        "PROCESS-NAME",
        "PROCESS-NAME-REGEX",
        "NETWORK",
        "DSCP",
        "UID",
        "PACKAGE-NAME",
        "RULE-SET",
    ];
    let rules: Vec<RuleEntry> = types
        .iter()
        .map(|rule_type| RuleEntry {
            rule: format!("{rule_type},payload,TARGET"),
            enabled: true,
        })
        .collect();
    let _ = state.update(Message::RulesLoaded(Ok(rules)));
    let rendered: Vec<String> = state
        .editor
        .rules_render_cache
        .iter()
        .map(|item| item.rule_type.clone())
        .collect();
    assert_eq!(rendered.len(), types.len());
    for rule_type in types {
        assert!(rendered.iter().any(|item| item == rule_type), "{rule_type}");
    }

    // DUAL-11-01: every catalogue spelling renders the shared display label and
    // resolves to a known semantic family (no per-surface spelling list).
    for spec in infiltrator_domain::rules::matrix::RULE_TYPE_MATRIX.iter() {
        assert_eq!(display_rule_type(spec.name), spec.label, "{}", spec.name);
        assert_eq!(
            infiltrator_domain::rules::matrix::matrix_family(spec.name),
            spec.family
        );
        assert!(
            infiltrator_domain::rules::matrix::matrix_family(spec.name)
                != infiltrator_domain::rules::matrix::RuleTypeFamily::Unknown
        );
        assert_eq!(
            semantic_badge_kind(spec.name, RuleBadgeKind::Other),
            match spec.family {
                infiltrator_domain::rules::matrix::RuleTypeFamily::Host => BadgeKind::Accent,
                infiltrator_domain::rules::matrix::RuleTypeFamily::Address => BadgeKind::Warning,
                _ => BadgeKind::Neutral,
            },
            "{}",
            spec.name
        );
    }
    // Separator/case tolerant lookup survives the delegation.
    assert_eq!(display_rule_type("domainsuffix"), "DomainSuffix");
    assert_eq!(display_rule_type("CUSTOM"), "CUSTOM");

    // DUAL-11-02: the surface relies on the shared logical-rule syntax gate.
    assert!(
        infiltrator_domain::sub_rules::validate_logical_rule_syntax(
            "AND((DOMAIN,a.com),(DST-PORT,443),T)"
        )
        .is_ok()
    );
    assert!(
        infiltrator_domain::sub_rules::validate_logical_rule_syntax("AND((DOMAIN,a.com),T")
            .is_err()
    );

    // DUAL-11-02: the builder draft is the shared `LogicalDraft` and the insert
    // builds through the shared reduction, so the persisted expression is the
    // canonical `OP((cond),(cond),TARGET)` form the parser accepts.
    let draft = infiltrator_domain::rules::logical::default_logical_draft(
        infiltrator_domain::rules::edit::DEFAULT_RULE_TARGET,
    );
    let built = infiltrator_domain::rules::logical::build_logical_rule(&draft).unwrap();
    assert_eq!(
        built.rule,
        "AND((DOMAIN-SUFFIX,company.com),(NETWORK,TCP),PROXY)"
    );
    assert_eq!(
        infiltrator_domain::rules::logical::draft_expression(&draft),
        built.rule
    );
    assert_eq!(state.editor.subrule_draft, draft);
    assert_eq!(
        infiltrator_domain::rules::logical::LOGICAL_OPERATOR_CHOICES.len(),
        4
    );

    // DUAL-11-15: the shared view/edit reductions are reachable from the
    // surface and use the same constants the Bevy page consumes.
    assert_eq!(infiltrator_domain::rules::view::DEFAULT_RULE_PAGE_SIZE, 200);
    assert_eq!(
        infiltrator_domain::rules::edit::CUSTOM_RULE_TYPE_CHOICES.len(),
        12
    );
}

/// DUAL-11-05/11-08: the provider's declared refresh interval and the shared
/// publish-truncation fact are projected from one surface snapshot.
#[test]
fn test_rules_provider_interval_and_publish_truncation_project_from_snapshot() {
    use infiltrator_contract::error::{ErrorCode, Failure};
    use infiltrator_contract::surface::{HostKind, SurfaceKind};
    use infiltrator_contract::surface_snapshot::{
        PageData, RuleProviderSnapshot, RulesPageSnapshot, SurfaceSnapshot,
    };

    let (mut state, _) = AppState::new();
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    snapshot.revision = 7;
    snapshot.pages.rules = PageData::ready(RulesPageSnapshot {
        total_rules: 12_000,
        default_action: "DIRECT".to_owned(),
        providers: vec![RuleProviderSnapshot {
            name: "ads".to_owned(),
            rule_count: 120,
            behavior: "domain".to_owned(),
            updated_at: "2026-09-01".to_owned(),
            source_url: Some("https://example.com/ads.mrs".to_owned()),
            refresh_interval_secs: Some(86_400),
            cache_fingerprint: Some(
                infiltrator_contract::provider_cache::ProviderCacheFingerprint {
                    provider: "ads".to_owned(),
                    path: "/home/u/.config/mihomo-rs/rules/1f0f1c3f0d0f0a0b".to_owned(),
                    change:
                        infiltrator_contract::provider_cache::ProviderFingerprintChange::Changed,
                    current: infiltrator_contract::provider_cache::ProviderFileFingerprint {
                        size_bytes: 4_096,
                        sha256: "abcdef0123456789".to_owned(),
                        modified_unix_secs: Some(1_700_000_000),
                    },
                    previous: Some(
                        infiltrator_contract::provider_cache::ProviderFileFingerprint {
                            size_bytes: 2_048,
                            sha256: "0123456789abcdef".to_owned(),
                            modified_unix_secs: Some(1_699_000_000),
                        },
                    ),
                },
            ),
        }],
        rules: vec![Default::default(); 5_000],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        total_hits: 0,
        rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support:
            infiltrator_contract::provider_cache::KernelEtagSupportSnapshot::from_declared(Some(
                true,
            )),
        json_documents: Vec::new(),
    });
    assert!(state.apply_shared_surface_snapshot(snapshot));

    // DUAL-11-05: the declared interval reaches the surface, keyed by provider.
    assert_eq!(
        state.editor.rule_provider_intervals.get("ads").copied(),
        Some(86_400)
    );
    // DUAL-11-05: the local cache fingerprint observation (a real file read,
    // compared with the previous observation) reaches the surface too.
    let observed = state
        .editor
        .rule_provider_fingerprints
        .get("ads")
        .expect("fingerprint observation");
    assert_eq!(observed.change_token(), "changed");
    assert_eq!(observed.current.size_bytes, 4_096);
    assert_eq!(
        observed.previous.as_ref().map(|fact| fact.size_bytes),
        Some(2_048)
    );
    // DUAL-11-05: the kernel's real `etag-support` declaration is projected from
    // the same shared read model; the state and the raw declared value survive.
    assert_eq!(
        state.editor.rule_etag_support.state,
        infiltrator_contract::provider_cache::KernelEtagSupportState::Enabled
    );
    assert_eq!(state.editor.rule_etag_support.declared, Some(true));
    // DUAL-11-08: the publish cap and the omitted count are honest facts.
    assert_eq!(
        state.editor.rule_publish_limit,
        infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT
    );
    assert_eq!(state.editor.rule_publish_omitted, Some(7_000));
    let lang = infiltrator_shared::locales::Lang("en");
    let note = crate::view::rules::publish_truncation_line(&state, &lang)
        .expect("truncation note while the shared view is capped");
    assert!(note.contains("7000"), "{note}");
    assert!(note.contains("5000"), "{note}");

    // A complete published list renders no truncation note.
    let mut complete = SurfaceSnapshot::unavailable(
        SurfaceKind::IcedDesktop,
        HostKind::Desktop,
        Failure::new(ErrorCode::NotReady, "test snapshot", true),
    );
    complete.revision = 8;
    complete.pages.rules = PageData::ready(RulesPageSnapshot {
        total_rules: 5,
        default_action: "DIRECT".to_owned(),
        providers: Vec::new(),
        rules: vec![Default::default(); 5],
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        total_hits: 0,
        rule_publish_limit: infiltrator_domain::rules::view::RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    });
    assert!(state.apply_shared_surface_snapshot(complete));
    assert_eq!(state.editor.rule_publish_omitted, None);
    assert!(crate::view::rules::publish_truncation_line(&state, &lang).is_none());
}

#[test]
fn test_rules_game_presets_and_geo_update() {
    let (mut state, _) = AppState::new();
    state.editor.rules = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];
    state.editor.new_rule_target = "Game-Proxy".into();

    let _ = state.update(Message::ApplyGameRoutingPresets);
    assert!(state.editor.rules.len() > 5);
    assert!(state.editor.rules[0].rule.contains("PROCESS-NAME"));
    assert!(state.editor.rules[0].rule.contains("Game-Proxy"));
    assert!(state.editor.rules_dirty);

    // Without a composed runtime the geo update is refused with a typed
    // toast instead of a fabricated sleep-then-success.
    let _ = state.update(Message::UpdateGeoDatabases);
    assert!(!state.editor.is_updating_geo_databases);

    // The finished handler still owns the busy-flag lifecycle.
    state.editor.is_updating_geo_databases = true;
    let _ = state.update(Message::GeoDatabasesUpdated(Ok(())));
    assert!(!state.editor.is_updating_geo_databases);
}

#[test]
fn test_rule_provider_diff_and_unpack_flow() {
    let (mut state, _) = AppState::new();
    assert!(state.editor.inspecting_rule_provider_diff.is_none());

    let diff = infiltrator_domain::rules::RuleProviderDiff {
        provider_name: "GoogleRules".into(),
        local_count: 5,
        remote_count: 7,
        added_rules: vec!["DOMAIN-SUFFIX,googlevideo.com".into()],
        removed_rules: vec![],
        unchanged_count: 5,
    };

    let _ = state.update(Message::RuleProviderDiffLoaded(Ok(diff.clone())));
    assert_eq!(
        state
            .editor
            .inspecting_rule_provider_diff
            .as_ref()
            .unwrap()
            .provider_name,
        "GoogleRules"
    );

    let _ = state.update(Message::InspectRuleProviderDiff(None));
    assert!(state.editor.inspecting_rule_provider_diff.is_none());

    // DUAL-11-06: without a declaration in the loaded profile nothing is
    // unpacked and no sample rule is invented.
    let prev_len = state.editor.rules.len();
    let _ = state.update(Message::UnpackRuleProvider("GoogleRules".into()));
    assert_eq!(state.editor.rules.len(), prev_len);
    assert!(!state.editor.rules_dirty);
    assert!(
        state
            .editor
            .provider_unpack
            .status_message
            .as_deref()
            .is_some_and(|status| status.contains("not declared")),
        "undeclared providers must be reported honestly"
    );
    // A diff for an undeclared provider is refused for the same reason.
    let _ = state.update(Message::InspectRuleProviderDiff(Some("GoogleRules".into())));
    assert!(state.editor.inspecting_rule_provider_diff.is_none());

    // Verify rule provider row formatting (Domain, IPCIDR, Classical), badges, format chips, and rendering
    let lang = infiltrator_shared::locales::Lang("en");
    let domain_provider = infiltrator_domain::runtime::RuleProvider {
        name: "GoogleRules".into(),
        provider_type: "http".into(),
        behavior: "domain".into(),
        vehicle_type: "HTTP".into(),
        updated_at: "2026-09-02 06:00:00".into(),
        rule_count: 1420,
    };
    let ipcidr_provider = infiltrator_domain::runtime::RuleProvider {
        name: "geoip-cn.mrs".into(),
        provider_type: "mrs".into(),
        behavior: "ipcidr".into(),
        vehicle_type: "File".into(),
        updated_at: "2026-09-01 12:00:00".into(),
        rule_count: 850,
    };
    let classical_provider = infiltrator_domain::runtime::RuleProvider {
        name: "custom-ads.yaml".into(),
        provider_type: "yaml".into(),
        behavior: "classical".into(),
        vehicle_type: "HTTP".into(),
        updated_at: "2026-08-30 18:30:00".into(),
        rule_count: 572,
    };

    assert_eq!(
        crate::view::rules::format_provider_behavior(&domain_provider.behavior),
        "Domain"
    );
    assert_eq!(
        crate::view::rules::format_provider_behavior(&ipcidr_provider.behavior),
        "IPCIDR"
    );
    assert_eq!(
        crate::view::rules::format_provider_behavior(&classical_provider.behavior),
        "Classical"
    );

    assert_eq!(
        crate::view::rules::format_rule_provider_format(&domain_provider),
        "HTTP"
    );
    assert_eq!(
        crate::view::rules::format_rule_provider_format(&ipcidr_provider),
        "MRS"
    );
    assert_eq!(
        crate::view::rules::format_rule_provider_format(&classical_provider),
        "YAML"
    );

    let providers = vec![
        domain_provider.clone(),
        ipcidr_provider.clone(),
        classical_provider.clone(),
    ];
    assert_eq!(
        crate::view::rules::total_external_rules(&providers),
        1420 + 850 + 572
    );

    let _dom_elem = crate::view::rules::rule_provider_row(
        &domain_provider,
        Some("https://example.com/domain.mrs"),
        Some(86_400),
        None,
        &lang,
    );
    let _ipc_elem =
        crate::view::rules::rule_provider_row(&ipcidr_provider, None, None, None, &lang);
    let _cls_elem =
        crate::view::rules::rule_provider_row(&classical_provider, None, None, None, &lang);

    state.editor.rule_providers = providers;
    let _providers_elem = crate::view::rules::providers_view(&state, &lang);
}

// ---- LEFT-05 L1 / DUAL-09-01: editor saves keep comments -------------------

/// The exact function `Message::SaveRules` runs: comment preservation is a
/// property of the shared domain writer, so the Iced editor inherits it.
#[test]
fn test_rule_save_path_preserves_comments_through_the_shared_fidelity_writer() {
    let source = "\
# 顶层手写注释
mode: rule
rules:
  # 规则块说明
  - MATCH,DIRECT   # 兜底规则
";
    let mut rules = infiltrator_domain::rules::load_rules_from_yaml(source).expect("rules");
    rules.insert(
        0,
        RuleEntry {
            rule: "DOMAIN-SUFFIX,google.com,PROXY".to_string(),
            enabled: false,
        },
    );
    let saved = infiltrator_domain::rules::apply_rules_to_yaml(source, &rules).expect("save");
    assert!(
        saved.contains("# 顶层手写注释"),
        "top comment kept: {saved}"
    );
    assert!(
        saved.contains("# 规则块说明"),
        "block comment kept: {saved}"
    );
    assert!(saved.contains("# 兜底规则"), "inline comment kept: {saved}");
    assert_eq!(
        infiltrator_domain::rules::load_rules_from_yaml(&saved).expect("reload"),
        rules,
        "the saved document still parses to the edited rule list"
    );
}

/// The exact function the Mixin pane commit runs.
#[test]
fn test_mixin_save_path_preserves_comments_through_the_shared_fidelity_writer() {
    let source = "\
# 手写头注释
mode: rule   # 行内说明

dns:
  enable: true
";
    let mixin = infiltrator_domain::mixin::MixinConfig {
        mode: Some("global".to_string()),
        ..Default::default()
    };
    let merged = infiltrator_domain::mixin::merge_profile_with_config_fidelity(source, &mixin)
        .expect("merge");
    assert!(
        merged.contains("# 手写头注释"),
        "top comment kept: {merged}"
    );
    assert!(
        merged.contains("# 行内说明"),
        "inline comment kept: {merged}"
    );
    assert!(merged.contains("mode: global"));
}

/// Mirrors the real Mixin-pane save flow: strip the outgoing mixin's appended
/// rules through the shared writer, then merge the new mixin through the
/// fidelity writer. Comments survive both steps and no rule duplicates.
#[test]
fn test_mixin_resave_cycle_keeps_comments_and_does_not_duplicate_rules() {
    let source = "\
# 头注释
mode: rule
rules:
  # 规则块
  - MATCH,DIRECT
";
    let first = infiltrator_domain::mixin::MixinConfig {
        mode: Some("global".to_string()),
        rules: Some(infiltrator_domain::mixin::RuleMixin {
            append: vec!["DOMAIN-SUFFIX,ads.example.com,REJECT".to_string()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let once = infiltrator_domain::mixin::merge_profile_with_config_fidelity(source, &first)
        .expect("first mixin");
    assert!(once.contains("# 头注释"));
    assert!(once.contains("# 规则块"));
    assert_eq!(once.matches("ads.example.com").count(), 1);

    let removals = vec!["DOMAIN-SUFFIX,ads.example.com,REJECT".to_string()];
    let base = infiltrator_domain::profile_options::strip_rule_lines(&once, &removals);
    assert!(!base.contains("ads.example.com"), "strip is exact: {base}");
    assert!(base.contains("# 头注释") && base.contains("# 规则块"));

    let second = infiltrator_domain::mixin::MixinConfig {
        mode: Some("direct".to_string()),
        ..Default::default()
    };
    let twice =
        infiltrator_domain::mixin::merge_profile_with_config_fidelity(&base, &second).expect("re");
    assert!(twice.contains("mode: direct"));
    assert!(twice.contains("# 头注释"));
    assert!(twice.contains("# 规则块"));
}

/// DUAL-11-06: the surface never invents a provider payload. The shared
/// application reads the declaration's inline payload and the accepted plan
/// lands in the local draft through the same edit seam as the other rule
/// edits; an unreadable source stays a typed failure.
#[tokio::test]
async fn test_rules_rule_provider_unpack_reads_shared_application() {
    use infiltrator_application::rule_provider_application::RuleProviderApplication;
    use infiltrator_domain::rules::RuleProviders;
    use infiltrator_domain::rules::provider_store::parse_rule_provider_declarations;

    let (mut state, _) = AppState::new();
    state.editor.rules = vec![RuleEntry {
        rule: "MATCH,DIRECT".into(),
        enabled: true,
    }];
    state.editor.rule_providers_json_cache =
        r#"{"ads":{"type":"inline","behavior":"domain","payload":["ads.com","tracker.net"]}}"#
            .to_owned();

    let providers: RuleProviders =
        serde_json::from_str(&state.editor.rule_providers_json_cache).expect("declarations");
    let declaration = parse_rule_provider_declarations(&providers)
        .into_iter()
        .next()
        .expect("declaration");
    let plan = RuleProviderApplication::default()
        .deconstruct(&declaration, "PROXY", None)
        .await
        .expect("inline plan");
    assert_eq!(
        plan.origin,
        infiltrator_contract::provider_cache::ProviderContentOrigin::InlinePayload
    );
    assert_eq!(plan.imported(), 2);

    let _ = state.update(Message::RuleProviderUnpacked(Ok(plan)));
    assert_eq!(state.editor.rules.len(), 3);
    // Same shared reduction as the application path: unpacked rules prepend.
    assert_eq!(state.editor.rules[0].rule, "DOMAIN-SUFFIX,ads.com,PROXY");
    assert_eq!(
        state.editor.rules[1].rule,
        "DOMAIN-SUFFIX,tracker.net,PROXY"
    );
    assert_eq!(state.editor.rules[2].rule, "MATCH,DIRECT");
    assert!(state.editor.rules_dirty);
    assert_eq!(state.editor.provider_unpack.unpacked_rules_count, 2);
    let status = state
        .editor
        .provider_unpack
        .status_message
        .clone()
        .expect("status");
    assert!(status.contains("inline-payload"), "{status}");
    assert!(!state.editor.provider_unpack.is_unpacking);

    // A host without a readable source reports the typed failure verbatim and
    // leaves the draft untouched.
    let failure = RuleProviderApplication::default()
        .deconstruct(
            &infiltrator_domain::rules::provider_store::RuleProviderDeclaration::from_value(
                "cn",
                &serde_json::json!({
                    "type": "http",
                    "behavior": "domain",
                    "format": "text",
                    "url": "https://example.com/cn.txt"
                }),
            ),
            "PROXY",
            None,
        )
        .await
        .expect_err("no source");
    let _ = state.update(Message::RuleProviderUnpacked(Err(
        infiltrator_contract::error::InfiltratorError::Config(failure.message.clone()),
    )));
    assert_eq!(state.editor.rules.len(), 3);
    assert!(
        state
            .editor
            .provider_unpack
            .status_message
            .as_deref()
            .is_some_and(|status| status.contains("no readable rule list"))
    );
}

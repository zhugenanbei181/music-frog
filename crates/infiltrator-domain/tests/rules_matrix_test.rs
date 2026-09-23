//! DUAL-11-15: shared rules-engine / MRS regression matrix.
//!
//! One headless test per shared fact the two surfaces depend on. The surface
//! suites (`infiltrator-bevy-ui` headless, `infiltrator-iced` GUI) ride on top
//! of these reductions, so this matrix is the common denominator both must
//! keep green. 11-06/11-07 now assert the real provider-source resolution,
//! payload deconstruction and purge arithmetic; no fabricated sample source
//! is reachable from this matrix.

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::rule_edit::{LogicalDraft, RuleDraft, RuleMoveDirection};
use infiltrator_domain::mrs::{
    Behavior, build_mrs_bytes, deconstruct_mrs_payload, parse_mrs_header,
};
use infiltrator_domain::rules::edit;
use infiltrator_domain::rules::logical;
use infiltrator_domain::rules::matrix::{RULE_TYPE_MATRIX, RuleTypeFamily, matrix_label};
use infiltrator_domain::rules::provider_store::{
    ProviderBehavior, ProviderFormat, ProviderSourceKind, RuleProviderDeclaration,
    deconstruct_provider_payload, parse_rule_provider_declarations, provider_cache_file_name,
    provider_source_candidates, unpack_provider_rules_with_behavior,
};
use infiltrator_domain::rules::types::parse_rule_str;
use infiltrator_domain::rules::view;
use infiltrator_domain::rules::{RuleEntry, RuleProviders, game_routing_presets};
use infiltrator_domain::sub_rules::validate_logical_rule_syntax;

fn entry(rule: &str) -> RuleEntry {
    RuleEntry {
        rule: rule.to_owned(),
        enabled: true,
    }
}

/// DUAL-11-01: every advertised rule-type spelling parses to its named type and
/// resolves to a catalogue entry the surfaces render.
#[test]
fn matrix_11_01_rule_type_vocabulary_parses() {
    let types = [
        "DOMAIN,a.com",
        "DOMAIN-SUFFIX,a.com",
        "DOMAIN-KEYWORD,key",
        "DOMAIN-REGEX,^a",
        "GEOSITE,cn",
        "IP-CIDR,1.1.1.1/32",
        "IP-CIDR6,2001:db8::/32",
        "IP-SUFFIX,1.1.1.1",
        "IP-ASN,13335",
        "GEOIP,CN",
        "SRC-GEOIP,US",
        "SRC-IP-CIDR,10.0.0.0/8",
        "SRC-IP-ASN,65000",
        "DST-PORT,443",
        "SRC-PORT,1234",
        "IN-PORT,7890",
        "IN-TYPE,INNER",
        "IN-NAME,eth0",
        "IN-USER,alice",
        "PROCESS-PATH,/usr/bin/curl",
        "PROCESS-PATH-REGEX,.*curl",
        "PROCESS-NAME,curl.exe",
        "PROCESS-NAME-REGEX,.*ssh",
        "NETWORK,tcp",
        "DSCP,46",
        "UID,1000",
        "PACKAGE-NAME,com.example",
        "RULE-SET,ads",
    ];
    for raw in types {
        let parsed = parse_rule_str(&format!("{raw},TARGET")).expect("parse rule type");
        let (expected, _) = raw.split_once(',').unwrap();
        assert_eq!(parsed.rule_type.name(), expected, "type {raw}");
        assert_eq!(parsed.target, "TARGET");
        // The parsed type resolves to the shared catalogue entry of the same
        // spelling, with a per-type display label and semantic family.
        let spec = parsed.rule_type.spec();
        assert_eq!(spec.name, expected);
        assert_eq!(matrix_label(expected), spec.label);
        assert_ne!(spec.family, RuleTypeFamily::Unknown);
    }
    // MATCH carries only a target.
    let matched = parse_rule_str("MATCH,DIRECT").unwrap();
    assert_eq!(matched.rule_type.name(), "MATCH");
    assert_eq!(matched.rule_type.spec().family, RuleTypeFamily::Terminal);

    // The catalogue has one entry per concrete spelling and every entry is
    // reachable from the parser; nothing renders as an unknown type.
    assert!(RULE_TYPE_MATRIX.len() >= 30);
    for spec in RULE_TYPE_MATRIX.iter() {
        let raw = if spec.is_logical {
            format!("{}((DOMAIN,a.com),T)", spec.name)
        } else {
            format!("{},payload,T", spec.name)
        };
        let parsed = parse_rule_str(&raw).unwrap_or_else(|error| panic!("{raw}: {error}"));
        assert_eq!(parsed.rule_type.spec(), spec);
    }
    assert_eq!(matrix_label("domain-keyword"), "DomainKeyword");
}

/// DUAL-11-02: the shared logical draft builds the canonical recursive
/// expression, rejects malformed compositions and routes into the same AST the
/// evaluator walks.
#[test]
fn matrix_11_02_logical_draft_builds_recursive_expression() {
    let mut draft = logical::default_logical_draft(edit::DEFAULT_RULE_TARGET);
    assert!(logical::select_operator(&mut draft, "or"));
    assert!(logical::set_target(&mut draft, "Streaming"));
    assert!(logical::add_condition(&mut draft, "DST-PORT,443"));
    assert!(!logical::add_condition(&mut draft, "  DST-PORT,443  "));
    assert_eq!(logical::draft_issue(&draft), None);

    let built = logical::build_logical_rule(&draft).unwrap();
    assert_eq!(logical::draft_expression(&draft), built.rule);
    let parsed = parse_rule_str(&built.rule).unwrap();
    assert_eq!(parsed.rule_type.name(), "OR");
    assert_eq!(parsed.target, "Streaming");

    // NOT is single-condition; an invalid draft never yields an entry.
    assert!(logical::select_operator(&mut draft, "NOT"));
    assert!(logical::build_logical_rule(&draft).is_err());
    assert!(logical::remove_condition(&mut draft, 0));
    assert!(logical::remove_condition(&mut draft, 0));
    assert_eq!(
        logical::build_logical_rule(&draft).unwrap().rule,
        "NOT((DST-PORT,443),Streaming)"
    );
    assert!(logical::remove_condition(&mut draft, 0));
    assert!(logical::build_logical_rule(&draft).is_err());
    let mut single = LogicalDraft {
        operator: "NOT".to_owned(),
        conditions: vec!["DOMAIN,a.com".to_owned()],
        target: "REJECT".to_owned(),
    };
    assert_eq!(
        logical::build_logical_rule(&single).unwrap().rule,
        "NOT((DOMAIN,a.com),REJECT)"
    );
    single.conditions = vec!["BOGUS,a.com".to_owned()];
    assert!(logical::build_logical_rule(&single).is_err());
    // The operator vocabulary the surfaces render is the shared one.
    assert_eq!(logical::LOGICAL_OPERATOR_CHOICES.len(), 4);
    assert!(
        logical::SUB_RULE_CONDITION_PRESETS
            .iter()
            .all(|preset| { logical::condition_issue(preset).is_none() })
    );
}

/// DUAL-11-02: logical sub-rules parse and evaluate recursively.
#[test]
fn matrix_11_02_logical_sub_rules_evaluate() {
    assert!(validate_logical_rule_syntax("AND((DOMAIN,a.com),(DST-PORT,443),T)").is_ok());
    assert!(validate_logical_rule_syntax("AND((DOMAIN,a.com),T").is_err());

    let and = parse_rule_str("AND((DOMAIN,a.com),(DST-PORT,443),T)").unwrap();
    assert_eq!(and.rule_type.name(), "AND");
    let or = parse_rule_str("OR((DOMAIN,a.com),(DOMAIN,b.com),T)").unwrap();
    assert_eq!(or.rule_type.name(), "OR");
    let not = parse_rule_str("NOT((DOMAIN,a.com),T)").unwrap();
    assert_eq!(not.rule_type.name(), "NOT");
    let sub = parse_rule_str("SUB-RULE((DOMAIN,a.com),(DST-PORT,443),T)").unwrap();
    assert_eq!(sub.rule_type.name(), "SUB-RULE");
}

/// DUAL-11-03: MRS header parse + payload deconstruction.
#[test]
fn matrix_11_03_mrs_binary_pipeline() {
    let payload = b"example.com\ngoogle.com\n";
    let bytes = build_mrs_bytes(Behavior::Domain, 2, 2, "Matrix", payload, None);
    let meta = parse_mrs_header(&bytes).unwrap();
    assert_eq!(meta.behavior, Behavior::Domain);
    assert_eq!(meta.rule_count, 2);
    let deconstructed = deconstruct_mrs_payload(&bytes).unwrap();
    assert!(
        deconstructed
            .iter()
            .any(|line| line.contains("example.com"))
    );
}

/// DUAL-11-04: the provider read model carries the declared source URL.
#[test]
fn matrix_11_04_rule_provider_source_url_projection() {
    let provider = infiltrator_contract::surface_snapshot::RuleProviderSnapshot {
        name: "geoip-cn".to_owned(),
        rule_count: 850,
        behavior: "ipcidr".to_owned(),
        updated_at: "2026-09-01".to_owned(),
        source_url: Some("https://example.com/cn.mrs".to_owned()),
        refresh_interval_secs: Some(86_400),
        cache_fingerprint: None,
    };
    assert_eq!(
        provider.source_url.as_deref(),
        Some("https://example.com/cn.mrs")
    );
}

/// DUAL-11-05: the refresh intent is real and the declared automatic-refresh
/// interval is what the client can honestly publish. The kernel's real
/// top-level `etag-support` declaration is now surfaced too; the per-request
/// `ETag` / `304` outcome still lives inside the mihomo kernel and must not be
/// invented by a surface.
#[test]
fn matrix_11_05_provider_refresh_intent_and_declared_interval() {
    let intent = CommandIntent::RefreshRuleProviders;
    assert_eq!(
        intent.kind(),
        infiltrator_contract::command::CommandKind::Profile
    );
    let declared = infiltrator_contract::surface_snapshot::RuleProviderSnapshot {
        name: "ads".to_owned(),
        rule_count: 12,
        behavior: "domain".to_owned(),
        updated_at: "2026-09-01".to_owned(),
        source_url: None,
        refresh_interval_secs: Some(3600),
        cache_fingerprint: None,
    };
    assert_eq!(declared.refresh_interval_secs, Some(3600));

    // The other honest half: the client's own observation of the local cache
    // file. It is a local content fingerprint compared with the previous local
    // read — never the kernel's `ETag`/304 result, which it cannot see.
    use infiltrator_contract::provider_cache::{
        ProviderCacheFingerprint, ProviderFileFingerprint, ProviderFingerprintChange,
    };
    let previous = ProviderFileFingerprint {
        size_bytes: 2_048,
        sha256: "0123456789abcdef".to_owned(),
        modified_unix_secs: Some(1_699_000_000),
    };
    let current = ProviderFileFingerprint {
        size_bytes: 4_096,
        sha256: "abcdef0123456789".to_owned(),
        modified_unix_secs: Some(1_700_000_000),
    };
    assert_eq!(
        ProviderCacheFingerprint::compare(Some(&previous), &current),
        ProviderFingerprintChange::Changed
    );
    let observed = infiltrator_contract::surface_snapshot::RuleProviderSnapshot {
        name: "ads".to_owned(),
        rule_count: 12,
        behavior: "domain".to_owned(),
        updated_at: "2026-09-01".to_owned(),
        source_url: Some("https://example.com/ads.mrs".to_owned()),
        refresh_interval_secs: Some(3600),
        cache_fingerprint: Some(ProviderCacheFingerprint {
            provider: "ads".to_owned(),
            path: "/home/u/.config/mihomo-rs/rules/8f14e45fceea167a5a36dedd4bea2543".to_owned(),
            change: ProviderFingerprintChange::Changed,
            current,
            previous: Some(previous),
        }),
    };
    let fingerprint = observed.cache_fingerprint.expect("observed fingerprint");
    assert_eq!(fingerprint.change_token(), "changed");
    assert_eq!(fingerprint.current.size_bytes, 4_096);
    // The observation never carries an HTTP validator verdict.
    assert!(!fingerprint.change_token().contains("304"));

    // DUAL-11-05: the kernel's real `etag-support` capability is a declaration
    // fact read from the active profile (top-level key, mihomo default true).
    // Only the three honest states are published; the per-request 304 outcome
    // is never inferred.
    use infiltrator_contract::provider_cache::{KernelEtagSupportSnapshot, KernelEtagSupportState};
    let enabled = KernelEtagSupportSnapshot::from_declared(Some(true));
    assert_eq!(enabled.state, KernelEtagSupportState::Enabled);
    assert_eq!(enabled.declared, Some(true));
    let disabled = KernelEtagSupportSnapshot::from_declared(Some(false));
    assert_eq!(disabled.state, KernelEtagSupportState::Disabled);
    let absent = KernelEtagSupportSnapshot::from_declared(None);
    assert_eq!(absent.state, KernelEtagSupportState::NotDeclared);
    assert_eq!(absent.declared, None);
    assert!(!absent.state.as_str().contains("304"));
}

/// DUAL-11-08/13: keyword search + pagination are shared arithmetic, and the
/// publish cap + omitted count are shared facts the surfaces render.
#[test]
fn matrix_11_08_search_and_pagination_reduce_in_shared_view() {
    let rules = vec![
        entry("DOMAIN,a.com,DIRECT"),
        entry("DOMAIN-SUFFIX,b.com,PROXY"),
        entry("GEOIP,CN,DIRECT"),
    ];
    assert_eq!(view::filter_rule_indices(&rules, "proxy"), vec![1]);
    assert_eq!(view::page_count(0, 0), 1);
    assert_eq!(view::page_bounds(9, 5, 2), (4, 5));

    // DUAL-11-08: the published view is capped and says so.
    assert_eq!(view::published_rule_count(120), 120);
    assert_eq!(view::published_rule_count(50_000), view::RULE_PUBLISH_LIMIT);
    assert_eq!(view::omitted_rule_count(50_000), 45_000);
    assert!(view::is_truncated_rule_list(50_000));
    let snapshot = infiltrator_contract::surface_snapshot::RulesPageSnapshot {
        total_rules: 50_000,
        default_action: "DIRECT".to_owned(),
        providers: Vec::new(),
        rules: Vec::new(),
        tracer: Default::default(),
        mrs_acceleration: Default::default(),
        total_hits: 0,
        rule_publish_limit: view::RULE_PUBLISH_LIMIT,
        provider_cache: Default::default(),
        etag_support: Default::default(),
        json_documents: Vec::new(),
    };
    assert_eq!(snapshot.omitted_rule_count(), 50_000);
    assert!(snapshot.is_truncated());
    let complete = infiltrator_contract::surface_snapshot::RulesPageSnapshot {
        total_rules: 2,
        rules: vec![
            infiltrator_contract::surface_snapshot::RuleSnapshot::default(),
            infiltrator_contract::surface_snapshot::RuleSnapshot::default(),
        ],
        ..snapshot
    };
    assert!(!complete.is_truncated());
    assert_eq!(complete.omitted_rule_count(), 0);

    // DUAL-11-08: the rendered view is a real virtual window — bounded by the
    // viewport, sliding with the scroll offset, and never one row per entry.
    let viewport = view::RULE_DEFAULT_VIEWPORT_PX;
    let bound = view::rendered_row_bound(viewport);
    for offset in [0.0, 5_000.0, 1.0e9, f32::NAN] {
        let window = view::rule_window(offset, viewport, 50_000);
        assert!(window.rendered_rows() <= bound, "offset {offset}");
        assert_eq!(
            window.top_spacer_px
                + window.rendered_rows() as f32 * view::RULE_ROW_HEIGHT_PX
                + window.bottom_spacer_px,
            window.content_height_px
        );
    }
    let middle = view::rule_window(view::rule_scroll_offset_for_index(25_000), viewport, 50_000);
    assert_eq!(middle.start + view::RULE_WINDOW_OVERSCAN, 25_000);
    assert!(middle.contains(25_000));
    // The offset->index projection is unclamped by design; the window clamps
    // it to the list, so scrolling past the end still renders the last rows.
    assert_eq!(
        view::rule_index_at_scroll_offset(view::rule_scroll_offset_for_index(25_000)),
        25_000
    );
    let past_end = view::rule_window(1.0e9, viewport, 50_000);
    assert!(past_end.contains(49_999));
    assert_eq!(past_end.bottom_spacer_px, 0.0);
}

/// DUAL-11-09/10/11/12: toggle, reorder, wizard and presets are shared edits.
#[test]
fn matrix_11_09_to_12_rule_edit_reductions() {
    let mut rules = vec![entry("A,1,DIRECT"), entry("B,2,DIRECT")];
    assert!(edit::toggle_rule_enabled(&mut rules, 0));
    assert!(!rules[0].enabled);
    assert!(edit::move_rule(&mut rules, 0, RuleMoveDirection::Down));
    assert_eq!(rules[0].rule, "B,2,DIRECT");

    let built = edit::build_custom_rule(&RuleDraft {
        rule_type: "AND".to_owned(),
        payload: "(DOMAIN,a.com),(DST-PORT,443)".to_owned(),
        target: "AI".to_owned(),
    })
    .unwrap();
    assert_eq!(built.rule, "AND((DOMAIN,a.com),(DST-PORT,443),AI)");

    let inserted = edit::inject_game_presets(&mut rules, "Game");
    assert_eq!(inserted, game_routing_presets("Game").len());
    assert!(rules[0].rule.contains("Game"));
}

/// DUAL-11-06: a provider's real rules are resolved from its declaration —
/// inline payload, kernel cache path (`rules/<md5(url)>`) or declared file —
/// and mapped to routing rules for the declaration's behavior. No sample rule
/// is ever produced.
#[test]
fn matrix_11_06_provider_declaration_and_payload_deconstruct() {
    let mut providers = RuleProviders::new();
    providers.insert(
        "cn".to_owned(),
        serde_json::json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/cn.yaml"
        }),
    );
    providers.insert(
        "ads".to_owned(),
        serde_json::json!({
            "type": "file",
            "behavior": "classical",
            "format": "yaml",
            "path": "assets/ads.yaml"
        }),
    );
    let declarations = parse_rule_provider_declarations(&providers);
    assert_eq!(declarations.len(), 2);
    let cn = declarations
        .iter()
        .find(|declaration| declaration.name == "cn")
        .expect("cn declaration");
    assert_eq!(cn.behavior, ProviderBehavior::Domain);
    assert_eq!(cn.format, ProviderFormat::Text);
    assert!(cn.is_remote());

    // mihomo's `GetPathByHash("rules", url)` naming is part of the shared fact.
    let home = std::path::Path::new("/kernel");
    let candidates = provider_source_candidates(cn, home);
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].kind, ProviderSourceKind::KernelCacheFile);
    assert_eq!(
        candidates[0].path,
        home.join("rules").join(provider_cache_file_name(
            "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/cn.yaml"
        ))
    );
    assert_eq!(
        provider_cache_file_name(
            "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/cn.yaml"
        ),
        "07225f3cebe3fe2955706742c6cbe5b0"
    );

    // The declared file wins, then the kernel cache; an inline provider has no
    // file at all.
    let ads = declarations
        .iter()
        .find(|declaration| declaration.name == "ads")
        .expect("ads declaration");
    let ads_candidates = provider_source_candidates(ads, home);
    assert_eq!(ads_candidates.len(), 1);
    assert_eq!(ads_candidates[0].kind, ProviderSourceKind::DeclaredFile);
    assert_eq!(ads_candidates[0].path, home.join("assets/ads.yaml"));

    // Real payload -> real rules, with the rejected line counted honestly.
    let payload = b"# comment\nexample.cn\nhupu.com\n";
    let deconstructed = deconstruct_provider_payload(payload, cn, "PROXY").expect("deconstruct");
    assert_eq!(deconstructed.considered, 2);
    assert_eq!(deconstructed.skipped, 0);
    assert_eq!(
        deconstructed.entries[0].rule,
        "DOMAIN-SUFFIX,example.cn,PROXY"
    );
    assert_eq!(
        deconstructed.entries[1].rule,
        "DOMAIN-SUFFIX,hupu.com,PROXY"
    );

    // ipcidr payloads become IP-CIDR/IP-CIDR6, never DOMAIN-SUFFIX guesses.
    let (entries, _) = unpack_provider_rules_with_behavior(
        &["10.0.0.0/8".to_owned(), "2001:db8::/32".to_owned()],
        ProviderBehavior::IpCidr,
        "DIRECT",
    );
    assert_eq!(entries[0].rule, "IP-CIDR,10.0.0.0/8,DIRECT");
    assert_eq!(entries[1].rule, "IP-CIDR6,2001:db8::/32,DIRECT");

    // A missing/empty source is an error, never an invented rule list.
    assert!(deconstruct_provider_payload(b"", cn, "PROXY").is_err());
    assert!(deconstruct_provider_payload(b"# only\n", cn, "PROXY").is_err());

    // The MRS binary path stays wired to the shared deconstructor.
    let bytes = build_mrs_bytes(
        Behavior::IpCidr,
        1,
        1,
        "cn",
        b"10.0.0.0/8\n",
        Some(infiltrator_domain::mrs::MAGIC_STANDARD_MRS),
    );
    let mut mrs = RuleProviderDeclaration::from_value(
        "cn-mrs",
        &serde_json::json!({ "type": "file", "behavior": "ipcidr", "format": "mrs", "path": "cn.mrs" }),
    );
    mrs.name = "cn-mrs".to_owned();
    let deconstructed = deconstruct_provider_payload(&bytes, &mrs, "DIRECT").expect("mrs");
    assert_eq!(deconstructed.entries[0].rule, "IP-CIDR,10.0.0.0/8,DIRECT");
}

/// DUAL-11-07: the purge fact is expressed over the kernel's own cache
/// directory name, and the application/port contract carries observed counts.
#[test]
fn matrix_11_07_provider_cache_purge_fact() {
    use infiltrator_contract::command::CommandIntent;
    use infiltrator_contract::provider_cache::{
        ProviderCachePurge, ProviderContentOrigin, RuleProviderCacheSnapshot,
        RuleProviderCacheState,
    };

    assert_eq!(
        infiltrator_domain::rules::provider_store::PROVIDER_CACHE_DIR_NAME,
        "rules"
    );

    let empty = RuleProviderCacheSnapshot::ready("/kernel/rules", 0, 0);
    assert_eq!(empty.state, RuleProviderCacheState::Empty);
    let ready = RuleProviderCacheSnapshot::ready("/kernel/rules", 2, 4096);
    assert_eq!(ready.state, RuleProviderCacheState::Ready);
    assert_eq!(ready.file_count, 2);
    let unsupported = RuleProviderCacheSnapshot::unsupported("no kernel home");
    assert!(!unsupported.is_available());

    let purge = ProviderCachePurge {
        directory: Some("/kernel/rules".to_owned()),
        files_removed: 2,
        bytes_freed: 4096,
    };
    assert!(!purge.is_noop());
    assert!(ProviderCachePurge::default().is_noop());
    assert_eq!(
        ProviderContentOrigin::KernelCacheFile.as_str(),
        "kernel-cache-file"
    );

    // Both surfaces submit the same intent; the kind classifies as a profile
    // command because the purge belongs to the profile's provider cache.
    let intent = CommandIntent::PurgeRuleProviderCache;
    assert_eq!(
        intent.kind(),
        infiltrator_contract::command::CommandKind::Profile
    );
}

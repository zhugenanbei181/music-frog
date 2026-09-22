use super::*;

#[test]
fn test_format_behavior_name() {
    assert_eq!(format_behavior_name("domain"), "Domain");
    assert_eq!(format_behavior_name("ipcidr"), "IP-CIDR");
    assert_eq!(format_behavior_name("IP-CIDR"), "IP-CIDR");
    assert_eq!(format_behavior_name("classical"), "Classical");
    assert_eq!(format_behavior_name("other"), "Domain");
}

#[test]
fn test_format_vehicle_behavior() {
    assert_eq!(
        format_vehicle_behavior(Some("HTTP"), "domain"),
        "HTTP::Domain"
    );
    assert_eq!(
        format_vehicle_behavior(Some("File"), "ipcidr"),
        "File::IP-CIDR"
    );
    assert_eq!(
        format_vehicle_behavior(None, "classical"),
        "HTTP::Classical"
    );
}

#[test]
fn test_format_rule_count() {
    assert_eq!(format_rule_count(179), "179 rules");
    assert_eq!(format_rule_count(52345), "52345 rules");
}

#[test]
fn test_mrs_card_empty_and_populated() {
    let (mut state, _) = AppState::new();
    assert!(mrs_card(&state).is_none());

    state.editor.mrs_details.push(MrsProviderDetail {
        name: "XiaoHongShu".into(),
        behavior: "domain".into(),
        file: None,
        metadata: Some(infiltrator_domain::mrs::MrsMetadata {
            behavior: infiltrator_domain::mrs::Behavior::Domain,
            rule_count: 179,
            version: 1,
            payload_size: 4096,
            description: "XiaoHongShu ruleset".into(),
        }),
        errors: Vec::new(),
    });
    state.editor.mrs_details.push(MrsProviderDetail {
        name: "blizzard".into(),
        behavior: "ipcidr".into(),
        file: None,
        metadata: None,
        errors: vec!["Cache missing".into()],
    });
    state.editor.mrs_details.push(MrsProviderDetail {
        name: "category-ai-chat".into(),
        behavior: "classical".into(),
        file: None,
        metadata: None,
        errors: Vec::new(),
    });

    assert!(mrs_card(&state).is_some());
}

#[test]
fn test_mrs_acceleration_card_renders_shared_status_and_items() {
    let (mut state, _) = AppState::new();
    let lang = Lang("en");

    // The shared card always renders (unlike the cache-scan card).
    drop(mrs_acceleration_card(&state));
    assert!(
        mrs_acceleration_status_line(&lang, &state.editor.mrs_acceleration).contains("Unavailable")
    );

    let item = infiltrator_contract::mrs_acceleration::MrsItemSnapshot {
        name: "geoip-cn.mrs".into(),
        behavior: infiltrator_contract::mrs_acceleration::MrsBehaviorKind::IpCidr,
        format_version: 1,
        compression: infiltrator_contract::mrs_acceleration::MrsCompressionKind::None,
        rule_count: 8500,
        payload_size_bytes: 128,
        file_size_bytes: 192,
        sha256_digest: Some("deadbeefcafebabe0123456789abcdef".into()),
        crc32_checksum: Some(1),
        is_mmap_accelerated: true,
        is_valid: true,
        description: String::new(),
        updated_at: String::new(),
        source_url: None,
        unpack_supported: true,
    };
    state.editor.mrs_acceleration =
        infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot::ready(
            1,
            1,
            vec![item.clone()],
            true,
        );

    let status = mrs_acceleration_status_line(&lang, &state.editor.mrs_acceleration);
    assert!(status.contains("Acceleration ready"));
    assert!(status.contains("1 rule sets"));
    assert!(status.contains("mmap enabled"));

    let label = mrs_acceleration_item_label(&lang, &item);
    assert!(label.contains("geoip-cn.mrs"));
    assert!(label.contains("8500 ipcidr"));
    assert!(label.contains("valid"));
    assert!(label.contains("sha256 deadbeefcafe"));
    drop(mrs_acceleration_card(&state));
}

#[test]
fn test_detail_row_render() {
    let lang = Lang("en");
    let detail = MrsProviderDetail {
        name: "test-mrs".into(),
        behavior: "domain".into(),
        file: None,
        metadata: Some(infiltrator_domain::mrs::MrsMetadata {
            behavior: infiltrator_domain::mrs::Behavior::Domain,
            rule_count: 350,
            version: 1,
            payload_size: 2048,
            description: "Test ruleset".into(),
        }),
        errors: Vec::new(),
    };
    let _element = detail_row(&lang, &detail, None);
}

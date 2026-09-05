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

use super::*;

#[test]
fn observation_round_trips_as_typed_snake_case_contract_data() {
    let observation = ProfileMixinStudioObservation {
        preflight: ProfileMixinPreflight::Accepted,
        cascade: ProfileMixinCascade::Merged {
            stages: vec![ProfileMixinCascadeStage {
                id: ProfileMixinCascadeStageId::PostMixin,
                declaration: ProfileMixinStageDeclaration::Applied,
                line_count: 2,
            }],
            line_count: 2,
        },
        columns: ProfileMixinColumns {
            base: ProfileMixinColumn {
                role: ProfileMixinColumnRole::Base,
                content: "mode: rule\n".into(),
                editable: false,
                line_count: 1,
            },
            overlay: ProfileMixinColumn {
                role: ProfileMixinColumnRole::Overlay,
                content: "ipv6: true\n".into(),
                editable: true,
                line_count: 1,
            },
            composed: ProfileMixinComposition::Produced(ProfileMixinColumn {
                role: ProfileMixinColumnRole::Composed,
                content: "mode: rule\nipv6: true\n".into(),
                editable: false,
                line_count: 2,
            }),
        },
        toggles: ProfileMixinToggleObservation::Available(vec![ProfileMixinToggle {
            id: ProfileMixinToggleId::DnsFakeIp,
            enabled: true,
        }]),
    };

    let encoded = serde_json::to_value(&observation).expect("serialize observation");
    assert_eq!(encoded["preflight"], "accepted");
    assert_eq!(
        encoded["cascade"]["merged"]["stages"][0]["id"],
        "post_mixin"
    );
    assert_eq!(encoded["toggles"]["available"][0]["id"], "dns_fake_ip");
    let decoded: ProfileMixinStudioObservation =
        serde_json::from_value(encoded).expect("decode observation");
    assert_eq!(decoded, observation);
}

#[test]
fn unknown_mixin_contract_variants_fail_closed() {
    assert!(serde_json::from_str::<ProfileMixinToggleId>("\"future_toggle\"").is_err());
    assert!(serde_json::from_str::<ProfileMixinPreflight>("{\"future_state\":null}").is_err());
}

#[test]
fn rejected_codec_observation_preserves_typed_reasons_across_the_wire() {
    let reason = "invalid overlay {reason}/中文🙂";
    let observation = ProfileMixinStudioObservation {
        preflight: ProfileMixinPreflight::Blocked {
            reason: reason.into(),
        },
        cascade: ProfileMixinCascade::Blocked {
            stages: vec![ProfileMixinCascadeStage {
                id: ProfileMixinCascadeStageId::PreMixin,
                declaration: ProfileMixinStageDeclaration::Undeclared,
                line_count: 0,
            }],
            reason: reason.into(),
        },
        columns: ProfileMixinColumns {
            base: ProfileMixinColumn {
                role: ProfileMixinColumnRole::Base,
                content: "mode: rule\n".into(),
                editable: false,
                line_count: 1,
            },
            overlay: ProfileMixinColumn {
                role: ProfileMixinColumnRole::Overlay,
                content: "mode: [broken\n".into(),
                editable: true,
                line_count: 1,
            },
            composed: ProfileMixinComposition::Blocked {
                reason: reason.into(),
            },
        },
        toggles: ProfileMixinToggleObservation::Unavailable {
            reason: reason.into(),
        },
    };

    let encoded = serde_json::to_value(&observation).expect("serialize rejected observation");
    assert_eq!(encoded["preflight"]["blocked"]["reason"], reason);
    assert_eq!(encoded["cascade"]["blocked"]["reason"], reason);
    assert_eq!(encoded["columns"]["composed"]["blocked"]["reason"], reason);
    assert_eq!(encoded["toggles"]["unavailable"]["reason"], reason);
    let decoded: ProfileMixinStudioObservation =
        serde_json::from_value(encoded).expect("decode rejected observation");
    assert_eq!(decoded, observation);
}

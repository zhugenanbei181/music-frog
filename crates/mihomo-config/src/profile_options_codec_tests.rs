//! Behavior checks for scoped Mihomo sidecar operations.
//! test-intent: behavior

use super::*;
use crate::profile_document_codec::MihomoProfileDocumentCodec;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::profile_document_identity::{
    ProfileDocumentRevision, ProfileDocumentSourceIdentity, ProfileDocumentSourceKind,
    ProfileFormatId, ProfileId, ProfileSchemaVersion, ProfileSourceId,
};
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_ports::error::PortError;
use infiltrator_ports::profile_document_codec::ProfileDocumentCodecPort;
use infiltrator_ports::profile_document_codec::{
    ProfileMixinCascade, ProfileMixinCascadeStageId, ProfileMixinComposition,
    ProfileMixinPreflight, ProfileMixinStageDeclaration, ProfileMixinToggleId,
    ProfileMixinToggleProjection,
};

fn identity(engine: &str, format: &str, schema: u32) -> ProfileDocumentIdentity {
    ProfileDocumentIdentity::new(
        ProfileId::new("main").unwrap(),
        EngineId::new(engine).unwrap(),
        ProfileDocumentSourceIdentity::new(
            ProfileDocumentSourceKind::Local,
            ProfileSourceId::new("options-test").unwrap(),
        ),
        ProfileDocumentRevision::new("document-revision").unwrap(),
        ProfileFormatId::new(format).unwrap(),
        ProfileSchemaVersion::new(schema).unwrap(),
    )
}

#[test]
fn scope_and_absent_empty_sidecar_states_are_explicit() {
    let codec = MihomoProfileDocumentCodec;
    let scope = codec.options_scope().unwrap();
    assert_eq!(scope.document.engine_id.as_str(), "mihomo");
    assert_eq!(scope.document.format_id.as_str(), "mihomo-yaml");
    assert_eq!(scope.document.schema_version.value(), 1);
    assert_eq!(scope.sidecar_format_id.as_str(), SIDECAR_FORMAT);
    assert_eq!(scope.sidecar_schema_version.value(), SIDECAR_SCHEMA_VERSION);

    let document_identity = identity("mihomo", "mihomo-yaml", 1);
    let absent = codec.inspect_options(&document_identity, None).unwrap();
    let empty = codec
        .inspect_options(&document_identity, Some(b""))
        .unwrap();
    assert_eq!(absent, empty);
    assert_eq!(absent.filter, SubscriptionFilterDraft::default());
    assert_eq!(absent.mixin_yaml, "{}\n");
}

#[test]
fn no_op_sidecar_encoding_preserves_absence_and_present_empty_bytes() {
    assert_eq!(encode_if_changed(None, &Default::default()).unwrap(), None);
    assert_eq!(
        encode_if_changed(Some(b""), &Default::default()).unwrap(),
        Some(Vec::new())
    );
}

#[test]
fn mixin_update_strips_old_injected_rules_preserves_filter_and_does_not_mutate_inputs() {
    let codec = MihomoProfileDocumentCodec;
    let identity = identity("mihomo", "mihomo-yaml", 1);
    let document = b"rules:\n  - DOMAIN-SUFFIX,old.example,DIRECT\n  - MATCH,DIRECT\n";
    let sidecar = b"# keep exact source on failure\r\nfilter:\r\n  include-keywords: [HK]\r\nmixin:\r\n  rules:\r\n    prepend:\r\n      - DOMAIN-SUFFIX,old.example,DIRECT\r\n";
    let document_before = document.to_vec();
    let sidecar_before = sidecar.to_vec();

    let changed = codec
        .apply_mixin(
            &identity,
            document,
            Some(sidecar),
            "rules:\n  prepend:\n    - DOMAIN-SUFFIX,new.example,DIRECT\n",
        )
        .unwrap();

    let merged = std::str::from_utf8(&changed.document_bytes).unwrap();
    assert!(!merged.contains("old.example"));
    assert!(merged.contains("new.example"));
    assert!(merged.contains("MATCH,DIRECT"));
    assert_eq!(changed.projection.filter.include, "HK");
    assert_eq!(document, document_before.as_slice());
    assert_eq!(sidecar, sidecar_before.as_slice());

    assert!(matches!(
        codec.apply_mixin(&identity, document, Some(sidecar), "mode: [broken"),
        Err(PortError::Rejected(failure)) if failure.code == ErrorCode::InvalidInput
    ));
    assert_eq!(document, document_before.as_slice());
    assert_eq!(sidecar, sidecar_before.as_slice());
}

#[test]
fn typed_mixin_toggle_and_studio_projection_are_pure_and_schema_scoped() {
    let codec = MihomoProfileDocumentCodec;
    let document_identity = identity("mihomo", "mihomo-yaml", 1);
    let base = b"mode: rule\nport: 7890\n";
    let mut overlay = "mode: global\n".to_string();
    let overlay_before = overlay.clone();

    assert!(
        !codec
            .inspect_mixin_toggle(&document_identity, &overlay, ProfileMixinToggleId::Ipv6)
            .unwrap()
    );
    for toggle_id in ProfileMixinToggleId::ALL {
        overlay = codec
            .set_mixin_toggle(&document_identity, &overlay, toggle_id, true)
            .unwrap();
        assert!(
            codec
                .inspect_mixin_toggle(&document_identity, &overlay, toggle_id)
                .unwrap()
        );
    }
    assert!(overlay.contains("ipv6: true"));
    assert!(overlay.contains("enhanced-mode: fake-ip"));
    assert!(overlay.contains("stack: system"));
    assert!(overlay.contains("log-level: debug"));
    assert_eq!(overlay_before, "mode: global\n");

    let studio = codec
        .inspect_mixin_studio(&document_identity, base, &overlay)
        .unwrap();
    assert_eq!(studio.preflight, ProfileMixinPreflight::Accepted);
    let ProfileMixinCascade::Merged { stages, line_count } = &studio.cascade else {
        panic!("valid Mixin should produce a typed cascade")
    };
    assert_eq!(
        stages.iter().map(|stage| stage.id).collect::<Vec<_>>(),
        vec![
            ProfileMixinCascadeStageId::Base,
            ProfileMixinCascadeStageId::Subscription,
            ProfileMixinCascadeStageId::Merge,
            ProfileMixinCascadeStageId::PreMixin,
            ProfileMixinCascadeStageId::PostMixin,
        ]
    );
    assert_eq!(stages[0].declaration, ProfileMixinStageDeclaration::Applied);
    assert_eq!(
        stages[1].declaration,
        ProfileMixinStageDeclaration::Undeclared
    );
    assert_eq!(stages[3].declaration, ProfileMixinStageDeclaration::Applied);
    assert_eq!(*line_count, stages[4].line_count);
    assert_eq!(studio.columns.base.content, "mode: rule\nport: 7890\n");
    assert!(!studio.columns.base.editable);
    assert_eq!(studio.columns.overlay.content, overlay);
    assert!(studio.columns.overlay.editable);
    let ProfileMixinComposition::Produced(composed) = &studio.columns.composed else {
        panic!("valid Mixin should produce a typed composed column")
    };
    assert!(composed.content.contains("port: 7890"));
    assert!(composed.content.contains("mode: global"));
    assert!(composed.content.contains("ipv6: true"));
    assert!(!composed.editable);
    let ProfileMixinToggleProjection::Available(toggles) = studio.toggles else {
        panic!("valid Mixin YAML should project its typed toggle values")
    };
    assert_eq!(
        toggles.iter().map(|toggle| toggle.id).collect::<Vec<_>>(),
        ProfileMixinToggleId::ALL.to_vec()
    );
    assert!(toggles.iter().all(|toggle| toggle.enabled));
    assert_eq!(base, b"mode: rule\nport: 7890\n");

    let invalid = codec
        .inspect_mixin_studio(&document_identity, base, "mode: [unterminated\n")
        .unwrap();
    assert!(matches!(
        invalid.preflight,
        ProfileMixinPreflight::Blocked { .. }
    ));
    assert!(matches!(
        invalid.cascade,
        ProfileMixinCascade::Blocked { .. }
    ));
    assert!(matches!(
        invalid.columns.composed,
        ProfileMixinComposition::Blocked { .. }
    ));
    assert!(matches!(
        invalid.toggles,
        ProfileMixinToggleProjection::Unavailable { .. }
    ));

    for foreign in [
        identity("sing-box", "mihomo-yaml", 1),
        identity("mihomo", "sing-box-json", 1),
        identity("mihomo", "mihomo-yaml", 2),
    ] {
        assert!(
            codec
                .inspect_mixin_toggle(&foreign, "not yaml", ProfileMixinToggleId::Ipv6)
                .is_err()
        );
        assert!(
            codec
                .set_mixin_toggle(&foreign, "not yaml", ProfileMixinToggleId::Ipv6, true)
                .is_err()
        );
        assert!(
            codec
                .inspect_mixin_studio(&foreign, b"not yaml", "not yaml")
                .is_err()
        );
    }
    assert_eq!(base, b"mode: rule\nport: 7890\n");
    assert!(overlay.contains("ipv6: true"));
}

#[test]
fn filter_update_returns_typed_report_and_rejects_invalid_draft_without_mutation() {
    let codec = MihomoProfileDocumentCodec;
    let identity = identity("mihomo", "mihomo-yaml", 1);
    let document = b"proxies:\n  - {name: keep, type: ss, server: 1.1.1.1, port: 443}\n  - {name: ad, type: ss, server: 8.8.8.8, port: 443}\n";
    let sidecar = b"# preserved if no semantic change\r\nmixin:\r\n  mode: rule\r\n";
    let document_before = document.to_vec();
    let sidecar_before = sidecar.to_vec();
    let draft = SubscriptionFilterDraft {
        exclude: "ad".into(),
        ..Default::default()
    };
    let applied = codec
        .apply_filter(&identity, document, Some(sidecar), &draft)
        .unwrap();
    assert_eq!((applied.report.total_input, applied.report.passed), (2, 1));
    let filtered = std::str::from_utf8(&applied.document_bytes).unwrap();
    assert!(filtered.contains("name: keep"));
    assert!(!filtered.contains("name: ad"));
    assert!(applied.projection.mixin_yaml.contains("mode: rule"));
    assert_eq!(applied.projection.filter.exclude, "ad");
    assert_eq!(document, document_before.as_slice());
    assert_eq!(sidecar, sidecar_before.as_slice());

    let invalid = SubscriptionFilterDraft {
        dedup_index: 9,
        ..Default::default()
    };
    assert!(
        codec
            .apply_filter(&identity, document, Some(sidecar), &invalid)
            .is_err()
    );
    assert_eq!(document, b"proxies:\n  - {name: keep, type: ss, server: 1.1.1.1, port: 443}\n  - {name: ad, type: ss, server: 8.8.8.8, port: 443}\n");
    assert_eq!(
        sidecar,
        b"# preserved if no semantic change\r\nmixin:\r\n  mode: rule\r\n"
    );
}

#[test]
fn foreign_engine_format_and_schema_fail_closed_before_decoding_sidecar() {
    let codec = MihomoProfileDocumentCodec;
    let bytes = b"not yaml and must not be read";
    for foreign in [
        identity("sing-box", "mihomo-yaml", 1),
        identity("mihomo", "sing-box-json", 1),
        identity("mihomo", "mihomo-yaml", 2),
    ] {
        assert!(matches!(
            codec.inspect_options(&foreign, Some(bytes)),
            Err(PortError::Unsupported { .. })
        ));
        assert!(matches!(
            codec.apply_mixin(&foreign, b"mode: rule\n", Some(bytes), "{}\n"),
            Err(PortError::Unsupported { .. })
        ));
        assert!(matches!(
            codec.apply_filter(
                &foreign,
                b"mode: rule\n",
                Some(bytes),
                &SubscriptionFilterDraft::default(),
            ),
            Err(PortError::Unsupported { .. })
        ));
        assert_eq!(bytes, b"not yaml and must not be read");
    }
}

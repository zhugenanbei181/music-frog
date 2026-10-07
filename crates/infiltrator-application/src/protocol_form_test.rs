//! test-intent: behavior
use super::*;

#[test]
fn invalid_port_and_boolean_keep_the_previous_draft_and_raw_input() {
    let mut draft = ProtocolDraft::new("vless");
    draft.port = 443;
    let mut inputs = ProtocolInputs::default();
    assert_eq!(
        inputs
            .edit(&draft, ProtocolField::Port, "70000".into())
            .unwrap_err()
            .code,
        ErrorCode::InvalidInput
    );
    assert_eq!(draft.port, 443);
    assert_eq!(inputs.values[&ProtocolField::Port], "70000");
    assert!(inputs.errors.contains_key(&ProtocolField::Port));
    assert!(edit_field(&draft, ProtocolField::Tls, "maybe").is_err());
    let next = inputs
        .edit(&draft, ProtocolField::Port, "8443".into())
        .unwrap();
    assert_eq!(next.port, 8443);
    assert!(inputs.errors.is_empty());
}

#[test]
fn family_fields_cover_every_native_parameter_editor() {
    let tuic = ProtocolDraft::new("tuic");
    let tuic_fields = project_fields(&tuic);
    assert!(
        tuic_fields
            .iter()
            .any(|f| f.id == ProtocolField::TuicTimeout)
    );
    assert!(tuic_fields.iter().any(|f| f.id == ProtocolField::Password));
    assert!(!tuic_fields.iter().any(|f| f.id == ProtocolField::WgPrivate));
    let wg = ProtocolDraft::new("wireguard");
    let wg_fields = project_fields(&wg);
    assert!(wg_fields.iter().any(|f| f.id == ProtocolField::AwgJmax));
    assert!(!wg_fields.iter().any(|f| f.id == ProtocolField::TuicTimeout));
    let optional = edit_field(&wg, ProtocolField::AwgJmax, "70").unwrap();
    assert_eq!(optional.params.wireguard.amnezia.jmax, Some(70));
    assert!(edit_field(&wg, ProtocolField::AwgJmax, "65536").is_err());
    let next = edit_field(&optional, ProtocolField::AwgJmax, "").unwrap();
    assert_eq!(next.params.wireguard.amnezia.jmax, None);
}

#[test]
fn typed_edits_preserve_credentials_transport_and_unknown_values() {
    let draft = ProtocolDraft::new("tuic");
    let next = edit_field(&draft, ProtocolField::Secret, "uuid-only").unwrap();
    assert_eq!(next.uuid, "uuid-only");
    assert!(next.password.is_empty());
    let ss = ProtocolDraft::new("ss");
    let next = edit_field(&ss, ProtocolField::Secret, "password-only").unwrap();
    assert_eq!(next.password, "password-only");
    assert!(next.uuid.is_empty());
    let next = edit_field(&next, ProtocolField::PluginHost, "bing.com").unwrap();
    assert_eq!(next.params.plugin.opt("host").as_deref(), Some("bing.com"));
    let mut draft = ProtocolDraft::new("vless");
    draft.preserved_fields.push("future-key".into());
    let next = edit_field(&draft, ProtocolField::WsHost, "ws.example.com").unwrap();
    assert_eq!(next.params.transport.ws.host(), "ws.example.com");
    assert_eq!(next.preserved_fields, draft.preserved_fields);
}

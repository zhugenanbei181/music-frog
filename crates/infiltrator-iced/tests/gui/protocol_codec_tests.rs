//! DUAL-05 Iced tests: the custom-node modal is a projection of the shared
//! protocol studio, and the save path splices losslessly.
//! Mounted via `src/test_mounts.rs`.
//! test-intent: behavior

use crate::state::AppState;
use crate::types::message::Message;
use infiltrator_application::protocol_codec_application::{ProtocolCodecApplication, clear_studio};
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolFamily};
use infiltrator_shared::locales::{Lang, Localizer};

// 2022-blake3-aes-256-gcm with a 32-byte PSK, base64 userinfo (SIP002).
const SS2022_URI: &str = "ss://MjAyMi1ibGFrZTMtYWVzLTI1Ni1nY206QUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQUFBQT0=@ss.example.com:8388#SS-2022";

const REALITY_URI: &str = "vless://b831381d-6324-4d53-ad4f-8cda48b30811@us.example.com:443?security=reality&pbk=PubKey1234567890AAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=abcd1234&spx=%2Fspider&fp=chrome&flow=xtls-rprx-vision&sni=reality.example.com#US-Reality";

fn state_with_modal() -> AppState {
    clear_studio();
    let (mut state, _) = AppState::new();
    let _ = state.update(Message::OpenCustomNodeModal);
    state
}

#[test]
fn opening_the_modal_seeds_an_explicit_shared_draft() {
    let state = state_with_modal();
    assert!(state.runtime.custom_node_modal_open);
    let studio = &state.runtime.custom_node_studio;
    let draft = studio.draft.as_ref().expect("seeded draft");
    assert_eq!(draft.node_type, "vless");
    assert_eq!(draft.family(), ProtocolFamily::Vless);
    assert!(studio.report.is_some());
    // An empty draft is honestly blocking: it has no credential yet.
    assert!(studio.has_blocking_issue());
    assert!(
        studio
            .issue_lines()
            .iter()
            .any(|line| line.contains("uuid")),
        "{:?}",
        studio.issue_lines()
    );
}

#[test]
fn importing_a_share_link_publishes_the_shared_typed_report() {
    let mut state = state_with_modal();
    let _ = state.update(Message::UpdateCustomNodeUriInput(REALITY_URI.to_owned()));
    let _ = state.update(Message::ParseAndImportCustomUri);

    let studio = &state.runtime.custom_node_studio;
    let draft = studio.draft.as_ref().expect("shared draft");
    assert_eq!(draft.name, "US-Reality");
    assert_eq!(draft.server, "us.example.com");
    assert_eq!(draft.port, 443);
    assert_eq!(draft.uuid, "b831381d-6324-4d53-ad4f-8cda48b30811");
    assert_eq!(draft.flow, "xtls-rprx-vision");
    assert_eq!(draft.reality.short_id, "abcd1234");
    assert_eq!(draft.reality.spider_x, "/spider");
    assert_eq!(draft.reality.fingerprint, "chrome");

    let report = studio.report.as_ref().expect("shared report");
    assert_eq!(report.family, ProtocolFamily::Vless);
    assert_eq!(report.flow_chip.as_deref(), Some("Vision 流控"));
    assert!(report.reality_chips.contains(&"Reality".to_string()));
    assert!(studio.uri_preview.is_some());
}

#[test]
fn importing_ss_2022_surfaces_the_cipher_family_and_psk_size() {
    let mut state = state_with_modal();
    let _ = state.update(Message::UpdateCustomNodeUriInput(SS2022_URI.to_owned()));
    let _ = state.update(Message::ParseAndImportCustomUri);

    let draft = state
        .runtime
        .custom_node_studio
        .draft
        .as_ref()
        .expect("shared draft");
    assert_eq!(draft.cipher, "2022-blake3-aes-256-gcm");
    assert!(draft.cipher_family().unwrap().is_2022());

    let report = state.runtime.custom_node_studio.report.as_ref().unwrap();
    assert!(report.cipher_is_2022);
    assert_eq!(report.cipher_key_bytes, Some(32));
    assert_eq!(report.cipher_chip.as_deref(), Some("2022 · AES-256-GCM"));

    // A wrong-length PSK is a shared blocking issue: the save path refuses.
    let mut broken = draft.clone();
    broken.password = "AAAAAAAAAAAAAAAAAAAAAA==".to_owned();
    let _ = state.update(Message::UpdateCustomNodeDraft(Box::new(broken)));
    assert!(state.runtime.custom_node_studio.has_blocking_issue());
    let issues = state.runtime.custom_node_studio.issue_lines().join(" ");
    assert!(issues.contains("key size"), "{issues}");
}

#[test]
fn a_malformed_share_link_publishes_an_honest_error_instead_of_a_draft() {
    let mut state = state_with_modal();
    let _ = state.update(Message::UpdateCustomNodeUriInput("not-a-uri".to_owned()));
    let _ = state.update(Message::ParseAndImportCustomUri);

    let studio = &state.runtime.custom_node_studio;
    assert!(studio.last_error.is_some());
    // The previous draft is kept and clearly marked as not-yet-parsed: the
    // failure never turns into a half-filled node.
    let draft = studio.draft.as_ref().expect("seeded draft is kept");
    assert!(draft.server.is_empty());
    assert!(draft.name.is_empty());
    assert!(studio.uri_preview.is_none());
}

#[test]
fn draft_edits_re_derive_the_report_and_the_uri_gaps() {
    let mut state = state_with_modal();
    let _ = state.update(Message::UpdateCustomNodeUriInput(REALITY_URI.to_owned()));
    let _ = state.update(Message::ParseAndImportCustomUri);

    let mut draft = state
        .runtime
        .custom_node_studio
        .draft
        .clone()
        .expect("shared draft");
    draft.smux.enabled = true;
    draft.smux.protocol = "yamux".to_owned();
    draft.smux.max_connections = 8;
    draft.smux.min_streams = 2;
    draft.smux.max_streams = 16;
    let _ = state.update(Message::UpdateCustomNodeDraft(Box::new(draft)));

    let studio = &state.runtime.custom_node_studio;
    let report = studio.report.as_ref().expect("shared report");
    assert_eq!(
        report.smux_chips,
        vec![
            "mux:yamux".to_string(),
            "mc:8".to_string(),
            "ms:16".to_string(),
            "min:2".to_string(),
        ]
    );
    // smux is YAML-only, and the gap is the one the application measured.
    assert!(
        studio.uri_gaps.contains(&"smux".to_string()),
        "{:?}",
        studio.uri_gaps
    );
    assert!(studio.uri_is_lossy());
}

#[test]
fn export_never_fabricates_a_placeholder_node() {
    let mut state = state_with_modal();
    // No draft at all: export refuses instead of inventing a server/secret.
    state.runtime.custom_node_studio = Default::default();
    let _ = state.update(Message::ExportCustomNodeUri);
    assert!(state.runtime.custom_node_studio.uri_preview.is_none());
    assert_eq!(
        state.runtime.custom_node_studio.last_error.as_deref(),
        Some("no node draft to export")
    );

    // With a real draft the exported link carries the real server and secret.
    let mut state = state_with_modal();
    let _ = state.update(Message::UpdateCustomNodeUriInput(REALITY_URI.to_owned()));
    let _ = state.update(Message::ParseAndImportCustomUri);
    let uri = state
        .runtime
        .custom_node_studio
        .uri_preview
        .clone()
        .expect("exported uri");
    assert!(uri.contains("us.example.com:443"), "{uri}");
    assert!(uri.contains("flow=xtls-rprx-vision"), "{uri}");
    assert!(!uri.contains("example.com:443?pbk=secret"), "{uri}");
}

#[test]
fn save_refuses_an_incomplete_draft_before_touching_the_profile() {
    let mut state = state_with_modal();
    let mut draft = ProtocolDraft::new("vless");
    draft.name = "incomplete".to_owned();
    let _ = state.update(Message::UpdateCustomNodeDraft(Box::new(draft)));
    assert!(state.runtime.custom_node_studio.has_blocking_issue());

    let _ = state.update(Message::SaveCustomNodeForm);
    // The modal stays open and the shared error is on screen; no profile write
    // task is scheduled from a draft that has not passed the shared validation.
    assert!(state.runtime.custom_node_modal_open);
    let live = state.runtime.custom_node_studio.last_error.as_deref();
    assert!(live.is_some_and(|text| text.contains("uuid")), "{live:?}");
}

#[test]
fn the_upsert_path_the_modal_calls_preserves_the_whole_profile() {
    // The handler feeds the whole document to the shared application; this
    // pins the exact call the modal makes (no parse/export round trip).
    let profile = r#"
mode: rule
proxies:
  - name: "SS-2022"
    type: ss
    server: 1.1.1.1
    port: 8388
    cipher: 2022-blake3-aes-256-gcm
    password: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
    custom-flag: keep-me
proxy-groups:
  - name: PROXY
    type: select
    proxies: [SS-2022]
rules:
  - MATCH,PROXY
"#;
    let mut draft = ProtocolCodecApplication::draft_from_uri(SS2022_URI).unwrap();
    assert_eq!(draft.cipher, "2022-blake3-aes-256-gcm");
    assert_eq!(draft.password.len(), 44, "32-byte PSK as base64");
    draft.server = "9.9.9.9".to_owned();
    let commit = ProtocolCodecApplication::upsert_draft_into_profile(profile, &draft).unwrap();

    assert!(commit.replaced_existing);
    assert!(commit.is_structure_preserving(), "{:?}", commit.audit);
    assert_eq!(commit.audit.unknown_fields, vec!["custom-flag".to_string()]);

    let document: serde_yaml_ng::Value = serde_yaml_ng::from_str(&commit.profile_yaml).unwrap();
    assert_eq!(document.get("mode").and_then(|v| v.as_str()), Some("rule"));
    assert_eq!(
        document
            .get("rules")
            .and_then(|v| v.as_sequence())
            .map(Vec::len),
        Some(1)
    );
    assert_eq!(
        document
            .get("proxy-groups")
            .and_then(|v| v.as_sequence())
            .map(Vec::len),
        Some(1)
    );
    let nodes = ProtocolCodecApplication::parse_nodes(
        &commit.profile_yaml,
        infiltrator_contract::protocol_fidelity::NodeCodecFormat::ClashYaml,
    )
    .unwrap();
    assert_eq!(nodes[0].server, "9.9.9.9");
    assert_eq!(
        nodes[0].extra.get("custom-flag"),
        Some(&serde_yaml_ng::Value::String("keep-me".to_string()))
    );
}

#[test]
fn new_locale_keys_resolve_in_both_languages() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    for key in [
        "custom_node_secret",
        "custom_node_cipher",
        "custom_node_flow",
        "custom_node_mux_enabled",
        "custom_node_mux_protocol",
        "custom_node_mux_max",
        "custom_node_mux_min_streams",
        "custom_node_mux_max_streams",
        "custom_node_mux_padding",
        "custom_node_skip_verify",
        "custom_node_issues_hint",
        "custom_node_uri_gap",
    ] {
        assert_ne!(zh.tr(key).as_ref(), key, "zh missing {key}");
        assert_ne!(en.tr(key).as_ref(), key, "en missing {key}");
    }
    let gap = infiltrator_shared::i18n_interpolator::interpolate(
        &zh.tr("custom_node_uri_gap"),
        &[("field", "smux")],
    );
    assert!(gap.contains("smux"), "{gap}");
}

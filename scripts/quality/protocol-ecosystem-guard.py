#!/usr/bin/env python3
"""Fail-closed guard for the group 05 protocol-ecosystem ledger (DUAL-05-01..15).

Group 05's closure standard matches groups 06/12/14: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts:

* the per-item ledger rows exist with the status vocabulary the rest of the
  master plan uses, and the batch-A note names the closed items;
* the shared contract keeps the typed protocol vocabulary (cipher family,
  REALITY/Vision, smux, draft, report, codec audit);
* the shared application keeps the URI codec, the *lossless* profile splice,
  the measured URI fidelity gaps and the studio publication;
* the studio reaches both surfaces through the page projection, and Bevy's
  custom-node card keeps its URI field, chip slots, observers and the
  `LastProxiesProjection` resource the save path reads;
* the fixed defects do not regress: Iced's node save must not fall back to the
  whole-document `parse_nodes`/`export_nodes` round trip (which drops every
  non-`proxies:` section) and must not fabricate a placeholder node;
* batch B (05-03…05-08/12/15) keeps its typed parameter blocks, the family
  dynamic owned-key splice, the mihomo key spellings and the shared
  regression matrix reachable from both surfaces;
* batch C (05-09/10/13) keeps the dialer-hop chain, the shared loop detector
  (a loop is never a valid chain) and the custom-CA/whitelist flow, including
  the hard rule that a host without a CA reader reports typed unsupported
  instead of claiming a load.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"
CONTRACT = "crates/infiltrator-contract/src/protocol_fidelity.rs"
APPLICATION = "crates/infiltrator-application/src/protocol_codec_application.rs"
PARAMS = "crates/infiltrator-contract/src/protocol_params.rs"
PARAMS_EXT = "crates/infiltrator-contract/src/protocol_params_ext.rs"
MATRIX = "crates/infiltrator-contract/src/protocol_matrix.rs"
PROJECTION = "crates/infiltrator-application/src/protocol_node_projection.rs"
NODE_PARAMS = "crates/infiltrator-application/src/protocol_node_params.rs"
MATRIX_APPLICATION = "crates/infiltrator-application/src/protocol_codec_matrix_application.rs"
SNAPSHOT = "crates/infiltrator-contract/src/surface_snapshot.rs"
READER = "crates/infiltrator-application/src/surface_reader.rs"
INTENT = "crates/infiltrator-contract/src/command.rs"
ROUTER = "crates/infiltrator-application/src/command_application.rs"
ICED_UPDATE = "crates/infiltrator-iced/src/update/ui.rs"
ICED_PROTOCOL = "crates/infiltrator-iced/src/update/protocol_codec.rs"
ICED_MODAL = "crates/infiltrator-iced/src/view_root/custom_node_modal.rs"
ICED_PARAMS = "crates/infiltrator-iced/src/view_root/custom_node_params.rs"
ICED_TRUST = "crates/infiltrator-iced/src/view_root/custom_node_trust.rs"
BEVY_CUSTOM = "crates/infiltrator-bevy-ui/src/pages/proxies_custom.rs"
BEVY_PAGE = "crates/infiltrator-bevy-ui/src/pages/proxies.rs"
BEVY_COMMAND = "crates/infiltrator-bevy-ui/src/command.rs"
DOMAIN_MODEL = "crates/infiltrator-domain/src/proxy_nodes/model.rs"
DOMAIN_VALIDATE = "crates/infiltrator-domain/src/proxy_nodes/validate.rs"
DOMAIN_CONVERTER = "crates/infiltrator-domain/src/profile_converter.rs"
DOMAIN_DIALER = "crates/infiltrator-domain/src/proxy_nodes/dialer.rs"
DOMAIN_TRUST = "crates/infiltrator-domain/src/tls_trust.rs"
CHAIN_CONTRACT = "crates/infiltrator-contract/src/dialer_chain.rs"
TRUST_CONTRACT = "crates/infiltrator-contract/src/protocol_trust.rs"
CA_PORT = "crates/infiltrator-ports/src/certificate_authority.rs"
HOST_RUNTIME = "crates/infiltrator-ports/src/host_runtime.rs"
CHAIN_APPLICATION = "crates/infiltrator-application/src/dialer_chain_application.rs"
CA_APPLICATION = "crates/infiltrator-application/src/certificate_authority_application.rs"
DESKTOP_CA = "crates/infiltrator-desktop/src/certificate_authority.rs"


def read(path: str) -> str:
    return (ROOT / path).read_text(encoding="utf-8")


def require(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    compact_text = " ".join(text.split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        rustfmt_marker = compact_marker.replace(" }", ", }")
        if (
            marker not in text
            and compact_marker not in compact_text
            and rustfmt_marker not in compact_text
        ):
            violations.append(f"{path} missing {marker!r}")


def forbid(violations: list[str], path: str, *markers: str) -> None:
    text = read(path)
    for marker in markers:
        if marker in text:
            violations.append(f"{path} still contains forbidden marker {marker!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=["report", "enforce"], default="enforce")
    args = parser.parse_args()
    violations: list[str] = []

    # 1. The full per-item ledger, with the plan's status vocabulary.
    require(
        violations,
        LEDGER,
        "DUAL-05-01",
        "DUAL-05-02",
        "DUAL-05-03",
        "DUAL-05-04",
        "DUAL-05-05",
        "DUAL-05-06",
        "DUAL-05-07",
        "DUAL-05-08",
        "DUAL-05-09",
        "DUAL-05-10",
        "DUAL-05-11",
        "DUAL-05-12",
        "DUAL-05-13",
        "DUAL-05-14",
        "DUAL-05-15",
        "parity-ready",
        "shared-ready",
        "planned",
        "组 05 逐项账目",
        "2026-09-22 组 05 批次 A",
        "2026-09-22 组 05 批次 B",
        "组 05 协议生态保真与多路复用 | 15 | `parity-ready (15/15)`",
        "2026-09-22 组 05 批次 C",
        "组 05 达到 **15/15**",
        "DialerChainReport",
        "DialerTopology::resolve_chain",
        "ProxyGroupTopology::detect_group_cycles",
        "TlsTrustParams",
        "CaTrustReport",
        "custom-certifactes",
        "DesktopCertificateAuthority",
        # Honest facts the ledger must keep stating in batch B.
        "listener 入站",
        "无 `quic` 传输网络",
        "没有 `trojan-go` 节点类型",
        "按 forward-compat typed+保留",
        # Closed items carry their distinctive evidence tokens.
        "ShadowsocksCipher",
        "VlessFlow",
        "RealityParams",
        "SmuxParams",
        "ProtocolCodecApplication",
        "upsert_draft_into_profile",
        "uri_fidelity_gaps",
        "ExportCustomNodeUri",
        # Batch B evidence tokens.
        "ProtocolCodecMatrixReport",
        "ProtocolCodecMatrixApplication::run_deterministic_matrix",
        "ProtocolParams::validate",
        "EchParams",
        "TuicParams",
        "Hysteria2Params",
        "WireGuardParams",
        "Sip003Plugin",
        "SshNode",
        "AnyTlsParams",
        "TrojanSsParams",
        "ReservedField",
        "pre-shared-key",
        "amnezia-wg-option",
    )

    # 2. Shared contract: the typed protocol vocabulary.
    require(
        violations,
        CONTRACT,
        "pub enum ShadowsocksCipher",
        "pub const ALL_2022",
        "pub const fn is_2022",
        "pub const fn key_bytes",
        "pub enum VlessFlow",
        "pub struct RealityParams",
        "pub fn fingerprint_known",
        "pub fn chips",
        "pub struct SmuxParams",
        "pub const PROTOCOLS",
        "pub fn has_overrides",
        "pub enum ProtocolFamily",
        "pub struct ProtocolIssue",
        "pub struct ProtocolDraft",
        "pub fn cipher_family",
        "pub fn required_credentials",
        "pub fn validate",
        "pub struct ProtocolFidelityReport",
        "pub enum NodeCodecFormat",
        "pub struct CodecAudit",
        "pub struct ProtocolStudioSnapshot",
        "pub fn uri_is_lossy",
        "pub fn with_error",
    )

    # 2b. Batch B typed parameter vocabulary + the 05-15 matrix contract.
    require(
        violations,
        PARAMS,
        "pub struct EchParams",
        "pub struct TuicParams",
        "pub struct Hysteria2Params",
        "pub struct ProtocolParams",
        "pub struct ProtocolParamsReport",
        "pub fn notes",
        "KNOWN_SIP003_PLUGINS",
        "KNOWN_CONGESTION_CONTROLLERS",
        "KNOWN_HYSTERIA2_OBFS",
        "KNOWN_TRANSPORT_NETWORKS",
        "v1.19.18",
    )
    require(
        violations,
        PARAMS_EXT,
        "pub struct AmneziaWgParams",
        "pub struct WireGuardParams",
        "pub struct TransportParams",
        "pub struct WsOptsParams",
        "pub struct Sip003Plugin",
        "pub enum PluginOptValue",
        "pub struct SshParams",
        "pub struct AnyTlsParams",
        "pub struct TrojanSsParams",
        "KNOWN_SIP003_PLUGINS",
        "KNOWN_TRANSPORT_NETWORKS",
        "XHTTP_MODES",
    )
    require(
        violations,
        MATRIX,
        "pub struct ProtocolCodecMatrixScenario",
        "pub struct ProtocolCodecMatrixReport",
        "pub fn all_covered_passed",
        "pub fn not_covered_ids",
        "pub fn covered_passed_count",
        "pub fn summary_zh",
    )
    require(
        violations,
        MATRIX_APPLICATION,
        "pub struct ProtocolCodecMatrixApplication",
        "pub fn run_deterministic_matrix",
        "DUAL-05-01",
        "DUAL-05-15",
        "fn check_dialer_chain",
        "fn check_dialer_cycles",
        "fn check_custom_ca",
        "UnsupportedCertificateAuthority",
        "MATRIX_CA_PEM",
    )
    require(
        violations,
        "crates/infiltrator-application/src/protocol_codec_matrix_application_test.rs",
        "deterministic_matrix_passes_every_covered_item_and_names_the_planned_ones",
        "matrix_is_deterministic_across_runs",
        "assert_eq!(report.covered_count(), 15",
    )

    # 2c. The one draft projection both surfaces and the codec share.
    require(
        violations,
        PROJECTION,
        "pub const DRAFT_OWNED_KEYS",
        "pub fn owned_keys",
        "pub const DRAFT_NESTED_KEYS",
        "pub const TYPED_EXTRA_KEYS",
        "pub fn is_typed_extra_key_for",
        "pub fn draft_from_node",
        "effective_spider_x",
        # DUAL-05-09/13: the new draft-owned keys.
        '"dialer-proxy",',
        '"fingerprint",',
        '"ca",',
        '"ca-str",',
    )
    forbid(violations, PROJECTION, "tokio::", "reqwest::")
    require(
        violations,
        NODE_PARAMS,
        "pub fn params_from_node",
        "pub fn node_from_draft",
        "fn write_params",
        "fn amnezia_to_json",
    )

    # 3. Shared application: codec + lossless splice + measured gaps + studio.
    require(
        violations,
        APPLICATION,
        "pub struct ProtocolCodecApplication",
        "pub fn draft_from_uri",
        "pub fn uri_from_draft",
        "pub fn draft_from_node",
        "pub fn node_from_draft",
        "pub fn report",
        "pub fn uri_fidelity_gaps",
        "pub fn publish_draft",
        "pub fn upsert_draft_into_profile",
        "pub fn audit_conversion",
        "pub fn parse_nodes",
        "pub fn export_nodes",
        "pub fn publish_error",
        "structure_preserved",
        "merge_preserving_unknown",
        "merge_nested",
        "is_typed_extra_key_for",
        "owned_keys",
        "node_from_draft",
        "pub fn publish_studio",
        "pub fn studio_snapshot",
        "pub fn clear_studio",
        # The application layer stays executor-neutral.
    )
    forbid(violations, APPLICATION, "tokio::", "reqwest::")
    require(
        violations,
        CHAIN_APPLICATION,
        "pub struct DialerChainApplication",
        "pub fn analyze_profile",
        "pub fn report_from_facts",
        "fn chain_view",
        "fn loop_finding",
        "DialerChainReport",
    )
    # A loop must never be mapped onto a valid chain end.
    forbid(
        violations,
        CHAIN_APPLICATION,
        "DomainChainEnd::Cycle(_) => DialerChainEnd::Complete",
    )
    require(
        violations,
        CA_APPLICATION,
        "pub struct CertificateAuthorityApplication",
        "pub fn resolve",
        "pub fn write_ca_path",
        "pub fn remove_ca_path",
        "pub fn trust_anchors_from_profile",
        "pub fn upsert_trust_anchors_into_profile",
        "CaLoadStatus::Unsupported",
        "CaLoadStatus::FingerprintMismatch",
        "tls_trust::sha256_fingerprint",
    )
    forbid(violations, CA_APPLICATION, "tokio::", "reqwest::")

    require(
        violations,
        "crates/infiltrator-application/src/protocol_codec_application_test.rs",
        "upsert_preserves_every_other_section_and_unknown_node_keys",
        "yaml_round_trip_audit_is_honest_about_section_loss",
        "smux_overrides_are_yaml_only_and_reported_as_a_uri_gap",
        "upsert_refuses_a_draft_with_unresolved_protocol_issues",
        "uri_draft_round_trip_keeps_the_2022_cipher_family",
        "wireguard_parameters_round_trip_with_mihomo_key_spellings",
        "tuic_and_hysteria2_parameters_reach_the_profile_and_back",
        "ssh_and_anytls_parameters_project_through_the_flat_node",
        "legacy_idle_timeout_and_amnezia_aliases_still_parse",
        "nestable_owned_maps_keep_unknown_sub_keys_during_the_splice",
        "typed_parameter_blocks_are_measured_as_uri_gaps",
        "unknown_field_audit_does_not_call_typed_keys_unknown",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/protocol_fidelity_test.rs",
        "shadowsocks_2022_family_is_complete_and_typed",
        "ss2022_psk_length_is_validated_against_the_cipher",
        "vless_flow_and_reality_chips_are_shared_vocabulary",
        "smux_parameters_are_typed_and_validated",
        "unsupported_reality_and_smux_are_reported_not_silently_dropped",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/protocol_params_test.rs",
        "ech_is_typed_validated_and_honest_about_dns_resolution",
        "tuic_congestion_parameters_are_typed_and_chipped",
        "hysteria2_port_hopping_and_obfs_are_validated",
        "wireguard_keys_reserved_and_amnezia_are_typed",
        "transports_cover_ws_early_data_grpc_xhttp_and_quic_with_notes",
        "sip003_plugins_enforce_their_required_options",
        "ssh_parameters_require_an_identity",
        "anytls_and_trojan_ss_opts_are_typed",
        "alpn_order_is_the_negotiation_model",
    )

    require(
        violations,
        DOMAIN_MODEL,
        "pub struct SshNode",
        "pub enum SshType",
        "ProxyNode::Ssh",
        "pub struct TrojanSsOpts",
        "pub ss_opts: Option<TrojanSsOpts>",
    )
    require(
        violations,
        DOMAIN_VALIDATE,
        "ProxyNode::Ssh(node) =>",
        "ssh: username is required",
        "ssh: password or private-key is required",
        "trojan ss-opts: method is required",
    )
    require(
        violations,
        DOMAIN_DIALER,
        "pub struct DialerTopology",
        "pub fn detect_cycles",
        "pub fn cycles_containing",
        "pub fn resolve_chain",
        "pub fn resolve_all",
        "pub struct DialerGraphFacts",
        "pub enum DialerChainEnd",
        "ProxyGroupTopology::detect_group_cycles",
        "GroupBoundary",
    )
    require(
        violations,
        DOMAIN_TRUST,
        "pub fn normalize_pem",
        "pub fn validate_pem_bundle",
        "pub fn sha256_fingerprint",
        "pub fn is_sha256_fingerprint",
        "Sha256::digest",
    )
    require(
        violations,
        CA_PORT,
        "pub trait CertificateAuthorityPort",
        "pub struct CaFile",
        "pub struct UnsupportedCertificateAuthority",
    )
    require(violations, HOST_RUNTIME, "fn certificate_authority_port")
    require(
        violations,
        DESKTOP_CA,
        "pub struct DesktopCertificateAuthority",
        "MAX_CA_BUNDLE_BYTES",
        "CertificateAuthorityPort for DesktopCertificateAuthority",
    )
    require(
        violations,
        CHAIN_CONTRACT,
        "pub struct DialerChainReport",
        "pub struct DialerChainView",
        "pub enum DialerChainEnd",
        "pub struct DialerLoopFinding",
        "pub fn chain_line",
        "pub fn valid_chains",
        "pub fn has_loop",
        "pub fn loop_message",
        "⛔",
    )
    require(
        violations,
        TRUST_CONTRACT,
        "pub struct TlsTrustParams",
        "pub enum CaLoadStatus",
        "pub struct CaTrustResolution",
        "pub struct CaTrustReport",
        "pub fn is_loaded",
        "custom-certifactes",
        "ca-str",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/dialer_chain_test.rs",
        "a_loop_is_never_presented_as_a_valid_chain",
        "a_missing_target_is_invalid_and_a_group_boundary_is_not",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/protocol_trust_test.rs",
        "resolutions_and_the_report_never_claim_an_unloaded_ca",
        "invalid_values_are_refused_with_real_reasons",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/proxy_nodes/dialer_test.rs",
        "self_loop_and_mutual_loop_are_detected_and_never_valid",
        "dialer_chain_resolves_hops_in_order",
        "proxy_group_member_cycles_reuse_the_group_semantics",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/tls_trust_test.rs",
        "a_real_bundle_validates_and_gets_a_stable_fingerprint",
        "an_invalid_bundle_is_rejected_with_a_real_reason",
    )
    require(
        violations,
        "crates/infiltrator-application/src/certificate_authority_application_test.rs",
        "a_host_without_a_reader_reports_typed_unsupported_instead_of_a_load",
        "a_real_reader_loads_the_bundle_and_computes_the_fingerprint",
        "the_path_anchor_lands_in_the_real_v11918_carrier_and_preserves_the_rest",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dialer_chain_application_test.rs",
        "a_loop_is_typed_and_never_a_valid_chain",
        "a_node_save_publishes_the_dialer_verdict_for_the_written_document",
    )
    require(
        violations,
        DOMAIN_CONVERTER,
        "pub enum ReservedField",
        'rename = "pre-shared-key"',
        'rename = "amnezia-wg-option"',
        'rename = "private-key-passphrase"',
    )

    # 4. One projection reaches both surfaces.
    require(
        violations,
        SNAPSHOT,
        "pub custom_node: crate::protocol_fidelity::ProtocolStudioSnapshot",
        "custom-node protocol studio",
    )
    require(violations, READER, "crate::protocol_codec_application::studio_snapshot()")
    require(
        violations,
        INTENT,
        "ImportCustomNodeUri {",
        "SaveCustomNodeDraft {",
        "crate::protocol_fidelity::ProtocolDraft",
        "ScanDialerChains",
        "UpdateCustomNodeDraftField {",
        "ResolveCertificateAuthority {",
        "crate::protocol_trust::TlsTrustParams",
    )
    require(
        violations,
        ROUTER,
        "CommandIntent::ImportCustomNodeUri",
        "CommandIntent::SaveCustomNodeDraft",
        "CommandIntent::ScanDialerChains",
        "CommandIntent::UpdateCustomNodeDraftField",
        "CommandIntent::ResolveCertificateAuthority",
        "ProtocolCodecApplication::draft_from_uri",
        "ProtocolCodecApplication::upsert_draft_into_profile",
        "ProtocolCodecApplication::publish_dialer_report",
        "ProtocolCodecApplication::publish_ca_trust",
        "with_certificate_authority",
    )

    # 5. Iced: shared-draft projection + lossless save + no fabricated export.
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "pub custom_node_studio",
    )
    require(
        violations,
        ICED_UPDATE,
        # The group 05 handlers live in their own module; `ui.rs` only routes.
        "self.update_protocol_codec(message)",
    )
    require(
        violations,
        ICED_PROTOCOL,
        "pub(super) fn update_protocol_codec",
        "Message::ParseAndImportCustomUri",
        "ProtocolCodecApplication::draft_from_uri",
        "Message::UpdateCustomNodeDraft",
        "Message::ExportCustomNodeUri",
        "ProtocolCodecApplication::uri_from_draft",
        "Message::SaveCustomNodeForm",
        "ProtocolCodecApplication::upsert_draft_into_profile",
        "Message::CustomNodeSaved",
        "ScanCustomNodeDialer",
        "VerifyCustomNodeCertificateAuthority",
        "scan_dialer_chains",
        "refresh_custom_node_ca_trust",
        "certificate_authority_port",
        "publish_ca_trust",
    )
    forbid(
        violations,
        ICED_UPDATE,
        # The old lossy / fabricated node-editor paths.
        "profile_converter::ProfileConverter::parse_uri",
        "profile_converter::ProfileConverter::export_uri",
        "ProfileConverter::parse_nodes",
        "ProfileConverter::export_nodes",
        "node.example.com",
        "custom_node_exported_uri",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "ExportNodeAsUri",
    )
    require(
        violations,
        ICED_MODAL,
        "pub fn custom_node_modal",
        "ProtocolStudioSnapshot",
        "custom_node_studio",
        "custom_node_cipher",
        "custom_node_flow",
        "custom_node_mux_enabled",
        "custom_node_mux_min_streams",
        "custom_node_uri_gap",
        "Message::UpdateCustomNodeDraft",
        "report.all_chips()",
    )
    require(
        violations,
        ICED_PARAMS,
        "pub(super) fn params_section",
        "custom_node_params_title",
        "custom_node_alpn",
        "custom_node_ech_config",
        "custom_node_tuic_cc",
        "custom_node_hy2_ports",
        "custom_node_wg_private_key",
        "custom_node_transport_network",
        "custom_node_ws_early_data",
        "custom_node_plugin_name",
        "custom_node_ssh_username",
        "custom_node_anytls_idle",
        "custom_node_trojan_ss",
        "Message::UpdateCustomNodeDraft",
    )
    require(
        violations,
        ICED_TRUST,
        "pub(super) fn hop_and_trust_params",
        "custom_node_dialer_proxy",
        "custom_node_dialer_chain",
        "custom_node_ca_path",
        "custom_node_ca_str",
        "custom_node_ca_fingerprint",
        "Message::ScanCustomNodeDialer",
        "Message::VerifyCustomNodeCertificateAuthority",
        "chain_line()",
        "studio.ca_trust.lines()",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/protocol_codec_tests.rs",
        "importing_ss_2022_surfaces_the_cipher_family_and_psk_size",
        "importing_a_share_link_publishes_the_shared_typed_report",
        "the_upsert_path_the_modal_calls_preserves_the_whole_profile",
        "export_never_fabricates_a_placeholder_node",
        "draft_edits_re_derive_the_report_and_the_uri_gaps",
        "importing_wireguard_surfaces_the_shared_typed_parameters",
        "importing_tuic_hysteria2_and_ssh_surfaces_the_shared_blocks",
        "quic_and_xhttp_notes_are_surfaced_not_hidden",
        "protocol_codec_matrix_passes_on_the_iced_surface",
        "the_shared_dialer_report_reaches_the_iced_studio_and_never_validates_a_loop",
        "a_host_without_a_ca_reader_renders_the_typed_unsupported_state",
        "an_inline_bundle_is_validated_in_process_and_a_mismatching_pin_is_refused",
    )
    # No surface may hard-code the CA status vocabulary: the labels come from
    # the shared report (`CaLoadStatus`), never from a view string.
    forbid(
        violations,
        ICED_TRUST,
        "宿主不支持",
        "已加载校验",
    )

    # 6. Bevy: URI field, chip slots, both observers, the mounted card and the
    #    resource the save path reads.
    require(
        violations,
        BEVY_CUSTOM,
        "pub fn custom_node_scene",
        "pub struct CustomNodeUriField",
        "pub struct CustomNodeText",
        "pub enum CustomNodeSlot",
        "pub(crate) fn on_custom_node_action_activated",
        "pub(crate) fn sync_custom_node_studio",
        "UiCommand::ImportCustomNodeUri",
        "UiCommand::SaveCustomNodeDraft",
        "LastProxiesProjection",
        "text_field_with_placeholder_scene",
        "report.all_chips()",
        "CustomNodeSlot::Notes",
        "无跨版本提示",
        "CustomNodeSlot::Chain",
        "CustomNodeSlot::CaTrust",
        "chain_initial",
        "ca_initial",
        "CustomNodeDialerField",
        "CustomNodeCaField",
        "ScanDialerChainsButton",
        "VerifyCustomNodeCaButton",
        "UiCommand::ScanCustomNodeDialer",
        "UiCommand::VerifyCustomNodeCa",
        "UiCommand::UpdateCustomNodeDraftField",
    )
    forbid(
        violations,
        BEVY_CUSTOM,
        "宿主不支持",
        "已加载校验",
    )
    require(
        violations,
        BEVY_PAGE,
        "pub custom_node: infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot",
        "custom_node_scene(&projection.custom_node, palette)",
        "on_custom_node_action_activated",
        "sync_custom_node_studio",
        "LastProxiesProjection::default()",
    )
    require(
        violations,
        BEVY_COMMAND,
        "ImportCustomNodeUri { uri: String }",
        "SaveCustomNodeDraft {",
        "UpdateCustomNodeDraftField { field: String, value: String }",
        "ScanCustomNodeDialer,",
        "VerifyCustomNodeCa {",
        "CommandIntent::ImportCustomNodeUri",
        "CommandIntent::SaveCustomNodeDraft",
        "CommandIntent::ScanDialerChains",
        "CommandIntent::ResolveCertificateAuthority",
        "CommandIntent::UpdateCustomNodeDraftField",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/protocol_codec_tests.rs",
        "custom_node_import_button_submits_the_typed_uri",
        "custom_node_save_refuses_without_a_shared_draft_and_submits_the_shared_one",
        "custom_node_slots_render_the_shared_studio_facts",
        "custom_node_audit_line_reports_the_measured_lossless_verdict",
        "custom_node_notes_slot_reports_pinned_core_fallbacks_verbatim",
        "protocol_codec_matrix_passes_on_the_bevy_surface",
        "custom_node_dialer_and_ca_slots_render_the_shared_facts",
        "custom_node_scan_and_ca_buttons_submit_the_shared_commands",
    )

    if violations:
        for violation in violations:
            print(f"protocol-ecosystem-guard: {violation}")
        if args.mode == "enforce":
            print("protocol-ecosystem-guard: FAIL")
            return 1
        print("protocol-ecosystem-guard: report mode, violations above")
        return 0
    print("protocol-ecosystem-guard: ok (group 05 DUAL-05-01..15)")
    return 0


if __name__ == "__main__":
    sys.exit(main())

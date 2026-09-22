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
  non-`proxies:` section) and must not fabricate a placeholder node.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"
CONTRACT = "crates/infiltrator-contract/src/protocol_fidelity.rs"
APPLICATION = "crates/infiltrator-application/src/protocol_codec_application.rs"
SNAPSHOT = "crates/infiltrator-contract/src/surface_snapshot.rs"
READER = "crates/infiltrator-application/src/surface_reader.rs"
INTENT = "crates/infiltrator-contract/src/command.rs"
ROUTER = "crates/infiltrator-application/src/command_application.rs"
ICED_UPDATE = "crates/infiltrator-iced/src/update/ui.rs"
ICED_PROTOCOL = "crates/infiltrator-iced/src/update/protocol_codec.rs"
ICED_MODAL = "crates/infiltrator-iced/src/view_root/custom_node_modal.rs"
BEVY_CUSTOM = "crates/infiltrator-bevy-ui/src/pages/proxies_custom.rs"
BEVY_PAGE = "crates/infiltrator-bevy-ui/src/pages/proxies.rs"
BEVY_COMMAND = "crates/infiltrator-bevy-ui/src/command.rs"


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
        "组 05 协议生态保真与多路复用 | 15 | `in progress (4/15)`",
        # Honest facts the ledger must keep stating.
        "全仓无 `masquerade` 字段",
        "全仓无 ECH 载体",
        "尚无组 06/12 那种**单一共享回归矩阵契约**",
        # Closed items carry their distinctive evidence tokens.
        "ShadowsocksCipher",
        "VlessFlow",
        "RealityParams",
        "SmuxParams",
        "ProtocolCodecApplication",
        "upsert_draft_into_profile",
        "uri_fidelity_gaps",
        "ExportCustomNodeUri",
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
        "DRAFT_OWNED_KEYS",
        "structure_preserved",
        "merge_preserving_unknown",
        "pub fn publish_studio",
        "pub fn studio_snapshot",
        "pub fn clear_studio",
        # The application layer stays executor-neutral.
    )
    forbid(violations, APPLICATION, "tokio::", "reqwest::")

    require(
        violations,
        "crates/infiltrator-application/src/protocol_codec_application_test.rs",
        "upsert_preserves_every_other_section_and_unknown_node_keys",
        "yaml_round_trip_audit_is_honest_about_section_loss",
        "smux_overrides_are_yaml_only_and_reported_as_a_uri_gap",
        "upsert_refuses_a_draft_with_unresolved_protocol_issues",
        "uri_draft_round_trip_keeps_the_2022_cipher_family",
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
    )
    require(
        violations,
        ROUTER,
        "CommandIntent::ImportCustomNodeUri",
        "CommandIntent::SaveCustomNodeDraft",
        "ProtocolCodecApplication::draft_from_uri",
        "ProtocolCodecApplication::upsert_draft_into_profile",
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
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/protocol_codec_tests.rs",
        "importing_ss_2022_surfaces_the_cipher_family_and_psk_size",
        "importing_a_share_link_publishes_the_shared_typed_report",
        "the_upsert_path_the_modal_calls_preserves_the_whole_profile",
        "export_never_fabricates_a_placeholder_node",
        "draft_edits_re_derive_the_report_and_the_uri_gaps",
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
        "CommandIntent::ImportCustomNodeUri",
        "CommandIntent::SaveCustomNodeDraft",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/protocol_codec_tests.rs",
        "custom_node_import_button_submits_the_typed_uri",
        "custom_node_save_refuses_without_a_shared_draft_and_submits_the_shared_one",
        "custom_node_slots_render_the_shared_studio_facts",
        "custom_node_audit_line_reports_the_measured_lossless_verdict",
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

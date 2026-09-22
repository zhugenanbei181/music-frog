#!/usr/bin/env python3
"""Fail-closed guard for the group 14 DNS workbench ledger (DUAL-14-01..15).

Group 14's closure standard matches groups 06/07/11/13: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts the per-item ledger rows exist, that the
shared DNS workbench contract and both surface wirings stay present, and that
the Bevy DNS page does not regress to its old fabricated switch command
(``UpdateSetting { key: "dns.*", value: "toggle" }``) or local ``DnsMode`` enum.
"""

from __future__ import annotations

import argparse
import pathlib
import sys


ROOT = pathlib.Path(__file__).resolve().parents[2]

LEDGER = "docs/DUAL_SURFACE_PARITY_MASTER_PLAN.md"


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

    # The full per-item ledger must exist for all fifteen items.
    require(
        violations,
        LEDGER,
        "DUAL-14-01",
        "DUAL-14-02",
        "DUAL-14-03",
        "DUAL-14-04",
        "DUAL-14-05",
        "DUAL-14-06",
        "DUAL-14-07",
        "DUAL-14-08",
        "DUAL-14-09",
        "DUAL-14-10",
        "DUAL-14-11",
        "DUAL-14-12",
        "DUAL-14-13",
        "DUAL-14-14",
        "DUAL-14-15",
        "parity-ready",
        "shared-ready",
        "planned",
    )
    # Closed items carry their distinctive evidence tokens in the ledger.
    require(
        violations,
        LEDGER,
        "DnsSettingsPatch",
        "DnsCoreSwitches",
        "DnsEnhancedMode",
        "DnsFakeIpFilterMode",
        "DnsServerTag",
        "test_dns_switch_submits_shared_patch",
        "test_dns_enhanced_mode_pill_submits_shared_patch",
        "test_dns_filter_mode_pill_submits_shared_patch",
        "test_domain_mapping_and_filter_mode_controls",
        "test_server_tag_labels_are_localized",
        "dns_settings_patch_maps_onto_domain_patch",
        "unmapped_mapping_mode_clears_the_key",
        "test_clear_enhanced_mode_removes_the_key",
        "test_filter_mode_accepts_rule_host_value",
    )

    # Shared contract: typed modes, switches, server tags and the patch.
    require(
        violations,
        "crates/infiltrator-contract/src/dns.rs",
        "pub enum DnsEnhancedMode",
        "pub enum DnsFakeIpFilterMode",
        "pub struct DnsCoreSwitches",
        "pub enum DnsServerTag",
        "pub struct DnsSettingsPatch",
        "pub fn classify",
        "pub fn toggle",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub enhanced_mode: crate::dns::DnsEnhancedMode",
        "pub switches: crate::dns::DnsCoreSwitches",
        "pub filter_mode: crate::dns::DnsFakeIpFilterMode",
        "pub tags: Vec<crate::dns::DnsServerTag>",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ApplyDnsSettings",
    )

    # Domain: `rule` filter mode + explicit mapping-mode clear.
    require(
        violations,
        "crates/infiltrator-domain/src/dns.rs",
        "pub clear_enhanced_mode: bool",
        'lower != "whitelist" && lower != "blacklist" && lower != "rule"',
    )

    # Shared application: the workbench patch maps onto the validated write.
    require(
        violations,
        "crates/infiltrator-application/src/configuration_application.rs",
        "pub async fn apply_dns_settings",
        "fn dns_patch_from_settings",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "fn build_dns_page",
        "fn dns_core_switches",
        "DnsServerTag::classify",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::ApplyDnsSettings { patch }",
    )

    # Iced consumes the shared typed modes and renders the tag chips.
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns.rs",
        "fn domain_mapping_mode_control",
        "fn filter_mode_control",
        "Message::UpdateDnsFormFilterMode",
        "fn server_tag_label",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/dns_config.rs",
        "clear_enhanced_mode",
        "Message::UpdateDnsFormFilterMode(value)",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table.rs",
        "dns_tag_domestic",
        "dns_tag_fallback",
        "dns_tag_encrypted",
        "dns_tag_plain",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "dns_leak_probe_title",
    )

    # Bevy consumes the same projection and submits the shared patch.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns.rs",
        "pub struct DnsSwitchButton(pub DnsSwitchField, pub bool)",
        "pub struct DnsSwitchTrack",
        "pub struct DnsEnhancedModePill",
        "pub struct DnsFilterModePill",
        "pub struct DnsServerItem",
        "UiCommand::ApplyDnsSettings",
        "pub(crate) fn on_dns_action_activated",
        "pub(crate) fn apply_dns_projection",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ApplyDnsSettings",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "crate::pages::dns::LastDnsProjection",
    )
    # The old fabricated command and local enum must not come back.
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns.rs",
        "dns.enable",
        "dns.respect_rules",
        "enum DnsMode",
    )

    # Dual headless evidence.
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_dns_page_mounting_and_default_state",
        "test_dns_switch_submits_shared_patch",
        "test_dns_enhanced_mode_pill_submits_shared_patch",
        "test_dns_filter_mode_pill_submits_shared_patch",
        "test_dns_projection_in_place_update",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_tests.rs",
        "dns_page_in_place_update",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_dns_tests.rs",
        "test_domain_mapping_and_filter_mode_controls",
        "test_server_tag_labels_are_localized",
    )
    require(
        violations,
        "crates/infiltrator-application/src/configuration_application.rs",
        "dns_settings_patch_maps_onto_domain_patch",
        "unmapped_mapping_mode_clears_the_key",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/dns_test.rs",
        "test_clear_enhanced_mode_removes_the_key",
        "test_filter_mode_accepts_rule_host_value",
    )

    # The guard itself is registered on both suites.
    require(
        violations,
        "scripts/test.sh",
        "dns-studio-guard.py --mode enforce",
    )
    require(
        violations,
        "scripts/test-bevy.sh",
        'dns-studio-guard.py" --mode enforce',
    )

    if violations:
        for violation in violations:
            print(f"dns-studio-guard: {violation}", file=sys.stderr)
        print(f"dns-studio-guard: violations={len(violations)}", file=sys.stderr)
        return 1 if args.mode == "enforce" else 0
    print("dns-studio-guard: DUAL-14 ledger and dual-surface markers=complete")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

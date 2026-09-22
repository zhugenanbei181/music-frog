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
        "FakeIpMappingPool",
        "DnsLatencyStatus",
        "DnsHostEntry",
        "hosts_map_from_entries",
        "test_dns_fake_ip_pool_search_filters_the_observed_listing",
        "test_dns_hosts_editor_submits_shared_patch",
        "test_dns_latency_policy_line_is_honest",
        "test_dns_fake_ip_pool_filters_the_observed_subset",
        "test_dns_hosts_panel_add_remove_rows_uses_the_shared_draft",
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
        "pub fallback_policy: crate::dns::DnsFallbackPolicy",
        "pub cache_flush: crate::dns::DnsCacheFlushReport",
        "pub default_nameserver: Vec<String>",
        "pub fake_ip_pool: crate::dns::FakeIpMappingPool",
        "pub latency: crate::dns::DnsLatencyStatus",
        "pub hosts: Vec<crate::dns::DnsHostEntry>",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "ApplyDnsSettings",
    )

    # DUAL-14-04/05: encrypted upstream transports and the fallback policy are
    # shared contract value objects, not per-surface strings.
    require(
        violations,
        "crates/infiltrator-contract/src/dns.rs",
        "pub enum DnsUpstreamProtocol",
        "pub fn from_address",
        "pub fn parse_server_list",
        "pub fn append_server",
        "pub struct DnsFallbackPolicy",
        "pub geoip_code: String",
        "pub trigger_ipcidr: Vec<String>",
        "pub struct DnsCacheFlushReport",
        "pub struct FakeIpMappingPool",
        "pub struct FakeIpMappingEntry",
        "pub enum FakeIpMappingSource",
        "pub enum DnsLatencyStatus",
        "pub struct DnsHostEntry",
        "pub fn parse_hosts_editor",
        "pub fn hosts_editor_text",
        "pub fn validate_hosts",
        "pub fn is_valid_hosts_address",
        "hosts: Option<Vec<DnsHostEntry>>",
        "pub clear_hosts: bool",
        "pub fn is_observed_subset",
        "fake_ip_pool_filters_observed_bindings",
        "hosts_editor_round_trips_rows_without_ambiguity",
        "hosts_validation_accepts_host_grammar_only",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/dns_form.rs",
        "pub enum DnsFormField",
        "pub const ALL: [Self; 18]",
        "pub struct DnsWorkbenchForm",
        "pub fn from_snapshot",
        "pub fn patch",
        "pub fn validate",
        "pub enum DnsFormIssue",
        "pub fn is_cidr_literal",
    )

    # Domain: `rule` filter mode + explicit mapping-mode/range clears.
    require(
        violations,
        "crates/infiltrator-domain/src/dns.rs",
        "pub clear_enhanced_mode: bool",
        "pub clear_fake_ip_range: bool",
        "pub struct FallbackFilterPatch",
        "pub fallback_filter_partial: Option<FallbackFilterPatch>",
        'lower != "whitelist" && lower != "blacklist" && lower != "rule"',
        "pub hosts: Option<BTreeMap<String, serde_json::Value>>",
        "pub clear_hosts: bool",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/dns_hosts.rs",
        "pub fn hosts_map_from_entries",
        "pub fn hosts_entries_from_map",
    )

    # Shared application: the workbench patch maps onto the validated write.
    require(
        violations,
        "crates/infiltrator-application/src/configuration_application.rs",
        "pub async fn apply_dns_settings",
        "pub async fn apply_dns_settings_with_runtime",
        "pub fn dns_patch_from_settings",
        "FallbackFilter",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "fn build_dns_page",
        "with_dns_cache",
        "cache_flush_report",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_workbench_application.rs",
        "fn dns_page_snapshot",
        "fn dns_core_switches",
        "DnsServerTag::classify",
        "fn cache_flush_report",
        "fn fake_ip_pool_from_connections",
        "fn hosts_entries",
    )
    require(
        violations,
        "crates/infiltrator-application/src/configuration_application.rs",
        "hosts_map_from_entries",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_cache_application.rs",
        "pub async fn flush_all",
        "SystemDnsCachePort",
        "flush_system_cache",
        "DnsFlushOutcome::Unsupported",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "CommandIntent::ApplyDnsSettings { patch }",
        "with_dns_cache",
    )

    # DUAL-14-07: the OS DNS cache refresh is a real host port with a typed
    # unsupported answer, and the desktop adapter carries the real commands.
    require(
        violations,
        "crates/infiltrator-ports/src/system_dns_cache.rs",
        "pub trait SystemDnsCachePort",
        "async fn flush_system_cache",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn system_dns_cache_port",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/system_dns_cache.rs",
        "DesktopSystemDnsCache",
        "flush_invocations",
        "resolvectl",
        "dscacheutil",
        "ipconfig",
    )

    # Iced consumes the shared typed form and renders the tag chips.
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns.rs",
        "fn dns_form_panel",
        "fn server_tag_label",
        "dns_cache_flush_status",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns_form_panel.rs",
        "Message::UpdateDnsFormFilterMode",
        "DnsFormField::ALL",
        "pub(crate) fn dns_form_field_widget",
        "pub(crate) fn localized_form_issue",
        "pub(crate) fn flush_outcome_label",
        "dns_flush_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/dns_config.rs",
        "dns_patch_from_form",
        "dns_patch_from_settings",
        "Message::UpdateDnsFormFallbackGeoip",
        "Message::DnsCacheFlushed",
        "form_from_config",
        "Message::AddDnsHostRow",
        "Message::SaveDnsHosts",
        "Message::DnsHostsSaved",
        "Message::UpdateDnsFakeIpQuery",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns_hosts_panel.rs",
        "pub(crate) fn fake_ip_pool_panel",
        "pub(crate) fn hosts_panel",
        "pub(crate) fn latency_policy_line",
        "dns_fakeip_pool_search",
        "dns_hosts_apply",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/state.rs",
        "DnsWorkbenchForm::from_snapshot",
        "dns_cache_flush",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "dns_form_issues",
        "dns_flush_unsupported",
        "dns_flush_flushed",
        "dns_fakeip_pool_title",
        "dns_hosts_title",
        "dns_latency_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "dns_form_issues",
        "dns_flush_unsupported",
        "dns_flush_flushed",
        "dns_fakeip_pool_title",
        "dns_hosts_title",
        "dns_latency_unsupported",
    )

    # Bevy consumes the same projection and submits the shared patch.
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns.rs",
        "pub struct DnsSwitchButton(pub DnsSwitchField, pub bool)",
        "pub struct DnsEnhancedModePill",
        "pub struct DnsFilterModePill",
        "pub struct DnsServerItem",
        "UiCommand::ApplyDnsSettings",
        "pub(crate) fn on_dns_action_activated",
        "pub(crate) fn apply_dns_projection",
        "fn cache_flush_label",
        "DnsLineKind::CacheFlush",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_edit.rs",
        "pub struct DnsFormState",
        "pub struct DnsEditField",
        "pub struct DnsEditApplyButton",
        "pub struct DnsEditTemplate",
        "pub struct DnsEditGeoipToggle",
        "pub fn dns_edit_card_scene",
        "pub(crate) fn on_dns_edit_activated",
        "pub(crate) fn apply_dns_edit_projection",
        "pub fn sync_dns_edit_dirty",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "ApplyDnsSettings",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_fakeip.rs",
        "pub struct DnsFakeIpSearchField",
        "pub fn fake_ip_mapping_listing",
        "pub fn fake_ip_mapping_count",
        "pub fn latency_policy_label",
        "pub fn sync_dns_fake_ip_filter",
        "pub fn dns_fakeip_pool_card_scene",
        "DnsLineKind::FakeIpMapping",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_hosts.rs",
        "pub struct DnsHostsEditorField",
        "pub struct DnsHostsApplyButton",
        "pub struct DnsHostsEditorState",
        "pub fn hosts_summary_label",
        "pub fn dns_hosts_card_scene",
        "pub(crate) fn on_dns_hosts_activated",
        "pub(crate) fn apply_dns_hosts_projection",
        "UiCommand::ApplyDnsSettings",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/route.rs",
        "crate::pages::dns::LastDnsProjection",
        "crate::pages::dns_edit::DnsFormState",
        "sync_dns_edit_dirty",
        "crate::pages::dns_hosts::DnsHostsEditorState",
        "crate::pages::dns_fakeip::sync_dns_fake_ip_filter",
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
        "test_dns_edit_rows_cover_every_shared_text_field",
        "test_dns_upstream_list_edit_submits_shared_patch",
        "test_dns_fallback_policy_toggle_and_trigger_submit_shared_patch",
        "test_dns_form_local_validation_blocks_invalid_scheme",
        "test_dns_quick_template_chip_appends_unique_entry",
        "test_dns_cache_flush_report_renders_honest_status",
        "test_dns_fake_ip_pool_search_filters_the_observed_listing",
        "test_dns_hosts_editor_submits_shared_patch",
        "test_dns_hosts_editor_local_validation_blocks_bad_rows",
        "test_dns_hosts_editor_clears_an_emptied_mapping",
        "test_dns_latency_policy_line_is_honest",
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
        "test_dns_form_field_widgets_cover_every_shared_field",
        "test_dns_form_validation_issues_localize_in_both_locales",
        "test_dns_cache_flush_outcome_labels_are_localized",
        "test_dns_form_patch_uses_the_shared_workbench_mapping",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_dns_hosts_tests.rs",
        "test_dns_fake_ip_pool_panel_filters_the_observed_subset",
        "test_dns_hosts_panel_add_remove_rows_uses_the_shared_draft",
        "test_dns_hosts_issue_copy_is_localized",
        "test_dns_latency_policy_line_is_localized_and_honest",
    )
    require(
        violations,
        "crates/infiltrator-application/src/configuration_application.rs",
        "dns_settings_patch_maps_onto_domain_patch",
        "unmapped_mapping_mode_clears_the_key",
        "workbench_lists_and_fallback_policy_map_onto_the_domain_patch",
        "an_emptied_range_maps_to_the_explicit_clear_flag",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_cache_application.rs",
        "reports_both_targets_honestly",
        "a_host_without_the_os_adapter_reports_typed_unsupported",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/dns_test.rs",
        "test_clear_enhanced_mode_removes_the_key",
        "test_filter_mode_accepts_rule_host_value",
        "test_clear_fake_ip_range_removes_the_key",
        "test_partial_fallback_filter_preserves_unedited_subfields",
        "test_dns_save_preserves_an_untouched_hosts_map",
        "test_hosts_patch_writes_and_clears_the_key",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_workbench_application.rs",
        "fake_ip_pool_publishes_only_observed_bindings",
        "hosts_entries_project_the_profile_map",
    )
    require(
        violations,
        "crates/infiltrator-domain/src/dns_hosts.rs",
        "scalar_and_list_values_round_trip_losslessly",
        "non_string_shapes_are_skipped_not_fabricated",
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

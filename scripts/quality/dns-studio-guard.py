#!/usr/bin/env python3
"""Fail-closed guard for the group 14 DNS workbench ledger (DUAL-14-01..15).

Group 14's closure standard matches groups 06/07/11/13: one shared
contract/application source, both surfaces, dual headless tests, and an honest
per-item ledger. This guard asserts the per-item ledger rows exist, that the
shared DNS workbench contract and both surface wirings stay present, and that
the Bevy DNS page does not regress to its old fabricated switch command
(``UpdateSetting { key: "dns.*", value: "toggle" }``) or local ``DnsMode`` enum.

DUAL-14-08 additionally pins the honest DNS leak cross-source seam: the host
echo port, the real UDP/DoH/system adapter, the shared cross-source
application and the dual panels must stay, while the old Iced panel's
hardcoded country/ISP/leak-flag values are forbidden from returning. The
default echo sources are real third-party public TXT authorities read with
explicit extraction rules, so the empty-source regression is forbidden too.
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


def forbid_compact(violations: list[str], path: str, *markers: str) -> None:
    """Forbid a marker regardless of how rustfmt wrapped the source line."""
    compact_text = " ".join(read(path).split())
    for marker in markers:
        compact_marker = " ".join(marker.split())
        if compact_marker in compact_text:
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
        "pub latency: crate::dns_latency::DnsLatencyReport",
        "pub self_heal: crate::dns_self_heal::DnsSelfHealSnapshot",
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

    # DUAL-14-10/13: the latency prober and the self-heal observation.
    #
    # The prober is a real host port (UDP + wire-format DoH), the application
    # publishes the measured report into the shared snapshot, and the
    # self-heal snapshot folds the `dns.listen` port observation, the upstream
    # reachability report and the static topology audit. These markers keep
    # the placeholder shape (`latency: DnsLatencyStatus::Unsupported`) and the
    # fabricated per-server numbers from coming back.
    require(
        violations,
        "crates/infiltrator-contract/src/dns_latency.rs",
        "pub enum DnsLatencyStatus",
        "pub enum DnsProbeTransport",
        "pub enum DnsProbeOutcome",
        "pub struct DnsServerLatency",
        "pub struct DnsLatencyReport",
        "pub struct DnsLatencyProbeRequest",
        "pub fn is_upstream_unreachable",
        "pub fn summary",
        "DEFAULT_PROBE_QUESTION",
        "a_default_report_never_claims_a_latency",
        "a_fully_unreachable_upstream_list_is_the_self_heal_trigger",
        "a_not_probed_address_is_not_reported_as_unreachable",
        "probe_requests_keep_the_shared_question_and_a_usable_deadline",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/dns_self_heal.rs",
        "pub enum DnsSelfHealKind",
        "pub enum DnsSelfHealState",
        "pub enum DnsSelfHealFix",
        "pub struct DnsSelfHealCheck",
        "pub struct DnsSelfHealSnapshot",
        "pub fn overall_state",
        "an_empty_snapshot_is_unknown_not_healthy",
        "the_overall_state_is_the_worst_observed_check",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/port_conflict.rs",
        "DnsListen",
        "dns.listen",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/dns_latency.rs",
        "pub trait DnsLatencyProbePort",
        "async fn probe",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn dns_latency_probe_port",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_latency_application.rs",
        "pub struct DnsLatencyApplication",
        "pub async fn probe",
        "NO_PROBER_REASON",
        "impl DnsLatencyProbePort for DnsLatencyApplication",
        "a_host_without_a_prober_reports_typed_unsupported_and_probes_nothing",
        "a_real_probe_publishes_the_measured_results_to_both_readers",
        "an_unreachable_upstream_list_is_published_not_hidden",
        "probing_without_a_configured_nameserver_is_a_typed_input_error",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_self_heal_application.rs",
        "pub fn dns_self_heal_snapshot",
        "pub fn dns_listen_conflict",
        "DnsSelfHealFix::RepairDnsListenPort",
        "DnsSelfHealFix::RecheckUpstreams",
        "validate_dns_topology",
        "an_unobserved_fact_is_unknown_and_never_healthy",
        "a_taken_listen_port_is_critical_and_suggests_the_real_repair",
        "an_unreachable_upstream_list_is_a_critical_self_heal_finding",
        "the_static_topology_audit_drives_its_own_fix",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_workbench_application.rs",
        "fn apply_latency_report",
        "fn latency_report",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_dns_latency",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "fn test_dns_latency",
        "CommandIntent::TestDnsLatency => self.test_dns_latency().await",
    )
    require(
        violations,
        "crates/infiltrator-core/src/dns_wire.rs",
        "pub fn encode_query",
        "pub fn validate_response",
        "pub fn encode_answer",
        "a_wrong_id_is_rejected_before_anything_is_timed",
        "a_query_a_truncated_datagram_and_a_mismatched_question_are_rejected",
    )
    require(
        violations,
        "crates/infiltrator-core/src/dns_latency_io.rs",
        "pub struct HttpDnsLatencyProber",
        "fn plan_for",
        "impl DnsLatencyProbePort for HttpDnsLatencyProber",
        "UdpSocket",
    )
    require(
        violations,
        "crates/infiltrator-core/src/dns_latency_io_test.rs",
        "a_real_udp_nameserver_on_loopback_is_measured",
        "a_wrong_response_id_is_rejected_instead_of_timed",
        "a_silent_nameserver_times_out_without_a_number",
        "an_unreachable_port_is_not_measured_but_is_still_reported",
        "a_doh_endpoint_answers_the_wire_format_query",
        "undrivable_transports_are_reported_instead_of_probed",
        "plan_decoding_covers_both_surface_editor_shapes",
    )
    require(
        violations,
        "crates/mihomo-config/src/port.rs",
        "pub fn split_listen_addr",
        "test_split_listen_addr",
    )
    require(
        violations,
        "crates/mihomo-config/src/manager/defaults.rs",
        "pub async fn ensure_dns_listen_port",
        "update_nested_field",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/port_conflict.rs",
        "fn dns_listen_port",
        "PortBinding::DnsListen",
        "ensure_dns_listen_port",
        "a_bound_dns_listen_port_is_observed_and_relocated",
        "the_dns_listen_address_is_decoded_from_the_profile_document",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "fn dns_latency_probe_port",
        "HttpDnsLatencyProber",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "dns_latency",
        "with_dns_latency",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns_hosts_panel.rs",
        "pub(crate) fn self_heal_panel",
        "pub(crate) fn latency_result_lines",
        "dns_self_heal_title",
        "dns_latency_run",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "RunDnsLatencyProbe",
        "DnsLatencyProbed",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/dns_config.rs",
        "Message::RunDnsLatencyProbe",
        "dns_latency_probe_port",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "dns_latency_all_measured",
        "dns_latency_none_reachable",
        "dns_self_heal_critical",
        "dns_self_heal_fix",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "dns_latency_all_measured",
        "dns_latency_none_reachable",
        "dns_self_heal_critical",
        "dns_self_heal_fix",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_fakeip.rs",
        "pub fn latency_result_listing",
        "pub fn self_heal_listing",
        "pub fn self_heal_state_label",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_self_heal.rs",
        "pub fn dns_self_heal_card_scene",
        "DnsLineKind::SelfHeal",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns.rs",
        "DnsLineKind::SelfHeal",
        "DnsLineKind::LatencyResults",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_dns_self_heal_card_renders_the_shared_observation",
        "test_dns_test_latency_submits_command",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_dns_hosts_tests.rs",
        "test_dns_self_heal_panel_renders_every_observed_check",
    )
    # The old latency placeholder must not come back on either surface.
    forbid(
        violations,
        "crates/infiltrator-application/src/dns_workbench_application.rs",
        "latency: DnsLatencyStatus::Unsupported",
    )
    forbid(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_fakeip.rs",
        "宿主无该事实源，不填充假延迟",
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

    # DUAL-14-08: the DNS leak cross-source probe.
    #
    # A leak conclusion needs a controlled echo authority. The seam is real
    # (host echo port + real UDP/DoH/system adapter + shared cross-source
    # application + one report on both surfaces); the old Iced panel's
    # hardcoded country/ISP values are gone and must not return. The ledger
    # row states the boundary: shared-ready, because no controlled echo zone
    # is configured by default.
    require(
        violations,
        LEDGER,
        "DnsLeakEchoPort",
        "DnsLeakProbePort",
        "DnsLeakApplication",
        "DnsLeakConclusion",
        "DnsLeakEchoRecord",
        "HttpDnsLeakEchoProbe",
        "default_echo_sources",
        "whoami.ds.akahelp.net",
        "o-o.myaddr.l.google.com",
        "TxtFirstIpAddress",
        "TxtKeyedValue",
        "test_dns_leak_card_renders_the_shared_cross_source_report",
        "test_dns_test_leak_submits_command",
        "test_dns_leak_panel_renders_the_shared_cross_source_report",
        "DUAL-14-08",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/dns_leak.rs",
        "pub enum DnsLeakStatus",
        "pub struct DnsLeakProbeSource",
        "pub enum DnsLeakProbeTransport",
        "pub enum DnsLeakObservationOutcome",
        "pub struct DnsLeakObservation",
        "pub struct DnsLeakObservedFact",
        "pub enum DnsLeakConclusion",
        "pub struct DnsLeakReport",
        "pub struct DnsLeakEchoProbe",
        "pub struct DnsLeakEchoRequest",
        "pub struct DnsLeakEchoReport",
        "pub enum DnsLeakEchoRecord",
        "pub enum DnsLeakProbeName",
        "pub fn observed_facts",
        "pub fn conclusion",
        "a_single_observation_is_unknown_not_consistent",
        "disagreeing_authorities_are_divergent_and_list_every_fact",
        "an_all_failed_probe_is_failed_not_unknown",
        "echo_record_rules_are_declared_and_round_trip",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/surface_snapshot.rs",
        "pub leak: crate::dns_leak::DnsLeakReport",
    )
    require(
        violations,
        "crates/infiltrator-contract/src/command.rs",
        "TestDnsLeak",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/dns_leak.rs",
        "pub trait DnsLeakEchoPort",
        "async fn observe",
        "pub trait DnsLeakProbePort",
        "async fn probe",
    )
    require(
        violations,
        "crates/infiltrator-ports/src/host_runtime.rs",
        "fn dns_leak_probe_port",
    )
    require(
        violations,
        "crates/infiltrator-core/src/dns_wire.rs",
        "pub fn response_rcode",
        "pub fn extract_a_record",
        "pub const TYPE_TXT",
        "pub fn extract_txt_records",
        "pub fn encode_txt_answer",
        "the_echo_observation_is_read_from_the_real_answer_record",
        "an_answer_without_an_a_record_yields_nothing_instead_of_a_guess",
        "txt_character_strings_are_read_in_record_order",
        "a_txt_answer_with_no_txt_record_is_none_not_a_guess",
    )
    require(
        violations,
        "crates/infiltrator-core/src/dns_leak_io.rs",
        "pub struct HttpDnsLeakEchoProbe",
        "impl DnsLeakEchoPort for HttpDnsLeakEchoProbe",
        "fn read_observation",
        "fn extract_identity",
        "fn first_ip_txt_value",
        "fn keyed_txt_value",
        "fn system_nameservers",
        "fn parse_resolv_conf",
        "UdpSocket",
    )
    require(
        violations,
        "crates/infiltrator-core/src/dns_leak_io_test.rs",
        "a_real_udp_echo_authority_reports_the_resolver_it_observed",
        "an_answer_without_an_a_record_is_not_an_identity",
        "a_nonzero_rcode_is_reported_instead_of_read_as_an_identity",
        "a_silent_echo_authority_times_out",
        "a_doh_echo_endpoint_answers_the_wire_format_query",
        "undrivable_resolvers_are_reported_instead_of_probed",
        "the_platform_resolver_path_reports_its_real_outcome",
        "a_real_udp_txt_authority_reports_the_keyed_resolver_value",
        "a_real_udp_txt_authority_reports_the_first_ip_value",
        "an_ambiguous_txt_key_is_not_an_identity",
        "resolv_conf_nameserver_lines_are_parsed_without_guessing",
        "live_public_txt_echo_authorities_are_observed",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_leak_application.rs",
        "pub struct DnsLeakApplication",
        "impl DnsLeakProbePort for DnsLeakApplication",
        "pub fn random_probe_question",
        "pub fn probe_question",
        "pub fn default_echo_sources",
        "whoami.ds.akahelp.net",
        "o-o.myaddr.l.google.com",
        "NO_ECHO_PORT_REASON",
        "NO_ECHO_SOURCE_REASON",
        "a_host_without_an_echo_prober_reports_typed_unsupported_and_probes_nothing",
        "a_host_without_a_configured_authority_never_claims_a_verdict",
        "one_probe_generates_fresh_subdomains_and_publishes_divergent_facts",
        "agreeing_authorities_are_consistent_and_a_silent_one_is_still_listed",
        "a_prober_that_omits_an_observation_makes_that_source_fail_honestly",
        "an_exact_authority_source_asks_the_fixed_name_and_carries_its_record",
        "default_echo_sources_are_two_real_public_txt_authorities",
    )
    require(
        violations,
        "crates/infiltrator-application/src/dns_workbench_application.rs",
        "fn leak_report",
        "a_host_without_a_leak_fact_source_keeps_the_typed_unsupported_state",
    )
    require(
        violations,
        "crates/infiltrator-application/src/surface_reader.rs",
        "with_dns_leak",
    )
    require(
        violations,
        "crates/infiltrator-application/src/command_application.rs",
        "fn test_dns_leak",
        "CommandIntent::TestDnsLeak => self.test_dns_leak().await",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/update/core/dns_leak.rs",
        "Message::RunDnsLeakProbe",
        "Message::DnsLeakProbed",
        "dns_leak_probe_port",
        "apply_dns_leak_snapshot",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns_leak_panel.rs",
        "pub(crate) fn leak_panel",
        "pub(crate) fn leak_conclusion_copy",
        "pub(crate) fn leak_observation_lines",
        "dns_leak_divergent",
        "dns_leak_unsupported",
        "dns_leak_observed",
    )
    require(
        violations,
        "crates/infiltrator-iced/src/view/dns.rs",
        "dns_leak_panel",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "dns_leak_consistent",
        "dns_leak_divergent",
        "dns_leak_unsupported",
        "dns_leak_observed",
        "dns_leak_sources",
    )
    require(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "dns_leak_consistent",
        "dns_leak_divergent",
        "dns_leak_unsupported",
        "dns_leak_observed",
        "dns_leak_sources",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "fn dns_leak_probe_port",
        "HttpDnsLeakEchoProbe",
        "default_echo_sources",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/boot.rs",
        "HttpDnsLeakEchoProbe",
        "default_echo_sources",
    )
    require(
        violations,
        "crates/infiltrator-desktop/src/surface.rs",
        "dns_leak",
        "with_dns_leak",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_leak.rs",
        "pub fn leak_conclusion_label",
        "pub fn leak_observation_listing",
        "pub fn dns_leak_card_scene",
        "pub fn sync_dns_leak_line",
        "DnsLineKind::Leak",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns.rs",
        "TestDnsLeakButton",
        "UiCommand::TestDnsLeak",
        "DnsLineKind::Leak",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/command.rs",
        "TestDnsLeak",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/tests/headless/pages_matrix_b_tests.rs",
        "test_dns_test_leak_submits_command",
        "test_dns_leak_card_renders_the_shared_cross_source_report",
    )
    require(
        violations,
        "crates/infiltrator-bevy-ui/src/pages/dns_leak.rs",
        "the_card_renders_the_configured_txt_sources",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/view_dns_leak_tests.rs",
        "test_dns_leak_panel_renders_the_shared_cross_source_report",
        "test_dns_leak_observation_outcome_copy_stays_typed",
        "test_dns_leak_panel_renders_the_configured_txt_sources",
    )
    require(
        violations,
        "crates/infiltrator-iced/tests/gui/iced_six_advancements_wave2_tests.rs",
        "test_advancement_w2_1_dns_leak_cross_source_probe_lifecycle",
    )
    # The hardcoded leak verdict must not come back on either surface: no
    # invented country / ISP / leak flag, and no fabricated public IP.
    forbid(
        violations,
        "crates/infiltrator-iced/src/update/ui.rs",
        '"US".to_string()',
        "Cloudflare",
        "104.28.19.42",
        "is_leak_detected",
        "tested_dns_servers",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/dns.rs",
        "pub country: String",
        "pub isp: String",
        "pub is_leak_detected: bool",
        "pub tested_dns_servers: Vec<String>",
        "DnsLeakReport",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/dns.rs",
        "rep.country",
        "rep.isp",
        "is_leak_detected",
        "dns_leak_probe",
        "DnsLeakProbeFinished",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/view/dns_leak_panel.rs",
        ".country",
        ".isp",
        "dns_leak_status_leaked",
        "dns_leak_status_secure",
    )
    forbid(
        violations,
        "crates/infiltrator-iced/src/types/message.rs",
        "DnsLeakProbeFinished",
    )
    forbid(
        violations,
        "crates/infiltrator-shared/src/locales_table_ext.rs",
        "dns_leak_status_leaked",
        "dns_leak_status_secure",
        "dns_leak_public_ip",
        "dns_leak_location",
        "dns_leak_isp",
        "dns_leak_tested_servers",
    )
    forbid(
        violations,
        "crates/infiltrator-shared/src/locales_table_en_ext.rs",
        "dns_leak_status_leaked",
        "dns_leak_status_secure",
        "dns_leak_public_ip",
        "dns_leak_location",
        "dns_leak_isp",
        "dns_leak_tested_servers",
    )
    # The empty-source regression must not come back: the desktop host ships
    # real default TXT echo authorities, not `Vec::new()`.
    forbid_compact(
        violations,
        "crates/infiltrator-desktop/src/runtime.rs",
        "HttpDnsLeakEchoProbe::new(), )), Vec::new(),",
    )
    forbid_compact(
        violations,
        "crates/infiltrator-desktop/src/boot.rs",
        "HttpDnsLeakEchoProbe::new(), )), Vec::new(),",
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

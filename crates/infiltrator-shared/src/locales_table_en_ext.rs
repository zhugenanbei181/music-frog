//! Extended en-US copy table backing [`super::Localizer`].
//! Modular table split to ensure strict conformance with the 800-line source budget.

use std::borrow::Cow;

pub(super) fn translate_en_ext(key: &str) -> Cow<'static, str> {
    match key {
        // Category 1: DNS Leak & Privacy Probe
        "dns_leak_probe_title" => "DNS Leak & IP Privacy Probe".into(),
        "dns_leak_probe_desc" => "Concurrently test public outbound IP, geo location and ISP, detecting DNS leaks.".into(),
        "dns_leak_btn_run" => "Run Privacy Probe".into(),
        "dns_leak_status_secure" => "Secure: No DNS or IP leak detected".into(),
        "dns_leak_status_leaked" => "Warning: Potential DNS leak detected".into(),
        "dns_leak_public_ip" => "Public Outbound IP".into(),
        "dns_leak_location" => "Geo Location".into(),
        "dns_leak_isp" => "ISP / ASN".into(),
        "dns_leak_tested_servers" => "Resolved DNS Servers".into(),

        // Category 2: Custom Node Editor & Universal URI Codec
        "custom_node_title" => "Custom Node Editor & URI Codec".into(),
        "custom_node_btn_add" => "Add Custom Node".into(),
        "custom_node_btn_import_uri" => "Import from URI".into(),
        "custom_node_btn_export_uri" => "Export as URI".into(),
        "custom_node_type" => "Protocol Type".into(),
        "custom_node_name" => "Node Name".into(),
        "custom_node_server" => "Server Address".into(),
        "custom_node_port" => "Port".into(),
        "custom_node_uuid_pass" => "UUID / Password".into(),
        "custom_node_sni" => "TLS / Reality SNI".into(),
        "custom_node_uri_placeholder" => "Paste vless://, ss://, hysteria2://, trojan:// share link...".into(),

        // Category 3: Multi-Profile Aggregator & Topology Generator
        "aggregator_title" => "Multi-Profile Aggregator".into(),
        "aggregator_desc" => "Select multiple profiles to deduplicate and generate regional auto-select groups.".into(),
        "aggregator_btn_merge" => "Merge Profiles".into(),
        "aggregator_name_placeholder" => "Aggregated Profile Name (e.g., Aggregated-All)".into(),
        "aggregator_selected_count" => "{count} profiles selected".into(),
        "aggregator_result_nodes" => "{count} valid nodes merged".into(),

        // Category 4: Connection Grouping & Quick-Rule Generator
        "conn_grouping_mode" => "Connection Grouping".into(),
        "conn_group_flat" => "Flat Stream".into(),
        "conn_group_process" => "By Process".into(),
        "conn_group_host" => "By Domain".into(),
        "quick_rule_btn" => "Add Route Rule".into(),
        "quick_rule_success" => "Route rule added successfully".into(),

        // Category 5: Config Snapshot Visual Diff & Rollback
        "snapshot_diff_title" => "Snapshot Visual Diff & Rollback".into(),
        "snapshot_diff_compare_with" => "Compare Snapshot".into(),
        "snapshot_diff_rollback_btn" => "Rollback to this Version".into(),
        "snapshot_diff_no_changes" => "Identical: No diff between current and snapshot".into(),

        // Category 6: Global Hotkey Manager & Keybinding Customizer
        "hotkey_manager_title" => "Global Hotkey Manager".into(),
        "hotkey_manager_desc" => "Manage system-wide global hotkeys for instant proxy control while in background.".into(),
        "hotkey_system_proxy" => "Toggle System Proxy".into(),
        "hotkey_tun_mode" => "Toggle TUN Mode".into(),
        "hotkey_mini_hud" => "Toggle Mini Speed HUD".into(),
        "hotkey_speed_test" => "Trigger Speed Test".into(),
        "hotkey_mode_switch" => "Cycle Proxy Modes".into(),
        "hotkey_conflict_warning" => "Shortcut conflict: Key combination already in use".into(),

        // Category 1: PCAP Exporter & Sniffer
        "pcap_title" => "PCAP Capture & Sniffer Inspector".into(),
        "pcap_btn_start" => "Start Capture".into(),
        "pcap_btn_stop" => "Stop Capture".into(),
        "pcap_btn_export" => "Export .pcap".into(),
        "pcap_capturing" => "Capturing ({count} packets / {bytes})".into(),
        "pcap_idle" => "Packet capture is idle".into(),

        // Category 2: Logical Sub-Rules Builder
        "subrules_title" => "Logical Sub-Rules Visual Builder".into(),
        "subrules_operator" => "Logical Operator (AND / OR / NOT)".into(),
        "subrules_btn_add_leaf" => "Add Condition".into(),
        "subrules_target" => "Target Policy".into(),
        "subrules_result_preview" => "Rule Expression Preview".into(),
        "subrules_btn_insert" => "Insert Rule".into(),

        // Category 3: Speedtest & Jitter Benchmark
        "speedtest_title" => "Node Bandwidth & Jitter Benchmark".into(),
        "speedtest_btn_start" => "Start Speedtest".into(),
        "speedtest_measuring" => "Testing...".into(),
        "speedtest_bandwidth" => "Downlink Bandwidth".into(),
        "speedtest_jitter" => "Latency Jitter".into(),
        "speedtest_packet_loss" => "Packet Loss".into(),
        "speedtest_stability" => "Stability Tier".into(),

        // Category 4: Geo Database Updater
        "geodata_title" => "GeoIP / GeoSite Database Manager".into(),
        "geodata_btn_check" => "Check Updates".into(),
        "geodata_btn_update" => "Update Databases".into(),
        "geodata_geoip_status" => "GeoIP Database".into(),
        "geodata_geosite_status" => "GeoSite Database".into(),
        "geodata_updated" => "Up to Date".into(),
        "geodata_updating" => "Updating databases...".into(),

        // Category 5: UWP Loopback Utility
        "uwp_title" => "Windows UWP Loopback Exemption Manager".into(),
        "uwp_desc" => "Exempt Microsoft Store apps from local loopback proxy isolation.".into(),
        "uwp_btn_scan" => "Scan UWP Apps".into(),
        "uwp_btn_exempt_all" => "Exempt All".into(),
        "uwp_btn_clear_all" => "Clear All".into(),
        "uwp_search" => "Search UWP app name or PackageId...".into(),
        "uwp_exempted_count" => "{count} apps exempted".into(),

        // Category 6: Encrypted Backup (.encpkg)
        "encpkg_title" => "Encrypted Backup Package (.encpkg)".into(),
        "encpkg_desc" => "AES-256 password-encrypted backup package for profiles, rules, and mixin.".into(),
        "encpkg_pass_placeholder" => "Backup passphrase (min 6 chars)...".into(),
        "encpkg_btn_export" => "Export Encrypted Package".into(),
        "encpkg_btn_import" => "Import Encrypted Package".into(),
        "encpkg_success" => "Encrypted package processed successfully".into(),
        // Category 1: Network Interface Roaming & Gateway Recovery
        "net_roam_title" => "Network Interface Roaming & Gateway Recovery".into(),
        "net_roam_desc" => "Detect network roaming, auto-adapt optimal MTU and self-heal routing tables.".into(),
        "net_roam_btn_reconnect" => "Force Gateway Recovery".into(),
        "net_roam_active_iface" => "Active Outbound Interface".into(),
        "net_roam_gateway" => "Default Gateway IP".into(),
        "net_roam_mtu" => "Optimal Adaptive MTU".into(),

        // Category 2: Crash Watchdog & Forensic Viewer
        "crash_watchdog_title" => "Crash Watchdog & Forensics Viewer".into(),
        "crash_watchdog_desc" => "Monitor abnormal exits, inspect sanitized backtraces and recover orphaned state.".into(),
        "crash_watchdog_btn_recover" => "Recover Network & Orphaned State".into(),
        "crash_watchdog_btn_export" => "Export Redacted Diagnostics".into(),
        "crash_watchdog_clean" => "System healthy: No abnormal exit or orphaned state detected.".into(),
        "crash_watchdog_recovered" => "Orphaned state recovered and network restored successfully.".into(),

        // Category 3: External Web Dashboard
        "web_dash_title" => "External Web Dashboard".into(),
        "web_dash_desc" => "Launch Metacubexd, Yacd, or Razord with auto-authenticated token handshake.".into(),
        "web_dash_btn_metacubexd" => "Open Metacubexd".into(),
        "web_dash_btn_yacd" => "Open Yacd".into(),
        "web_dash_btn_razord" => "Open Razord".into(),

        // Category 4: Log Regex Highlighting & Redacted Export
        "logs_regex_placeholder" => "Regex filter (e.g. connect|error|dns)...".into(),
        "logs_btn_export_redacted" => "Export Redacted Logs".into(),
        "logs_level_all" => "All".into(),
        "logs_export_success" => "Redacted logs exported to local file".into(),

        // Category 5: Subscription Quota & Cron Scheduler
        "sub_quota_title" => "Subscription Quota & Expiry Alert".into(),
        "sub_quota_desc" => "Monitor bandwidth quota, remaining days, and customize update scheduler.".into(),
        "sub_quota_used" => "Bandwidth Used".into(),
        "sub_quota_remaining" => "Remaining Quota".into(),
        "sub_quota_expire" => "Expires At".into(),
        "sub_quota_cron" => "Update Scheduler Interval".into(),

        // Category 6: PAC Auto-Proxy & Bypass CIDR Manager
        "pac_title" => "PAC Auto-Proxy & Bypass CIDR Manager".into(),
        "pac_desc" => "Generate universal PAC script and configure local subnet bypass rules.".into(),
        "pac_url_label" => "Local PAC Service URL".into(),
        "pac_bypass_cidrs" => "Bypass CIDR list (comma or semicolon separated)".into(),
        "pac_btn_compile" => "Compile & Validate PAC".into(),
        "pac_compile_success" => "PAC script compiled and loaded successfully".into(),
        // Wave 5 Category 1: Rule Hit Counter & Stale Rule Analyzer
        "rule_hit_title" => "Rule Hit Counter & Stale Rule Audit".into(),
        "rule_hit_desc" => "Track hit counts per rule in current session and audit zero-hit stale rules.".into(),
        "rule_hit_btn_audit" => "Audit Stale Rules".into(),
        "rule_hit_btn_clean" => "Disable 0-Hit Rules".into(),
        "rule_hit_total_hits" => "Total Rule Hits".into(),
        "rule_hit_stale_count" => "{count} stale rules detected".into(),

        // Wave 5 Category 2: Latency Time-Series & Stability Radar
        "latency_radar_title" => "Latency Time-Series & Stability Radar".into(),
        "latency_radar_desc" => "Multi-point time-series sampling analyzing RTT fluctuations, jitter and stability tier.".into(),
        "latency_radar_avg" => "Avg RTT".into(),
        "latency_radar_min_max" => "RTT Range (Min/Max)".into(),
        "latency_radar_score" => "Stability Score".into(),

        // Wave 5 Category 3: TUN Multi-Stack & MTU Negotiator
        "tun_stack_title" => "TUN Multi-Stack & MTU Negotiator".into(),
        "tun_stack_desc" => "Select kernel or userspace network stack and negotiate optimal physical MTU.".into(),
        "tun_stack_gvisor" => "gVisor (Userspace Sandbox)".into(),
        "tun_stack_system" => "System (Native Kernel)".into(),
        "tun_stack_mixed" => "Mixed (Hybrid Mode)".into(),
        "tun_mtu_probe_btn" => "Probe Optimal MTU".into(),

        // Wave 5 Category 4: Rule-Provider Lifecycle & Rule Unpacker
        "provider_unpack_title" => "Rule-Provider Unpacker & Local Extraction".into(),
        "provider_unpack_desc" => "Unpack and extract remote Rule-Provider items into local editable rules.".into(),
        "provider_btn_unpack" => "Unpack to Custom Rules".into(),
        "provider_btn_purge_cache" => "Purge Provider Cache".into(),
        "provider_cache_purged" => "Provider local cache purged successfully".into(),

        // Wave 5 Category 5: Config Apply Multi-Stage Transaction Guard
        "apply_guard_title" => "Atomic Config Apply & Rollback Guard".into(),
        "apply_guard_desc" => "Preflight -> Stage -> Reload -> Health Probe -> Atomic Rollback on Failure.".into(),
        "apply_guard_stage_preflight" => "Syntax Preflight".into(),
        "apply_guard_stage_reloading" => "Core Reloading".into(),
        "apply_guard_stage_probing" => "Health Probing".into(),
        "apply_guard_status_committed" => "Transaction Committed".into(),
        "apply_guard_status_rolled_back" => "Health probe failed, rolled back safely".into(),

        // Wave 5 Category 6: LAN Proxy Sharing & Client Access Whitelist
        "lan_sharing_title" => "LAN Proxy Sharing & Access Control ACL".into(),
        "lan_sharing_desc" => "Allow LAN devices to share proxy connection with strict IP/CIDR access control.".into(),
        "lan_sharing_enable" => "Enable LAN Sharing (Allow LAN)".into(),
        "lan_sharing_port" => "LAN Mixed Proxy Port".into(),
        "lan_sharing_acl" => "Allowed Client IP Whitelist (CIDR)".into(),
        _ => key.to_string().into(),
    }
}

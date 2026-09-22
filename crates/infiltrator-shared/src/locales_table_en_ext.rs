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

        // Category 1b: DNS workbench form parity & cache flush (DUAL-14)
        "dns_form_issues" => "Form validation failed".into(),
        "dns_form_err_scheme" => "Unsupported upstream scheme in {field}: {entry}".into(),
        "dns_form_err_bootstrap" => "Bootstrap resolver must be a pure IP: {entry}".into(),
        "dns_form_err_cidr" => "Fallback trigger is not a valid CIDR network: {entry}".into(),
        "dns_form_err_geoip_code" => "geoip-code must be a 2-letter country code: {value}".into(),
        "dns_flush_target_fakeip" => "Fake-IP cache".into(),
        "dns_flush_target_os" => "OS DNS cache".into(),
        "dns_flush_not_requested" => "not run yet".into(),
        "dns_flush_flushed" => "flushed".into(),
        "dns_flush_unsupported" => "unsupported by this host".into(),
        "dns_flush_failed" => "flush failed".into(),

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
        "conn_aggregate_count" => "{count} connections".into(),
        "conn_aggregate_empty" => "No aggregates".into(),
        "conn_close_filtered_btn" => "Close Filtered".into(),
        "quick_rule_btn" => "Add Route Rule".into(),
        "quick_rule_success" => "Route rule added successfully".into(),
        "conn_idle_timeout_label" => "Idle Timeout".into(),
        "conn_idle_sweep_btn" => "Sweep Idle".into(),
        "conn_idle_last_sweep_none" => "Last sweep: not run yet".into(),
        "conn_idle_last_sweep" => "Last sweep: {count} idle connection(s)".into(),

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
        "speedtest_cancel" => "Cancel Speedtest".into(),
        "speedtest_dead_archive" => "Timed-out / unreachable archive".into(),
        "speedtest_history_title" => "Speedtest History".into(),
        "speedtest_history_empty" => "No history yet".into(),
        "speedtest_scope_all_groups" => "All nodes".into(),
        "speedtest_scope_group" => "Group".into(),
        "speedtest_scope_node" => "Node".into(),
        "speedtest_history_alive" => "Alive".into(),
        "speedtest_history_latency" => "Avg latency".into(),
        "speedtest_history_bandwidth" => "Avg bandwidth".into(),
        "speedtest_history_jitter" => "Avg jitter".into(),
        "speedtest_target_url_label" => "Speedtest target URL".into(),
        "speedtest_target_url_placeholder" => "Blank uses the shared engine default".into(),
        "speedtest_concurrency_label" => "Concurrency".into(),
        "speedtest_detail_open" => "Result details".into(),
        "speedtest_detail_title" => "Speedtest result details".into(),
        "speedtest_detail_empty" => "No speedtest results yet".into(),
        "speedtest_detail_failed" => "Speedtest failed".into(),
        "speedtest_detail_egress" => "Egress".into(),
        "speedtest_detail_delay" => "Delay".into(),
        "speedtest_detail_stars" => "Stars".into(),
        "speedtest_detail_match" => "Label matches".into(),
        "speedtest_detail_mismatch" => "Label mismatch".into(),
        "speedtest_detail_unlabelled" => "No label country".into(),
        "speedtest_detail_unknown" => "Egress not probed".into(),

        // Category 4: Geo Database Updater
        "geodata_title" => "GeoIP / GeoSite Database Manager".into(),
        "geodata_btn_check" => "Check Updates".into(),
        "geodata_btn_update" => "Update Databases".into(),
        "geodata_geoip_status" => "GeoIP Database".into(),
        "geodata_geosite_status" => "GeoSite Database".into(),
        "geodata_updated" => "Up to Date".into(),
        "geodata_updating" => "Updating databases...".into(),
        "geodata_version_unknown" => "Unknown".into(),
        "geodata_check_unavailable" => "The core exposes no geo version query; the current version cannot be verified.".into(),
        "geodata_unsupported_host" => "Geo database update is not available on this host".into(),
        "geodata_update_triggered" => "Geo database update triggered; the core downloads in the background".into(),

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
        "net_roam_status_unknown" => "Not probed".into(),
        "net_roam_status_stable" => "Link stable".into(),
        "net_roam_status_recovering" => "Repairing TUN routes".into(),
        "net_roam_status_degraded" => "Degraded".into(),
        "net_roam_status_unsupported" => "Host unsupported".into(),
        "net_roam_status_failed" => "Repair failed".into(),
        "net_roam_event_initial" => "Observed".into(),
        "net_roam_event_gateway_changed" => "Gateway changed".into(),
        "net_roam_event_address_changed" => "Address changed".into(),
        "net_roam_event_routes_repaired" => "Routes repaired".into(),
        "net_roam_event_repair_skipped" => "Repair skipped".into(),
        "net_roam_event_repair_failed" => "Repair failed".into(),
        "net_roam_active_badge" => "Active".into(),

        // Category 7: Android VpnService
        "vpn_card_title" => "Android VpnService & Foreground Keepalive".into(),
        "vpn_card_desc" => "Protect mobile traffic with system VPN consent, a foreground service, and a TUN tunnel.".into(),
        "vpn_start_active" => "VPN started".into(),
        "vpn_start" => "Start VPN".into(),
        "vpn_stop" => "Stop VPN".into(),
        "vpn_stopped" => "Stopped".into(),
        "vpn_status_idle" => "Not started".into(),
        "vpn_status_permission" => "Waiting for consent".into(),
        "vpn_status_starting" => "Starting foreground service".into(),
        "vpn_status_running" => "Running".into(),
        "vpn_status_stopping" => "Stopping".into(),
        "vpn_status_stopped" => "Stopped".into(),
        "vpn_status_revoked" => "Authorization revoked".into(),
        "vpn_status_unsupported" => "Host unsupported".into(),
        "vpn_status_failed" => "Failed".into(),

        // Category 8: Privileged network regression
        "privileged_network_title" => "Privileged Network Headless Regression".into(),
        "privileged_network_desc" => "Inject, read back, and clean up host privilege adapters; verify rollback on failure.".into(),
        "privileged_network_run" => "Run Regression".into(),
        "privileged_network_status_idle" => "Not run".into(),
        "privileged_network_status_injecting" => "Injecting".into(),
        "privileged_network_status_active" => "Injected".into(),
        "privileged_network_status_rolling_back" => "Rolling back".into(),
        "privileged_network_status_cleaned" => "Cleaned".into(),
        "privileged_network_status_unsupported" => "Host unsupported".into(),
        "privileged_network_status_failed" => "Failed".into(),

        "uwp_found_count" => "{count} UWP AppContainers found".into(),

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
        "rule_hit_btn_clear" => "Clear Hit Counters".into(),
        "rule_hit_dead_count" => "Dead & Shadowed Rules".into(),
        "rule_hit_cidr_conflicts" => "CIDR Mask Overlaps".into(),
        "rule_hit_match_latency" => "Avg Match Latency".into(),
        "rule_hit_last_hit" => "Most Recent Hit".into(),
        "rule_hit_none" => "No hit data yet".into(),

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
        "lan_sharing_bind" => "LAN Bind Address".into(),
        "lan_sharing_apply" => "Apply LAN Listener Settings".into(),
        "lan_sharing_acl" => "Allowed Client IP Whitelist (CIDR)".into(),
        "lan_security_title" => "LAN ACL & HTTP Basic Authentication".into(),
        "lan_security_desc" => "Allow only whitelisted networks and optionally require credentials for HTTP/SOCKS/Mixed proxy access.".into(),
        "lan_security_allowed" => "Allowed Networks (CIDR)".into(),
        "lan_security_disallowed" => "Denied Networks (CIDR)".into(),
        "lan_security_skip_auth" => "Authentication Bypass Networks (CIDR)".into(),
        "lan_security_auth" => "HTTP Basic Authentication".into(),
        "lan_security_username" => "Username".into(),
        "lan_security_password" => "Password".into(),
        "lan_security_enabled" => "Enabled".into(),
        "lan_security_disabled" => "Disabled".into(),
        "lan_security_apply" => "Apply ACL & Authentication".into(),
        "settings_ipv6_routing" => "Allow IPv6 Kernel Traffic".into(),
        "settings_ipv6_routing_desc" => "When disabled, Mihomo rejects IPv6 traffic to reduce dual-stack bypass leakage.".into(),
        "common_copy" => "Copy".into(),
        "overview_current_ip" => "Current Outbound IP".into(),
        "overview_scale_max" => "Max".into(),
        "tracer_src_ip_label" => "Source IP:".into(),
        "tracer_src_ip_placeholder" => "Simulated source IP (e.g. 192.168.1.100)".into(),
        "tracer_override_label" => "Reverse-apply outbound (edit this rule)".into(),
        "tracer_override_placeholder" => {
            "Outbound target or group name (e.g. PROXY / DIRECT / REJECT)".into()
        }
        "tracer_override_apply" => "Apply to This Rule".into(),
        "profiles_user_agent_placeholder" => {
            "User-Agent (e.g. Clash.Meta / ClashVerge / Shadowrocket)".into()
        }
        "profiles_insecure_skip_verify" => "Skip TLS Certificate Verification (Insecure)".into(),
        "profiles_insecure_skip_verify_hint" => {
            "Applies to this subscription only. Skipping certificate checks lowers security; use it only with self-signed providers.".into()
        }
        "profiles_cron_placeholder" => "Cron expression (e.g. 0 */6 * * *, empty = hourly)".into(),
        "profiles_cron_hint" => {
            "5-field UTC cron (min hour day month weekday); @daily / @hourly macros supported; takes precedence over the hourly interval".into()
        }
        "profiles_conditional_request" => "Conditional request cached".into(),
        "profiles_conditional_request_empty" => {
            "Conditional request: no ETag / Last-Modified cached yet".into()
        }
        "sub_update_not_modified" => "Subscription unchanged (304 Not Modified)".into(),
        "sub_update_quota_warning" => "quota or expiry warning".into(),
        "profiles_update_all" => "Update All Subscriptions".into(),
        "profiles_backup_available" => {
            "Safe backup ready: a .bak is written when a subscription config is saved".into()
        }
        "profiles_backup_none" => "Safe backup: none yet (created when a config is saved)".into(),
        "profiles_restore_backup" => "Restore Safe Backup".into(),
        "profiles_backup_restored" => "Restored the config from before the last write".into(),
        "profiles_backup_missing" => "No safe backup is available to restore".into(),
        "mrs_accel_title" => "MRS Binary Acceleration".into(),
        "mrs_accel_ready" => "Acceleration ready".into(),
        "mrs_accel_empty" => "No binary rule sets declared in this profile".into(),
        "mrs_accel_unsupported" => "Unsupported".into(),
        "mrs_accel_failed" => "Acceleration failed".into(),
        "mrs_accel_unavailable" => "Unavailable".into(),
        "mrs_accel_providers" => "rule sets".into(),
        "mrs_accel_rules" => "rules".into(),
        "mrs_accel_memory_saved" => "memory saved".into(),
        "mrs_accel_mmap_on" => "mmap enabled".into(),
        "mrs_accel_mmap_off" => "mmap disabled".into(),
        "mrs_accel_valid" => "valid".into(),
        "mrs_accel_invalid" => "invalid".into(),
        "mrs_accel_no_digest" => "no digest".into(),
        _ => key.to_string().into(),
    }
}

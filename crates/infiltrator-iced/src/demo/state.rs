//! Demo-mode [`AppState`] constructor: seeds every page with the fixture
//! tables so the full UI renders without any runtime or side effects.

use super::DemoEnv;
use super::fixtures::{
    demo_connections, demo_logs, demo_profiles, demo_rules, demo_traffic_history, dns_json_fixture,
    fake_ip_json_fixture, profile_yaml_fixture, proxy_providers_json_fixture,
    rule_providers_json_fixture, sniffer_json_fixture, tun_json_fixture,
};
use super::proxy_fixtures::demo_proxy_tables;
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use mihomo_api::types::{MemoryData, ProxyProvider, RuleProvider};
use mihomo_version::manager::VersionInfo;
use std::path::PathBuf;

impl AppState {
    /// Fully populated demo state: every page renders with realistic data and
    /// no runtime is created (`AppState::runtime` stays `None`), so every
    /// network/runtime guard in the update paths no-ops naturally.
    ///
    /// Public so the `tests/headless` integration harnesses can drive the
    /// demo fixture through the crate's public API.
    pub fn demo(env: &DemoEnv) -> (Self, iced::Task<Message>) {
        let mut state = Self::empty();
        // demo-mode: mark the session before anything else can touch a system.
        state.shell.demo = true;
        state.shell.capture_marker = env.capture_marker.clone();
        state.shell.current_route = env.page;
        state.shell.theme = env.skin.clone();
        state.shell.lang = env.lang.clone();

        // ---- runtime status -------------------------------------------
        state.runtime.status = RuntimeStatus::Running;
        state.runtime.proxy_mode = Some("rule".to_string());
        // Demo profile pretends to carry a top-level script: block so the
        // Script mode segment (sidebar + tray) is exercised in captures.
        state.runtime.script_block_present = true;
        state.runtime.system_proxy_enabled = true;
        state.runtime.tun_enabled = Some(false);
        state.editor.tun_stack = "gvisor".to_string();
        state.editor.tun_auto_route = true;
        state.editor.tun_strict_route = false;
        state.editor.sniffer_enabled = true;
        state.diag.public_ip = Some("203.0.113.7".to_string());
        state.shell.error_msg = None;
        state.runtime.autostart_enabled = false;
        state.shell.is_admin = false;

        // ---- proxies page ----------------------------------------------
        let (proxies, groups) = demo_proxy_tables();
        state.runtime.proxies = proxies;
        state.runtime.filtered_groups = groups;
        state.runtime.runtime_selected_group = "GLOBAL".to_string();
        state.runtime.runtime_selected_proxy = "节点选择".to_string();

        // ---- traffic / memory / connections ----------------------------
        let history = demo_traffic_history();
        let last = *history.back().expect("demo traffic history is seeded");
        state.diag.traffic_history = history;
        state.diag.traffic = Some(mihomo_api::types::TrafficData {
            up: last.0,
            down: last.1,
        });
        state.diag.memory = Some(MemoryData {
            in_use: 96_468_992,
            os_limit: 0,
        });
        state.diag.connections = Some(demo_connections());
        state.diag.log_level = "info".to_string();
        state.diag.logs = demo_logs();

        // ---- rules page ---------------------------------------------------
        state.editor.rules = demo_rules();
        state.editor.rules_loaded_once = true;
        state.editor.rules_heavy_ready = true;
        // Capture variant `rules-providers` opens the Providers tab so the
        // provider lists and MRS metadata panel are in frame.
        if env.providers_tab {
            state.editor.rules_tab = crate::types::rules::RulesTab::Providers;
            state.editor.rules_providers_expanded = true;
        }
        state.rebuild_rules_render_cache();
        state.apply_rules_filter();
        state.editor.rule_providers = vec![
            RuleProvider {
                name: "reject".to_string(),
                provider_type: "classical".to_string(),
                behavior: "domain".to_string(),
                vehicle_type: "HTTP".to_string(),
                updated_at: "2026-08-29T12:00:00.000000+08:00".to_string(),
                rule_count: 52_345,
            },
            RuleProvider {
                name: "cn-cidr".to_string(),
                provider_type: "classical".to_string(),
                behavior: "ipcidr".to_string(),
                vehicle_type: "HTTP".to_string(),
                updated_at: "2026-08-29T12:00:00.000000+08:00".to_string(),
                rule_count: 9_412,
            },
        ];
        state.editor.proxy_providers = vec![ProxyProvider {
            name: "机场订阅".to_string(),
            provider_type: "proxy".to_string(),
            vehicle_type: "HTTP".to_string(),
            updated_at: "2026-08-29T13:45:00.000000+08:00".to_string(),
        }];
        state.editor.rule_providers_json_cache = rule_providers_json_fixture();
        state.editor.proxy_providers_json_cache = proxy_providers_json_fixture();
        state.editor.sniffer_json_cache = sniffer_json_fixture();
        // MRS metadata details paired with the rule providers above; the
        // last entry demonstrates the missing-cache failure projection.
        state.editor.mrs_details = vec![
            crate::types::options::MrsProviderDetail {
                name: "reject".to_string(),
                behavior: "domain".to_string(),
                file: Some(PathBuf::from(
                    "/home/demo/.config/musicfrog-infiltrator/providers/rules/reject.mrs",
                )),
                metadata: Some(infiltrator_core::mrs::MrsMetadata {
                    behavior: infiltrator_core::mrs::Behavior::Domain,
                    rule_count: 52_345,
                    version: 1,
                    payload_size: 1_882_304,
                    description: "reject-list".to_string(),
                }),
                errors: Vec::new(),
            },
            crate::types::options::MrsProviderDetail {
                name: "cn-cidr".to_string(),
                behavior: "ipcidr".to_string(),
                file: Some(PathBuf::from(
                    "/home/demo/.config/musicfrog-infiltrator/providers/rules/cn-cidr.mrs",
                )),
                metadata: Some(infiltrator_core::mrs::MrsMetadata {
                    behavior: infiltrator_core::mrs::Behavior::IpCidr,
                    rule_count: 9_412,
                    version: 1,
                    payload_size: 264_180,
                    description: "cn-cidr-list".to_string(),
                }),
                errors: Vec::new(),
            },
            crate::types::options::MrsProviderDetail {
                name: "google".to_string(),
                behavior: "domain".to_string(),
                file: None,
                metadata: None,
                errors: vec!["本地缓存文件不存在（内核首次拉取后可用）".to_string()],
            },
        ];

        // ---- DNS page -------------------------------------------------------
        state.editor.dns_nameservers = vec![
            "223.5.5.5".to_string(),
            "119.29.29.29".to_string(),
            "https://doh.pub/dns-query".to_string(),
        ];
        state.editor.dns_fallback_servers = vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()];
        state.editor.dns_enhanced_mode = "fake-ip".to_string();
        state.editor.dns_form = crate::types::dns::DnsFormDraft {
            enable: true,
            nameserver: "223.5.5.5\n119.29.29.29\nhttps://doh.pub/dns-query".to_string(),
            fallback: "8.8.8.8\n1.1.1.1".to_string(),
            enhanced_mode: "fake-ip".to_string(),
            fake_ip_range: "198.18.0.1/16".to_string(),
            fake_ip_filter: "*.lan\n*.local".to_string(),
            ipv6: true,
            cache: true,
            use_hosts: true,
            use_system_hosts: true,
            respect_rules: false,
            proxy_server_nameserver: "https://doh.pub/dns-query".to_string(),
            direct_nameserver: String::new(),
        };
        state.editor.fake_ip_form = crate::types::dns::FakeIpFormDraft {
            fake_ip_range: "198.18.0.1/16".to_string(),
            fake_ip_filter: "*.lan\n*.local".to_string(),
            store_fake_ip: true,
        };
        state.editor.tun_form = crate::types::dns::TunFormDraft {
            enable: false,
            stack: "gvisor".to_string(),
            mtu: "9001".to_string(),
            dns_hijack: "any:53".to_string(),
            auto_route: true,
            auto_detect_interface: true,
            strict_route: false,
        };
        state.editor.dns_json_cache = dns_json_fixture();
        state.editor.fake_ip_json_cache = fake_ip_json_fixture();
        state.editor.tun_json_cache = tun_json_fixture();
        state.editor.advanced_configs_loaded_once = true;
        state.editor.dns_heavy_ready = true;

        // ---- profiles page ---------------------------------------------------
        state.profile.profiles = demo_profiles();
        state.profile.subscription_profile_name = "机场订阅".to_string();
        state.profile.subscription_url =
            "https://sub.example.com/api/v1/client/subscribe?token=demo-token".to_string();
        state.profile.subscription_auto_update_enabled = true;
        state.profile.subscription_update_interval_hours = "24".to_string();
        // Per-profile subscription filter editor: prefilled from a fixture
        // spec so the Profiles page card shows a realistic working draft.
        state.editor.filter_draft = crate::types::options::FilterDraft::from_spec(Some(
            &infiltrator_core::profile_options::FilterSpec {
                include_keywords: vec!["香港".to_string(), "日本".to_string()],
                exclude_keywords: vec!["剩余流量".to_string(), "官网".to_string()],
                rename_rules: vec![infiltrator_core::profile_options::RenameSpec {
                    pattern: r"🇭🇰 香港-(\d+)".to_string(),
                    replacement: "HK-$1".to_string(),
                }],
                exclude_types: vec!["trojan".to_string()],
                deduplication: infiltrator_core::profile_options::FilterDedup::AppendIndex,
            },
        ));
        state.editor.filter_loaded_for = Some("机场订阅".to_string());

        // ---- sync / settings page ---------------------------------------------
        state.profile.webdav_enabled = true;
        state.profile.webdav_url = "https://dav.jianguoyun.com/dav/".to_string();
        state.profile.webdav_user = "demo@example.com".to_string();
        state.profile.webdav_pass = "demo-app-password".to_string();
        state.profile.webdav_sync_interval_mins = "60".to_string();
        state.profile.webdav_sync_on_startup = false;
        // Sync-conflict key-level merge session: one conflict plus the
        // computed diff so the per-key picker renders on the Sync page.
        state.profile.sync_conflicts = vec![crate::types::app::SyncConflict {
            profile: "机场订阅".to_string(),
            remote_path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/冲突副本.yaml"),
        }];
        state.profile.sync_diff = Some(crate::types::options::SyncDiffState::new(
            crate::types::options::SyncDiffBundle {
                profile: "机场订阅".to_string(),
                remote_path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/冲突副本.yaml"),
                added: vec!["dns".to_string()],
                removed: vec!["tun".to_string()],
                modified: vec![(
                    "mixed-port".to_string(),
                    "7890".to_string(),
                    "7897".to_string(),
                )],
            },
        ));
        if let Some(diff) = &mut state.profile.sync_diff {
            diff.picks.insert("dns".to_string(), true);
        }
        state.runtime.installed_kernels = vec![
            VersionInfo {
                version: "v1.19.12".to_string(),
                path: PathBuf::from("/opt/musicfrog-infiltrator/versions/v1.19.12/mihomo"),
                is_default: true,
            },
            VersionInfo {
                version: "v1.18.8".to_string(),
                path: PathBuf::from("/opt/musicfrog-infiltrator/versions/v1.18.8/mihomo"),
                is_default: false,
            },
        ];
        // Admin entry stays consistent with the (never started) demo server:
        // settings show the default port 25210 but the server is not running.
        state.shell.admin_enabled = true;
        state.shell.admin_port = crate::admin_server::ADMIN_DEFAULT_PORT;
        state.shell.admin_port_input = crate::admin_server::ADMIN_DEFAULT_PORT.to_string();

        // ---- editor page --------------------------------------------------------
        state.editor.editor_path = Some(PathBuf::from(
            "/home/demo/.config/musicfrog-infiltrator/profiles/机场订阅.yaml",
        ));
        state.editor.editor_path_setting = String::new();
        state.editor.editor_content =
            iced::widget::text_editor::Content::with_text(&profile_yaml_fixture());
        // Mixin pane fixture: the overlay document the pane edits, plus the
        // pane selected via the capture scenario (`mixin-*` rows).
        state.editor.editor_pane = env.pane;
        state.editor.mixin_loaded_for = Some("机场订阅".to_string());
        state.editor.mixin_content = iced::widget::text_editor::Content::with_text(
            r#"# Mixin 覆写：深合并到当前 profile 之上
mode: rule
ipv6: true
mixed-port: 7897

dns:
  respect-rules: true

rules:
  prepend:
    - DOMAIN-SUFFIX,internal.example.com,DIRECT
  append:
    - MATCH,节点选择
"#,
        );

        (state, iced::Task::none())
    }
}

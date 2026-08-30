//! Demo-mode [`AppState`] constructor: seeds every page with the fixture
//! tables so the full UI renders without any runtime or side effects.

use super::DemoEnv;
use super::fixtures::{
    demo_connections, demo_logs, demo_profiles, demo_rules, demo_traffic_history,
    dns_json_fixture, fake_ip_json_fixture, profile_yaml_fixture, proxy_providers_json_fixture,
    rule_providers_json_fixture, sniffer_json_fixture, tun_json_fixture,
};
use super::proxy_fixtures::demo_proxy_tables;
use crate::state::AppState;
use crate::types::{Message, RuntimeStatus};
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
        state.demo = true;
        state.capture_marker = env.capture_marker.clone();
        state.current_route = env.page;
        state.theme = env.skin.clone();
        state.lang = "zh-CN".to_string();

        // ---- runtime status -------------------------------------------
        state.status = RuntimeStatus::Running;
        state.proxy_mode = Some("rule".to_string());
        state.system_proxy_enabled = true;
        state.tun_enabled = Some(false);
        state.tun_stack = "gvisor".to_string();
        state.tun_auto_route = true;
        state.tun_strict_route = false;
        state.sniffer_enabled = true;
        state.public_ip = Some("203.0.113.7".to_string());
        state.error_msg = None;
        state.autostart_enabled = false;
        state.is_admin = false;

        // ---- proxies page ----------------------------------------------
        let (proxies, groups) = demo_proxy_tables();
        state.proxies = proxies;
        state.filtered_groups = groups;
        state.runtime_selected_group = "GLOBAL".to_string();
        state.runtime_selected_proxy = "节点选择".to_string();

        // ---- traffic / memory / connections ----------------------------
        let history = demo_traffic_history();
        let last = *history.back().expect("demo traffic history is seeded");
        state.traffic_history = history;
        state.traffic = Some(mihomo_api::types::TrafficData {
            up: last.0,
            down: last.1,
        });
        state.memory = Some(MemoryData {
            in_use: 96_468_992,
            os_limit: 0,
        });
        state.connections = Some(demo_connections());
        state.log_level = "info".to_string();
        state.logs = demo_logs();

        // ---- rules page ---------------------------------------------------
        state.rules = demo_rules();
        state.rules_loaded_once = true;
        state.rules_heavy_ready = true;
        state.rebuild_rules_render_cache();
        state.apply_rules_filter();
        state.rule_providers = vec![
            RuleProvider {
                name: "reject".to_string(),
                provider_type: "classical".to_string(),
                behavior: "domain".to_string(),
                vehicle_type: "HTTP".to_string(),
                updated_at: "2026-08-29T12:00:00.000000+08:00".to_string(),
                rule_count: 52_345,
            },
            RuleProvider {
                name: "icloud".to_string(),
                provider_type: "classical".to_string(),
                behavior: "domain".to_string(),
                vehicle_type: "HTTP".to_string(),
                updated_at: "2026-08-29T12:00:00.000000+08:00".to_string(),
                rule_count: 1_482,
            },
            RuleProvider {
                name: "google".to_string(),
                provider_type: "classical".to_string(),
                behavior: "domain".to_string(),
                vehicle_type: "HTTP".to_string(),
                updated_at: "2026-08-29T12:00:00.000000+08:00".to_string(),
                rule_count: 785,
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
        state.proxy_providers = vec![ProxyProvider {
            name: "机场订阅".to_string(),
            provider_type: "proxy".to_string(),
            vehicle_type: "HTTP".to_string(),
            updated_at: "2026-08-29T13:45:00.000000+08:00".to_string(),
        }];
        state.rule_providers_json_cache = rule_providers_json_fixture();
        state.proxy_providers_json_cache = proxy_providers_json_fixture();
        state.sniffer_json_cache = sniffer_json_fixture();

        // ---- DNS page -------------------------------------------------------
        state.dns_nameservers = vec![
            "223.5.5.5".to_string(),
            "119.29.29.29".to_string(),
            "https://doh.pub/dns-query".to_string(),
        ];
        state.dns_fallback_servers = vec!["8.8.8.8".to_string(), "1.1.1.1".to_string()];
        state.dns_enhanced_mode = "fake-ip".to_string();
        state.dns_form = crate::types::DnsFormDraft {
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
        state.fake_ip_form = crate::types::FakeIpFormDraft {
            fake_ip_range: "198.18.0.1/16".to_string(),
            fake_ip_filter: "*.lan\n*.local".to_string(),
            store_fake_ip: true,
        };
        state.tun_form = crate::types::TunFormDraft {
            enable: false,
            stack: "gvisor".to_string(),
            mtu: "9001".to_string(),
            dns_hijack: "any:53".to_string(),
            auto_route: true,
            auto_detect_interface: true,
            strict_route: false,
        };
        state.dns_json_cache = dns_json_fixture();
        state.fake_ip_json_cache = fake_ip_json_fixture();
        state.tun_json_cache = tun_json_fixture();
        state.advanced_configs_loaded_once = true;
        state.dns_heavy_ready = true;

        // ---- profiles page ---------------------------------------------------
        state.profiles = demo_profiles();
        state.subscription_profile_name = "机场订阅".to_string();
        state.subscription_url =
            "https://sub.example.com/api/v1/client/subscribe?token=demo-token".to_string();
        state.subscription_auto_update_enabled = true;
        state.subscription_update_interval_hours = "24".to_string();

        // ---- sync / settings page ---------------------------------------------
        state.webdav_enabled = true;
        state.webdav_url = "https://dav.jianguoyun.com/dav/".to_string();
        state.webdav_user = "demo@example.com".to_string();
        state.webdav_pass = "demo-app-password".to_string();
        state.webdav_sync_interval_mins = "60".to_string();
        state.webdav_sync_on_startup = false;
        state.installed_kernels = vec![
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
        state.admin_enabled = true;
        state.admin_port = crate::admin_server::ADMIN_DEFAULT_PORT;
        state.admin_port_input = crate::admin_server::ADMIN_DEFAULT_PORT.to_string();

        // ---- editor page --------------------------------------------------------
        state.editor_path = Some(PathBuf::from(
            "/home/demo/.config/musicfrog-infiltrator/profiles/机场订阅.yaml",
        ));
        state.editor_path_setting = String::new();
        state.editor_content =
            iced::widget::text_editor::Content::with_text(&profile_yaml_fixture());

        (state, iced::Task::none())
    }
}

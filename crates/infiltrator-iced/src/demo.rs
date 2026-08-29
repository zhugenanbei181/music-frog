//! Demo mode: render the REAL application against in-memory fixture data.
//!
//! Enabled by `--demo` on the command line or `INFILTRATOR_DEMO=1` in the
//! environment. A demo session shares every view/update code path with the
//! production app but:
//!
//! * never spawns mihomo, the admin server, the system tray or any process,
//! * never writes settings / profiles / rules files, never touches the
//!   system proxy, autostart or the network,
//! * pre-populates [`AppState`] with realistic Chinese-locale fixtures so
//!   every page renders fully (see [`AppState::demo`]).
//!
//! Environment contract used by the visual-capture tooling (names are part
//! of the contract — do not rename):
//!
//! | Variable                    | Meaning                                        |
//! |-----------------------------|------------------------------------------------|
//! | `INFILTRATOR_DEMO=1`        | enable demo mode (`--demo` argv also works)    |
//! | `INFILTRATOR_PAGE`          | initial route: overview\|proxies\|runtime\|rules\|dns\|profiles\|sync\|editor\|settings |
//! | `INFILTRATOR_SKIN`          | `light` or `dark` (default dark)               |
//! | `INFILTRATOR_WINDOW_SIZE`   | `WxH` (default 1180x780)                       |
//! | `INFILTRATOR_CAPTURE_MARKER`| file getting `CAPTURE_READY page=<p> skin=<s>` appended after the first rendered frame |

use crate::state::AppState;
use crate::types::{Message, Route, RuntimeStatus};
use iced::{application, window};
use infiltrator_core::rules::RuleEntry;
use mihomo_api::proxy::types::{
    Hysteria2, Proxy, ProxyBase, ProxyGroup, ProxyHistory, Shadowsocks, Trojan, Vmess,
};
use mihomo_api::types::{
    Connection, ConnectionMetadata, ConnectionSnapshot, MemoryData, ProxyProvider, RuleProvider,
};
use mihomo_config::profile::Profile;
use mihomo_version::manager::VersionInfo;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

/// Default window size, mirrors the production window in `main.rs`.
const DEFAULT_WINDOW: (f32, f32) = (1180.0, 780.0);

/// Everything demo mode needs to know, resolved once from argv + environment.
#[derive(Debug, Clone)]
pub struct DemoEnv {
    pub enabled: bool,
    pub page: Route,
    pub skin: iced::Theme,
    pub window_size: (f32, f32),
    pub capture_marker: Option<PathBuf>,
}

impl DemoEnv {
    /// Resolve demo settings from `--demo` argv and the `INFILTRATOR_*`
    /// environment variables. Invalid values fall back to defaults.
    pub fn from_environment() -> Self {
        let enabled = std::env::args().any(|arg| arg == "--demo")
            || std::env::var("INFILTRATOR_DEMO").is_ok_and(|v| v.trim() == "1");
        Self {
            enabled,
            page: std::env::var("INFILTRATOR_PAGE")
                .ok()
                .and_then(|v| parse_page(&v))
                .unwrap_or(Route::Overview),
            skin: std::env::var("INFILTRATOR_SKIN")
                .ok()
                .map(|v| parse_skin(&v))
                .unwrap_or(iced::Theme::Dark),
            window_size: std::env::var("INFILTRATOR_WINDOW_SIZE")
                .ok()
                .map(|v| parse_window_size(&v))
                .unwrap_or(DEFAULT_WINDOW),
            capture_marker: std::env::var("INFILTRATOR_CAPTURE_MARKER")
                .ok()
                .map(PathBuf::from)
                .filter(|p| !p.as_os_str().is_empty()),
        }
    }
}

/// `overview|proxies|runtime|rules|dns|profiles|sync|editor|settings` -> Route.
/// Unknown values yield `None` (callers fall back to [`Route::Overview`]).
pub fn parse_page(value: &str) -> Option<Route> {
    match value.trim().to_ascii_lowercase().as_str() {
        "overview" => Some(Route::Overview),
        "proxies" => Some(Route::Proxies),
        "runtime" => Some(Route::Runtime),
        "rules" => Some(Route::Rules),
        "dns" => Some(Route::Dns),
        "profiles" => Some(Route::Profiles),
        "sync" => Some(Route::Sync),
        "editor" => Some(Route::Editor),
        "settings" => Some(Route::Settings),
        _ => None,
    }
}

/// Inverse of [`parse_page`] — the canonical env name of a route.
pub fn route_env_name(route: Route) -> &'static str {
    match route {
        Route::Overview => "overview",
        Route::Proxies => "proxies",
        Route::Runtime => "runtime",
        Route::Rules => "rules",
        Route::Dns => "dns",
        Route::Profiles => "profiles",
        Route::Sync => "sync",
        Route::Editor => "editor",
        Route::Settings => "settings",
    }
}

/// `light|dark` -> iced theme; unknown values fall back to dark.
pub fn parse_skin(value: &str) -> iced::Theme {
    if value.trim().eq_ignore_ascii_case("light") {
        iced::Theme::Light
    } else {
        iced::Theme::Dark
    }
}

/// Canonical `light` / `dark` name of an iced theme (for the capture marker).
pub fn skin_name(theme: &iced::Theme) -> &'static str {
    if matches!(theme, iced::Theme::Light) {
        "light"
    } else {
        "dark"
    }
}

/// `WxH` -> `(w, h)`; anything unparsable or non-positive falls back to the
/// default production window size.
pub fn parse_window_size(value: &str) -> (f32, f32) {
    let parts: Option<Vec<f32>> = value
        .trim()
        .split(['x', 'X'])
        .map(|part| part.trim().parse::<f32>().ok())
        .collect();
    match parts.as_deref() {
        Some([w, h]) if *w > 0.0 && *h > 0.0 => (*w, *h),
        _ => DEFAULT_WINDOW,
    }
}

/// Run the iced application in demo mode. Same views/update/subscription as
/// the production entry point, but booting from [`AppState::demo`], sized
/// from `INFILTRATOR_WINDOW_SIZE` and without any system integration
/// (no single-instance mutex, no tray, no settings bootstrap).
pub fn run(env: DemoEnv) -> iced::Result {
    let window_size = env.window_size;
    application(
        move || AppState::demo(&env),
        AppState::update,
        AppState::view,
    )
    .title(AppState::title)
    .theme(AppState::theme)
    .subscription(AppState::subscription)
    // Bundled typography: identical to the production window (see main.rs).
    .font(include_bytes!("../assets/fonts/Inter-Regular.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Inter-Medium.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/Inter-SemiBold.ttf").as_slice())
    .font(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf").as_slice())
    .default_font(iced::Font::with_name("Inter"))
    .window(window::Settings {
        size: window_size.into(),
        min_size: Some((960.0, 640.0).into()),
        exit_on_close_request: false,
        ..Default::default()
    })
    .run()
}

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

    /// Append the capture-ready marker, exactly once per process, after the
    /// first real `view()` pass (called from `AppState::view`). Idempotent:
    /// repeat calls are ignored via an atomic flag.
    pub(crate) fn write_capture_marker(&self) {
        if !self.demo {
            return;
        }
        if self.capture_marker_written.swap(true, Ordering::SeqCst) {
            return;
        }
        let Some(path) = self.capture_marker.as_ref() else {
            return;
        };
        let line = format!(
            "CAPTURE_READY page={} skin={}\n",
            route_env_name(self.current_route),
            skin_name(&self.theme),
        );
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            use std::io::Write as _;
            let _ = file.write_all(line.as_bytes());
        }
    }
}

// ---------------------------------------------------------------------------
// Proxy fixtures
// ---------------------------------------------------------------------------

/// Proxy group name constants shared between the table builder and the
/// runtime-selection fixture.
const G_MAIN: &str = "节点选择";
const G_DIRECT: &str = "全球直连";
const G_CAMPUS: &str = "校园网";
const G_AI: &str = "AI 服务";
const G_GAME: &str = "游戏平台";
const G_AUTO: &str = "自动选择";
const G_GLOBAL: &str = "GLOBAL";

const N_HK1: &str = "香港 IEPL-01";
const N_HK2: &str = "香港 IEPL-02";
const N_JP: &str = "日本 NTT";
const N_SG: &str = "新加坡 BGP";
const N_US: &str = "美国 CN2";
const N_DMIT: &str = "DMIT";
const N_ZGO: &str = "ZGO";

/// Build the demo proxy map plus a deterministic `filtered_groups` ordering
/// (GLOBAL last so the capture always sees the business groups first).
fn demo_proxy_tables() -> (HashMap<String, Proxy>, Vec<(String, Vec<String>)>) {
    let mut proxies: HashMap<String, Proxy> = HashMap::new();

    let mut insert_group = |name: &str, now: &str, all: Vec<&str>| {
        proxies.insert(
            name.to_string(),
            Proxy::Selector(ProxyGroup {
                name: name.to_string(),
                now: now.to_string(),
                all: all.iter().map(|s| s.to_string()).collect(),
                history: vec![ProxyHistory {
                    time: "2026-08-29T15:30:00.000000+08:00".to_string(),
                    delay: 0,
                }],
            }),
        );
    };

    insert_group(
        G_MAIN,
        N_HK1,
        vec![N_HK1, N_DMIT, N_HK2, N_SG, N_JP, N_US, N_ZGO],
    );
    insert_group(G_DIRECT, "DIRECT", vec!["DIRECT", N_HK1, N_DMIT]);
    insert_group(
        G_CAMPUS,
        "DIRECT",
        vec!["DIRECT", "REJECT", N_HK2, N_DMIT],
    );
    insert_group(G_AI, N_US, vec![N_US, N_SG, N_JP, N_DMIT]);
    insert_group(G_GAME, N_JP, vec![N_JP, N_HK1, N_HK2, N_US]);
    insert_group(
        G_GLOBAL,
        G_MAIN,
        vec![G_MAIN, G_DIRECT, G_CAMPUS, G_AI, G_GAME, G_AUTO],
    );
    proxies.insert(
        G_AUTO.to_string(),
        Proxy::URLTest(ProxyGroup {
            name: G_AUTO.to_string(),
            now: N_HK1.to_string(),
            all: vec![
                N_HK1.to_string(),
                N_DMIT.to_string(),
                N_HK2.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
            ],
            history: vec![ProxyHistory {
                time: "2026-08-29T15:30:00.000000+08:00".to_string(),
                delay: 0,
            }],
        }),
    );

    // Node fixtures with mixed latency tiers so every badge color renders:
    // 148 (fast) / 189 / 233 / 254 / 312 / 512 (slow) / untested ("—").
    let nodes: Vec<Proxy> = vec![
        ss_node(N_HK1, 148, "hk-iepl-01.example.com", 8443),
        vmess_node(N_DMIT, 189, "dmit.example.com", 443),
        ss_node(N_HK2, 233, "hk-iepl-02.example.com", 8443),
        trojan_node(N_SG, 254, "sg-bgp.example.com", 443),
        vmess_node(N_JP, 312, "jp-ntt.example.com", 443),
        trojan_node(N_US, 512, "us-cn2.example.com", 443),
        hysteria2_node(N_ZGO, "zgo.example.com", 36712),
        Proxy::Direct(direct_base("DIRECT")),
        Proxy::Reject(reject_base("REJECT")),
    ];
    for node in nodes {
        let name = node.name().to_string();
        proxies.insert(name, node);
    }

    let filtered_groups: Vec<(String, Vec<String>)> = vec![
        (
            G_MAIN.to_string(),
            vec![
                N_HK1.to_string(),
                N_DMIT.to_string(),
                N_HK2.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
                N_US.to_string(),
                N_ZGO.to_string(),
            ],
        ),
        (
            G_DIRECT.to_string(),
            vec!["DIRECT".to_string(), N_HK1.to_string(), N_DMIT.to_string()],
        ),
        (
            G_CAMPUS.to_string(),
            vec![
                "DIRECT".to_string(),
                "REJECT".to_string(),
                N_HK2.to_string(),
                N_DMIT.to_string(),
            ],
        ),
        (
            G_AI.to_string(),
            vec![
                N_US.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
                N_DMIT.to_string(),
            ],
        ),
        (
            G_GAME.to_string(),
            vec![
                N_JP.to_string(),
                N_HK1.to_string(),
                N_HK2.to_string(),
                N_US.to_string(),
            ],
        ),
        (
            G_AUTO.to_string(),
            vec![
                N_HK1.to_string(),
                N_DMIT.to_string(),
                N_HK2.to_string(),
                N_SG.to_string(),
                N_JP.to_string(),
            ],
        ),
        (
            G_GLOBAL.to_string(),
            vec![
                G_MAIN.to_string(),
                G_DIRECT.to_string(),
                G_CAMPUS.to_string(),
                G_AI.to_string(),
                G_GAME.to_string(),
                G_AUTO.to_string(),
            ],
        ),
    ];

    (proxies, filtered_groups)
}

/// A tested node base with one historical sample.
fn node_base(name: &str, delay: u32) -> ProxyBase {
    ProxyBase {
        name: name.to_string(),
        udp: true,
        history: vec![ProxyHistory {
            time: "2026-08-29T15:31:00.000000+08:00".to_string(),
            delay,
        }],
        alive: true,
        delay: Some(delay),
    }
}

/// An untested node base (no history) — renders the "—" latency tier.
fn untested_base(name: &str) -> ProxyBase {
    ProxyBase {
        name: name.to_string(),
        udp: true,
        history: Vec::new(),
        alive: true,
        delay: None,
    }
}

fn ss_node(name: &str, delay: u32, server: &str, port: u16) -> Proxy {
    Proxy::Shadowsocks(Shadowsocks {
        base: node_base(name, delay),
        server: server.to_string(),
        port,
        cipher: "aes-256-gcm".to_string(),
        plugin: None,
        plugin_opts: None,
    })
}

fn vmess_node(name: &str, delay: u32, server: &str, port: u16) -> Proxy {
    Proxy::Vmess(Vmess {
        base: node_base(name, delay),
        server: server.to_string(),
        port,
        uuid: "88888888-4444-4444-4444-cccccccccccc".to_string(),
        ..Vmess::default()
    })
}

fn trojan_node(name: &str, delay: u32, server: &str, port: u16) -> Proxy {
    Proxy::Trojan(Trojan {
        base: node_base(name, delay),
        server: server.to_string(),
        port,
        ..Trojan::default()
    })
}

fn hysteria2_node(name: &str, server: &str, port: u16) -> Proxy {
    Proxy::Hysteria2(Hysteria2 {
        base: untested_base(name),
        server: server.to_string(),
        port,
        ..Hysteria2::default()
    })
}

fn direct_base(name: &str) -> mihomo_api::proxy::types::Direct {
    mihomo_api::proxy::types::Direct {
        base: untested_base(name),
    }
}

fn reject_base(name: &str) -> mihomo_api::proxy::types::Reject {
    mihomo_api::proxy::types::Reject {
        base: untested_base(name),
    }
}

// ---------------------------------------------------------------------------
// Traffic / logs / connections / rules / profiles / JSON fixtures
// ---------------------------------------------------------------------------

/// 60 samples (the app's own history cap and the chart's `max_points`) of a
/// believable wave: up 0.5–4 MB/s, down 1–12 MB/s.
fn demo_traffic_history() -> VecDeque<(u64, u64)> {
    const MB: f64 = 1024.0 * 1024.0;
    (0..60u32)
        .map(|i| {
            let up_wave = 0.5 + (f64::from(i) * 0.35).sin().mul_add(0.5, 0.5) * 3.5;
            let down_wave = (f64::from(i) * 0.22 + 1.3)
                .sin()
                .mul_add(0.5, 0.5)
                .mul_add(11.0, 1.0);
            ((up_wave * MB) as u64, (down_wave * MB) as u64)
        })
        .collect()
}

/// ~40 mixed mihomo-style log lines (info/warn/error, Chinese included).
fn demo_logs() -> VecDeque<String> {
    let lines: &[&str] = &[
        "INFO[0001] Start initial configuration in progress",
        "INFO[0001] MMDB(geoip.metadb) 已加载，包含 320547 条记录",
        "INFO[0002] Level-1 负载均衡已启用",
        "INFO[0002] RESTful API listening at 127.0.0.1:9090",
        "INFO[0003] [TCP] 192.168.1.23:52118 --> www.google.com:443 match DomainSuffix(google.com) using 节点选择[香港 IEPL-01]",
        "INFO[0004] [UDP] 192.168.1.23:52119 --> 8.8.8.8:53 match Ip CIDR(8.8.8.8/32) using 全球直连[DIRECT]",
        "WARN[0006] [TCP] connect error (ZGO): dial tcp 104.21.0.0:36712: i/o timeout",
        "INFO[0007] [TCP] 192.168.1.23:52124 --> api.openai.com:443 match DomainSuffix(openai.com) using AI 服务[美国 CN2]",
        "INFO[0008] [TCP] 192.168.1.23:52130 --> www.youtube.com:443 match DomainKeyword(youtube) using 节点选择[香港 IEPL-01]",
        "ERROR[0009] DNS 解析失败：resolver default: lookup doh.privatedns.example: server misbehaving",
        "INFO[0010] [TCP] 192.168.1.23:52131 --> github.com:443 match DomainSuffix(github.com) using 节点选择[DMIT]",
        "INFO[0011] external controller 已就绪",
        "WARN[0013] provider 机场订阅 的更新间隔小于推荐值 (24h)",
        "INFO[0014] [TCP] 192.168.1.23:52140 --> cdn.jsdelivr.net:443 match DomainSuffix(jsdelivr.net) using 节点选择[香港 IEPL-02]",
        "INFO[0015] [TCP] 192.168.1.23:52144 --> mail.qq.com:443 match DomainSuffix(qq.com) using 全球直连[DIRECT]",
        "INFO[0016] Sniffer 已启用：对 443 端口执行 TLS 嗅探",
        "INFO[0017] [UDP] 192.168.1.23:52150 --> 1.1.1.1:53 match RuleSet(cn-cidr) using 全球直连[DIRECT]",
        "ERROR[0019] 延迟测试失败 (ZGO): context deadline exceeded",
        "INFO[0020] [TCP] 192.168.1.23:52155 --> store.steampowered.com:443 match DomainSuffix(steamcontent.com) using 游戏平台[日本 NTT]",
        "INFO[0021] [TCP] 192.168.1.23:52158 --> chat.openai.com:443 match DomainSuffix(openai.com) using AI 服务[美国 CN2]",
        "WARN[0023] TUN 未启用，系统代理模式运行中",
        "INFO[0024] [TCP] 192.168.1.23:52160 --> www.bilibili.com:443 match GeoIP(CN) using 全球直连[DIRECT]",
        "INFO[0025] Fake-IP 缓存已持久化 (198.18.0.1/16)",
        "INFO[0026] [TCP] 192.168.1.23:52166 --> api.telegram.org:443 match DomainSuffix(telegram.org) using 节点选择[新加坡 BGP]",
        "ERROR[0028] WebDAV 同步失败：401 Unauthorized (请检查账号密码)",
        "INFO[0029] [TCP] 192.168.1.23:52170 --> twitch.tv:443 match DomainSuffix(twitch.tv) using 节点选择[DMIT]",
        "INFO[0030] 规则集 google 更新完成 (785 条)",
        "INFO[0031] [UDP] 192.168.1.23:52175 --> 223.5.5.5:443 match RuleSet(cn-cidr) using 全球直连[DIRECT]",
        "INFO[0032] [TCP] 192.168.1.23:52180 --> www.icloud.com:443 match RuleSet(icloud) using 全球直连[DIRECT]",
        "WARN[0034] 节点 香港 IEPL-02 连续 3 次延迟高于 200ms",
        "INFO[0035] [TCP] 192.168.1.23:52188 --> twitter.com:443 match DomainSuffix(twitter.com) using 节点选择[DMIT]",
        "INFO[0036] 内存占用：92.00 MB (上限 0)",
        "INFO[0037] [TCP] 192.168.1.23:52192 --> xiaomi.com:443 match DomainSuffix(xiaomi.com) using 全球直连[DIRECT]",
        "INFO[0038] 订阅更新成功：机场订阅 (24h 后自动更新)",
        "INFO[0039] [TCP] 192.168.1.23:52196 --> edge.microsoft.com:443 match DomainSuffix(microsoft.com) using 节点选择[香港 IEPL-01]",
        "ERROR[0041] 连接中断 (游戏平台[日本 NTT]): connection reset by peer",
        "INFO[0042] 自动重连成功：api.openai.com:443",
        "INFO[0043] [TCP] 192.168.1.23:52200 --> v2ex.com:443 match Match(漏网之鱼) using 节点选择[香港 IEPL-01]",
        "INFO[0044] 当前出口：香港 IEPL-01 (148ms)",
        "INFO[0045] 心跳正常，运行时间 02:41:17",
    ];
    lines.iter().map(|s| s.to_string()).collect()
}

/// 10 mixed connection rows (hosts / ports / rules / chains / traffic).
fn demo_connections() -> ConnectionSnapshot {
    // (host, port, rule type, rule payload, chain group, exit node, down, up)
    let rows: [(&str, &str, &str, &str, &str, &str, u64, u64); 10] = [
        ("www.google.com", "443", "DomainSuffix", "google.com", "节点选择", "香港 IEPL-01", 48_234_112, 2_097_152),
        ("api.openai.com", "443", "DomainSuffix", "openai.com", "AI 服务", "美国 CN2", 12_884_901, 6_291_456),
        ("www.youtube.com", "443", "DomainKeyword", "youtube", "节点选择", "香港 IEPL-01", 335_544_320, 15_728_640),
        ("github.com", "443", "DomainSuffix", "github.com", "节点选择", "DMIT", 22_020_096, 4_194_304),
        ("cdn.jsdelivr.net", "443", "DomainSuffix", "jsdelivr.net", "节点选择", "香港 IEPL-02", 8_388_608, 1_572_864),
        ("mail.qq.com", "443", "DomainSuffix", "qq.com", "全球直连", "DIRECT", 6_291_456, 2_621_440),
        ("www.bilibili.com", "443", "GeoIP", "CN", "全球直连", "DIRECT", 18_874_368, 3_355_443),
        ("store.steampowered.com", "443", "DomainSuffix", "steamcontent.com", "游戏平台", "日本 NTT", 96_468_992, 8_388_608),
        ("chat.openai.com", "443", "DomainSuffix", "openai.com", "AI 服务", "新加坡 BGP", 5_242_880, 1_048_576),
        ("v2ex.com", "443", "Match", "漏网之鱼", "节点选择", "香港 IEPL-01", 3_145_728, 786_432),
    ];

    let connections = rows
        .iter()
        .enumerate()
        .map(|(i, (host, port, rule, payload, chain, node, download, upload))| Connection {
            id: format!("demo-conn-{i:03}"),
            metadata: ConnectionMetadata {
                network: if i % 4 == 3 { "udp" } else { "tcp" }.to_string(),
                connection_type: if i % 4 == 3 { "UDP" } else { "TLS" }.to_string(),
                source_ip: "192.168.1.23".to_string(),
                destination_ip: format!("203.0.113.{}", 10 + i),
                source_port: format!("{}", 52_100 + i * 7),
                destination_port: port.to_string(),
                host: host.to_string(),
                dns_mode: "fake-ip".to_string(),
                process_path: if i % 3 == 0 {
                    "/usr/bin/chromium".to_string()
                } else {
                    String::new()
                },
                special_proxy: String::new(),
            },
            upload: *upload,
            download: *download,
            start: format!("2026-08-29T15:{:02}:{:02}.000000+08:00", 20 + i, (i * 7) % 60),
            rule: rule.to_string(),
            rule_payload: payload.to_string(),
            chains: vec![node.to_string(), chain.to_string()],
        })
        .collect();

    ConnectionSnapshot {
        download_total: 3_758_096_384,
        upload_total: 728_766_464,
        connections,
    }
}

/// 15 rules covering the DOMAIN* / IP-CIDR / GEOIP / RuleSet / MATCH families.
fn demo_rules() -> Vec<RuleEntry> {
    let rules = [
        "DOMAIN-SUFFIX,google.com,节点选择",
        "DOMAIN-SUFFIX,openai.com,AI 服务",
        "DOMAIN-KEYWORD,youtube,节点选择",
        "DOMAIN-SUFFIX,telegram.org,节点选择",
        "DOMAIN-SUFFIX,github.com,节点选择",
        "DOMAIN-SUFFIX,qq.com,DIRECT",
        "DOMAIN-SUFFIX,bilibili.com,DIRECT",
        "DOMAIN-SUFFIX,xiaomi.com,DIRECT",
        "DOMAIN-SUFFIX,steamcontent.com,游戏平台",
        "DOMAIN-SUFFIX,microsoft.com,节点选择",
        "IP-CIDR,8.8.8.8/32,全球直连,no-resolve",
        "IP-CIDR,192.168.0.0/16,DIRECT,no-resolve",
        "IP-CIDR6,2620:0:2d0:200::7/32,REJECT,no-resolve",
        "GEOIP,CN,DIRECT",
        "MATCH,漏网之鱼",
    ];
    rules
        .into_iter()
        .map(|rule| RuleEntry {
            rule: rule.to_string(),
            enabled: true,
        })
        .collect()
}

/// 3 profiles: an active subscription, a local file and a standby subscription.
fn demo_profiles() -> Vec<Profile> {
    use chrono::TimeZone;
    let updated = chrono::Utc
        .with_ymd_and_hms(2026, 8, 29, 13, 45, 0)
        .unwrap();
    vec![
        Profile {
            name: "机场订阅".to_string(),
            path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/profiles/机场订阅.yaml"),
            active: true,
            subscription_url: Some(
                "https://sub.example.com/api/v1/client/subscribe?token=demo-token".to_string(),
            ),
            auto_update_enabled: true,
            update_interval_hours: Some(24),
            last_updated: Some(updated),
            next_update: Some(updated + chrono::Duration::hours(24)),
        },
        Profile {
            name: "本地配置".to_string(),
            path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/profiles/本地配置.yaml"),
            active: false,
            subscription_url: None,
            auto_update_enabled: false,
            update_interval_hours: None,
            last_updated: Some(updated),
            next_update: None,
        },
        Profile {
            name: "备用线路".to_string(),
            path: PathBuf::from("/home/demo/.config/musicfrog-infiltrator/profiles/备用线路.yaml"),
            active: false,
            subscription_url: Some("https://backup.example.com/link/demo".to_string()),
            auto_update_enabled: false,
            update_interval_hours: Some(12),
            last_updated: Some(updated),
            next_update: None,
        },
    ]
}

fn rule_providers_json_fixture() -> String {
    [
        "{",
        "  \"reject\": { \"type\": \"classical\", \"behavior\": \"domain\", \"vehicleType\": \"HTTP\", \"ruleCount\": 52345, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" },",
        "  \"icloud\": { \"type\": \"classical\", \"behavior\": \"domain\", \"vehicleType\": \"HTTP\", \"ruleCount\": 1482, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" },",
        "  \"google\": { \"type\": \"classical\", \"behavior\": \"domain\", \"vehicleType\": \"HTTP\", \"ruleCount\": 785, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" },",
        "  \"cn-cidr\": { \"type\": \"classical\", \"behavior\": \"ipcidr\", \"vehicleType\": \"HTTP\", \"ruleCount\": 9412, \"updatedAt\": \"2026-08-29T12:00:00.000000+08:00\" }",
        "}",
    ]
    .join("\n")
}

fn proxy_providers_json_fixture() -> String {
    [
        "{",
        "  \"机场订阅\": { \"type\": \"proxy\", \"vehicleType\": \"HTTP\", \"updatedAt\": \"2026-08-29T13:45:00.000000+08:00\", \"proxies\": 7 }",
        "}",
    ]
    .join("\n")
}

fn sniffer_json_fixture() -> String {
    [
        "{",
        "  \"enable\": true,",
        "  \"parse-pure-ip\": true,",
        "  \"override-destination\": true,",
        "  \"sniff\": {",
        "    \"TLS\": { \"ports\": [443, 8443] },",
        "    \"HTTP\": { \"ports\": [80, \"8080-8880\"] },",
        "    \"QUIC\": { \"ports\": [443, 8443] }",
        "  }",
        "}",
    ]
    .join("\n")
}

fn dns_json_fixture() -> String {
    [
        "{",
        "  \"enable\": true,",
        "  \"ipv6\": true,",
        "  \"cache\": true,",
        "  \"use-hosts\": true,",
        "  \"use-system-hosts\": true,",
        "  \"enhanced-mode\": \"fake-ip\",",
        "  \"fake-ip-range\": \"198.18.0.1/16\",",
        "  \"nameserver\": [\"223.5.5.5\", \"119.29.29.29\", \"https://doh.pub/dns-query\"],",
        "  \"fallback\": [\"8.8.8.8\", \"1.1.1.1\"]",
        "}",
    ]
    .join("\n")
}

fn fake_ip_json_fixture() -> String {
    [
        "{",
        "  \"fake-ip-range\": \"198.18.0.1/16\",",
        "  \"fake-ip-filter\": [\"*.lan\", \"*.local\"],",
        "  \"store-fake-ip\": true",
        "}",
    ]
    .join("\n")
}

fn tun_json_fixture() -> String {
    [
        "{",
        "  \"enable\": false,",
        "  \"stack\": \"gvisor\",",
        "  \"mtu\": 9001,",
        "  \"dns-hijack\": [\"any:53\"],",
        "  \"auto-route\": true,",
        "  \"auto-detect-interface\": true,",
        "  \"strict-route\": false",
        "}",
    ]
    .join("\n")
}

fn profile_yaml_fixture() -> String {
    [
        "# MusicFrog Infiltrator 演示配置 (demo fixture)",
        "mixed-port: 7890",
        "allow-lan: false",
        "mode: rule",
        "log-level: info",
        "external-controller: 127.0.0.1:9090",
        "",
        "proxies:",
        "  - name: \"香港 IEPL-01\"",
        "    type: ss",
        "    server: hk-iepl-01.example.com",
        "    port: 8443",
        "    cipher: aes-256-gcm",
        "    password: \"demo-password\"",
        "    udp: true",
        "",
        "proxy-groups:",
        "  - name: \"节点选择\"",
        "    type: select",
        "    proxies:",
        "      - 香港 IEPL-01",
        "      - DMIT",
        "      - 自动选择",
        "",
        "rules:",
        "  - DOMAIN-SUFFIX,google.com,节点选择",
        "  - GEOIP,CN,DIRECT",
        "  - MATCH,漏网之鱼",
        "",
    ]
    .join("\n")
}

#[cfg(test)]
#[path = "../tests/gui/demo_tests.rs"]
mod tests;

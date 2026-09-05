//! Per-app proxy routing configuration, corporate subnet detection, process traffic tracking,
//! and multi-platform process alias registry.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::{Arc, RwLock};

#[path = "app_routing_aliases.rs"]
mod app_routing_aliases;
#[path = "app_routing_bridges.rs"]
mod app_routing_bridges;
#[path = "app_routing_matching.rs"]
mod app_routing_matching;
#[path = "app_routing_process.rs"]
mod app_routing_process;
#[path = "app_routing_subnets.rs"]
mod app_routing_subnets;

/// Routing mode for per-app proxy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppRoutingMode {
    #[default]
    ProxyAll,
    ProxySelected,
    BypassSelected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AppRoutingRule {
    #[default]
    Proxy,
    Direct,
    Block,
}

/// Per-app routing configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppRoutingConfig {
    pub mode: AppRoutingMode,
    #[serde(default)]
    pub packages: HashSet<String>,
    #[serde(default)]
    pub rules: HashMap<String, AppRoutingRule>,
}

impl AppRoutingConfig {
    pub fn should_proxy(&self, package: &str) -> bool {
        if let Some(rule) = self.rules.get(package) {
            return matches!(rule, AppRoutingRule::Proxy);
        }
        match self.mode {
            AppRoutingMode::ProxyAll => true,
            AppRoutingMode::ProxySelected => self.packages.contains(package),
            AppRoutingMode::BypassSelected => !self.packages.contains(package),
        }
    }

    pub fn get_allowed_packages(&self) -> Option<Vec<String>> {
        match self.mode {
            AppRoutingMode::ProxyAll => None,
            AppRoutingMode::ProxySelected => {
                if self.packages.is_empty() && self.rules.is_empty() {
                    None
                } else {
                    Some(
                        self.packages
                            .iter()
                            .cloned()
                            .chain(
                                self.rules
                                    .iter()
                                    .filter_map(|(package, rule)| {
                                        matches!(rule, AppRoutingRule::Proxy)
                                            .then_some(package.clone())
                                    }),
                            )
                            .collect(),
                    )
                }
            }
            AppRoutingMode::BypassSelected => None,
        }
    }

    pub fn get_disallowed_packages(&self) -> Option<Vec<String>> {
        match self.mode {
            AppRoutingMode::BypassSelected => {
                if self.packages.is_empty()
                    && !self
                        .rules
                        .values()
                        .any(|rule| matches!(rule, AppRoutingRule::Direct | AppRoutingRule::Block))
                {
                    None
                } else {
                    Some(
                        self.packages
                            .iter()
                            .cloned()
                            .chain(self.rules.iter().filter_map(|(package, rule)| {
                                matches!(rule, AppRoutingRule::Direct | AppRoutingRule::Block)
                                    .then_some(package.clone())
                            }))
                            .collect(),
                    )
                }
            }
            _ => None,
        }
    }
}

// ============================================================================
// Corporate Subnet Detector
// ============================================================================

/// Categorization of detected subnets
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubnetCategory {
    /// RFC 1918 Private IPv4 (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
    PrivateIpv4,
    /// RFC 6598 Carrier-Grade NAT (100.64.0.0/10)
    Cgnat,
    /// RFC 4193 IPv6 Unique Local Address (fd00::/8)
    PrivateIpv6,
    /// RFC 4291 IPv6 Link-Local Unicast (fe80::/10)
    LinkLocalIpv6,
    /// User-defined corporate or intranet subnet
    CustomCorporate,
}

fn matches_ipv4_cidr(net: Ipv4Addr, prefix: u8, target: Ipv4Addr) -> bool {
    if prefix > 32 {
        return false;
    }
    if prefix == 0 {
        return true;
    }
    let mask = !0u32 << (32 - prefix);
    (u32::from(net) & mask) == (u32::from(target) & mask)
}

fn matches_ipv6_cidr(net: Ipv6Addr, prefix: u8, target: Ipv6Addr) -> bool {
    if prefix > 128 {
        return false;
    }
    if prefix == 0 {
        return true;
    }
    let mask = !0u128 << (128 - prefix);
    (u128::from(net) & mask) == (u128::from(target) & mask)
}

pub fn parse_cidr(cidr: &str) -> Option<(IpAddr, u8)> {
    let trimmed = cidr.trim();
    if let Some((ip_str, prefix_str)) = trimmed.split_once('/') {
        let ip = ip_str.trim().parse::<IpAddr>().ok()?;
        let prefix = prefix_str.trim().parse::<u8>().ok()?;
        let max_prefix = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix <= max_prefix {
            Some((ip, prefix))
        } else {
            None
        }
    } else {
        let ip = trimmed.parse::<IpAddr>().ok()?;
        let prefix = match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        Some((ip, prefix))
    }
}

pub fn matches_cidr(cidr: &str, ip: IpAddr) -> bool {
    let Some((net_ip, prefix)) = parse_cidr(cidr) else {
        return false;
    };
    match (net_ip, ip) {
        (IpAddr::V4(net), IpAddr::V4(target)) => matches_ipv4_cidr(net, prefix, target),
        (IpAddr::V6(net), IpAddr::V6(target)) => matches_ipv6_cidr(net, prefix, target),
        _ => false,
    }
}

/// Detector identifying private, CGNAT, and corporate subnets, and generating Direct bypass rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorporateSubnetDetector {
    custom_subnets: Vec<String>,
}

// ============================================================================
// Process Usage Tracker
// ============================================================================

/// In-memory snapshot of bandwidth usage and connections for a process
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessTrafficSnapshot {
    pub process_name: String,
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
    pub connection_count: u64,
    pub active_connections: u64,
    pub last_active_epoch_secs: u64,
}

#[derive(Debug, Clone, Default)]
struct ProcessUsageMetrics {
    upload_bytes: u64,
    download_bytes: u64,
    connection_count: u64,
    active_connections: u64,
    last_active_epoch_secs: u64,
}

/// Thread-safe in-memory bandwidth and connection tracker per process
#[derive(Debug, Clone)]
pub struct ProcessUsageTracker {
    state: Arc<RwLock<HashMap<String, ProcessUsageMetrics>>>,
    registry: ProcessAliasRegistry,
    auto_canonicalize: bool,
}

// ============================================================================
// Process Alias Registry
// ============================================================================

/// Registry that canonicalizes multi-platform binary and package names
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessAliasRegistry {
    aliases: HashMap<String, String>,
}

const BUILTIN_ALIAS_TABLE: &[(&[&str], &str)] = &[
    (
        &[
            "Google Chrome",
            "chrome.exe",
            "google-chrome-stable",
            "google-chrome",
            "google-chrome-beta",
            "google-chrome-unstable",
            "com.android.chrome",
            "Chrome.app",
            "chrome",
        ],
        "chrome",
    ),
    (
        &[
            "Firefox",
            "firefox.exe",
            "firefox-bin",
            "firefox-esr",
            "org.mozilla.firefox",
            "Firefox.app",
            "firefox",
        ],
        "firefox",
    ),
    (
        &[
            "Microsoft Edge",
            "msedge.exe",
            "msedge",
            "microsoft-edge-stable",
            "microsoft-edge",
            "com.microsoft.emmx",
            "Microsoft Edge.app",
        ],
        "msedge",
    ),
    (
        &[
            "Brave",
            "brave.exe",
            "brave-browser",
            "com.brave.browser",
            "Brave Browser.app",
            "brave",
        ],
        "brave",
    ),
    (
        &[
            "Safari",
            "safari.exe",
            "Safari.app",
            "com.apple.mobilesafari",
            "safari",
        ],
        "safari",
    ),
    (
        &[
            "Opera",
            "opera.exe",
            "com.opera.browser",
            "Opera.app",
            "opera",
        ],
        "opera",
    ),
    (
        &[
            "Vivaldi",
            "vivaldi.exe",
            "vivaldi-bin",
            "com.vivaldi.browser",
            "vivaldi",
        ],
        "vivaldi",
    ),
    (
        &["Arc", "Arc.exe", "Arc.app", "company.thebrowser.Arc", "arc"],
        "arc",
    ),
    (
        &[
            "Chromium",
            "chromium.exe",
            "chromium-browser",
            "org.chromium.Chromium",
            "chromium",
        ],
        "chromium",
    ),
    (
        &["Tor Browser", "torbrowser.exe", "tor-browser", "torbrowser"],
        "torbrowser",
    ),
    (
        &[
            "Visual Studio Code",
            "code.exe",
            "Code.exe",
            "code",
            "Code",
            "Code - OSS",
            "com.microsoft.VSCode",
        ],
        "code",
    ),
    (
        &[
            "IntelliJ IDEA",
            "idea64.exe",
            "idea.exe",
            "idea",
            "com.jetbrains.intellij",
        ],
        "intellij",
    ),
    (&["CLion", "clion64.exe", "clion.exe", "clion"], "clion"),
    (
        &["PyCharm", "pycharm64.exe", "pycharm.exe", "pycharm"],
        "pycharm",
    ),
    (
        &["WebStorm", "webstorm64.exe", "webstorm.exe", "webstorm"],
        "webstorm",
    ),
    (
        &["GoLand", "goland64.exe", "goland.exe", "goland"],
        "goland",
    ),
    (
        &["DataGrip", "datagrip64.exe", "datagrip.exe", "datagrip"],
        "datagrip",
    ),
    (
        &["Sublime Text", "sublime_text.exe", "sublime_text", "subl"],
        "sublime_text",
    ),
    (&["Cursor", "cursor.exe", "Cursor.app", "cursor"], "cursor"),
    (&["Zed", "zed.exe", "Zed.app", "zed"], "zed"),
    (
        &["Postman", "postman.exe", "Postman.app", "postman"],
        "postman",
    ),
    (
        &["Insomnia", "insomnia.exe", "Insomnia.app", "insomnia"],
        "insomnia",
    ),
    (
        &["Wireshark", "wireshark.exe", "Wireshark.app", "wireshark"],
        "wireshark",
    ),
    (&["git", "git.exe"], "git"),
    (&["curl", "curl.exe"], "curl"),
    (&["wget", "wget.exe"], "wget"),
    (&["node", "node.exe", "nodejs"], "node"),
    (
        &[
            "python",
            "python.exe",
            "python3",
            "python3.exe",
            "python3.11",
            "python3.12",
        ],
        "python",
    ),
    (
        &[
            "docker",
            "docker.exe",
            "dockerd",
            "dockerd.exe",
            "com.docker.backend",
        ],
        "docker",
    ),
    (
        &[
            "Telegram",
            "Telegram.exe",
            "telegram-desktop",
            "telegram",
            "org.telegram.desktop",
            "org.telegram.messenger",
        ],
        "telegram",
    ),
    (
        &[
            "Discord",
            "Discord.exe",
            "discord",
            "DiscordCanary.exe",
            "com.discord",
        ],
        "discord",
    ),
    (
        &["Slack", "Slack.exe", "slack", "com.tinyspeck.slackmacgap"],
        "slack",
    ),
    (
        &[
            "Teams",
            "Teams.exe",
            "teams",
            "ms-teams.exe",
            "msteams.exe",
            "com.microsoft.teams",
        ],
        "teams",
    ),
    (
        &[
            "WeChat",
            "WeChat.exe",
            "wechat",
            "Weixin.exe",
            "com.tencent.mm",
        ],
        "wechat",
    ),
    (&["QQ", "QQ.exe", "qq", "com.tencent.mobileqq"], "qq"),
    (
        &[
            "DingTalk",
            "DingTalk.exe",
            "dingtalk",
            "com.alibaba.android.rimet",
        ],
        "dingtalk",
    ),
    (
        &[
            "Feishu",
            "Feishu.exe",
            "feishu",
            "Lark.exe",
            "lark",
            "Lark",
            "com.ss.android.lark",
        ],
        "feishu",
    ),
    (
        &["Zoom", "Zoom.exe", "zoom", "us.zoom.videomeetings"],
        "zoom",
    ),
    (
        &["Skype", "Skype.exe", "skype", "com.skype.raider"],
        "skype",
    ),
    (
        &[
            "Signal",
            "Signal.exe",
            "signal-desktop",
            "signal",
            "org.thoughtcrime.securesms",
        ],
        "signal",
    ),
    (
        &["WhatsApp", "WhatsApp.exe", "whatsapp", "com.whatsapp"],
        "whatsapp",
    ),
    (
        &["Spotify", "Spotify.exe", "spotify", "com.spotify.music"],
        "spotify",
    ),
    (
        &[
            "Steam",
            "Steam.exe",
            "steam",
            "steamwebhelper.exe",
            "steamwebhelper",
            "com.valvesoftware.Steam",
        ],
        "steam",
    ),
    (
        &[
            "Epic Games Launcher",
            "EpicGamesLauncher.exe",
            "EpicWebHelper.exe",
            "epicgames",
        ],
        "epicgames",
    ),
    (
        &["Battle.net", "Battle.net.exe", "Agent.exe", "battlenet"],
        "battlenet",
    ),
    (
        &[
            "OBS Studio",
            "OBS64.exe",
            "obs.exe",
            "obs",
            "obs-studio",
            "com.obsproject.Studio",
        ],
        "obs",
    ),
    (&["VLC", "vlc.exe", "vlc", "org.videolan.vlc"], "vlc"),
    (&["mpv", "mpv.exe", "io.mpv.Mpv"], "mpv"),
    (
        &[
            "Apple Music",
            "AppleMusic.exe",
            "Music.app",
            "com.apple.Music",
        ],
        "applemusic",
    ),
    (
        &[
            "NeteaseMusic",
            "cloudmusic.exe",
            "netease-cloud-music",
            "com.netease.cloudmusic",
        ],
        "neteasemusic",
    ),
    (
        &["QQMusic", "QQMusic.exe", "qqmusic", "com.tencent.qqmusic"],
        "qqmusic",
    ),
    (
        &["YouTube", "YouTube.exe", "com.google.android.youtube"],
        "youtube",
    ),
    (
        &["Netflix", "Netflix.exe", "com.netflix.mediaclient"],
        "netflix",
    ),
    (
        &["Dropbox", "Dropbox.exe", "dropbox", "com.dropbox.android"],
        "dropbox",
    ),
    (
        &[
            "OneDrive",
            "OneDrive.exe",
            "onedrive",
            "com.microsoft.skydrive",
        ],
        "onedrive",
    ),
    (
        &[
            "Google Drive",
            "GoogleDriveFS.exe",
            "googledrive",
            "com.google.android.apps.docs",
        ],
        "googledrive",
    ),
    (&["Notion", "Notion.exe", "notion-app", "notion"], "notion"),
    (
        &["Microsoft Word", "WINWORD.EXE", "winword.exe", "word"],
        "word",
    ),
    (
        &["Microsoft Excel", "EXCEL.EXE", "excel.exe", "excel"],
        "excel",
    ),
    (
        &[
            "Microsoft PowerPoint",
            "POWERPNT.EXE",
            "powerpnt.exe",
            "powerpoint",
        ],
        "powerpoint",
    ),
    (&["Microsoft Outlook", "Outlook.exe", "outlook"], "outlook"),
    (
        &[
            "mihomo",
            "mihomo.exe",
            "clash-meta.exe",
            "clash-meta",
            "clash.exe",
            "clash",
        ],
        "mihomo",
    ),
    (&["sing-box", "sing-box.exe"], "sing-box"),
    (&["xray", "xray.exe", "v2ray.exe", "v2ray"], "xray"),
];

// ============================================================================
// Virtual Network Bridge & Container Isolation
// ============================================================================

/// Type classification for virtual network interfaces and container bridges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualBridgeType {
    /// WSL2 NAT virtual switch (vEthernet WSL)
    Wsl2Nat,
    /// WSL2 Mirrored networking mode (Windows 11 23H2+)
    Wsl2Mirrored,
    /// Docker default bridge network (docker0 / 172.17.0.0/16)
    DockerDefaultBridge,
    /// Docker user-defined bridge network (br-* / 172.18.0.0/16+)
    DockerCustomBridge,
    /// Podman container bridge network (podman0 / 10.88.0.0/16)
    PodmanBridge,
    /// Hyper-V Default Switch / Virtual NAT
    HyperVDefault,
    /// Custom virtual network bridge
    CustomVirtual,
}

/// Routing policy for virtual bridges and container networks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BridgeRoutingMode {
    /// Direct bypass for all container and virtual network bridges (recommended)
    #[default]
    BypassAllBridges,
    /// Route WSL2 outbound traffic through proxy, bypass Docker containers
    ProxyWslOnly,
    /// Route Docker container traffic through proxy, bypass WSL2
    ProxyDockerOnly,
    /// Route all virtual bridge traffic through proxy
    ProxyAllBridges,
}

/// Representation of an identified virtual network bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualNetworkBridge {
    pub bridge_type: VirtualBridgeType,
    pub interface_name: String,
    pub subnet_cidr: String,
    pub gateway_ip: Option<IpAddr>,
    pub is_mirrored_mode: bool,
}

/// Detector and rule compiler for virtual network bridges (WSL2, Docker, Hyper-V).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VirtualNetworkBridgeDetector {
    bridges: Vec<VirtualNetworkBridge>,
}
#[cfg(test)]
#[path = "app_routing_test.rs"]
mod tests;

// ============================================================================
// Linux Cgroup v2 Path Classifier
// ============================================================================

/// Classifier for Linux cgroup v2 slice and service hierarchies.
pub struct CgroupV2Classifier;

// ============================================================================
// Cross-Platform Canonical App Routing Rules
// ============================================================================

/// Cross-platform application routing entry referencing a canonical app identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalAppRule {
    pub canonical_id: String,
    pub target_policy: String,
    pub enabled: bool,
}

use super::ProcessCategory;
use std::path::Path;

/// Known system daemon names across Windows, Linux, and macOS.
const SYSTEM_DAEMONS: &[&str] = &[
    // Windows core & services
    "system",
    "registry",
    "smss",
    "csrss",
    "wininit",
    "services",
    "lsass",
    "lsm",
    "winlogon",
    "spoolsv",
    "dwm",
    "fontdrvhost",
    "sihost",
    "taskhostw",
    "taskhost",
    "rundll32",
    "conhost",
    "runtimebroker",
    "searchhost",
    "searchindexer",
    "startmenuexperiencehost",
    "shellexperiencehost",
    "audiodg",
    "wlanext",
    "dashost",
    "ctfmon",
    "smartscreen",
    "securityhealthservice",
    "securityhealthsystray",
    "mpcmdrun",
    "mssense",
    "msmpeng",
    "nisrv",
    "dllhost",
    "svchost",
    "wmsvc",
    "msdtc",
    // Linux core & daemons
    "systemd",
    "systemd-journald",
    "systemd-udevd",
    "systemd-networkd",
    "systemd-resolved",
    "systemd-timesyncd",
    "systemd-logind",
    "systemd-oomd",
    "systemd-userwork",
    "kthreadd",
    "dbus-daemon",
    "dbus-broker",
    "polkitd",
    "cron",
    "crond",
    "atd",
    "rsyslogd",
    "syslogd",
    "sshd",
    "cupsd",
    "avahi-daemon",
    "accounts-daemon",
    "upowerd",
    "udisksd",
    "boltd",
    "colord",
    "rtkit-daemon",
    "wpa_supplicant",
    "networkmanager",
    "modemmanager",
    "irqbalance",
    "thermald",
    "pipewire",
    "pipewire-pulse",
    "wireplumber",
    "seatd",
    "greetd",
    "gdm",
    "gdm-session-worker",
    "sddm",
    "lightdm",
    "iio-sensor-proxy",
    // macOS daemons & frameworks
    "kernel_task",
    "launchd",
    "logd",
    "fseventsd",
    "coreaudiod",
    "distnoted",
    "notifyd",
    "powerd",
    "diskarbitrationd",
    "usereventagent",
    "opendirectoryd",
    "securityd",
    "trustd",
    "tccd",
    "mds",
    "mds_stores",
    "mdworker",
    "windowserver",
    "airportd",
    "bluetoothd",
    "identityservicesd",
    "rapportd",
    "runningboardd",
    "amfid",
    "containermanagerd",
    "sysmond",
    "thermalmonitord",
];

/// Known user desktop application signatures.
const KNOWN_USER_APPS: &[&str] = &[
    // Browsers
    "chrome",
    "google-chrome",
    "google-chrome-stable",
    "firefox",
    "firefox-bin",
    "msedge",
    "edge",
    "brave",
    "brave-browser",
    "opera",
    "opera-developer",
    "vivaldi",
    "vivaldi-bin",
    "safari",
    "tor",
    "tor-browser",
    "chromium",
    "chromium-browser",
    "arc",
    "zen",
    "zen-bin",
    "waterfox",
    "librewolf",
    // IM & Chat
    "discord",
    "telegram",
    "telegram-desktop",
    "telegramdesktop",
    "slack",
    "wechat",
    "weixin",
    "qq",
    "dingtalk",
    "feishu",
    "lark",
    "whatsapp",
    "whatsapp-desktop",
    "signal",
    "signal-desktop",
    "teams",
    "ms-teams",
    "element",
    "element-desktop",
    "skype",
    "skypeforlinux",
    "zoom",
    "zoom.us",
    "mattermost",
    // Dev Tools
    "code",
    "code-oss",
    "vscode",
    "cursor",
    "zed",
    "clion",
    "idea",
    "pycharm",
    "webstorm",
    "rustrover",
    "goland",
    "rider",
    "datagrip",
    "android-studio",
    "sublime_text",
    "subl",
    "neovim",
    "nvim",
    "emacs",
    "warp",
    "iterm2",
    "alacritty",
    "kitty",
    "wezterm",
    "wezterm-gui",
    "hyper",
    "postman",
    "insomnia",
    "dbeaver",
    "docker",
    "docker desktop",
    "git-cola",
    "gitkraken",
    "fork",
    // Gaming & Media
    "steam",
    "steamwebhelper",
    "epicgameslauncher",
    "battle.net",
    "riotclientux",
    "riotclientservices",
    "gog galaxy",
    "galaxyclient",
    "origin",
    "eadesktop",
    "lutris",
    "heroic",
    "spotify",
    "music",
    "applemusic",
    "neteasemusic",
    "cloudmusic",
    "qqmusic",
    "vlc",
    "mpv",
    "foobar2000",
    "obs",
    "obs64",
    "potplayer",
    // Office & Proxies
    "notion",
    "obsidian",
    "logseq",
    "winword",
    "excel",
    "powerpnt",
    "onenote",
    "wps",
    "wpp",
    "et",
    "thunderbird",
    "figma",
    "canva",
    "mihomo",
    "clash",
    "clash-meta",
    "clash-nyanpasu",
    "clash-verge",
    "infiltrator",
    "sing-box",
    "v2ray",
    "xray",
    "surge",
    "shadowsocks",
    "tailscale",
    "wireguard",
];

/// Classifies a process into semantic categories for grouping in UI.
pub fn classify_process_category(
    name: &str,
    binary_path: Option<&str>,
    is_system: bool,
) -> ProcessCategory {
    if is_system {
        return ProcessCategory::SystemDaemon;
    }

    let lower = name.trim().to_ascii_lowercase();
    let s = lower.strip_suffix(".exe").unwrap_or(&lower);

    // Browsers
    if s.contains("chrome")
        || s.contains("firefox")
        || s.contains("msedge")
        || s == "edge"
        || s.contains("brave")
        || s.contains("opera")
        || s.contains("vivaldi")
        || s.contains("safari")
        || s.contains("tor")
        || s.contains("chromium")
        || s.contains("arc")
        || s.contains("zen")
        || s.contains("waterfox")
        || s.contains("librewolf")
    {
        return ProcessCategory::Browser;
    }

    // Communication
    if s.contains("discord")
        || s.contains("telegram")
        || s.contains("slack")
        || s.contains("wechat")
        || s.contains("weixin")
        || s == "qq"
        || s.starts_with("qq")
        || s.contains("dingtalk")
        || s.contains("feishu")
        || s.contains("lark")
        || s.contains("whatsapp")
        || s.contains("signal")
        || s.contains("teams")
        || s.contains("zoom")
        || s.contains("skype")
        || s.contains("element")
    {
        return ProcessCategory::Communication;
    }

    // Developer tools
    if s.contains("code")
        || s.contains("vscode")
        || s.contains("cursor")
        || s.contains("zed")
        || s.contains("clion")
        || s.contains("idea")
        || s.contains("pycharm")
        || s.contains("webstorm")
        || s.contains("rustrover")
        || s.contains("goland")
        || s.contains("rider")
        || s.contains("datagrip")
        || s.contains("android-studio")
        || s.contains("sublime")
        || s.contains("nvim")
        || s.contains("neovim")
        || s.contains("emacs")
        || s.contains("terminal")
        || s.contains("alacritty")
        || s.contains("kitty")
        || s.contains("wezterm")
        || s.contains("iterm")
        || s.contains("warp")
        || s.contains("postman")
        || s.contains("insomnia")
        || s.contains("docker")
        || s.contains("git")
    {
        return ProcessCategory::Developer;
    }

    // Gaming
    if s.contains("steam")
        || s.contains("epicgames")
        || s.contains("battle.net")
        || s.contains("riot")
        || s.contains("origin")
        || s.contains("eadesktop")
        || s.contains("lutris")
        || s.contains("heroic")
    {
        return ProcessCategory::Gaming;
    }

    // Office & Notes
    if s.contains("notion")
        || s.contains("obsidian")
        || s.contains("logseq")
        || s.contains("winword")
        || s.contains("excel")
        || s.contains("powerpnt")
        || s.contains("onenote")
        || s.contains("wps")
        || s.contains("thunderbird")
        || s.contains("figma")
    {
        return ProcessCategory::Office;
    }

    // Media
    if s.contains("spotify")
        || s.contains("cloudmusic")
        || s.contains("netease")
        || s.contains("qqmusic")
        || s.contains("applemusic")
        || s.contains("vlc")
        || s.contains("mpv")
        || s.contains("foobar")
        || s == "obs"
        || s == "obs64"
        || s.starts_with("obs-")
        || s.starts_with("obs64")
        || s.contains("obs-studio")
        || s.contains("potplayer")
    {
        return ProcessCategory::Media;
    }

    // Network / Proxy / VPN
    if s.contains("mihomo")
        || s.contains("clash")
        || s.contains("infiltrator")
        || s.contains("sing-box")
        || s.contains("v2ray")
        || s.contains("xray")
        || s.contains("tailscale")
        || s.contains("wireguard")
        || s.contains("surge")
    {
        return ProcessCategory::NetworkVpn;
    }

    if let Some(path) = binary_path {
        let p_lower = path.to_ascii_lowercase();
        if p_lower.contains("/games/") || p_lower.contains("\\games\\") {
            return ProcessCategory::Gaming;
        }
        if p_lower.contains("/office/") || p_lower.contains("\\office\\") {
            return ProcessCategory::Office;
        }
    }

    ProcessCategory::Other
}

/// Identifies whether a process is a background system daemon, kernel thread, or OS service.
pub fn is_system_process(name: &str, binary_path: Option<&str>, pid: u32) -> bool {
    let clean = name.trim();
    if clean.is_empty() || pid == 0 {
        return true;
    }
    if clean.starts_with('[') && clean.ends_with(']') {
        return true;
    }

    let lower = clean.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(&lower);

    if is_known_user_app(stripped) {
        return false;
    }

    if (pid == 1 || pid == 2)
        && (stripped == "systemd" || stripped == "init" || stripped == "kthreadd")
    {
        return true;
    }

    if SYSTEM_DAEMONS.contains(&stripped) {
        return true;
    }

    if stripped.starts_with("kworker")
        || stripped.starts_with("ksoftirqd")
        || stripped.starts_with("migration")
        || stripped.starts_with("rcu_")
        || stripped.starts_with("systemd-")
    {
        return true;
    }

    if let Some(path) = binary_path {
        let norm = path.replace('\\', "/").to_ascii_lowercase();
        if norm.contains("/usr/lib/systemd/")
            || norm.contains("/lib/systemd/")
            || norm.contains("/usr/libexec/")
            || norm.contains("/system/library/")
        {
            return true;
        }
        if norm.starts_with("/sbin/") || norm.starts_with("/usr/sbin/") {
            return true;
        }
        if (norm.contains("/windows/system32/") || norm.contains("/windows/syswow64/"))
            && (norm.ends_with("/svchost.exe")
                || norm.ends_with("/csrss.exe")
                || norm.ends_with("/smss.exe")
                || norm.ends_with("/services.exe")
                || norm.ends_with("/lsass.exe")
                || norm.ends_with("/wininit.exe")
                || norm.ends_with("/conhost.exe")
                || norm.ends_with("/dllhost.exe")
                || norm.ends_with("/dwm.exe")
                || norm.ends_with("/sihost.exe")
                || norm.ends_with("/taskhostw.exe"))
        {
            return true;
        }
    }

    false
}

/// Determines if an executable or process name belongs to a recognized desktop user application.
pub fn is_known_user_app(name: &str) -> bool {
    let lower = name.trim().to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(&lower);
    KNOWN_USER_APPS
        .iter()
        .any(|&app| app == stripped || stripped.starts_with(app))
}

/// Resolves an appropriate icon hint identifier for UI presentation.
pub fn resolve_icon_hint(name: &str, binary_path: Option<&str>) -> Option<String> {
    let lower = name.trim().to_ascii_lowercase();
    let s = lower.strip_suffix(".exe").unwrap_or(&lower);

    let hint = if s.contains("chrome") {
        "google-chrome"
    } else if s.contains("firefox") || s.contains("librewolf") || s.contains("waterfox") {
        "firefox"
    } else if s.contains("msedge") || s == "edge" {
        "microsoft-edge"
    } else if s.contains("brave") {
        "brave-browser"
    } else if s.contains("opera") {
        "opera"
    } else if s.contains("vivaldi") {
        "vivaldi"
    } else if s.contains("tor") {
        "tor-browser"
    } else if s.contains("discord") {
        "discord"
    } else if s.contains("telegram") {
        "telegram"
    } else if s.contains("slack") {
        "slack"
    } else if s.contains("wechat") || s.contains("weixin") {
        "wechat"
    } else if s == "qq" || s.starts_with("qq") {
        "qq"
    } else if s.contains("steam") {
        "steam"
    } else if s.contains("spotify") {
        "spotify"
    } else if s.contains("cloudmusic") || s.contains("netease") {
        "netease-cloud-music"
    } else if s.contains("code") || s.contains("vscode") {
        "visual-studio-code"
    } else if s.contains("cursor") {
        "cursor"
    } else if s.contains("zed") {
        "zed"
    } else if s.contains("idea")
        || s.contains("clion")
        || s.contains("pycharm")
        || s.contains("webstorm")
        || s.contains("rustrover")
        || s.contains("goland")
        || s.contains("rider")
    {
        "jetbrains"
    } else if s.contains("obsidian") {
        "obsidian"
    } else if s.contains("notion") {
        "notion"
    } else if s.contains("vlc") {
        "vlc"
    } else if s.contains("mpv") {
        "mpv"
    } else if s.contains("obs") || s.contains("obs64") {
        "obs-studio"
    } else if s.contains("postman") {
        "postman"
    } else if s.contains("docker") {
        "docker"
    } else if s.contains("thunderbird") {
        "thunderbird"
    } else if s.contains("terminal")
        || s.contains("alacritty")
        || s.contains("kitty")
        || s.contains("wezterm")
        || s.contains("iterm")
        || s.contains("warp")
    {
        "utilities-terminal"
    } else if s.contains("mihomo")
        || s.contains("clash")
        || s.contains("infiltrator")
        || s.contains("sing-box")
    {
        "network-vpn"
    } else if let Some(path) = binary_path {
        if let Some(stem) = Path::new(path).file_stem().and_then(|st| st.to_str()) {
            let clean_stem = stem.trim().to_ascii_lowercase();
            if !clean_stem.is_empty() && clean_stem != s {
                return Some(clean_stem);
            }
        }
        s
    } else if !s.is_empty() {
        s
    } else {
        return None;
    };

    Some(hint.to_string())
}

/// Normalizes raw executable names to friendly user-facing labels.
pub fn normalize_display_name(raw_name: &str) -> String {
    let trimmed = raw_name.trim();
    let lower = trimmed.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(&lower);

    if stripped.contains("chrome") {
        "Google Chrome".to_string()
    } else if stripped.contains("firefox")
        || stripped.contains("librewolf")
        || stripped.contains("waterfox")
    {
        "Mozilla Firefox".to_string()
    } else if stripped.contains("msedge") || stripped == "edge" {
        "Microsoft Edge".to_string()
    } else if stripped.contains("brave") {
        "Brave Browser".to_string()
    } else if stripped.contains("opera") {
        "Opera".to_string()
    } else if stripped.contains("vivaldi") {
        "Vivaldi".to_string()
    } else if stripped.contains("tor") {
        "Tor Browser".to_string()
    } else if stripped.contains("discord") {
        "Discord".to_string()
    } else if stripped.contains("telegram") {
        "Telegram".to_string()
    } else if stripped.contains("slack") {
        "Slack".to_string()
    } else if stripped.contains("wechat") || stripped.contains("weixin") {
        "WeChat".to_string()
    } else if stripped == "qq" || stripped.starts_with("qq") {
        "QQ".to_string()
    } else if stripped.contains("steam") {
        "Steam".to_string()
    } else if stripped.contains("spotify") {
        "Spotify".to_string()
    } else if stripped.contains("cloudmusic") || stripped.contains("neteasemusic") {
        "NetEase Cloud Music".to_string()
    } else if stripped == "code" || stripped == "code-oss" || stripped == "vscode" {
        "Visual Studio Code".to_string()
    } else if stripped.contains("cursor") {
        "Cursor".to_string()
    } else if stripped.contains("zed") {
        "Zed".to_string()
    } else if stripped.contains("clion") {
        "CLion".to_string()
    } else if stripped.contains("idea") {
        "IntelliJ IDEA".to_string()
    } else if stripped.contains("pycharm") {
        "PyCharm".to_string()
    } else if stripped.contains("webstorm") {
        "WebStorm".to_string()
    } else if stripped.contains("rustrover") {
        "RustRover".to_string()
    } else if stripped.contains("goland") {
        "GoLand".to_string()
    } else if stripped.contains("rider") {
        "Rider".to_string()
    } else if stripped.contains("obsidian") {
        "Obsidian".to_string()
    } else if stripped.contains("notion") {
        "Notion".to_string()
    } else if stripped.contains("vlc") {
        "VLC Media Player".to_string()
    } else if stripped.contains("mpv") {
        "MPV Media Player".to_string()
    } else if stripped.contains("obs") || stripped.contains("obs64") {
        "OBS Studio".to_string()
    } else if stripped.contains("postman") {
        "Postman".to_string()
    } else if stripped.contains("docker") {
        "Docker Desktop".to_string()
    } else if stripped.contains("thunderbird") {
        "Mozilla Thunderbird".to_string()
    } else if stripped.contains("mihomo") {
        "Mihomo Core".to_string()
    } else if stripped.contains("infiltrator") {
        "Infiltrator".to_string()
    } else if stripped.contains("sing-box") {
        "sing-box".to_string()
    } else {
        trimmed.strip_suffix(".exe").unwrap_or(trimmed).to_string()
    }
}

//! Process-level connection icon tokens and Top-Talkers traffic aggregation (FLPARITY-032).
//!
//! Provides strongly-typed models for classifying applications, aggregating live
//! and historic network consumption per process, and driving the Top-Talkers
//! ranking inspector.

use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

/// High-level application category classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessCategory {
    /// Web browsers (Chrome, Firefox, Edge, Safari, Brave, Arc, etc.).
    Browser,
    /// Chat and collaboration (Telegram, Discord, WeChat, Slack, etc.).
    Chat,
    /// Command line and developer terminals (Bash, Zsh, PowerShell, Cmd, Git, etc.).
    Terminal,
    /// Media and streaming (Spotify, YouTube, VLC, Music, etc.).
    Media,
    /// Developer tools and IDEs (VSCode, Cursor, Zed, IntelliJ, Xcode, etc.).
    Development,
    /// Download managers and network utilities (Curl, Wget, Aria2, qBittorrent, etc.).
    Download,
    /// Operating system internal processes (systemd, svchost, launchd, etc.).
    System,
    /// Unknown or unclassified user process.
    Generic,
}

impl ProcessCategory {
    pub fn label_zh(&self) -> &'static str {
        match self {
            Self::Browser => "浏览器",
            Self::Chat => "通讯社交",
            Self::Terminal => "终端工具",
            Self::Media => "多媒体",
            Self::Development => "开发工具",
            Self::Download => "下载工具",
            Self::System => "系统进程",
            Self::Generic => "普通应用",
        }
    }
}

/// Normalized process icon token metadata.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProcessIconToken {
    pub category: ProcessCategory,
    pub icon_name: String,
}

impl ProcessIconToken {
    pub fn new(category: ProcessCategory, icon_name: impl Into<String>) -> Self {
        Self {
            category,
            icon_name: icon_name.into(),
        }
    }

    /// Classify a process name or executable filename into a canonical icon token.
    pub fn from_process_name(raw: &str) -> Self {
        let clean = raw.trim().to_ascii_lowercase();
        let filename = clean.rsplit(['/', '\\']).next().unwrap_or(&clean);
        let name = filename.strip_suffix(".exe").unwrap_or(filename);

        match name {
            "chrome" | "google-chrome" | "chromium" => {
                Self::new(ProcessCategory::Browser, "chrome")
            }
            "firefox" | "firefox-esr" | "firefox-bin" => {
                Self::new(ProcessCategory::Browser, "firefox")
            }
            "msedge" | "edge" => Self::new(ProcessCategory::Browser, "edge"),
            "safari" => Self::new(ProcessCategory::Browser, "safari"),
            "brave" | "brave-browser" => Self::new(ProcessCategory::Browser, "brave"),
            "arc" => Self::new(ProcessCategory::Browser, "arc"),
            "opera" | "vivaldi" | "tor-browser" => Self::new(ProcessCategory::Browser, "browser"),

            "telegram" | "telegram-desktop" => Self::new(ProcessCategory::Chat, "telegram"),
            "discord" | "discord-canary" => Self::new(ProcessCategory::Chat, "discord"),
            "wechat" | "weixin" => Self::new(ProcessCategory::Chat, "wechat"),
            "qq" | "tim" => Self::new(ProcessCategory::Chat, "qq"),
            "slack" => Self::new(ProcessCategory::Chat, "slack"),
            "whatsapp" => Self::new(ProcessCategory::Chat, "whatsapp"),
            "dingtalk" => Self::new(ProcessCategory::Chat, "dingtalk"),
            "feishu" | "lark" => Self::new(ProcessCategory::Chat, "feishu"),

            "bash" | "zsh" | "sh" | "pwsh" | "powershell" | "cmd" | "alacritty" | "kitty"
            | "wezterm" | "iterm2" | "conhost" | "windowsterminal" => {
                Self::new(ProcessCategory::Terminal, "terminal")
            }

            "spotify" => Self::new(ProcessCategory::Media, "spotify"),
            "vlc" | "mpv" | "iina" => Self::new(ProcessCategory::Media, "vlc"),
            "neteasemusic" | "cloudmusic" | "qqmusic" => Self::new(ProcessCategory::Media, "music"),

            "code" | "vscode" => Self::new(ProcessCategory::Development, "vscode"),
            "cursor" => Self::new(ProcessCategory::Development, "cursor"),
            "zed" => Self::new(ProcessCategory::Development, "zed"),
            "idea" | "pycharm" | "clion" | "webstorm" | "goland" | "rustrover" => {
                Self::new(ProcessCategory::Development, "jetbrains")
            }
            "xcode" | "studio" => Self::new(ProcessCategory::Development, "ide"),

            "curl" | "wget" | "aria2c" | "transmission" | "qbittorrent" => {
                Self::new(ProcessCategory::Download, "download")
            }

            "systemd" | "svchost" | "system" | "launchd" | "init" => {
                Self::new(ProcessCategory::System, "system")
            }

            _ => Self::new(ProcessCategory::Generic, "application"),
        }
    }
}

/// Aggregated traffic consumption for a specific application.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppTrafficAggregate {
    /// Normalized process name (e.g. "chrome", "telegram").
    pub process_name: String,
    /// Categorized process icon token.
    pub icon_token: ProcessIconToken,
    /// Number of concurrent active connections.
    pub connection_count: usize,
    /// Cumulative uploaded bytes across these connections.
    pub upload_total_bytes: u64,
    /// Cumulative downloaded bytes across these connections.
    pub download_total_bytes: u64,
    /// Identifiers of all active connections belonging to this process.
    pub connection_ids: Vec<String>,
}

impl AppTrafficAggregate {
    pub fn new(process_name: impl Into<String>) -> Self {
        let name = process_name.into();
        let icon_token = ProcessIconToken::from_process_name(&name);
        Self {
            process_name: name,
            icon_token,
            connection_count: 0,
            upload_total_bytes: 0,
            download_total_bytes: 0,
            connection_ids: Vec::new(),
        }
    }

    /// Total bandwidth volume (upload + download) in bytes.
    pub fn total_bytes(&self) -> u64 {
        self.upload_total_bytes
            .saturating_add(self.download_total_bytes)
    }

    /// Record a connection into this aggregate.
    pub fn add_connection(&mut self, conn_id: impl Into<String>, upload: u64, download: u64) {
        self.connection_count += 1;
        self.upload_total_bytes = self.upload_total_bytes.saturating_add(upload);
        self.download_total_bytes = self.download_total_bytes.saturating_add(download);
        self.connection_ids.push(conn_id.into());
    }
}

/// Ordering dimension for the Top-Talkers application traffic ranking.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TopTalkersSort {
    /// Sort by total traffic volume (upload + download) descending.
    #[default]
    TotalBytesDesc,
    /// Sort by downloaded bytes descending.
    DownloadBytesDesc,
    /// Sort by uploaded bytes descending.
    UploadBytesDesc,
    /// Sort by number of concurrent active connections descending.
    ConnectionCountDesc,
}

/// Top-Talkers summary report.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopTalkersReport {
    /// Sorted list of application traffic aggregates.
    pub applications: Vec<AppTrafficAggregate>,
    /// Global sum of all active connections.
    pub total_connections: usize,
    /// Global sum of total bandwidth consumed.
    pub total_bandwidth_bytes: u64,
}

impl TopTalkersReport {
    pub fn of(mut apps: Vec<AppTrafficAggregate>, sort: TopTalkersSort) -> Self {
        match sort {
            TopTalkersSort::TotalBytesDesc => {
                apps.sort_by_key(|app| Reverse(app.total_bytes()));
            }
            TopTalkersSort::DownloadBytesDesc => {
                apps.sort_by_key(|app| Reverse(app.download_total_bytes));
            }
            TopTalkersSort::UploadBytesDesc => {
                apps.sort_by_key(|app| Reverse(app.upload_total_bytes));
            }
            TopTalkersSort::ConnectionCountDesc => {
                apps.sort_by_key(|app| Reverse(app.connection_count));
            }
        }

        let total_connections = apps.iter().map(|a| a.connection_count).sum();
        let total_bandwidth_bytes = apps.iter().map(|a| a.total_bytes()).sum();

        Self {
            applications: apps,
            total_connections,
            total_bandwidth_bytes,
        }
    }

    /// Highest consuming application name, if any.
    pub fn top_talker(&self) -> Option<&AppTrafficAggregate> {
        self.applications.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_icon_token_classification() {
        assert_eq!(
            ProcessIconToken::from_process_name("/usr/bin/chrome"),
            ProcessIconToken::new(ProcessCategory::Browser, "chrome")
        );
        assert_eq!(
            ProcessIconToken::from_process_name(r"C:\Program Files\Telegram\Telegram.exe"),
            ProcessIconToken::new(ProcessCategory::Chat, "telegram")
        );
        assert_eq!(
            ProcessIconToken::from_process_name("pwsh.exe"),
            ProcessIconToken::new(ProcessCategory::Terminal, "terminal")
        );
        assert_eq!(
            ProcessIconToken::from_process_name("vlc"),
            ProcessIconToken::new(ProcessCategory::Media, "vlc")
        );
        assert_eq!(
            ProcessIconToken::from_process_name("custom-app-123"),
            ProcessIconToken::new(ProcessCategory::Generic, "application")
        );
    }

    #[test]
    fn top_talkers_report_aggregation_and_sorting() {
        let mut app1 = AppTrafficAggregate::new("chrome");
        app1.add_connection("c1", 100, 500);

        let mut app2 = AppTrafficAggregate::new("curl");
        app2.add_connection("c2", 1000, 2000);

        let report = TopTalkersReport::of(vec![app1, app2], TopTalkersSort::TotalBytesDesc);
        assert_eq!(report.applications.len(), 2);
        assert_eq!(report.top_talker().unwrap().process_name, "curl");
        assert_eq!(report.total_connections, 2);
        assert_eq!(report.total_bandwidth_bytes, 3600);
    }
}

//! Linux cgroup matching and canonical application route tables.

use infiltrator_domain::rules::RuleEntry;

use super::{CanonicalAppRule, CgroupV2Classifier};

impl CgroupV2Classifier {
    /// Extracts an application or service identifier from a raw cgroup v2 path.
    ///
    /// Examples:
    /// - `/user.slice/user-1000.slice/app-firefox.slice/app-firefox-1234.scope` -> Some("firefox")
    /// - `/system.slice/docker-abcdef.scope` -> Some("docker")
    /// - `/user.slice/user-1000.slice/app-flatpak-org.mozilla.firefox-5678.scope` -> Some("firefox")
    pub fn extract_app_name(cgroup_path: &str) -> Option<String> {
        let trimmed = cgroup_path.trim();
        if trimmed.is_empty() {
            return None;
        }

        for segment in trimmed.split('/') {
            let s = segment.trim();
            if s.starts_with("app-flatpak-") {
                let flatpak_id = s.strip_prefix("app-flatpak-").unwrap_or(s);
                let clean = flatpak_id.split('-').next().unwrap_or(flatpak_id);
                if let Some(last_dot) = clean.rsplit('.').next() {
                    return Some(last_dot.to_ascii_lowercase());
                }
                return Some(clean.to_ascii_lowercase());
            } else if s.starts_with("app-") && (s.ends_with(".slice") || s.ends_with(".scope")) {
                let stripped = s
                    .strip_prefix("app-")
                    .unwrap_or(s)
                    .strip_suffix(".slice")
                    .or_else(|| s.strip_prefix("app-")?.strip_suffix(".scope"))
                    .unwrap_or(s);
                let app_stem = stripped.split(['-', '@', ':']).next().unwrap_or(stripped);
                return Some(app_stem.to_ascii_lowercase());
            } else if s.starts_with("docker-") {
                return Some("docker".to_string());
            } else if s.starts_with("containerd-") {
                return Some("containerd".to_string());
            } else if s.starts_with("podman-") {
                return Some("podman".to_string());
            }
        }

        None
    }
}

impl CanonicalAppRule {
    /// Creates a new canonical routing rule.
    pub fn new(canonical_id: impl Into<String>, target_policy: impl Into<String>) -> Self {
        Self {
            canonical_id: canonical_id.into(),
            target_policy: target_policy.into(),
            enabled: true,
        }
    }

    /// Compiles the canonical rule into platform-appropriate `RuleEntry` items.
    pub fn compile_rules(&self) -> Vec<RuleEntry> {
        if !self.enabled {
            return Vec::new();
        }

        let cid = self.canonical_id.trim().to_ascii_lowercase();
        let target = self.target_policy.trim();

        let mut entries = Vec::new();

        match cid.as_str() {
            "chrome" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,chrome.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,chrome,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,google-chrome,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,google-chrome-stable,{target}"),
                    enabled: true,
                });
            }
            "firefox" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,firefox.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,firefox,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,firefox-bin,{target}"),
                    enabled: true,
                });
            }
            "msedge" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,msedge.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,msedge,{target}"),
                    enabled: true,
                });
            }
            "telegram" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,Telegram.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,telegram-desktop,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,telegram,{target}"),
                    enabled: true,
                });
            }
            "discord" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,Discord.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,discord,{target}"),
                    enabled: true,
                });
            }
            "code" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,Code.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,code,{target}"),
                    enabled: true,
                });
            }
            "steam" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,steam.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,steamwebhelper.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,steam,{target}"),
                    enabled: true,
                });
            }
            "spotify" => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,Spotify.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,spotify,{target}"),
                    enabled: true,
                });
            }
            _ => {
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,{cid}.exe,{target}"),
                    enabled: true,
                });
                entries.push(RuleEntry {
                    rule: format!("PROCESS-NAME,{cid},{target}"),
                    enabled: true,
                });
            }
        }

        entries
    }
}

//! Client desktop and mobile application self-updater checking and asset validation.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppReleaseInfo {
    pub version: String,
    pub tag_name: String,
    pub release_notes: String,
    pub published_at: String,
    pub download_url: String,
    pub sha256_checksum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheckOutcome {
    UpToDate {
        current_version: String,
    },
    UpdateAvailable {
        current_version: String,
        release: AppReleaseInfo,
    },
}

pub struct AppUpdater;

impl AppUpdater {
    pub fn check_version_diff(current: &str, latest: &AppReleaseInfo) -> UpdateCheckOutcome {
        let clean_cur = current.trim_start_matches('v').trim();
        let clean_lat = latest.version.trim_start_matches('v').trim();

        if clean_cur == clean_lat {
            UpdateCheckOutcome::UpToDate {
                current_version: current.to_string(),
            }
        } else {
            UpdateCheckOutcome::UpdateAvailable {
                current_version: current.to_string(),
                release: latest.clone(),
            }
        }
    }

    pub fn target_asset_suffix(os: &str) -> &'static str {
        match os {
            "windows" => "setup.exe",
            "macos" => "universal.dmg",
            "linux" => "x86_64.AppImage",
            "android" => "arm64-v8a.apk",
            _ => "tar.gz",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_version_diff() {
        let release = AppReleaseInfo {
            version: "0.21.0".to_string(),
            tag_name: "v0.21.0".to_string(),
            release_notes: "New features".to_string(),
            published_at: "2026-09-01T00:00:00Z".to_string(),
            download_url: "https://github.com/release/app.exe".to_string(),
            sha256_checksum: Some("abcdef".to_string()),
        };

        let outcome = AppUpdater::check_version_diff("0.20.0", &release);
        assert!(matches!(
            outcome,
            UpdateCheckOutcome::UpdateAvailable { .. }
        ));

        let outcome_same = AppUpdater::check_version_diff("0.21.0", &release);
        assert!(matches!(outcome_same, UpdateCheckOutcome::UpToDate { .. }));
    }

    #[test]
    fn test_target_asset_suffix() {
        assert_eq!(AppUpdater::target_asset_suffix("windows"), "setup.exe");
        assert_eq!(AppUpdater::target_asset_suffix("macos"), "universal.dmg");
        assert_eq!(AppUpdater::target_asset_suffix("linux"), "x86_64.AppImage");
        assert_eq!(AppUpdater::target_asset_suffix("android"), "arm64-v8a.apk");
    }
}

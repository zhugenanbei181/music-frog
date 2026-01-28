use mihomo_api::Result;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl Channel {
    pub fn as_str(&self) -> &str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
            Channel::Nightly => "nightly",
        }
    }
}

impl FromStr for Channel {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "nightly" | "alpha" => Ok(Channel::Nightly),
            _ => Err(format!("Invalid channel: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub channel: Channel,
    pub version: String,
    pub release_date: String,
}

pub async fn fetch_latest(channel: Channel) -> Result<ChannelInfo> {
    let url = match channel {
        Channel::Stable => "https://api.github.com/repos/MetaCubeX/mihomo/releases/latest",
        Channel::Beta => {
            "https://api.github.com/repos/MetaCubeX/mihomo/releases?per_page=1&prerelease=true"
        }
        Channel::Nightly => "https://api.github.com/repos/MetaCubeX/mihomo/releases?per_page=1",
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "mihomo-rs")
        .send()
        .await?;

    let data: serde_json::Value = resp.json().await?;

    let (version, date) = if channel == Channel::Stable {
        let tag = data["tag_name"].as_str().unwrap_or("").to_string();
        let date = data["published_at"].as_str().unwrap_or("").to_string();
        (tag, date)
    } else {
        let empty_vec = vec![];
        let releases = data.as_array().unwrap_or(&empty_vec);
        if let Some(release) = releases.first() {
            let tag = release["tag_name"].as_str().unwrap_or("").to_string();
            let date = release["published_at"].as_str().unwrap_or("").to_string();
            (tag, date)
        } else {
            return Err(mihomo_api::MihomoError::Version(
                "No releases found".to_string(),
            ));
        }
    };

    Ok(ChannelInfo {
        channel,
        version,
        release_date: date,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    #[serde(rename = "tag_name")]
    pub version: String,
    pub name: String,
    pub published_at: String,
    pub prerelease: bool,
}

pub async fn fetch_releases(limit: usize) -> Result<Vec<ReleaseInfo>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "https://api.github.com/repos/MetaCubeX/mihomo/releases?per_page={}",
            limit
        ))
        .header("User-Agent", "mihomo-rs")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(mihomo_api::MihomoError::Version(format!(
            "GitHub API error: {}",
            resp.status()
        )));
    }

    let releases: Vec<ReleaseInfo> = resp.json().await?;
    Ok(releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_stable_as_str() {
        assert_eq!(Channel::Stable.as_str(), "stable");
    }

    #[test]
    fn test_channel_beta_as_str() {
        assert_eq!(Channel::Beta.as_str(), "beta");
    }

    #[test]
    fn test_channel_nightly_as_str() {
        assert_eq!(Channel::Nightly.as_str(), "nightly");
    }

    #[test]
    fn test_channel_from_str_stable() {
        assert_eq!(Channel::from_str("stable"), Ok(Channel::Stable));
        assert_eq!(Channel::from_str("Stable"), Ok(Channel::Stable));
        assert_eq!(Channel::from_str("STABLE"), Ok(Channel::Stable));
    }

    #[test]
    fn test_channel_from_str_beta() {
        assert_eq!(Channel::from_str("beta"), Ok(Channel::Beta));
        assert_eq!(Channel::from_str("Beta"), Ok(Channel::Beta));
        assert_eq!(Channel::from_str("BETA"), Ok(Channel::Beta));
    }

    #[test]
    fn test_channel_from_str_nightly() {
        assert_eq!(Channel::from_str("nightly"), Ok(Channel::Nightly));
        assert_eq!(Channel::from_str("Nightly"), Ok(Channel::Nightly));
        assert_eq!(Channel::from_str("alpha"), Ok(Channel::Nightly));
        assert_eq!(Channel::from_str("Alpha"), Ok(Channel::Nightly));
    }

    #[test]
    fn test_channel_from_str_invalid() {
        assert!(Channel::from_str("invalid").is_err());
        assert!(Channel::from_str("").is_err());
    }

    #[test]
    fn test_channel_info_serialization() {
        let info = ChannelInfo {
            channel: Channel::Stable,
            version: "v1.19.0".to_string(),
            release_date: "2024-01-01".to_string(),
        };

        let serialized = serde_json::to_string(&info);
        assert!(serialized.is_ok());
    }

    #[test]
    fn test_channel_info_deserialization() {
        let json = r#"{
            "channel": "Stable",
            "version": "v1.19.0",
            "release_date": "2024-01-01"
        }"#;

        let info: ChannelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.channel, Channel::Stable);
        assert_eq!(info.version, "v1.19.0");
        assert_eq!(info.release_date, "2024-01-01");
    }

    #[test]
    fn test_release_info_serialization() {
        let info = ReleaseInfo {
            version: "v1.19.0".to_string(),
            name: "Mihomo v1.19.0".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            prerelease: false,
        };

        let serialized = serde_json::to_string(&info);
        assert!(serialized.is_ok());
    }

    #[test]
    fn test_release_info_deserialization() {
        let json = r#"{
            "tag_name": "v1.19.0",
            "name": "Mihomo v1.19.0",
            "published_at": "2024-01-01T00:00:00Z",
            "prerelease": false
        }"#;

        let info: ReleaseInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "v1.19.0");
        assert_eq!(info.name, "Mihomo v1.19.0");
        assert!(!info.prerelease);
    }
}

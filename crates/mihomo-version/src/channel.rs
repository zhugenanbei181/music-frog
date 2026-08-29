use mihomo_api::error::Result;
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

/// Release-asset archive file name for this platform (e.g.
/// `mihomo-linux-amd64-v1.19.18.gz`).
pub fn asset_file_name(version: &str) -> String {
    use crate::download::Downloader;
    format!(
        "mihomo-{}-{}-{}.{}",
        Downloader::get_os_name(),
        Downloader::detect_platform(),
        version,
        Downloader::get_file_extension()
    )
}

/// Extract the SHA-256 digest for `asset_name` from a GitHub release API
/// payload. Fail-closed: a missing asset, a missing `digest` field, or a
/// non-`sha256` digest is an error — such artifacts must never be installed.
pub fn extract_asset_digest(
    release: &serde_json::Value,
    asset_name: &str,
) -> std::result::Result<String, String> {
    let assets = release["assets"].as_array().ok_or_else(|| {
        format!("release payload has no assets array for {asset_name}")
    })?;
    for asset in assets {
        if asset["name"].as_str() == Some(asset_name) {
            let digest = asset["digest"].as_str().ok_or_else(|| {
                format!(
                    "asset {asset_name} has no published digest; refusing to install (fail-closed)"
                )
            })?;
            if !digest.starts_with("sha256:") {
                return Err(format!(
                    "asset {asset_name} digest {digest:?} is not a sha256 digest; refusing to install (fail-closed)"
                ));
            }
            return Ok(digest.to_string());
        }
    }
    Err(format!(
        "release does not contain asset {asset_name}; refusing to install (fail-closed)"
    ))
}

/// Fetch the SHA-256 digest of this platform's release archive for `version`
/// from the GitHub release API. This is the trusted provenance the install
/// pipeline verifies downloads against (UP-001).
pub async fn fetch_asset_digest(
    version: &str,
) -> mihomo_api::error::Result<String> {
    let asset_name = asset_file_name(version);
    let url = format!("https://api.github.com/repos/MetaCubeX/mihomo/releases/tags/{version}");
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "mihomo-rs")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(mihomo_api::error::MihomoError::Version(format!(
            "cannot resolve digest for {version}: GitHub API returned {}",
            resp.status()
        )));
    }

    let release: serde_json::Value = resp.json().await?;
    extract_asset_digest(&release, &asset_name)
        .map_err(mihomo_api::error::MihomoError::Version)
}

/// Base URL of the GitHub REST API used by the channel resolution helpers.
/// Tests point [`fetch_latest_from`] at a mock server instead.
pub const GITHUB_API_BASE: &str = "https://api.github.com";

/// Select the target release from a GitHub `GET /releases` list payload
/// (newest first, as returned by the API).
///
/// Channel semantics ([缺口28]):
/// - [`Channel::Stable`]: `fetch_latest` does not use a list at all — it goes
///   through `releases/latest`, which never contains prereleases. If a caller
///   nonetheless passes a list, the newest entry is returned unchanged.
/// - [`Channel::Beta`]: the **first entry with `prerelease == true`**. The
///   previous implementation relied on `?prerelease=true`, a query parameter
///   the GitHub API silently ignores, so "beta" was effectively identical to
///   "newest release of any kind" — including stable ones.
/// - [`Channel::Nightly`] (alpha): the **first list entry**, of any kind — the
///   newest published release (stable, beta or alpha) is by definition the
///   freshest build.
///
/// Returns `None` when no entry matches (empty list, or a Beta query over a
/// list that contains only stable releases).
pub fn pick_release(
    releases: &[serde_json::Value],
    channel: Channel,
) -> Option<&serde_json::Value> {
    match channel {
        Channel::Stable | Channel::Nightly => releases.first(),
        Channel::Beta => releases
            .iter()
            .find(|release| release["prerelease"].as_bool() == Some(true)),
    }
}

/// Fetch the latest [`ChannelInfo`] for `channel` against an arbitrary API
/// base URL. Production goes through [`fetch_latest`]; tests inject a mock
/// server URL here.
pub async fn fetch_latest_from(base_url: &str, channel: Channel) -> Result<ChannelInfo> {
    let url = match channel {
        // `/releases/latest` excludes prereleases by definition, so the
        // Stable channel needs no client-side filtering (unchanged behavior).
        Channel::Stable => format!("{base_url}/repos/MetaCubeX/mihomo/releases/latest"),
        // The GitHub API has no server-side prerelease filter, so fetch a
        // small window and let `pick_release` do the channel selection.
        Channel::Beta | Channel::Nightly => {
            format!("{base_url}/repos/MetaCubeX/mihomo/releases?per_page=10")
        }
    };

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("User-Agent", "mihomo-rs")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(mihomo_api::error::MihomoError::Version(format!(
            "GitHub API error while resolving channel {}: {}",
            channel.as_str(),
            resp.status()
        )));
    }

    let data: serde_json::Value = resp.json().await?;

    let (version, date) = match channel {
        Channel::Stable => (
            data["tag_name"].as_str().unwrap_or("").to_string(),
            data["published_at"].as_str().unwrap_or("").to_string(),
        ),
        Channel::Beta | Channel::Nightly => {
            let empty = Vec::new();
            let releases = data.as_array().unwrap_or(&empty);
            let release = pick_release(releases, channel).ok_or_else(|| {
                mihomo_api::error::MihomoError::Version(format!(
                    "no suitable release found for channel {}",
                    channel.as_str()
                ))
            })?;
            (
                release["tag_name"].as_str().unwrap_or("").to_string(),
                release["published_at"].as_str().unwrap_or("").to_string(),
            )
        }
    };

    Ok(ChannelInfo {
        channel,
        version,
        release_date: date,
    })
}

pub async fn fetch_latest(channel: Channel) -> Result<ChannelInfo> {
    fetch_latest_from(GITHUB_API_BASE, channel).await
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
        return Err(mihomo_api::error::MihomoError::Version(format!(
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

    fn release_payload(asset_name: &str, digest: Option<&str>) -> serde_json::Value {
        let mut asset = serde_json::json!({ "name": asset_name });
        match digest {
            Some(d) => asset["digest"] = serde_json::Value::String(d.to_string()),
            None => {
                let _ = &asset;
            }
        }
        serde_json::json!({ "assets": [asset, { "name": "checksums.txt" }] })
    }

    #[test]
    fn extract_digest_returns_published_sha256() {
        let payload = release_payload(
            &asset_file_name("v1.19.18"),
            Some("sha256:abc123"),
        );
        assert_eq!(
            extract_asset_digest(&payload, &asset_file_name("v1.19.18")).unwrap(),
            "sha256:abc123"
        );
    }

    #[test]
    fn extract_digest_fails_closed_on_missing_digest_field() {
        let payload = release_payload(&asset_file_name("v1.19.18"), None);
        let err = extract_asset_digest(&payload, &asset_file_name("v1.19.18")).unwrap_err();
        assert!(err.contains("fail-closed"), "{err}");
    }

    #[test]
    fn extract_digest_fails_closed_on_missing_asset() {
        let payload = release_payload("some-other-asset", Some("sha256:abc"));
        let err = extract_asset_digest(&payload, &asset_file_name("v1.19.18")).unwrap_err();
        assert!(err.contains("does not contain asset"), "{err}");
    }

    #[test]
    fn extract_digest_fails_closed_on_non_sha256() {
        let payload = release_payload(&asset_file_name("v1.19.18"), Some("md5:abc"));
        let err = extract_asset_digest(&payload, &asset_file_name("v1.19.18")).unwrap_err();
        assert!(err.contains("not a sha256 digest"), "{err}");
    }

    #[test]
    fn asset_file_name_matches_platform_conventions() {
        let name = asset_file_name("v1.19.18");
        assert!(name.starts_with("mihomo-"), "{name}");
        assert!(name.contains("v1.19.18"), "{name}");
        assert!(name.ends_with(".gz") || name.ends_with(".zip"), "{name}");
    }

    // ---- [缺口28] pick_release: channel selection over release lists ----

    fn list_release(tag: &str, prerelease: bool) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag,
            "published_at": "2024-01-01T00:00:00Z",
            "prerelease": prerelease,
        })
    }

    #[test]
    fn pick_release_beta_skips_stable_and_selects_prerelease() {
        let releases = vec![
            list_release("v1.19.18", false),
            list_release("v1.19.1-beta", true),
            list_release("v1.19.0", false),
        ];
        let picked = pick_release(&releases, Channel::Beta).unwrap();
        assert_eq!(picked["tag_name"].as_str(), Some("v1.19.1-beta"));
    }

    #[test]
    fn pick_release_beta_selects_first_prerelease_when_multiple() {
        let releases = vec![
            list_release("v1.19.18", false),
            list_release("v1.19.2-beta", true),
            list_release("v1.19.1-beta", true),
        ];
        let picked = pick_release(&releases, Channel::Beta).unwrap();
        assert_eq!(picked["tag_name"].as_str(), Some("v1.19.2-beta"));
    }

    #[test]
    fn pick_release_beta_returns_none_without_prereleases() {
        let releases = vec![
            list_release("v1.19.18", false),
            list_release("v1.19.17", false),
        ];
        assert!(pick_release(&releases, Channel::Beta).is_none());
    }

    #[test]
    fn pick_release_beta_returns_none_on_empty_list() {
        let releases: Vec<serde_json::Value> = vec![];
        assert!(pick_release(&releases, Channel::Beta).is_none());
    }

    #[test]
    fn pick_release_nightly_takes_newest_of_any_kind() {
        let releases = vec![
            list_release("v1.19.18", false),
            list_release("v1.19.1-beta", true),
        ];
        let picked = pick_release(&releases, Channel::Nightly).unwrap();
        assert_eq!(picked["tag_name"].as_str(), Some("v1.19.18"));
    }

    #[test]
    fn pick_release_nightly_takes_prerelease_when_newest() {
        let releases = vec![
            list_release("v1.19.2-alpha-abc", true),
            list_release("v1.19.1", false),
        ];
        let picked = pick_release(&releases, Channel::Nightly).unwrap();
        assert_eq!(picked["tag_name"].as_str(), Some("v1.19.2-alpha-abc"));
    }

    #[test]
    fn pick_release_stable_takes_first_list_entry() {
        let releases = vec![list_release("v1.19.18", false), list_release("v1.19.1-beta", true)];
        let picked = pick_release(&releases, Channel::Stable).unwrap();
        assert_eq!(picked["tag_name"].as_str(), Some("v1.19.18"));
    }

    #[test]
    fn pick_release_beta_ignores_missing_prerelease_field() {
        // A payload without a `prerelease` field must not be treated as a
        // prerelease (absence != true).
        let releases = vec![
            serde_json::json!({"tag_name": "v1.19.18"}),
            list_release("v1.19.1-beta", true),
        ];
        let picked = pick_release(&releases, Channel::Beta).unwrap();
        assert_eq!(picked["tag_name"].as_str(), Some("v1.19.1-beta"));
    }

    // ---- [缺口28] fetch_latest_from over a mock GitHub API ----

    async fn mock_releases_server(body: serde_json::Value) -> mockito::ServerGuard {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/repos/MetaCubeX/mihomo/releases?per_page=10")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body.to_string())
            .create_async()
            .await;
        server
    }

    #[tokio::test]
    async fn fetch_latest_beta_picks_prerelease_over_http() {
        let server = mock_releases_server(serde_json::json!([
            list_release("v1.19.18", false),
            list_release("v1.19.1-beta", true),
        ]))
        .await;

        let info = fetch_latest_from(&server.url(), Channel::Beta).await.unwrap();
        assert_eq!(info.channel, Channel::Beta);
        assert_eq!(info.version, "v1.19.1-beta");
    }

    #[tokio::test]
    async fn fetch_latest_beta_errors_when_only_stable_releases() {
        let server = mock_releases_server(serde_json::json!([
            list_release("v1.19.18", false),
        ]))
        .await;

        let err = fetch_latest_from(&server.url(), Channel::Beta).await.unwrap_err();
        assert!(err.to_string().contains("no suitable release"), "{err}");
    }

    #[tokio::test]
    async fn fetch_latest_nightly_takes_newest_over_http() {
        let server = mock_releases_server(serde_json::json!([
            list_release("v1.19.2-alpha-cb6ac1e", true),
            list_release("v1.19.18", false),
        ]))
        .await;

        let info = fetch_latest_from(&server.url(), Channel::Nightly)
            .await
            .unwrap();
        assert_eq!(info.version, "v1.19.2-alpha-cb6ac1e");
    }

    #[tokio::test]
    async fn fetch_latest_stable_uses_releases_latest_endpoint() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/repos/MetaCubeX/mihomo/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                serde_json::json!({
                    "tag_name": "v1.19.18",
                    "published_at": "2024-06-01T00:00:00Z",
                    "prerelease": false,
                })
                .to_string(),
            )
            .create_async()
            .await;

        let info = fetch_latest_from(&server.url(), Channel::Stable).await.unwrap();
        assert_eq!(info.version, "v1.19.18");
        assert_eq!(info.release_date, "2024-06-01T00:00:00Z");
    }

    #[tokio::test]
    async fn fetch_latest_from_reports_http_error_status() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock(
                "GET",
                mockito::Matcher::Regex(r"^/repos/MetaCubeX/mihomo/releases".to_string()),
            )
            .with_status(503)
            .create_async()
            .await;

        let err = fetch_latest_from(&server.url(), Channel::Nightly).await.unwrap_err();
        assert!(err.to_string().contains("503"), "{err}");
    }
}

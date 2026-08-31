use std::str::FromStr;

use mihomo_version::channel::{Channel, ReleaseInfo, fetch_releases};
use mihomo_version::manager::{VersionInfo, VersionManager};

use crate::commands::KernelAction;
use crate::context::Runtime;
use crate::output::{self, print_info, print_success, print_table};

pub(crate) async fn handle(action: KernelAction) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    let manager = runtime.version_manager()?;
    match action {
        KernelAction::Install { target } => install(&manager, &target).await?,
        KernelAction::Use { version } => {
            manager.set_default(&version).await?;
            print_success(&format!("Default kernel version set to {version}"));
        }
        KernelAction::List { json } => list(&manager, json).await?,
        KernelAction::ListRemote { limit } => list_remote(limit).await?,
        KernelAction::Uninstall { version } => {
            manager.uninstall(&version).await?;
            print_success(&format!("Uninstalled kernel version {version}"));
        }
        KernelAction::UpdateStable => update_stable(&manager).await?,
    }
    Ok(())
}

/// A target is a channel when it parses as one, otherwise a version tag.
pub(crate) fn split_target(target: &str) -> Option<Channel> {
    Channel::from_str(target).ok()
}

async fn install(manager: &VersionManager, target: &str) -> anyhow::Result<()> {
    match split_target(target) {
        Some(channel) => {
            print_info(&format!(
                "Resolving latest {} channel release...",
                channel.as_str()
            ));
            let version = manager.install_channel(channel).await?;
            print_success(&format!(
                "Installed kernel {version} ({} channel)",
                channel.as_str()
            ));
        }
        None => {
            print_info(&format!("Installing kernel {target}..."));
            manager.install(target).await?;
            print_success(&format!("Installed kernel {target}"));
        }
    }
    Ok(())
}

/// `update-stable`: the version crate has no dedicated update entry point, so
/// update = install latest stable + make it default (mihomo-rs semantics).
async fn update_stable(manager: &VersionManager) -> anyhow::Result<()> {
    print_info("Resolving latest stable channel release...");
    let version = manager.install_channel(Channel::Stable).await?;
    manager.set_default(&version).await?;
    print_success(&format!(
        "Updated stable kernel to {version} and set it as default"
    ));
    Ok(())
}

async fn list(manager: &VersionManager, json: bool) -> anyhow::Result<()> {
    let versions = manager.list_installed().await?;
    if json {
        output::print_json(&versions)?;
        return Ok(());
    }
    if versions.is_empty() {
        print_info("No kernel versions installed; try `infiltrator kernel install stable`");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = versions.iter().map(version_row).collect();
    print_table(&["Version", "Default", "Path"], &rows);
    Ok(())
}

fn version_row(info: &VersionInfo) -> Vec<String> {
    vec![
        info.version.clone(),
        (if info.is_default { "yes" } else { "" }).to_string(),
        info.path.display().to_string(),
    ]
}

async fn list_remote(limit: usize) -> anyhow::Result<()> {
    print_info(&format!("Fetching the {limit} latest upstream releases..."));
    let releases = fetch_releases(limit).await?;
    if releases.is_empty() {
        print_info("No upstream releases found");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = releases.iter().map(release_row).collect();
    print_table(&["Version", "Name", "Prerelease", "Date"], &rows);
    Ok(())
}

fn release_row(release: &ReleaseInfo) -> Vec<String> {
    vec![
        release.version.clone(),
        release.name.clone(),
        (if release.prerelease { "yes" } else { "no" }).to_string(),
        truncate(&release.published_at, 10),
    ]
}

pub(crate) fn truncate(input: &str, max_chars: usize) -> String {
    input.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use mihomo_version::channel::Channel;

    use super::{split_target, truncate, version_row};
    use mihomo_version::manager::VersionInfo;

    #[test]
    fn channel_targets_are_recognized_case_insensitively() {
        assert_eq!(split_target("stable"), Some(Channel::Stable));
        assert_eq!(split_target("Stable"), Some(Channel::Stable));
        assert_eq!(split_target("beta"), Some(Channel::Beta));
        assert_eq!(split_target("alpha"), Some(Channel::Nightly));
        assert_eq!(split_target("nightly"), Some(Channel::Nightly));
    }

    #[test]
    fn version_targets_are_not_channels() {
        assert_eq!(split_target("v1.19.18"), None);
        assert_eq!(split_target(""), None);
        assert_eq!(split_target("latest"), None);
    }

    #[test]
    fn truncate_cuts_at_char_boundary() {
        assert_eq!(truncate("2026-08-31T10:00:00Z", 10), "2026-08-31");
        assert_eq!(truncate("短", 10), "短");
    }

    #[test]
    fn version_row_marks_the_default() {
        let info = VersionInfo {
            version: "v1.19.18".to_string(),
            path: "/x/versions/v1.19.18".into(),
            is_default: true,
        };
        let row = version_row(&info);
        assert_eq!(row[0], "v1.19.18");
        assert_eq!(row[1], "yes");
        assert!(row[2].contains("v1.19.18"));
    }
}

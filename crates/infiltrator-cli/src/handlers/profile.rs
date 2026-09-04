use anyhow::anyhow;
use mihomo_config::profile::Profile;

use crate::commands::{ConfigsDirAction, ProfileAction};
use crate::context::Runtime;
use crate::output::{self, print_info, print_success, print_table};

pub(crate) async fn handle(action: ProfileAction) -> anyhow::Result<()> {
    let runtime = Runtime::detect().await?;
    match action {
        ProfileAction::List { json } => list(&runtime, json).await?,
        ProfileAction::Current => current(&runtime).await?,
        ProfileAction::Path => println!("{}", runtime.configs_dir()?.display()),
        ProfileAction::Use { name } => {
            let manager = runtime.config_manager()?;
            manager.set_current(&name).await?;
            print_success(&format!("Active profile set to '{name}'"));
        }
        ProfileAction::Show { name } => show(&runtime, name).await?,
        ProfileAction::Delete { name } => {
            let manager = runtime.config_manager()?;
            manager.delete_profile(&name).await?;
            print_success(&format!("Deleted profile '{name}'"));
        }
        ProfileAction::Import { name, url } => import(&runtime, &name, &url).await?,
        ProfileAction::ConfigsDir { action } => configs_dir(&runtime, action).await?,
    }
    Ok(())
}

async fn list(runtime: &Runtime, json: bool) -> anyhow::Result<()> {
    let profiles = runtime.config_manager()?.list_profiles().await?;
    if json {
        let values: Vec<serde_json::Value> = profiles.iter().map(profile_json).collect();
        output::print_json(&values)?;
        return Ok(());
    }
    if profiles.is_empty() {
        print_info("No profiles found; run `infiltrator bootstrap` first");
        return Ok(());
    }
    let rows: Vec<Vec<String>> = profiles.iter().map(profile_row).collect();
    print_table(&["Name", "Active", "Path"], &rows);
    Ok(())
}

pub(crate) fn profile_row(profile: &Profile) -> Vec<String> {
    vec![
        profile.name.clone(),
        (if profile.active { "*" } else { "" }).to_string(),
        profile.path.display().to_string(),
    ]
}

pub(crate) fn profile_json(profile: &Profile) -> serde_json::Value {
    serde_json::json!({
        "name": profile.name,
        "active": profile.active,
        "path": profile.path.display().to_string(),
    })
}

async fn current(runtime: &Runtime) -> anyhow::Result<()> {
    let manager = runtime.config_manager()?;
    let name = manager.get_current().await?;
    let path = manager.get_current_path().await?;
    println!("current profile: {name}");
    println!("config path: {}", path.display());
    Ok(())
}

async fn show(runtime: &Runtime, name: Option<String>) -> anyhow::Result<()> {
    let manager = runtime.config_manager()?;
    let name = match name {
        Some(name) => name,
        None => manager.get_current().await?,
    };
    let content = manager.load(&name).await?;
    println!("{content}");
    Ok(())
}

/// Import a subscription into a new profile. Mirrors
/// `infiltrator_core::profiles::create_profile_from_url` but targets the
/// configs directory resolved from the settings override, which the core
/// facade (built on an injected `ConfigManager`) cannot express.
async fn import(runtime: &Runtime, name: &str, url: &str) -> anyhow::Result<()> {
    let profile_name = infiltrator_core::profiles::sanitize_profile_name(name)?;
    let checked_url = infiltrator_core::subscription::CheckedSubscriptionUrl::parse(url)?;
    let client = infiltrator_http::build_http_client();
    let raw_client = infiltrator_http::build_raw_http_client(&client);
    let content =
        infiltrator_core::subscription::fetch_subscription_text(&client, &raw_client, &checked_url)
            .await?;
    let content = infiltrator_core::subscription::strip_utf8_bom(&content);
    let configs_dir = runtime.configs_dir()?;
    let (content, _report) = infiltrator_core::profile_options::apply_saved_options(
        &configs_dir,
        &profile_name,
        content,
    )
    .await?;
    infiltrator_core::config::validate_yaml(&content)
        .map_err(|err| anyhow!("订阅内容不是有效的 YAML: {err}"))?;

    let manager = runtime.config_manager()?;
    manager.save(&profile_name, &content).await?;
    print_success(&format!("Imported profile '{profile_name}'"));
    Ok(())
}

async fn configs_dir(runtime: &Runtime, action: ConfigsDirAction) -> anyhow::Result<()> {
    match action {
        ConfigsDirAction::Get => {
            match runtime.settings.configs_dir.as_deref().map(str::trim) {
                Some(dir) if !dir.is_empty() => println!("configs_dir (settings): {dir}"),
                _ => println!("configs_dir: (not set)"),
            }
            println!("resolved: {}", runtime.configs_dir()?.display());
        }
        ConfigsDirAction::Set { path } => {
            validate_override(&path)?;
            runtime
                .update_settings(|settings| settings.configs_dir = Some(path.clone()))
                .await?;
            print_success(&format!("configs_dir override set to '{path}'"));
        }
        ConfigsDirAction::Unset => {
            runtime
                .update_settings(|settings| settings.configs_dir = None)
                .await?;
            print_success("configs_dir override removed");
        }
    }
    Ok(())
}

/// The settings layer treats blank values as "not set", but storing one would
/// silently drop the user's intent; reject it at the CLI boundary instead.
fn validate_override(path: &str) -> anyhow::Result<()> {
    if path.trim().is_empty() {
        return Err(anyhow!("configs_dir override cannot be empty"));
    }
    Ok(())
}

/// Set/unset the override against an explicit home; exercise point for the
/// offline settings tests.
#[cfg(test)]
pub(crate) async fn apply_configs_dir_override(
    home: &std::path::Path,
    override_dir: Option<String>,
) -> anyhow::Result<()> {
    let runtime = Runtime::with_home(home.to_path_buf()).await?;
    runtime
        .update_settings(|settings| settings.configs_dir = override_dir)
        .await
}

#[cfg(test)]
#[path = "profile_test.rs"]
mod profile_test;

use anyhow::anyhow;
use infiltrator_domain::profiles::ProfileInfo;

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
            runtime
                .profile_application()?
                .select_profile(&name)
                .await
                .map_err(|failure| anyhow!(failure.message))?;
            print_success(&format!("Active profile set to '{name}'"));
        }
        ProfileAction::Show { name } => show(&runtime, name).await?,
        ProfileAction::Delete { name } => {
            runtime
                .profile_application()?
                .delete_profile(&name)
                .await
                .map_err(|failure| anyhow!(failure.message))?;
            print_success(&format!("Deleted profile '{name}'"));
        }
        ProfileAction::Import { name, url } => import(&runtime, &name, &url).await?,
        ProfileAction::ConfigsDir { action } => configs_dir(&runtime, action).await?,
    }
    Ok(())
}

async fn list(runtime: &Runtime, json: bool) -> anyhow::Result<()> {
    let profiles = runtime
        .profile_application()?
        .list_profiles()
        .await
        .map_err(|failure| anyhow!(failure.message))?;
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

pub(crate) fn profile_row(profile: &ProfileInfo) -> Vec<String> {
    vec![
        profile.name.clone(),
        (if profile.active { "*" } else { "" }).to_string(),
        profile.path.clone(),
    ]
}

pub(crate) fn profile_json(profile: &ProfileInfo) -> serde_json::Value {
    serde_json::json!({
        "name": profile.name,
        "active": profile.active,
        "path": profile.path,
    })
}

async fn current(runtime: &Runtime) -> anyhow::Result<()> {
    let application = runtime.profile_application()?;
    let name = application
        .current_profile()
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    let path = application.config_dir().join(format!("{name}.yaml"));
    println!("current profile: {name}");
    println!("config path: {}", path.display());
    Ok(())
}

async fn show(runtime: &Runtime, name: Option<String>) -> anyhow::Result<()> {
    let application = runtime.profile_application()?;
    let name = match name {
        Some(name) => name,
        None => application
            .current_profile()
            .await
            .map_err(|failure| anyhow!(failure.message))?,
    };
    let detail = application
        .load_profile_detail(&name)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
    println!("{}", detail.content);
    Ok(())
}

/// Import a subscription into a new profile through the shared application
/// use-case and the host HTTP adapter.
async fn import(runtime: &Runtime, name: &str, url: &str) -> anyhow::Result<()> {
    let profile_name = infiltrator_domain::profiles::sanitize_profile_name(name)?;
    let source = runtime.subscription_source();
    runtime
        .profile_application()?
        .import_subscription(&source, &profile_name, url)
        .await
        .map_err(|failure| anyhow!(failure.message))?;
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

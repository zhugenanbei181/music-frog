//! Shared profile-document commit path for Iced configuration editors.
//!
//! Each editor supplies a pure `YAML -> YAML` transform. The helper then
//! commits the complete document through `MihomoRuntime::apply_profile_content`
//! when the core is live, or through the validated atomic config manager when
//! it is stopped. This keeps individual page handlers from bypassing the
//! apply/reload/readiness/rollback contract.

use crate::types::message::Message;
use iced::Task;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_desktop::runtime::MihomoRuntime;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

pub(super) fn save_task<F>(
    runtime: Option<Arc<MihomoRuntime>>,
    transform: F,
    result_message: fn(Result<(), InfiltratorError>) -> Message,
) -> Task<Message>
where
    F: FnOnce(&str) -> anyhow::Result<String> + Send + 'static,
{
    save_task_with_strategy(
        runtime,
        ApplyStrategy::PreferReload,
        transform,
        result_message,
    )
}

pub(super) fn save_task_with_strategy<F>(
    runtime: Option<Arc<MihomoRuntime>>,
    strategy: ApplyStrategy,
    transform: F,
    result_message: fn(Result<(), InfiltratorError>) -> Message,
) -> Task<Message>
where
    F: FnOnce(&str) -> anyhow::Result<String> + Send + 'static,
{
    Task::perform(
        save_current_profile_content(runtime, strategy, transform),
        result_message,
    )
}

pub(super) async fn save_current_profile_content<F>(
    runtime: Option<Arc<MihomoRuntime>>,
    strategy: ApplyStrategy,
    transform: F,
) -> Result<(), InfiltratorError>
where
    F: FnOnce(&str) -> anyhow::Result<String> + Send + 'static,
{
    let manager = crate::configs_dir::config_manager().await?;
    let profile = manager
        .get_current()
        .await
        .map_err(infiltrator_contract::error::from_mihomo)?;
    let content = manager
        .load(&profile)
        .await
        .map_err(infiltrator_contract::error::from_mihomo)?;
    let updated =
        transform(&content).map_err(|error| InfiltratorError::Config(error.to_string()))?;

    save_profile_content(runtime, profile, updated, strategy).await
}

/// Commit an arbitrary profile document. Active profiles use the running
/// core's atomic apply transaction; inactive profiles still use the validated
/// manager writer and clear their transient backup immediately.
pub(crate) async fn save_profile_content(
    runtime: Option<Arc<MihomoRuntime>>,
    profile: String,
    content: String,
    strategy: ApplyStrategy,
) -> Result<(), InfiltratorError> {
    let manager = crate::configs_dir::config_manager().await?;
    let current = manager
        .get_current()
        .await
        .map_err(infiltrator_contract::error::from_mihomo)?;
    if let Some(runtime) = runtime
        && current == profile
    {
        ManagedRuntime::apply_profile_content(runtime.as_ref(), &content, strategy)
            .await
            .map_err(|error| InfiltratorError::Config(error.to_string()))?;
    } else {
        manager
            .save(&profile, &content)
            .await
            .map_err(infiltrator_contract::error::from_mihomo)?;
        manager
            .clear_backup(&profile)
            .await
            .map_err(infiltrator_contract::error::from_mihomo)?;
    }
    Ok(())
}

/// Switch the active profile without leaving a running core on a half-applied
/// target. If applying the target fails, restore the pointer and explicitly
/// re-apply the previous profile so the old core configuration is live again.
pub(crate) async fn activate_profile(
    runtime: Option<Arc<MihomoRuntime>>,
    profile: &str,
) -> Result<bool, InfiltratorError> {
    let manager = crate::configs_dir::config_manager().await?;
    let previous = manager
        .get_current()
        .await
        .map_err(infiltrator_contract::error::from_mihomo)?;
    if previous == profile {
        return Ok(runtime.is_some());
    }
    manager
        .set_current(profile)
        .await
        .map_err(infiltrator_contract::error::from_mihomo)?;

    let Some(runtime) = runtime else {
        return Ok(false);
    };
    if let Err(error) = ManagedRuntime::apply_current_config(
        runtime.as_ref(),
        ApplyStrategy::AlwaysRestart,
    )
    .await
    {
        let _ = manager.set_current(&previous).await;
        if let Err(recovery) = ManagedRuntime::apply_current_config(
            runtime.as_ref(),
            ApplyStrategy::AlwaysRestart,
        )
        .await
        {
            let _ = manager.clear_backup(profile).await;
            return Err(InfiltratorError::Mihomo(format!(
                "切换配置失败: {error}; 恢复上一配置也失败: {recovery}"
            )));
        }
        let _ = manager.clear_backup(profile).await;
        return Err(InfiltratorError::Mihomo(format!(
            "切换配置失败，已恢复上一配置: {error}"
        )));
    }
    Ok(true)
}

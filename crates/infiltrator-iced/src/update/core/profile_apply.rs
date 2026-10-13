//! Shared profile-document commit path for Iced configuration editors.
//!
//! Each editor supplies a pure `YAML -> YAML` transform. The helper then
//! commits the complete document through the host active-profile applier port
//! when the core is live, or through the validated atomic config manager when
//! it is stopped. This keeps individual page handlers from bypassing the
//! apply/reload/readiness/rollback contract.

use crate::configs_dir::config_manager;
use crate::types::message::Message;
use iced::Task;
use infiltrator_application::profile_application::{
    ProfileActivationOutcome, ProfileApplication, ProfileRuntimeAccess,
};
use infiltrator_contract::error::InfiltratorError;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_ports::host_runtime::HostRuntime;
use std::sync::Arc;

/// Convert the host's optional capability slot at the Iced-to-application
/// boundary. Application use cases themselves receive only the explicit
/// Offline/CoreOwner access enum.
pub(crate) fn runtime_access(
    runtime: Option<Arc<dyn HostRuntime>>,
) -> ProfileRuntimeAccess<dyn HostRuntime> {
    match runtime {
        Some(runtime) => ProfileRuntimeAccess::CoreOwner(runtime),
        None => ProfileRuntimeAccess::Offline,
    }
}

pub(crate) fn save_task<F>(
    runtime: ProfileRuntimeAccess<dyn HostRuntime>,
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

pub(crate) fn save_task_with_strategy<F>(
    runtime: ProfileRuntimeAccess<dyn HostRuntime>,
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
    runtime: ProfileRuntimeAccess<dyn HostRuntime>,
    strategy: ApplyStrategy,
    transform: F,
) -> Result<(), InfiltratorError>
where
    F: FnOnce(&str) -> anyhow::Result<String> + Send + 'static,
{
    let store = config_manager().await?;
    let application = ProfileApplication::new(store);
    application
        .save_current_profile_content(runtime, strategy, transform)
        .await
        .map(|_| ())
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

/// Commit an arbitrary profile document. Active profiles use the running
/// core's atomic apply transaction; inactive profiles still use the validated
/// manager writer and clear their transient backup immediately.
pub(crate) async fn save_profile_content(
    runtime: ProfileRuntimeAccess<dyn HostRuntime>,
    profile: String,
    content: String,
    strategy: ApplyStrategy,
) -> Result<(), InfiltratorError> {
    let store = config_manager().await?;
    ProfileApplication::new(store)
        .save_profile_content(runtime, profile, content, strategy)
        .await
        .map(|_| ())
        .map_err(|failure| InfiltratorError::Config(failure.message))
}

/// Switch the active profile without leaving a running core on a half-applied
/// target. If applying the target fails, restore the pointer and explicitly
/// re-apply the previous profile so the old core configuration is live again.
pub(crate) async fn activate_profile(
    runtime: ProfileRuntimeAccess<dyn HostRuntime>,
    profile: &str,
) -> Result<ProfileActivationOutcome, InfiltratorError> {
    let store = config_manager().await?;
    ProfileApplication::new(store)
        .activate_profile(runtime, profile)
        .await
        .map_err(|failure| InfiltratorError::Backend(failure.message))
}

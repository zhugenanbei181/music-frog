//! Core owner 授权后的活动配置应用。

use crate::error::PortError;
use crate::profile_workspace::{ProfileWorkspace, ProfileWorkspaceUpdate};
use async_trait::async_trait;
use infiltrator_contract::active_profile_reference::{
    ActiveProfileReference, ActiveProfileReferenceWriteAuthorization,
};
use infiltrator_contract::capability::Capability;
use infiltrator_contract::profile_document_identity::ProfileWorkspaceIdentity;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_domain::apply::ApplyStrategy;
use lurker_core_contract::identity::EngineId;

/// 仅在宿主提供的活动引擎 owner 与 revision 授权精确匹配时应用配置。
#[async_trait]
pub trait ActiveProfileReferenceApplier: Send + Sync {
    fn engine_id(&self) -> Option<EngineId>;

    fn core_lifecycle(&self) -> CoreLifecycle;

    fn write_authorization(&self) -> Result<ActiveProfileReferenceWriteAuthorization, PortError>;

    async fn apply_active_profile_reference(
        &self,
        expected: &ActiveProfileReference,
        strategy: ApplyStrategy,
    ) -> Result<u64, PortError> {
        let _ = (expected, strategy);
        Err(PortError::unsupported(
            Capability::Profiles,
            "revision-bound active-profile apply is unavailable",
        ))
    }

    /// 持有选中引用和 workspace lease，直到原生读回及条件回滚结束。
    async fn apply_active_profile_workspace(
        &self,
        expected_reference: &ActiveProfileReference,
        expected_workspace: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
        authorization: ActiveProfileReferenceWriteAuthorization,
        strategy: ApplyStrategy,
    ) -> Result<ProfileWorkspace, PortError> {
        let _ = (
            expected_reference,
            expected_workspace,
            update,
            authorization,
            strategy,
        );
        Err(PortError::unsupported(
            Capability::Profiles,
            "revision- and workspace-bound active-profile apply is unavailable",
        ))
    }
}

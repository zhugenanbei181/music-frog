//! Engine-scoped capability declarations for the sing-box adapter.

use infiltrator_contract::adapter_operation_declaration::{
    AdapterOperationDeclarationFile, OperationEvidenceLock, ValidatedAdapterOperationDeclarations,
};
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreOperation, CoreVersionRequirement, EngineBinaryTarget,
    HostCapabilityDecision,
};
use infiltrator_contract::core_identity::{CoreVersion, EngineId};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use std::collections::HashSet;
use std::sync::OnceLock;

const SING_BOX_ADAPTER_REVISION: u64 = 1;
const SING_BOX_EVIDENCE_MANIFEST: &str =
    include_str!("../../../packaging/sing-box-evidence-assets.json");
const SING_BOX_OPERATION_DECLARATION: &str = include_str!("../adapter-operation-declarations.json");

fn operation_declarations()
-> Result<&'static ValidatedAdapterOperationDeclarations, CoreCapabilityProviderError> {
    static DECLARATIONS: OnceLock<Result<ValidatedAdapterOperationDeclarations, String>> =
        OnceLock::new();
    match DECLARATIONS.get_or_init(load_operation_declarations) {
        Ok(declarations) => Ok(declarations),
        Err(reason) => Err(CoreCapabilityProviderError::MatrixUnavailable {
            reason: format!("sing-box operation declaration is invalid: {reason}"),
        }),
    }
}

fn load_operation_declarations() -> Result<ValidatedAdapterOperationDeclarations, String> {
    let declaration: AdapterOperationDeclarationFile =
        serde_json::from_str(SING_BOX_OPERATION_DECLARATION)
            .map_err(|error| format!("declaration JSON: {error}"))?;
    let evidence_lock =
        OperationEvidenceLock::from_manifest_json(SING_BOX_EVIDENCE_MANIFEST, "sing-box")?;
    ValidatedAdapterOperationDeclarations::validate(declaration, "sing-box", &evidence_lock)
        .map_err(|error| error.to_string())
}

/// Builds a sing-box capability matrix from its validated declaration and
/// target-bound evidence. Product roots supply only the operations they have
/// actually connected to this provider.
#[derive(Clone, Debug)]
pub struct SingBoxCoreCapabilityProvider {
    host: HostKind,
    binary_target: EngineBinaryTarget,
    adapter_revision: AdapterRevision,
    adapter_operations: HashSet<CoreOperation>,
}

impl SingBoxCoreCapabilityProvider {
    pub fn new(
        host: HostKind,
        adapter_operations: impl IntoIterator<Item = CoreOperation>,
    ) -> Self {
        Self {
            host,
            binary_target: EngineBinaryTarget::current(),
            adapter_revision: AdapterRevision::new(SING_BOX_ADAPTER_REVISION),
            adapter_operations: adapter_operations.into_iter().collect(),
        }
    }

    #[cfg(test)]
    fn new_for_target(
        host: HostKind,
        binary_target: EngineBinaryTarget,
        adapter_operations: impl IntoIterator<Item = CoreOperation>,
    ) -> Self {
        Self {
            host,
            binary_target,
            adapter_revision: AdapterRevision::new(SING_BOX_ADAPTER_REVISION),
            adapter_operations: adapter_operations.into_iter().collect(),
        }
    }
}

impl CoreCapabilityProvider for SingBoxCoreCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let sing_box_id = crate::engine_id();
        if engine_id != &sing_box_id {
            return Err(CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "the sing-box adapter cannot author capabilities for another engine"
                    .to_owned(),
            });
        }
        let declarations = operation_declarations()?;
        let scope = CoreCapabilityScope::new_for_target(
            sing_box_id,
            core_version.cloned(),
            self.host,
            self.binary_target,
            self.adapter_revision,
        );
        let entries = CoreOperation::all()
            .iter()
            .copied()
            .map(|operation| {
                CoreCapabilityEntry::new(
                    operation,
                    declarations.engine_decision_for_scope(
                        operation,
                        core_version,
                        self.binary_target,
                        self.adapter_revision,
                    ),
                    host_decision(
                        self.host,
                        self.binary_target,
                        operation,
                        self.adapter_revision,
                        declarations,
                    ),
                    adapter_decision(operation, &self.adapter_operations),
                )
            })
            .collect();
        CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: error.to_string(),
            }
        })
    }
}

fn host_decision(
    host: HostKind,
    binary_target: EngineBinaryTarget,
    operation: CoreOperation,
    adapter_revision: AdapterRevision,
    declarations: &ValidatedAdapterOperationDeclarations,
) -> HostCapabilityDecision {
    if binary_target.host_kind() != Some(host)
        || !declarations.has_registered_target(binary_target, adapter_revision)
    {
        HostCapabilityDecision::HostUnavailable {
            reason: format!(
                "sing-box has no verified asset for target {binary_target:?} at adapter revision {}",
                adapter_revision.value()
            ),
        }
    } else if matches!(
        operation.version_requirement(),
        CoreVersionRequirement::RequiresActiveVersion
    ) && declarations
        .verified_supported_evidence(operation)
        .is_none()
    {
        HostCapabilityDecision::HostUnavailable {
            reason: format!("sing-box has no verified support for {operation:?}"),
        }
    } else {
        HostCapabilityDecision::Supported
    }
}

fn adapter_decision(
    operation: CoreOperation,
    adapter_operations: &HashSet<CoreOperation>,
) -> AdapterCapabilityDecision {
    if adapter_operations.contains(&operation) {
        AdapterCapabilityDecision::Supported
    } else {
        AdapterCapabilityDecision::AdapterMissing {
            reason:
                "the product composition has not connected this operation to a sing-box adapter"
                    .to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "capability_provider_tests.rs"]
mod tests;

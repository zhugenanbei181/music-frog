//! Mihomo's engine-scoped capability declarations.

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

const MIHOMO_ADAPTER_REVISION: u64 = 2;

const MIHOMO_EVIDENCE_MANIFEST: &str =
    include_str!("../../../packaging/mihomo-evidence-assets.json");
const MIHOMO_OPERATION_DECLARATION: &str = include_str!("../adapter-operation-declarations.json");

#[cfg(test)]
fn evidence_manifest() -> Option<&'static serde_json::Value> {
    static MANIFEST: OnceLock<Option<serde_json::Value>> = OnceLock::new();
    MANIFEST
        .get_or_init(|| serde_json::from_str(MIHOMO_EVIDENCE_MANIFEST).ok())
        .as_ref()
}

fn operation_declarations()
-> Result<&'static ValidatedAdapterOperationDeclarations, CoreCapabilityProviderError> {
    static DECLARATIONS: OnceLock<Result<ValidatedAdapterOperationDeclarations, String>> =
        OnceLock::new();
    match DECLARATIONS.get_or_init(load_operation_declarations) {
        Ok(declarations) => Ok(declarations),
        Err(reason) => Err(CoreCapabilityProviderError::MatrixUnavailable {
            reason: format!("Mihomo operation declaration is invalid: {reason}"),
        }),
    }
}

fn load_operation_declarations() -> Result<ValidatedAdapterOperationDeclarations, String> {
    let declaration: AdapterOperationDeclarationFile =
        serde_json::from_str(MIHOMO_OPERATION_DECLARATION)
            .map_err(|error| format!("declaration JSON: {error}"))?;
    let evidence_lock =
        OperationEvidenceLock::from_manifest_json(MIHOMO_EVIDENCE_MANIFEST, "mihomo")?;
    ValidatedAdapterOperationDeclarations::validate(declaration, "mihomo", &evidence_lock)
        .map_err(|error| error.to_string())
}

// Connection.List is intentionally absent: the smoke check observed
// `connections=null` and counters, not an actual connection entry.

/// Capability source for the currently integrated Mihomo product adapter.
#[derive(Clone, Debug)]
pub struct MihomoCoreCapabilityProvider {
    host: HostKind,
    binary_target: EngineBinaryTarget,
    adapter_revision: AdapterRevision,
    adapter_operations: HashSet<CoreOperation>,
}

impl MihomoCoreCapabilityProvider {
    pub fn new(
        host: HostKind,
        adapter_operations: impl IntoIterator<Item = CoreOperation>,
    ) -> Self {
        Self {
            host,
            binary_target: EngineBinaryTarget::current(),
            adapter_revision: AdapterRevision::new(MIHOMO_ADAPTER_REVISION),
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
            adapter_revision: AdapterRevision::new(MIHOMO_ADAPTER_REVISION),
            adapter_operations: adapter_operations.into_iter().collect(),
        }
    }

    #[cfg(test)]
    fn new_with_scope(
        host: HostKind,
        binary_target: EngineBinaryTarget,
        adapter_revision: AdapterRevision,
        adapter_operations: impl IntoIterator<Item = CoreOperation>,
    ) -> Self {
        Self {
            host,
            binary_target,
            adapter_revision,
            adapter_operations: adapter_operations.into_iter().collect(),
        }
    }
}

impl CoreCapabilityProvider for MihomoCoreCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let mihomo_id = EngineId::new("mihomo").expect("static engine identifier is valid");
        if engine_id != &mihomo_id {
            return Err(CoreCapabilityProviderError::EngineUnavailable {
                engine_id: engine_id.clone(),
                reason: "the Mihomo adapter cannot author capabilities for another engine"
                    .to_string(),
            });
        }
        let scope = CoreCapabilityScope::new_for_target(
            mihomo_id,
            core_version.cloned(),
            self.host,
            self.binary_target,
            self.adapter_revision,
        );
        let declarations = operation_declarations()?;
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
                        declarations,
                        self.adapter_revision,
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

#[cfg(test)]
fn is_canonical_mihomo_version(version: &str) -> bool {
    let Some(numeric_version) = version.strip_prefix('v') else {
        return false;
    };
    let mut components = numeric_version.split('.');
    let is_canonical_component = |component: Option<&str>| {
        component.is_some_and(|value| {
            !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0'))
        })
    };

    is_canonical_component(components.next())
        && is_canonical_component(components.next())
        && is_canonical_component(components.next())
        && components.next().is_none()
}

fn host_decision(
    host: HostKind,
    binary_target: EngineBinaryTarget,
    operation: CoreOperation,
    declarations: &ValidatedAdapterOperationDeclarations,
    adapter_revision: AdapterRevision,
) -> HostCapabilityDecision {
    if binary_target.host_kind() != Some(host)
        || (matches!(
            operation.version_requirement(),
            CoreVersionRequirement::RequiresActiveVersion
        ) && !declarations.has_registered_target(binary_target, adapter_revision))
    {
        return HostCapabilityDecision::HostUnavailable {
            reason: format!(
                "no Mihomo operation evidence is registered for target {binary_target:?} at adapter revision {}",
                adapter_revision.value()
            ),
        };
    }
    match (host, operation) {
        (HostKind::Ios, CoreOperation::Tun(_)) => HostCapabilityDecision::HostUnavailable {
            reason: "the iOS host does not expose Mihomo TUN controls".to_string(),
        },
        (HostKind::Ios, CoreOperation::CoreGarbageCollection(_)) => {
            HostCapabilityDecision::HostUnavailable {
                reason: "the iOS product has no core cache garbage collector".to_string(),
            }
        }
        (HostKind::Ios, CoreOperation::Version(_)) => HostCapabilityDecision::HostUnavailable {
            reason: "the iOS product does not manage Mihomo binary versions".to_string(),
        },
        _ => HostCapabilityDecision::Supported,
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
            reason: "the product composition has not connected this operation to a Mihomo adapter"
                .to_string(),
        }
    }
}

#[cfg(test)]
#[path = "core_capability_provider_tests.rs"]
mod tests;

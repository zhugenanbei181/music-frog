//! Owner-scoped Core resource observation and memory soft-quota reclamation.

use infiltrator_contract::core_identity::CoreScope;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::resources::{
    CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus, CoreResourceSnapshot, ResourceAvailability,
};
use infiltrator_ports::core_scope::CoreScopeSource;
use infiltrator_ports::resources::{
    CoreResourcePort, GarbageCollectionOutcome, ScopedResourceObservation,
};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const GC_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct ResourceApplication {
    scope_source: Arc<dyn CoreScopeSource>,
    resource_port: Arc<dyn CoreResourcePort>,
    state: Arc<Mutex<ResourceState>>,
}

enum ResourceState {
    Ready,
    Collecting {
        scope: CoreScope,
    },
    CoolingDown {
        scope: CoreScope,
        attempted_at: Instant,
        outcome: CoreGcStatus,
    },
}

impl ResourceApplication {
    pub fn new(
        scope_source: Arc<dyn CoreScopeSource>,
        resource_port: Arc<dyn CoreResourcePort>,
    ) -> Self {
        Self {
            scope_source,
            resource_port,
            state: Arc::new(Mutex::new(ResourceState::Ready)),
        }
    }

    /// Observe resources and reclaim over-limit memory at most once per
    /// cooldown. `Collected` is published only when the adapter returns a
    /// same-scope post-GC memory readback.
    pub async fn poll(&self) -> Result<CoreResourceSnapshot, Failure> {
        let (scope, observation) = self.observe_scoped().await?;

        let gc = match &observation.memory_bytes {
            ResourceAvailability::Available(memory_bytes)
                if *memory_bytes > CORE_MEMORY_SOFT_LIMIT_BYTES =>
            {
                let admission = {
                    let mut state = self.state.lock().expect("resource state lock");
                    match &*state {
                        ResourceState::Collecting { scope: active } if active == &scope => {
                            GcAdmission::InProgress
                        }
                        ResourceState::CoolingDown {
                            scope: active,
                            attempted_at,
                            outcome,
                        } if active == &scope && attempted_at.elapsed() < GC_COOLDOWN => {
                            GcAdmission::CoolingDown(outcome.clone())
                        }
                        _ => {
                            *state = ResourceState::Collecting {
                                scope: scope.clone(),
                            };
                            GcAdmission::Start
                        }
                    }
                };
                match admission {
                    GcAdmission::InProgress => CoreGcStatus::Collecting,
                    GcAdmission::CoolingDown(outcome) => outcome,
                    GcAdmission::Start => self.collect_and_record(&scope, *memory_bytes).await?,
                }
            }
            ResourceAvailability::Available(_) => CoreGcStatus::NotNeeded,
            ResourceAvailability::NotObserved
            | ResourceAvailability::Unsupported
            | ResourceAvailability::Failed(_) => CoreGcStatus::NotAttempted,
        };

        self.ensure_current_scope(&scope)?;
        Ok(resource_snapshot(scope, observation, gc))
    }

    /// Return a read-only typed observation without triggering GC. Admin and
    /// diagnostic consumers use this when observation must have no side effect.
    pub async fn observe(&self) -> Result<CoreResourceSnapshot, Failure> {
        let (scope, observation) = self.observe_scoped().await?;
        Ok(resource_snapshot(
            scope,
            observation,
            CoreGcStatus::NotAttempted,
        ))
    }

    async fn observe_scoped(&self) -> Result<(CoreScope, ScopedResourceObservation), Failure> {
        let scope = crate::runtime_scope::active_scope(self.scope_source.as_ref())?;
        let observation = self
            .resource_port
            .observe(scope.clone())
            .await
            .map_err(Failure::from)?;
        ensure_scope(&scope, &observation.scope)?;
        self.ensure_current_scope(&scope)?;
        Ok((scope, observation))
    }

    async fn collect_and_record(
        &self,
        scope: &CoreScope,
        before_bytes: u64,
    ) -> Result<CoreGcStatus, Failure> {
        let receipt = self
            .resource_port
            .collect_garbage(scope.clone(), before_bytes)
            .await;
        let outcome = match receipt {
            Ok(receipt) => {
                if let Err(failure) = ensure_scope(scope, &receipt.scope) {
                    self.reset_collection(scope);
                    return Err(failure);
                }
                match receipt.outcome {
                    GarbageCollectionOutcome::Collected {
                        before_bytes,
                        after_bytes,
                    } => CoreGcStatus::Collected {
                        before_bytes,
                        after_bytes,
                    },
                    GarbageCollectionOutcome::Unsupported => CoreGcStatus::Unsupported,
                    GarbageCollectionOutcome::Failed(failure) => CoreGcStatus::Failed { failure },
                }
            }
            Err(error) if error.error_code() == ErrorCode::InvalidState => {
                self.reset_collection(scope);
                return Err(Failure::from(error));
            }
            Err(error) => CoreGcStatus::Failed {
                failure: Failure::from(error),
            },
        };
        if let Err(failure) = self.ensure_current_scope(scope) {
            self.reset_collection(scope);
            return Err(failure);
        }
        let mut state = self.state.lock().expect("resource state lock");
        if matches!(&*state, ResourceState::Collecting { scope: active } if active == scope) {
            *state = ResourceState::CoolingDown {
                scope: scope.clone(),
                attempted_at: Instant::now(),
                outcome: outcome.clone(),
            };
        }
        Ok(outcome)
    }

    fn reset_collection(&self, scope: &CoreScope) {
        let mut state = self.state.lock().expect("resource state lock");
        if matches!(&*state, ResourceState::Collecting { scope: active } if active == scope) {
            *state = ResourceState::Ready;
        }
    }

    fn ensure_current_scope(&self, requested: &CoreScope) -> Result<(), Failure> {
        let current = crate::runtime_scope::active_scope(self.scope_source.as_ref())?;
        ensure_scope(requested, &current)
    }
}

fn resource_snapshot(
    scope: CoreScope,
    observation: ScopedResourceObservation,
    gc: CoreGcStatus,
) -> CoreResourceSnapshot {
    CoreResourceSnapshot::Observed {
        scope,
        memory: observation.memory_bytes,
        cpu_percent: observation.cpu_percent,
        memory_soft_limit_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES,
        gc,
    }
}

enum GcAdmission {
    Start,
    InProgress,
    CoolingDown(CoreGcStatus),
}

fn ensure_scope(expected: &CoreScope, observed: &CoreScope) -> Result<(), Failure> {
    if expected == observed {
        return Ok(());
    }
    Err(Failure::new(
        ErrorCode::InvalidState,
        "resource observation belongs to a retired Core scope",
        false,
    ))
}

#[cfg(test)]
mod tests {
    use infiltrator_contract::core_identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };

    use super::*;

    #[test]
    fn soft_limit_is_exactly_512_mib_and_strictly_exceeded() {
        let scope = test_scope(1);
        let snapshot = CoreResourceSnapshot::Observed {
            scope: scope.clone(),
            memory: ResourceAvailability::Available(CORE_MEMORY_SOFT_LIMIT_BYTES),
            cpu_percent: ResourceAvailability::Unsupported,
            memory_soft_limit_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES,
            gc: CoreGcStatus::NotNeeded,
        };
        assert!(!snapshot.over_memory_limit());
        let snapshot = CoreResourceSnapshot::Observed {
            scope,
            memory: ResourceAvailability::Available(CORE_MEMORY_SOFT_LIMIT_BYTES + 1),
            cpu_percent: ResourceAvailability::Unsupported,
            memory_soft_limit_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES,
            gc: CoreGcStatus::NotAttempted,
        };
        assert!(snapshot.over_memory_limit());
    }

    fn test_scope(generation: u64) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::mihomo(),
                CoreInstanceId::new("resource-test").expect("instance"),
                Some(CoreVersion::new("test").expect("version")),
            ),
            generation: CoreGeneration::new(generation),
            session_token: None,
        }
    }
}

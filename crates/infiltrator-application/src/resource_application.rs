//! Core resource observation and memory soft-quota reclamation.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::resources::{
    CORE_MEMORY_SOFT_LIMIT_BYTES, CoreGcStatus, CoreResourceSnapshot,
};
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const GC_COOLDOWN: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct ResourceApplication {
    gateway: Arc<dyn RuntimeGateway>,
    state: Arc<Mutex<ResourceState>>,
}

struct ResourceState {
    gc: CoreGcStatus,
    last_gc: Option<Instant>,
}

impl Default for ResourceState {
    fn default() -> Self {
        Self {
            gc: CoreGcStatus::Unknown,
            last_gc: None,
        }
    }
}

impl ResourceApplication {
    pub fn new(gateway: Arc<dyn RuntimeGateway>) -> Self {
        Self {
            gateway,
            state: Arc::new(Mutex::new(ResourceState::default())),
        }
    }

    /// Observe resources and trigger Mihomo GC once per cooldown when the
    /// memory soft limit is exceeded. The post-GC memory readback is retained
    /// in the shared snapshot; a failed/unsupported GC is never reported as
    /// reclaimed.
    pub async fn poll(&self) -> Result<CoreResourceSnapshot, Failure> {
        let memory = self.gateway.get_memory().await.map_err(Failure::from)?;
        let cpu_percent = self.gateway.get_cpu_percent().await.map_err(Failure::from)?;
        let over_limit = memory.in_use > CORE_MEMORY_SOFT_LIMIT_BYTES;
        let should_collect = over_limit && {
            let state = self.state.lock().expect("resource state lock");
            state
                .last_gc
                .is_none_or(|last| last.elapsed() >= GC_COOLDOWN)
        };

        if should_collect {
            let gc = match self.gateway.trigger_gc().await {
                Ok(()) => {
                    let after = self.gateway.get_memory().await.ok().map(|value| value.in_use);
                    CoreGcStatus::Triggered {
                        before_bytes: memory.in_use,
                        after_bytes: after,
                    }
                }
                Err(error) if error.error_code() == ErrorCode::Unsupported => {
                    CoreGcStatus::Unsupported
                }
                Err(error) => CoreGcStatus::Failed {
                    failure: Failure::from(error),
                },
            };
            let mut state = self.state.lock().expect("resource state lock");
            state.last_gc = Some(Instant::now());
            state.gc = gc;
        } else if !over_limit {
            self.state.lock().expect("resource state lock").gc = CoreGcStatus::NotNeeded;
        }

        let gc = self.state.lock().expect("resource state lock").gc.clone();
        Ok(CoreResourceSnapshot {
            memory_bytes: Some(memory.in_use),
            cpu_percent,
            memory_soft_limit_bytes: CORE_MEMORY_SOFT_LIMIT_BYTES,
            gc,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_limit_is_exactly_512_mib_and_strictly_exceeded() {
        let mut snapshot = CoreResourceSnapshot {
            memory_bytes: Some(CORE_MEMORY_SOFT_LIMIT_BYTES),
            ..CoreResourceSnapshot::default()
        };
        assert!(!snapshot.over_memory_limit());
        snapshot.memory_bytes = Some(CORE_MEMORY_SOFT_LIMIT_BYTES + 1);
        assert!(snapshot.over_memory_limit());
    }
}

//! Profile reset use-case over a host-provided reset port.

use infiltrator_contract::error::Failure;
use infiltrator_ports::profile_reset::ProfileResetPort;
use std::sync::Arc;

#[derive(Clone)]
pub struct ProfileResetApplication {
    port: Arc<dyn ProfileResetPort>,
}

impl ProfileResetApplication {
    pub fn new(port: Arc<dyn ProfileResetPort>) -> Self {
        Self { port }
    }

    pub async fn reset_to_default(&self) -> Result<(), Failure> {
        self.port.reset_to_default().await.map_err(Failure::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_ports::error::PortError;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct FakeReset {
        called: Arc<AtomicBool>,
    }

    #[async_trait]
    impl ProfileResetPort for FakeReset {
        async fn reset_to_default(&self) -> Result<(), PortError> {
            self.called.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn resets_profiles_through_the_port() {
        let called = Arc::new(AtomicBool::new(false));
        let application = ProfileResetApplication::new(Arc::new(FakeReset {
            called: Arc::clone(&called),
        }));
        application.reset_to_default().await.expect("reset profiles");
        assert!(called.load(Ordering::SeqCst));
    }
}

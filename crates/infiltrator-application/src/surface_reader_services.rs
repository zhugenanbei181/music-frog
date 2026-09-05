//! Host-service state reads kept outside the large surface assembler.

use infiltrator_contract::controller::{ControllerAuthSnapshot, ControllerAuthStatus};

use super::ApplicationSurfaceReader;

impl ApplicationSurfaceReader {
    pub(super) async fn read_controller_auth(&self) -> ControllerAuthSnapshot {
        let Some(source) = &self.endpoint_source else {
            return ControllerAuthSnapshot::default();
        };
        match source.resolve().await {
            Ok(endpoint) => ControllerAuthSnapshot {
                status: if endpoint
                    .secret
                    .as_deref()
                    .is_some_and(|secret| !secret.trim().is_empty())
                {
                    ControllerAuthStatus::Secured
                } else {
                    ControllerAuthStatus::Missing
                },
            },
            Err(_) => ControllerAuthSnapshot {
                status: ControllerAuthStatus::Unavailable,
            },
        }
    }

    pub(super) async fn read_service_mode(
        &self,
    ) -> infiltrator_contract::service_mode::ServiceModeSnapshot {
        match &self.service_mode {
            Some(application) => application.snapshot().await.unwrap_or_default(),
            None => Default::default(),
        }
    }
}

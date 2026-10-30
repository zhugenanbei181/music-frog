//! Mihomo controller adapter for the scoped Fake-IP cache operation.

use infiltrator_ports::controller_fake_ip_cache::{
    ControllerFakeIpCacheFlushReceipt, ControllerFakeIpCacheFlushRequest,
    ControllerFakeIpCacheFlushState, ControllerFakeIpCachePort,
};
use lurker_core_contract::capability::Capability;
use lurker_core_contract::identity::CoreScope;
use lurker_core_ports::error::PortError;
use mihomo_driver::api::client::MihomoClient;
use mihomo_driver::network_error::network_error;

#[derive(Clone)]
pub struct MihomoControllerFakeIpCacheAdapter {
    client: MihomoClient,
}

impl MihomoControllerFakeIpCacheAdapter {
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl ControllerFakeIpCachePort for MihomoControllerFakeIpCacheAdapter {
    async fn flush(
        &self,
        request: ControllerFakeIpCacheFlushRequest,
    ) -> Result<ControllerFakeIpCacheFlushReceipt, PortError> {
        ensure_mihomo_scope(&request.scope)?;
        self.client
            .flush_fakeip_cache()
            .await
            .map_err(network_error)?;
        Ok(ControllerFakeIpCacheFlushReceipt {
            scope: request.scope,
            operation: request.operation,
            state: ControllerFakeIpCacheFlushState::Flushed,
        })
    }
}

fn ensure_mihomo_scope(scope: &CoreScope) -> Result<(), PortError> {
    if scope.identity.engine.as_str() == "mihomo" {
        Ok(())
    } else {
        Err(PortError::unsupported(
            Capability::Dns,
            "Mihomo Fake-IP adapter received a non-Mihomo Core scope",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns_cache::DnsCacheOperationId;
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion, EngineId,
    };
    use mockito::Server;

    fn scope(engine: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new(engine).expect("valid engine ID"),
                CoreInstanceId::new("fakeip-test").expect("valid instance ID"),
                Some(CoreVersion::new("v1.19.33").expect("valid core version")),
            ),
            generation: CoreGeneration::new(3),
            session_token: None,
        }
    }

    fn request(engine: &str) -> ControllerFakeIpCacheFlushRequest {
        ControllerFakeIpCacheFlushRequest {
            scope: scope(engine),
            operation: DnsCacheOperationId(41),
        }
    }

    #[tokio::test]
    async fn foreign_engine_is_unsupported_before_controller_io() {
        let mut server = Server::new_async().await;
        let endpoint = server
            .mock("POST", "/cache/fakeip/flush")
            .expect(0)
            .with_status(204)
            .create_async()
            .await;
        let client = MihomoClient::new(&server.url(), None).expect("valid controller URL");
        let adapter = MihomoControllerFakeIpCacheAdapter::new(client);

        let error = adapter
            .flush(request("sing-box"))
            .await
            .expect_err("foreign engine must fail closed");

        assert!(matches!(error, PortError::Unsupported { .. }));
        endpoint.assert_async().await;
    }

    #[tokio::test]
    async fn successful_controller_response_returns_a_typed_scoped_receipt() {
        let mut server = Server::new_async().await;
        let endpoint = server
            .mock("POST", "/cache/fakeip/flush")
            .expect(1)
            .match_header("authorization", "Bearer test-secret")
            .with_status(204)
            .create_async()
            .await;
        let client = MihomoClient::new(&server.url(), Some("test-secret".to_owned()))
            .expect("valid controller URL");
        let adapter = MihomoControllerFakeIpCacheAdapter::new(client);
        let request = request("mihomo");

        let receipt = adapter
            .flush(request.clone())
            .await
            .expect("controller accepted the flush");

        assert_eq!(receipt.scope, request.scope);
        assert_eq!(receipt.operation, request.operation);
        assert_eq!(receipt.state, ControllerFakeIpCacheFlushState::Flushed);
        endpoint.assert_async().await;
    }
}

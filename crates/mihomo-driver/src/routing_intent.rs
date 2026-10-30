//! Mihomo adapter for common traffic-routing policies.

use crate::api::client::MihomoClient;
use crate::network_error::network_error;
use lurker_core_contract::capability::Capability;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_contract::routing_intent::{
    EngineDefinedRoutingIntent, RoutingIntent, RoutingIntentExtensionId,
};
use lurker_core_ports::error::PortError;
use lurker_core_ports::routing_intent::RoutingIntentPort;
use serde_json::json;

/// The Mihomo adapter of the shared routing intent.
#[derive(Clone)]
pub struct MihomoRoutingIntentAdapter {
    client: MihomoClient,
}

impl MihomoRoutingIntentAdapter {
    /// Creates the routing-intent adapter over one controller client.
    pub fn new(client: MihomoClient) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl RoutingIntentPort for MihomoRoutingIntentAdapter {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        ensure_mihomo_scope(scope)?;
        let config = self.client.get_config().await.map_err(network_error)?;
        routing_intent_from_mihomo_mode(&config.mode)
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        let mode = requested_mihomo_mode(scope, &intent)?;
        self.client
            .patch_config(json!({ "mode": mode }))
            .await
            .map_err(network_error)?;

        let observed = self.observe(scope).await?;
        if observed != intent {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                format!("Mihomo retained routing intent {observed:?} after requesting {intent:?}"),
                true,
            )));
        }
        Ok(observed)
    }
}

fn requested_mihomo_mode(
    scope: &CoreScope,
    intent: &RoutingIntent,
) -> Result<&'static str, PortError> {
    ensure_mihomo_scope(scope)?;
    let mihomo = EngineId::new("mihomo").expect("static engine identifier is valid");
    match intent {
        RoutingIntent::FollowRules => Ok("rule"),
        RoutingIntent::RouteAllThroughProxy => Ok("global"),
        RoutingIntent::BypassProxy => Ok("direct"),
        RoutingIntent::EngineDefined(extension) => {
            if extension.engine() != &mihomo {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::InvalidState,
                    "engine-defined routing intent belongs to another engine",
                    false,
                )));
            }
            Err(unsupported_engine_defined_intent(extension))
        }
    }
}

fn ensure_mihomo_scope(scope: &CoreScope) -> Result<(), PortError> {
    let mihomo = EngineId::new("mihomo").expect("static engine identifier is valid");
    if scope.identity.engine == mihomo {
        Ok(())
    } else {
        Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            "Mihomo routing adapter received a scope for another engine",
            false,
        )))
    }
}

pub(crate) fn routing_intent_from_mihomo_mode(mode: &str) -> Result<RoutingIntent, PortError> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "rule" => Ok(RoutingIntent::FollowRules),
        "global" => Ok(RoutingIntent::RouteAllThroughProxy),
        "direct" => Ok(RoutingIntent::BypassProxy),
        "script" => Ok(RoutingIntent::EngineDefined(
            EngineDefinedRoutingIntent::new(
                EngineId::new("mihomo").expect("static engine identifier is valid"),
                RoutingIntentExtensionId::new("script")
                    .expect("static extension identifier is valid"),
            ),
        )),
        _ => Err(PortError::Rejected(Failure::new(
            ErrorCode::InvalidState,
            format!("Mihomo returned an unknown routing policy: {mode}"),
            false,
        ))),
    }
}

fn unsupported_engine_defined_intent(extension: &EngineDefinedRoutingIntent) -> PortError {
    let reason = if extension.engine().as_str() == "mihomo"
        && extension.extension_id().as_str() == "script"
    {
        "Mihomo Script policy is engine-specific and is not a shared routing intent"
    } else {
        "Mihomo does not implement the requested engine-defined routing intent"
    };
    PortError::unsupported(Capability::RoutingIntent, reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ConfigResponse;
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreVersion,
    };
    use mockito::{Matcher, Server};
    use serde_json::json;

    fn scope(engine: &str) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new(engine).expect("valid engine ID"),
                CoreInstanceId::new("core-a").expect("valid instance ID"),
                Some(CoreVersion::new("v1.19.33").expect("valid version")),
            ),
            generation: CoreGeneration::new(1),
            session_token: None,
        }
    }

    fn adapter() -> MihomoRoutingIntentAdapter {
        let client = MihomoClient::new("http://127.0.0.1:1", None).expect("valid URL");
        MihomoRoutingIntentAdapter::new(client)
    }

    #[test]
    fn common_intents_map_to_the_three_shared_mihomo_modes() {
        let scope = scope("mihomo");
        assert_eq!(
            requested_mihomo_mode(&scope, &RoutingIntent::FollowRules),
            Ok("rule")
        );
        assert_eq!(
            requested_mihomo_mode(&scope, &RoutingIntent::RouteAllThroughProxy),
            Ok("global")
        );
        assert_eq!(
            requested_mihomo_mode(&scope, &RoutingIntent::BypassProxy),
            Ok("direct")
        );
    }

    #[tokio::test]
    async fn script_extension_is_typed_unsupported_before_network_io() {
        let intent = RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
            EngineId::new("mihomo").expect("valid engine ID"),
            RoutingIntentExtensionId::new("script").expect("valid extension ID"),
        ));
        let error = adapter()
            .apply(&scope("mihomo"), intent)
            .await
            .expect_err("Script is not part of the common routing contract");

        assert!(matches!(
            error,
            PortError::Unsupported {
                capability: Capability::RoutingIntent,
                ..
            }
        ));
    }

    #[tokio::test]
    async fn foreign_engine_scope_is_rejected_before_network_io() {
        let error = adapter()
            .apply(&scope("sing-box"), RoutingIntent::FollowRules)
            .await
            .expect_err("Mihomo adapter cannot write a foreign engine");

        assert!(matches!(
            error,
            PortError::Rejected(Failure {
                code: ErrorCode::InvalidState,
                ..
            })
        ));
    }

    #[test]
    fn mihomo_script_readback_stays_engine_defined() {
        assert_eq!(
            routing_intent_from_mihomo_mode("script").expect("known Mihomo mode"),
            RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
                EngineId::new("mihomo").expect("valid engine ID"),
                RoutingIntentExtensionId::new("script").expect("valid extension ID"),
            ))
        );
    }

    #[tokio::test]
    async fn common_intent_writes_mihomo_wire_value_and_checks_live_readback() {
        let mut server = Server::new_async().await;
        let patch = server
            .mock("PATCH", "/configs")
            .match_body(Matcher::JsonString(json!({"mode":"global"}).to_string()))
            .with_status(204)
            .create_async()
            .await;
        let config = ConfigResponse {
            mode: "global".to_owned(),
            ..ConfigResponse::default()
        };
        let readback = server
            .mock("GET", "/configs")
            .with_status(200)
            .with_body(serde_json::to_string(&config).expect("serialize config"))
            .create_async()
            .await;
        let client = MihomoClient::new(&server.url(), None).expect("valid test URL");
        let adapter = MihomoRoutingIntentAdapter::new(client);

        let actual = adapter
            .apply(&scope("mihomo"), RoutingIntent::RouteAllThroughProxy)
            .await
            .expect("matching controller readback");

        assert_eq!(actual, RoutingIntent::RouteAllThroughProxy);
        patch.assert_async().await;
        readback.assert_async().await;
    }

    #[tokio::test]
    async fn typed_observe_owns_mihomo_wire_conversion() {
        let mut server = Server::new_async().await;
        let config = ConfigResponse {
            mode: "script".to_owned(),
            ..ConfigResponse::default()
        };
        let readback = server
            .mock("GET", "/configs")
            .with_status(200)
            .with_body(serde_json::to_string(&config).expect("serialize config"))
            .create_async()
            .await;
        let client = MihomoClient::new(&server.url(), None).expect("valid test URL");
        let adapter = MihomoRoutingIntentAdapter::new(client);

        let observed = adapter
            .observe(&scope("mihomo"))
            .await
            .expect("known Mihomo route is translated by the adapter");

        assert_eq!(
            observed,
            RoutingIntent::EngineDefined(EngineDefinedRoutingIntent::new(
                EngineId::new("mihomo").expect("valid engine ID"),
                RoutingIntentExtensionId::new("script").expect("valid extension ID"),
            ))
        );
        readback.assert_async().await;
    }
}

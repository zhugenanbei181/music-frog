//! Shared scripted fixtures for the Clash API adapter unit tests.

use crate::api::clash_api::SingBoxClashApiClient;
use crate::api::controller_configuration::SingBoxControllerConfiguration;
use crate::api::http::{
    SingBoxClashHttpPort, SingBoxClashMethod, SingBoxClashResponse, SingBoxControllerAuthorization,
};
use async_trait::async_trait;
use lurker_core_contract::identity::{
    CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
};
use lurker_core_contract::outbound::identity::OutboundId;
use lurker_core_ports::core_scope::{CoreScopeObservation, CoreScopeSource};
use lurker_core_ports::error::PortError;
use std::collections::VecDeque;
use std::future::pending;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub(crate) struct ObservedRequest {
    pub(crate) url: String,
    pub(crate) method: SingBoxClashMethod,
    pub(crate) authorization: SingBoxControllerAuthorization,
    pub(crate) body: Option<Vec<u8>>,
    pub(crate) timeout: Duration,
}

/// One scripted answer of the fake controller.
enum ScriptedAnswer {
    /// The controller answers with this response.
    Response(SingBoxClashResponse),
    /// The controller never answers.
    Hang,
    /// The controller fails at the transport level.
    Failure,
}

#[derive(Default)]
pub(crate) struct ScriptedHttpPort {
    responses: Mutex<VecDeque<ScriptedAnswer>>,
    requests: Mutex<Vec<ObservedRequest>>,
}

impl ScriptedHttpPort {
    pub(crate) fn push_json(&self, status: u16, body: serde_json::Value) {
        self.responses
            .lock()
            .unwrap()
            .push_back(ScriptedAnswer::Response(SingBoxClashResponse {
                status,
                body: serde_json::to_vec(&body).unwrap(),
            }));
    }

    pub(crate) fn push_text(&self, status: u16, body: &str) {
        self.responses
            .lock()
            .unwrap()
            .push_back(ScriptedAnswer::Response(SingBoxClashResponse {
                status,
                body: body.as_bytes().to_vec(),
            }));
    }

    pub(crate) fn push_empty(&self, status: u16) {
        self.responses
            .lock()
            .unwrap()
            .push_back(ScriptedAnswer::Response(SingBoxClashResponse {
                status,
                body: Vec::new(),
            }));
    }

    /// Scripts a controller that never answers the request.
    pub(crate) fn push_hang(&self) {
        self.responses
            .lock()
            .unwrap()
            .push_back(ScriptedAnswer::Hang);
    }

    /// Scripts a controller whose request fails at the transport level.
    pub(crate) fn push_failure(&self) {
        self.responses
            .lock()
            .unwrap()
            .push_back(ScriptedAnswer::Failure);
    }

    pub(crate) fn requests(&self) -> Vec<ObservedRequest> {
        self.requests.lock().unwrap().clone()
    }

    pub(crate) fn remaining_responses(&self) -> usize {
        self.responses.lock().unwrap().len()
    }
}

#[async_trait]
impl SingBoxClashHttpPort for ScriptedHttpPort {
    async fn request(
        &self,
        url: &Url,
        method: SingBoxClashMethod,
        authorization: &SingBoxControllerAuthorization,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<SingBoxClashResponse, PortError> {
        self.requests.lock().unwrap().push(ObservedRequest {
            url: url.to_string(),
            method,
            authorization: authorization.clone(),
            body,
            timeout,
        });
        let answer = self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| PortError::Failed("scripted HTTP response is missing".to_owned()))?;
        match answer {
            ScriptedAnswer::Response(response) => Ok(response),
            ScriptedAnswer::Failure => Err(PortError::Network(
                "scripted HTTP request failed".to_owned(),
            )),
            ScriptedAnswer::Hang => {
                pending::<()>().await;
                unreachable!("a hanging scripted answer never completes")
            }
        }
    }
}

#[derive(Clone)]
pub(crate) struct FixedScope(Arc<Mutex<CoreScopeObservation>>);

impl FixedScope {
    pub(crate) fn new(scope: CoreScope) -> Self {
        Self(Arc::new(Mutex::new(CoreScopeObservation::Observed(scope))))
    }
}

impl CoreScopeSource for FixedScope {
    fn scope_observation(&self) -> CoreScopeObservation {
        self.0.lock().unwrap().clone()
    }
}

pub(crate) fn scope(generation: u64) -> CoreScope {
    CoreScope {
        identity: CoreIdentity::new(
            EngineId::new("sing-box").unwrap(),
            CoreInstanceId::new("primary").unwrap(),
            Some(CoreVersion::new("v1.14.3").unwrap()),
        ),
        generation: CoreGeneration::new(generation),
        session_token: None,
    }
}

pub(crate) fn client(http: Arc<ScriptedHttpPort>) -> SingBoxClashApiClient {
    SingBoxClashApiClient::new(
        SingBoxControllerConfiguration::from_profile_json(
            br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090","secret":"fixture-secret"}}}"#,
        )
        .unwrap(),
        http,
    )
    .unwrap()
}

pub(crate) fn version_response() -> serde_json::Value {
    serde_json::json!({"version":"sing-box 1.14.3","meta":true})
}

pub(crate) fn proxy_response(selected: &str) -> serde_json::Value {
    serde_json::json!({
        "proxies": {
            "selector #1": {
                "type":"Selector", "name":"selector #1", "now":selected,
                "all":["proxy-a", "proxy-b"], "history":[], "udp":true
            },
            "proxy-a":{"type":"Direct", "name":"proxy-a", "udp":true, "history":[]},
            "proxy-b":{"type":"Direct", "name":"proxy-b", "udp":true, "history":[]},
            "GLOBAL":{"type":"Fallback", "name":"GLOBAL", "now":selected,
                "all":["selector #1", "proxy-a", "proxy-b"], "history":[]}
        }
    })
}

pub(crate) fn group_proxy_response() -> serde_json::Value {
    serde_json::json!({
        "proxies": {
            "automatic": {
                "type":"URLTest", "name":"automatic", "now":"proxy-a",
                "all":["proxy-a"], "history":[]
            },
            "future-group": {
                "type":"FutureGroup", "name":"future-group", "now":"proxy-a",
                "all":["proxy-a"], "history":[]
            },
            "proxy-a":{"type":"Direct", "name":"proxy-a", "history":[]}
        }
    })
}

pub(crate) fn enqueue_inventory_observation(http: &ScriptedHttpPort, selected: &str) {
    http.push_json(200, version_response());
    http.push_json(200, proxy_response(selected));
    http.push_json(200, version_response());
}

pub(crate) fn outbound_id(value: &str) -> OutboundId {
    OutboundId::new(EngineId::new("sing-box").unwrap(), value).unwrap()
}

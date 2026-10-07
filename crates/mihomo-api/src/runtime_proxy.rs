//! The controller wire boundary accepts partial observations without weakening config models.
use infiltrator_domain::proxy::Proxy;
use infiltrator_domain::proxy_observation::RuntimeProxyObservation;
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use serde_json::{Value, from_value};
use std::collections::HashMap;

pub struct ControllerProxy(pub Proxy);
impl<'de> Deserialize<'de> for ControllerProxy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        // Parse observed field types even when an older controller also includes
        // complete configuration metadata. Malformed facts must never disappear.
        let facts: RuntimeProxyObservation = from_value(value.clone()).map_err(D::Error::custom)?;
        if facts.name.trim().is_empty() || facts.proxy_type.trim().is_empty() {
            return Err(D::Error::custom("controller proxy identity/type is empty"));
        }
        let proxy = match from_value::<Proxy>(value) {
            Ok(proxy) if !matches!(proxy, Proxy::Unknown) => proxy,
            _ => Proxy::Observed(facts),
        };
        Ok(Self(proxy))
    }
}

pub fn deserialize_proxy_map<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<HashMap<String, Proxy>, D::Error> {
    let observed = HashMap::<String, ControllerProxy>::deserialize(deserializer)?;
    observed
        .into_iter()
        .map(|(identity, ControllerProxy(proxy))| {
            if identity != proxy.name() {
                Err(D::Error::custom(
                    "controller proxy key differs from its reported name",
                ))
            } else {
                Ok((identity, proxy))
            }
        })
        .collect()
}

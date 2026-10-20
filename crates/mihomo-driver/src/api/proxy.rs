//! Neutral decoder for the controller `/proxies` observation wire.
//!
//! Controller observations are partial runtime facts, never complete
//! configuration nodes: an omitted field stays absent instead of being
//! backfilled with a configuration default. Decoding rejects malformed
//! reported fields and an identity key that disagrees with the reported name.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// One latency sample published in a proxy's observation history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyHistory {
    /// The timestamp of the sample.
    pub time: String,
    /// The measured delay in milliseconds.
    pub delay: u32,
}

/// Partial runtime facts reported by the controller for one outbound.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyObservation {
    /// The outbound type the controller reports, for example `Selector`.
    #[serde(rename = "type")]
    pub proxy_type: String,
    /// The name of the outbound.
    pub name: String,
    /// Whether the outbound carries UDP traffic, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp: Option<bool>,
    /// Whether the outbound is alive, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alive: Option<bool>,
    /// The published latency history of the outbound.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<ProxyHistory>>,
    /// The measured delay of the outbound, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<u32>,
    /// The server address of the outbound, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    /// The server port of the outbound, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// The cipher of the outbound, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,
    /// The published member list, when the outbound is a group.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<Vec<String>>,
    /// The currently selected member, when the controller reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
}

impl ProxyObservation {
    /// Returns the reported name of the outbound.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the reported outbound type.
    pub fn proxy_type(&self) -> &str {
        &self.proxy_type
    }

    /// A reported member list is the only group evidence. A group name without
    /// a published member list keeps its selection but is not a collection.
    pub fn is_group(&self) -> bool {
        self.all.is_some()
    }

    /// Returns the published member list of a group, when there is one.
    pub fn all(&self) -> Option<&[String]> {
        self.all.as_deref()
    }

    /// Returns the currently selected member, when the controller reports it.
    pub fn now(&self) -> Option<&str> {
        self.now.as_deref()
    }

    /// Returns the published latency history, or an empty slice.
    pub fn history(&self) -> &[ProxyHistory] {
        self.history.as_deref().unwrap_or_default()
    }

    /// Returns the reported UDP support, when the controller reports it.
    ///
    /// A group or an `Unknown` outbound carries no UDP fact.
    pub fn udp_observation(&self) -> Option<bool> {
        if self.is_group() || self.proxy_type == "Unknown" {
            return None;
        }
        self.udp
    }

    /// Returns the reported health, when the controller reports it.
    ///
    /// A group or an `Unknown` outbound carries no health fact.
    pub fn health_observation(&self) -> Option<bool> {
        if self.is_group() || self.proxy_type == "Unknown" {
            return None;
        }
        self.alive
    }

    /// The explicit reported delay, or the most recent history sample when the
    /// controller did not publish the field directly.
    pub fn delay(&self) -> Option<u32> {
        self.delay
            .or_else(|| self.history().last().map(|sample| sample.delay))
    }
}

/// The controller `/proxies` response.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProxiesResponse {
    /// The observed outbounds, keyed by their reported name.
    #[serde(deserialize_with = "deserialize_proxy_map")]
    pub proxies: HashMap<String, ProxyObservation>,
}

/// Decodes one controller proxy payload, rejecting empty identity/type facts.
pub fn decode_proxy<'de, D>(deserializer: D) -> Result<ProxyObservation, D::Error>
where
    D: Deserializer<'de>,
{
    let observation = ProxyObservation::deserialize(deserializer)?;
    if observation.name.trim().is_empty() || observation.proxy_type.trim().is_empty() {
        return Err(D::Error::custom("controller proxy identity/type is empty"));
    }
    Ok(observation)
}

/// Decodes the controller `/proxies` map.
///
/// A key that disagrees with the reported name is rejected: the identity of
/// an outbound must be unambiguous.
pub fn deserialize_proxy_map<'de, D>(
    deserializer: D,
) -> Result<HashMap<String, ProxyObservation>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wire(#[serde(deserialize_with = "decode_proxy")] ProxyObservation);

    let observed = HashMap::<String, Wire>::deserialize(deserializer)?;
    observed
        .into_iter()
        .map(|(identity, Wire(proxy))| {
            if identity != proxy.name {
                Err(D::Error::custom(
                    "controller proxy key differs from its reported name",
                ))
            } else {
                Ok((identity, proxy))
            }
        })
        .collect()
}

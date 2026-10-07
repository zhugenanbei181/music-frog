//! Controller observations are partial runtime facts, never complete configuration nodes.
use crate::proxy::ProxyHistory;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RuntimeProxyObservation {
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub udp: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alive: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<ProxyHistory>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cipher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub now: Option<String>,
}

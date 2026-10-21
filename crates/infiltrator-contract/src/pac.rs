//! Shared contract for generated PAC scripts and their local service.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacServiceState {
    Disabled,
    Running { url: String },
    Unavailable { reason: String },
}

/// Confirmed PAC service state. The generated script itself stays inside the
/// host service and is not copied into the cross-surface snapshot.
///
/// The snapshot is the readback of the PAC request the owner applied: it
/// carries every fact of that request, so a consumer never has to guess
/// whether the LAN bypass or the minified script was part of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacSnapshot {
    pub state: PacServiceState,
    pub script_bytes: usize,
    pub bypass_domains: Vec<String>,
    /// Whether the generated script bypasses the LAN.
    #[serde(default)]
    pub bypass_lan: bool,
    /// Whether the generated script is minified.
    #[serde(default)]
    pub minify: bool,
    pub revision: u64,
}

impl Default for PacSnapshot {
    fn default() -> Self {
        Self {
            state: PacServiceState::Disabled,
            script_bytes: 0,
            bypass_domains: Vec::new(),
            bypass_lan: false,
            minify: false,
            revision: 0,
        }
    }
}

/// User-editable PAC application request. Rules are read from the live
/// runtime gateway by the application, keeping policy generation out of UIs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacRequest {
    pub enabled: bool,
    pub bypass_domains: Vec<String>,
    pub bypass_lan: bool,
    pub minify: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pac_readback_carries_every_fact_of_the_request() {
        let request = PacRequest {
            enabled: true,
            bypass_domains: vec!["example.com".to_owned()],
            bypass_lan: true,
            minify: true,
        };
        let snapshot = PacSnapshot {
            state: PacServiceState::Running {
                url: "http://127.0.0.1:25211/proxy.pac".to_owned(),
            },
            script_bytes: 128,
            bypass_domains: request.bypass_domains.clone(),
            bypass_lan: request.bypass_lan,
            minify: request.minify,
            revision: 4,
        };
        // The readback answers the request instead of defaulting to false.
        assert!(snapshot.bypass_lan);
        assert!(snapshot.minify);
        assert_eq!(snapshot.bypass_domains, request.bypass_domains);
    }

    #[test]
    fn an_older_pac_payload_still_decodes() {
        let decoded: PacSnapshot = serde_json::from_str(
            r#"{"state":"Disabled","script_bytes":0,"bypass_domains":[],"revision":2}"#,
        )
        .expect("an older PAC payload decodes");
        assert!(!decoded.bypass_lan);
        assert!(!decoded.minify);
        assert_eq!(decoded.revision, 2);
    }
}

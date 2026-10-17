//! Engine-scoped typed updates for profile configuration sections.

use lurker_core_contract::profile_document_identity::ProfileDocumentIdentity;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A fully decoded configuration update bound to the document revision that
/// supplied its editor buffer. Adapters still validate the engine-specific
/// schema before producing bytes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileConfigurationRequest {
    pub identity: ProfileDocumentIdentity,
    pub update: ProfileConfigurationUpdate,
}

/// Typed sections accepted by the profile configuration codec seam.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileConfigurationUpdate {
    RuleProviders(BTreeMap<String, serde_json::Value>),
    ProxyProviders(BTreeMap<String, serde_json::Value>),
    Sniffer(SnifferConfiguration),
}

/// Contract DTO for Mihomo's `sniffer` configuration section.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct SnifferConfiguration {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniff: Option<BTreeMap<String, serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_domain: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_whitelist: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sniffing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_dns_mapping: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_pure_ip: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::CommandIntent;
    use lurker_core_contract::identity::EngineId;
    use lurker_core_contract::profile_document_identity::{
        ProfileDocumentRevision, ProfileDocumentSourceIdentity, ProfileDocumentSourceKind,
        ProfileId, ProfileSourceId,
    };
    use lurker_core_contract::profile_document_identity::{ProfileFormatId, ProfileSchemaVersion};

    #[test]
    fn typed_profile_configuration_wire_keeps_engine_format_schema_and_revision() {
        let identity = ProfileDocumentIdentity::new(
            ProfileId::new("main").unwrap(),
            EngineId::new("mihomo").unwrap(),
            ProfileDocumentSourceIdentity::new(
                ProfileDocumentSourceKind::Local,
                ProfileSourceId::new("local-main").unwrap(),
            ),
            ProfileDocumentRevision::new("document-r5").unwrap(),
            ProfileFormatId::new("mihomo-yaml").unwrap(),
            ProfileSchemaVersion::new(2).unwrap(),
        );
        let intent = CommandIntent::ApplyProfileConfiguration {
            request: Box::new(ProfileConfigurationRequest {
                identity: identity.clone(),
                update: ProfileConfigurationUpdate::Sniffer(SnifferConfiguration {
                    enable: Some(true),
                    ..Default::default()
                }),
            }),
        };

        let encoded = serde_json::to_value(&intent).unwrap();
        assert_eq!(
            encoded["ApplyProfileConfiguration"]["request"]["identity"]["engine_id"],
            "mihomo"
        );
        assert_eq!(
            encoded["ApplyProfileConfiguration"]["request"]["identity"]["format_id"],
            "mihomo-yaml"
        );
        assert_eq!(
            encoded["ApplyProfileConfiguration"]["request"]["identity"]["schema_version"],
            2
        );
        assert_eq!(
            encoded["ApplyProfileConfiguration"]["request"]["identity"]["revision"],
            "document-r5"
        );
        let decoded: CommandIntent = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded, intent);
    }
}

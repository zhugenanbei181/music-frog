//! Typed configuration use-cases over an engine-scoped profile codec.

use crate::profile_application::{ProfileApplication, ProfileRuntimeAccess};
use infiltrator_contract::dns_hosts::DnsHostEntry;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_configuration;
use infiltrator_contract::profile_configuration::ProfileConfigurationRequest;
use infiltrator_contract::provider_cache::KernelEtagSupportSnapshot;
use infiltrator_contract::rules_workspace::{RulesJsonDocumentSnapshot, RulesJsonSection};
use infiltrator_domain::{dns, fake_ip, proxy_providers, rules, sniffer, tun};
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationQuery, ProfileConfigurationReadout, ProfileConfigurationUpdate,
};
#[cfg(test)]
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::profile_workspace::{ProfileWorkspacePurpose, ProfileWorkspaceUpdate};
use infiltrator_ports::rule_tracer::RuleWorkspace;
use infiltrator_ports::runtime_gateway::ActiveProfileReferenceApplier;
#[cfg(test)]
use std::sync::Arc;

#[path = "configuration_dns.rs"]
mod dns_commands;

#[derive(Clone)]
pub struct ConfigurationApplication {
    profiles: ProfileApplication,
}

impl ConfigurationApplication {
    #[cfg(test)]
    pub(crate) fn test_configuration_application(store: Arc<dyn ProfileStore>) -> Self {
        let profiles = ProfileApplication::new(store).with_document_codec(Arc::new(
            crate::profile_document_codec_test_support::TestProfileDocumentCodec::mihomo(),
        ));
        Self::from_profile_application(profiles)
    }

    pub fn from_profile_application(profiles: ProfileApplication) -> Self {
        Self { profiles }
    }

    async fn read_configuration(
        &self,
        query: ProfileConfigurationQuery,
    ) -> Result<ProfileConfigurationReadout, Failure> {
        let workspace = self.profiles.load_active_workspace().await?;
        self.profiles.read_profile_configuration(
            &workspace.document.identity,
            &workspace.document.raw_bytes,
            query,
        )
    }

    async fn apply_configuration<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected_profile: Option<&str>,
        expected_hosts: Option<&[DnsHostEntry]>,
        update: ProfileConfigurationUpdate,
    ) -> Result<ProfileConfigurationReadout, Failure> {
        let workspace = self.profiles.load_active_workspace().await?;
        let profile = workspace.document.identity.profile_id.as_str();
        if expected_profile.is_some_and(|expected| expected != profile) {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "the active profile changed; review the latest mappings before applying",
                false,
            ));
        }
        if let Some(expected) = expected_hosts {
            let observed = self.profiles.read_profile_configuration(
                &workspace.document.identity,
                &workspace.document.raw_bytes,
                ProfileConfigurationQuery::Hosts,
            )?;
            let ProfileConfigurationReadout::Hosts { entries, .. } = observed else {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "the profile codec returned an unexpected Hosts readout",
                    false,
                ));
            };
            if entries != expected {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Hosts mappings changed; review the current rows before applying",
                    false,
                ));
            }
        }

        let expected_identity = workspace.identity();
        let mutation = self.profiles.apply_profile_configuration(
            &workspace.document.identity,
            &workspace.document.raw_bytes,
            update,
        )?;
        let workspace_update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: mutation.document_bytes,
            sidecar_bytes: workspace.sidecar_bytes,
        };
        self.profiles
            .commit_workspace(runtime, &expected_identity, &workspace_update)
            .await?;
        Ok(mutation.readback)
    }

    pub async fn load_rule_workspace(&self) -> Result<RuleWorkspace, Failure> {
        let workspace = self.profiles.load_active_workspace().await?;
        self.profiles.read_profile_rule_workspace(&workspace)
    }

    /// Read the editor projections and their exact source revision from one
    /// coherent active workspace observation.
    pub async fn load_rules_json_documents(
        &self,
    ) -> Result<Vec<RulesJsonDocumentSnapshot>, Failure> {
        let workspace = self.profiles.load_active_workspace().await?;
        let identity = workspace.document.identity.clone();
        let bytes = &workspace.document.raw_bytes;
        let mut documents = Vec::new();
        for (section, query) in [
            (
                RulesJsonSection::RuleProviders,
                ProfileConfigurationQuery::RuleProviders,
            ),
            (
                RulesJsonSection::ProxyProviders,
                ProfileConfigurationQuery::ProxyProviders,
            ),
            (
                RulesJsonSection::Sniffer,
                ProfileConfigurationQuery::Sniffer,
            ),
        ] {
            let readout = match self
                .profiles
                .read_profile_configuration(&identity, bytes, query)
            {
                Ok(readout) => readout,
                // Preserve the read model's honest omission behavior for a
                // section the active engine cannot decode.
                Err(_) => continue,
            };
            let json = match readout {
                ProfileConfigurationReadout::RuleProviders(value)
                    if section == RulesJsonSection::RuleProviders =>
                {
                    serde_json::to_string_pretty(&value)
                }
                ProfileConfigurationReadout::ProxyProviders(value)
                    if section == RulesJsonSection::ProxyProviders =>
                {
                    serde_json::to_string_pretty(&value)
                }
                ProfileConfigurationReadout::Sniffer(value)
                    if section == RulesJsonSection::Sniffer =>
                {
                    serde_json::to_string_pretty(&value)
                }
                _ => continue,
            };
            let Ok(json) = json else {
                continue;
            };
            documents.push(RulesJsonDocumentSnapshot {
                section,
                identity: identity.clone(),
                json,
            });
        }
        Ok(documents)
    }

    /// Apply a typed rules-workspace update only to the active document
    /// revision that supplied the editor buffer.
    pub async fn apply_profile_configuration_request(
        &self,
        request: ProfileConfigurationRequest,
    ) -> Result<ProfileConfigurationReadout, Failure> {
        let workspace = self.profiles.load_active_workspace().await?;
        if workspace.document.identity != request.identity {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "The profile document changed; reload the editor before applying this update",
                false,
            ));
        }
        let update = match request.update {
            profile_configuration::ProfileConfigurationUpdate::RuleProviders(value) => {
                ProfileConfigurationUpdate::RuleProviders(value)
            }
            profile_configuration::ProfileConfigurationUpdate::ProxyProviders(value) => {
                ProfileConfigurationUpdate::ProxyProviders(value)
            }
            profile_configuration::ProfileConfigurationUpdate::Sniffer(value) => {
                ProfileConfigurationUpdate::Sniffer(sniffer_config_from_contract(value))
            }
        };
        let expected_workspace = workspace.identity();
        let mutation = self.profiles.apply_profile_configuration(
            &request.identity,
            &workspace.document.raw_bytes,
            update,
        )?;
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: mutation.document_bytes,
            sidecar_bytes: workspace.sidecar_bytes,
        };
        self.profiles
            .commit_workspace::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                &expected_workspace,
                &update,
            )
            .await?;
        Ok(mutation.readback)
    }

    pub async fn load_dns_config(&self) -> Result<dns::DnsConfig, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::Dns)
            .await?
        {
            ProfileConfigurationReadout::Dns(config) => Ok(config),
            _ => Err(unexpected_readout("DNS")),
        }
    }

    pub async fn save_dns_config(
        &self,
        patch: dns::DnsConfigPatch,
    ) -> Result<dns::DnsConfig, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::Dns(patch),
            )
            .await?
        {
            ProfileConfigurationReadout::Dns(config) => Ok(config),
            _ => Err(unexpected_readout("DNS")),
        }
    }

    pub async fn load_fake_ip_config(&self) -> Result<fake_ip::FakeIpConfig, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::FakeIp)
            .await?
        {
            ProfileConfigurationReadout::FakeIp(config) => Ok(config),
            _ => Err(unexpected_readout("Fake-IP")),
        }
    }

    pub async fn save_fake_ip_config(
        &self,
        patch: fake_ip::FakeIpConfigPatch,
    ) -> Result<fake_ip::FakeIpConfig, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::FakeIp(patch),
            )
            .await?
        {
            ProfileConfigurationReadout::FakeIp(config) => Ok(config),
            _ => Err(unexpected_readout("Fake-IP")),
        }
    }

    pub async fn load_tun_config(&self) -> Result<tun::TunConfig, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::Tun)
            .await?
        {
            ProfileConfigurationReadout::Tun(config) => Ok(config),
            _ => Err(unexpected_readout("TUN")),
        }
    }

    pub async fn save_tun_config(
        &self,
        patch: tun::TunConfigPatch,
    ) -> Result<tun::TunConfig, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::Tun(patch),
            )
            .await?
        {
            ProfileConfigurationReadout::Tun(config) => Ok(config),
            _ => Err(unexpected_readout("TUN")),
        }
    }

    pub async fn load_rules(&self) -> Result<Vec<rules::RuleEntry>, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::Rules)
            .await?
        {
            ProfileConfigurationReadout::Rules(entries) => Ok(entries),
            _ => Err(unexpected_readout("rules")),
        }
    }

    pub async fn save_rules(
        &self,
        entries: Vec<rules::RuleEntry>,
    ) -> Result<Vec<rules::RuleEntry>, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::Rules(entries),
            )
            .await?
        {
            ProfileConfigurationReadout::Rules(entries) => Ok(entries),
            _ => Err(unexpected_readout("rules")),
        }
    }

    pub async fn load_rule_providers(&self) -> Result<rules::RuleProviders, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::RuleProviders)
            .await?
        {
            ProfileConfigurationReadout::RuleProviders(providers) => Ok(providers),
            _ => Err(unexpected_readout("rule providers")),
        }
    }

    /// Read the profile's declared `etag-support` setting through its format
    /// adapter. This is a declaration fact, not an HTTP cache outcome.
    pub async fn load_etag_support(&self) -> Result<KernelEtagSupportSnapshot, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::EtagSupport)
            .await?
        {
            ProfileConfigurationReadout::EtagSupport(snapshot) => Ok(snapshot),
            _ => Err(unexpected_readout("ETag support")),
        }
    }

    pub async fn save_rule_providers(
        &self,
        providers: rules::RuleProviders,
    ) -> Result<rules::RuleProviders, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::RuleProviders(providers),
            )
            .await?
        {
            ProfileConfigurationReadout::RuleProviders(providers) => Ok(providers),
            _ => Err(unexpected_readout("rule providers")),
        }
    }

    pub async fn load_proxy_providers(&self) -> Result<proxy_providers::ProxyProviders, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::ProxyProviders)
            .await?
        {
            ProfileConfigurationReadout::ProxyProviders(providers) => Ok(providers),
            _ => Err(unexpected_readout("proxy providers")),
        }
    }

    pub async fn save_proxy_providers(
        &self,
        providers: proxy_providers::ProxyProviders,
    ) -> Result<proxy_providers::ProxyProviders, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::ProxyProviders(providers),
            )
            .await?
        {
            ProfileConfigurationReadout::ProxyProviders(providers) => Ok(providers),
            _ => Err(unexpected_readout("proxy providers")),
        }
    }

    /// Read-only JSON editor projection; writes use the typed configuration
    /// operation below.
    pub async fn load_sniffer_config(&self) -> Result<serde_json::Value, Failure> {
        let config = self.load_typed_sniffer_config().await?;
        serde_json::to_value(config)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))
    }

    pub async fn load_typed_sniffer_config(&self) -> Result<sniffer::SnifferConfig, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::Sniffer)
            .await?
        {
            ProfileConfigurationReadout::Sniffer(config) => Ok(config),
            _ => Err(unexpected_readout("sniffer")),
        }
    }

    /// Read the endpoint used by Android's tun2proxy worker through the
    /// active engine codec instead of parsing the native profile format in
    /// the platform adapter.
    pub async fn load_proxy_endpoint(&self) -> Result<Option<String>, Failure> {
        match self
            .read_configuration(ProfileConfigurationQuery::ProxyEndpoint)
            .await?
        {
            ProfileConfigurationReadout::ProxyEndpoint(endpoint) => Ok(endpoint),
            _ => Err(unexpected_readout("proxy endpoint")),
        }
    }

    pub async fn save_typed_sniffer_config(
        &self,
        config: sniffer::SnifferConfig,
    ) -> Result<sniffer::SnifferConfig, Failure> {
        match self
            .apply_configuration::<dyn ActiveProfileReferenceApplier>(
                ProfileRuntimeAccess::Offline,
                None,
                None,
                ProfileConfigurationUpdate::Sniffer(config),
            )
            .await?
        {
            ProfileConfigurationReadout::Sniffer(config) => Ok(config),
            _ => Err(unexpected_readout("sniffer")),
        }
    }
}

fn sniffer_config_from_contract(
    value: infiltrator_contract::profile_configuration::SnifferConfiguration,
) -> sniffer::SnifferConfig {
    sniffer::SnifferConfig {
        enable: value.enable,
        sniff: value.sniff,
        skip_domain: value.skip_domain,
        force_domain: value.force_domain,
        port_whitelist: value.port_whitelist,
        sniffing: value.sniffing,
        force_dns_mapping: value.force_dns_mapping,
        parse_pure_ip: value.parse_pure_ip,
    }
}

fn unexpected_readout(section: &str) -> Failure {
    Failure::new(
        ErrorCode::InvalidState,
        format!("profile codec returned a readout other than {section}"),
        false,
    )
}

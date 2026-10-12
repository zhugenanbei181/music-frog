//! Profile-fenced DNS settings and the actual top-level kernel Hosts key.

use super::ConfigurationApplication;
use crate::profile_application::ProfileRuntimeAccess;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::{DnsHostsProfile, validate_hosts};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::dns::DnsConfig;
use infiltrator_ports::profile_document_codec::{
    ProfileConfigurationQuery, ProfileConfigurationReadout, ProfileConfigurationUpdate,
};
use infiltrator_ports::runtime_gateway::ManagedRuntime;

impl ConfigurationApplication {
    pub async fn load_hosts_profile(&self) -> Result<DnsHostsProfile, Failure> {
        let workspace = self.profiles.load_active_workspace().await?;
        let profile = workspace.document.identity.profile_id.as_str().to_owned();
        let readout = self.profiles.read_profile_configuration(
            &workspace.document.identity,
            &workspace.document.raw_bytes,
            ProfileConfigurationQuery::Hosts,
        )?;
        let ProfileConfigurationReadout::Hosts {
            entries,
            legacy_entries,
        } = readout
        else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "profile codec returned an unexpected Hosts readout",
                false,
            ));
        };
        Ok(DnsHostsProfile {
            profile,
            entries,
            legacy_entries,
        })
    }

    pub async fn apply_dns_settings(&self, patch: DnsSettingsPatch) -> Result<DnsConfig, Failure> {
        self.apply_dns_settings_with_runtime::<dyn ManagedRuntime>(
            ProfileRuntimeAccess::Offline,
            patch,
        )
        .await
    }

    pub async fn apply_dns_settings_with_runtime<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        patch: DnsSettingsPatch,
    ) -> Result<DnsConfig, Failure> {
        if patch.clear_hosts && patch.hosts.is_some()
            || patch.remove_legacy_hosts && patch.hosts.is_none() && !patch.clear_hosts
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                "ambiguous Hosts patch",
                false,
            ));
        }
        if let Some(rows) = &patch.hosts
            && let Some(issue) = validate_hosts(rows).first()
        {
            return Err(Failure::new(
                ErrorCode::InvalidInput,
                format!("invalid Hosts mapping: {issue:?}"),
                false,
            ));
        }

        let expected_profile = patch.expected_profile.clone();
        let expected_hosts = patch.expected_hosts.clone();
        let readout = self
            .apply_configuration(
                runtime,
                expected_profile.as_deref(),
                expected_hosts.as_deref(),
                ProfileConfigurationUpdate::DnsSettings(patch),
            )
            .await?;
        match readout {
            ProfileConfigurationReadout::Dns(config) => Ok(config),
            _ => Err(Failure::new(
                ErrorCode::InvalidState,
                "profile codec returned an unexpected DNS readout",
                false,
            )),
        }
    }
}

#[cfg(test)]
#[path = "configuration_dns_test.rs"]
mod tests;

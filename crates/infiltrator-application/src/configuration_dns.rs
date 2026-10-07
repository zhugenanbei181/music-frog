//! Profile-fenced DNS writes and the actual kernel Hosts key.
use super::{ConfigurationApplication, config_failure, dns_patch_from_settings, parse_yaml};
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::dns_hosts::{DnsHostsProfile, validate_hosts};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::dns::{
    DnsConfig, DnsConfigPatch, apply_dns_patch_to_yaml, extract_dns_config_from_doc,
};
use infiltrator_domain::dns_hosts::{apply_hosts_patch_to_yaml, entries_from_document};
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use std::sync::Arc;

impl ConfigurationApplication {
    pub async fn load_hosts_profile(&self) -> Result<DnsHostsProfile, Failure> {
        let (profile, content) = self.current().await?;
        let document = parse_yaml(&content)?;
        Ok(DnsHostsProfile {
            profile,
            entries: entries_from_document(&document, false).map_err(config_failure)?,
            legacy_entries: entries_from_document(&document, true).map_err(config_failure)?,
        })
    }
    pub async fn apply_dns_settings(&self, patch: DnsSettingsPatch) -> Result<DnsConfig, Failure> {
        self.apply_dns_settings_with_runtime::<dyn ManagedRuntime>(None, patch)
            .await
    }
    pub async fn apply_dns_settings_with_runtime<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
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
        let (profile, content) = self.current().await?;
        if patch
            .expected_profile
            .as_ref()
            .is_some_and(|expected| expected != &profile)
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "the active profile changed; review the latest mappings before applying",
                false,
            ));
        }
        if let Some(expected) = &patch.expected_hosts {
            let observed =
                entries_from_document(&parse_yaml(&content)?, false).map_err(config_failure)?;
            if &observed != expected {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    "Hosts mappings changed; review the current rows before applying",
                    false,
                ));
            }
        }
        let hosts = if patch.clear_hosts {
            Some(Vec::new())
        } else {
            patch.hosts.clone()
        };
        let remove_legacy = patch.remove_legacy_hosts;
        let domain_patch = dns_patch_from_settings(patch);
        let mut updated = if domain_patch == DnsConfigPatch::default() {
            content
        } else {
            apply_dns_patch_to_yaml(&content, domain_patch).map_err(config_failure)?
        };
        if let Some(rows) = hosts {
            updated = apply_hosts_patch_to_yaml(&updated, &rows, remove_legacy)
                .map_err(config_failure)?;
        }
        // Decode before commit. A later read failure must not misreport a successful write as rejected.
        let config = extract_dns_config_from_doc(&parse_yaml(&updated)?).map_err(config_failure)?;
        self.profiles
            .save_profile_content(runtime, profile, updated, ApplyStrategy::PreferReload)
            .await?;
        Ok(config)
    }
}

#[cfg(test)]
#[path = "configuration_dns_test.rs"]
mod tests;

//! Configuration-file use-cases over the profile store port.

use crate::profile_application::ProfileApplication;
use infiltrator_contract::dns::DnsSettingsPatch;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_domain::{dns, fake_ip, proxy_providers, rules, sniffer, tun};
use infiltrator_ports::profile_store::ProfileStore;
use infiltrator_ports::runtime_gateway::ManagedRuntime;
use serde_yaml_ng::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct ConfigurationApplication {
    profiles: ProfileApplication,
}

impl ConfigurationApplication {
    pub fn new(store: Arc<dyn ProfileStore>) -> Self {
        Self {
            profiles: ProfileApplication::new(store),
        }
    }

    async fn current(&self) -> Result<(String, String), Failure> {
        self.profiles.current_content().await
    }

    pub async fn load_dns_config(&self) -> Result<dns::DnsConfig, Failure> {
        let (_, content) = self.current().await?;
        let doc = parse_yaml(&content)?;
        dns::extract_dns_config_from_doc(&doc).map_err(config_failure)
    }

    pub async fn save_dns_config(
        &self,
        patch: dns::DnsConfigPatch,
    ) -> Result<dns::DnsConfig, Failure> {
        let (profile, content) = self.current().await?;
        let updated = dns::apply_dns_patch_to_yaml(&content, patch).map_err(config_failure)?;
        let config =
            dns::extract_dns_config_from_doc(&parse_yaml(&updated)?).map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(config)
    }

    /// Apply the shared DNS workbench patch (six switches, mapping mode and
    /// Fake-IP filter mode) through the validated profile write path.
    pub async fn apply_dns_settings(
        &self,
        patch: DnsSettingsPatch,
    ) -> Result<dns::DnsConfig, Failure> {
        self.apply_dns_settings_with_runtime::<dyn ManagedRuntime>(None, patch)
            .await
    }

    /// Apply the shared workbench patch and, when a managed runtime is
    /// supplied, commit it through the running core's apply transaction.
    pub async fn apply_dns_settings_with_runtime<R: ManagedRuntime + ?Sized>(
        &self,
        runtime: Option<Arc<R>>,
        patch: DnsSettingsPatch,
    ) -> Result<dns::DnsConfig, Failure> {
        let domain_patch = dns_patch_from_settings(patch);
        self.profiles
            .save_current_profile_content(runtime, ApplyStrategy::PreferReload, move |content| {
                dns::apply_dns_patch_to_yaml(content, domain_patch)
            })
            .await?;
        self.load_dns_config().await
    }

    pub async fn load_fake_ip_config(&self) -> Result<fake_ip::FakeIpConfig, Failure> {
        let (_, content) = self.current().await?;
        fake_ip::extract_fake_ip_config_from_doc(&parse_yaml(&content)?).map_err(config_failure)
    }

    pub async fn save_fake_ip_config(
        &self,
        patch: fake_ip::FakeIpConfigPatch,
    ) -> Result<fake_ip::FakeIpConfig, Failure> {
        let (profile, content) = self.current().await?;
        let updated =
            fake_ip::apply_fake_ip_patch_to_yaml(&content, patch).map_err(config_failure)?;
        let config = fake_ip::extract_fake_ip_config_from_doc(&parse_yaml(&updated)?)
            .map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(config)
    }

    pub async fn load_tun_config(&self) -> Result<tun::TunConfig, Failure> {
        let (_, content) = self.current().await?;
        tun::extract_tun_config_from_doc(&parse_yaml(&content)?).map_err(config_failure)
    }

    pub async fn save_tun_config(
        &self,
        patch: tun::TunConfigPatch,
    ) -> Result<tun::TunConfig, Failure> {
        let (profile, content) = self.current().await?;
        let updated = tun::apply_tun_patch_to_yaml(&content, patch).map_err(config_failure)?;
        let config =
            tun::extract_tun_config_from_doc(&parse_yaml(&updated)?).map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(config)
    }

    pub async fn load_rules(&self) -> Result<Vec<rules::RuleEntry>, Failure> {
        let (_, content) = self.current().await?;
        rules::load_rules_from_yaml(&content).map_err(config_failure)
    }

    pub async fn save_rules(
        &self,
        entries: Vec<rules::RuleEntry>,
    ) -> Result<Vec<rules::RuleEntry>, Failure> {
        rules::validate_rules(&entries).map_err(config_failure)?;
        let (profile, content) = self.current().await?;
        let updated = rules::apply_rules_to_yaml(&content, &entries).map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(entries)
    }

    pub async fn load_rule_providers(&self) -> Result<rules::RuleProviders, Failure> {
        let (_, content) = self.current().await?;
        rules::extract_rule_providers_from_doc(&parse_yaml(&content)?).map_err(config_failure)
    }

    /// DUAL-11-05: the active profile's top-level `etag-support` declaration.
    ///
    /// mihomo reads `etag-support` at the config root (its `General.ETagSupport`,
    /// default `true`) to gate its `ETag`/`If-None-Match` conditional cache for
    /// downloaded resources. The client publishes that declaration; the
    /// per-request `304` outcome is not exposed by the controller, so it is
    /// never inferred here.
    pub async fn load_etag_support(
        &self,
    ) -> Result<infiltrator_contract::provider_cache::KernelEtagSupportSnapshot, Failure> {
        let (_, content) = self.current().await?;
        let doc = parse_yaml(&content)?;
        Ok(
            infiltrator_contract::provider_cache::KernelEtagSupportSnapshot::from_declared(
                doc.get("etag-support").and_then(Value::as_bool),
            ),
        )
    }

    pub async fn save_rule_providers(
        &self,
        providers: rules::RuleProviders,
    ) -> Result<rules::RuleProviders, Failure> {
        let (profile, content) = self.current().await?;
        let updated =
            rules::apply_rule_providers_to_yaml(&content, &providers).map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(providers)
    }

    pub async fn load_proxy_providers(&self) -> Result<proxy_providers::ProxyProviders, Failure> {
        let (_, content) = self.current().await?;
        proxy_providers::extract_proxy_providers_from_doc(&parse_yaml(&content)?)
            .map_err(config_failure)
    }

    pub async fn save_proxy_providers(
        &self,
        providers: proxy_providers::ProxyProviders,
    ) -> Result<proxy_providers::ProxyProviders, Failure> {
        let (profile, content) = self.current().await?;
        let updated = proxy_providers::apply_proxy_providers_to_yaml(&content, &providers)
            .map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(providers)
    }

    pub async fn load_sniffer_config(&self) -> Result<serde_json::Value, Failure> {
        let (_, content) = self.current().await?;
        sniffer::extract_sniffer_config_from_doc(&parse_yaml(&content)?).map_err(config_failure)
    }

    pub async fn save_sniffer_config(
        &self,
        config: serde_json::Value,
    ) -> Result<serde_json::Value, Failure> {
        sniffer::validate_sniffer_config(&config).map_err(config_failure)?;
        let (profile, content) = self.current().await?;
        let updated = sniffer::apply_sniffer_to_yaml(&content, &config).map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(config)
    }

    pub async fn load_typed_sniffer_config(&self) -> Result<sniffer::SnifferConfig, Failure> {
        let (_, content) = self.current().await?;
        sniffer::extract_sniffer_config(&parse_yaml(&content)?).map_err(config_failure)
    }

    pub async fn save_typed_sniffer_config(
        &self,
        config: sniffer::SnifferConfig,
    ) -> Result<sniffer::SnifferConfig, Failure> {
        sniffer::validate_typed_sniffer_config(&config).map_err(config_failure)?;
        let (profile, content) = self.current().await?;
        let updated =
            sniffer::apply_typed_sniffer_to_yaml(&content, &config).map_err(config_failure)?;
        self.profiles.save_profile(&profile, &updated).await?;
        Ok(config)
    }
}

/// Map the shared workbench patch onto the domain merge patch.
///
/// This is the single mapping used by both surfaces' form submission, so the
/// Iced form draft and a Bevy `ApplyDnsSettings` command write identical
/// profile keys.
pub fn dns_patch_from_settings(patch: DnsSettingsPatch) -> dns::DnsConfigPatch {
    let mut domain_patch = dns::DnsConfigPatch::default();
    if let Some(switches) = patch.switches {
        domain_patch.enable = Some(switches.enable);
        domain_patch.ipv6 = Some(switches.ipv6);
        domain_patch.cache = Some(switches.cache);
        domain_patch.use_hosts = Some(switches.use_hosts);
        domain_patch.use_system_hosts = Some(switches.use_system_hosts);
        domain_patch.respect_rules = Some(switches.respect_rules);
    }
    if let Some(mode) = patch.enhanced_mode {
        match mode.config_value() {
            Some(value) => domain_patch.enhanced_mode = Some(value.to_owned()),
            None => domain_patch.clear_enhanced_mode = true,
        }
    }
    if let Some(mode) = patch.filter_mode {
        domain_patch.fake_ip_filter_mode = Some(mode.config_value().to_owned());
    }
    if let Some(nameserver) = patch.nameserver {
        domain_patch.nameserver = Some(nameserver);
    }
    if let Some(fallback) = patch.fallback {
        domain_patch.fallback = Some(fallback);
    }
    if let Some(policy) = patch.fallback_policy {
        domain_patch.fallback_filter_partial = Some(dns::FallbackFilterPatch {
            geoip: Some(policy.geoip),
            // An emptied geoip-code keeps the configured value (the host
            // rejects an empty code); an emptied trigger list clears it.
            geoip_code: (!policy.geoip_code.is_empty()).then_some(policy.geoip_code),
            ipcidr: Some(policy.trigger_ipcidr),
        });
    }
    if let Some(range) = patch.fake_ip_range {
        domain_patch.fake_ip_range = Some(range);
    }
    if patch.clear_fake_ip_range {
        domain_patch.clear_fake_ip_range = true;
    }
    if let Some(filter) = patch.fake_ip_filter {
        domain_patch.fake_ip_filter = Some(filter);
    }
    if let Some(servers) = patch.proxy_server_nameserver {
        domain_patch.proxy_server_nameserver = Some(servers);
    }
    if let Some(servers) = patch.direct_nameserver {
        domain_patch.direct_nameserver = Some(servers);
    }
    if let Some(hosts) = patch.hosts {
        domain_patch.hosts = Some(infiltrator_domain::dns_hosts::hosts_map_from_entries(
            &hosts,
        ));
    }
    if patch.clear_hosts {
        domain_patch.clear_hosts = true;
    }
    domain_patch
}

fn parse_yaml(content: &str) -> Result<Value, Failure> {
    serde_yaml_ng::from_str(content).map_err(config_failure)
}

fn config_failure(error: impl std::fmt::Display) -> Failure {
    Failure::new(ErrorCode::Configuration, error.to_string(), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::dns::{
        DnsCoreSwitches, DnsEnhancedMode, DnsFakeIpFilterMode, DnsSwitchField,
    };

    #[test]
    fn dns_settings_patch_maps_onto_domain_patch() {
        let switches = DnsCoreSwitches {
            enable: true,
            respect_rules: true,
            ..DnsCoreSwitches::default()
        };
        let patch = dns_patch_from_settings(DnsSettingsPatch {
            switches: Some(switches),
            enhanced_mode: Some(DnsEnhancedMode::RedirHost),
            filter_mode: Some(DnsFakeIpFilterMode::Rules),
            ..DnsSettingsPatch::default()
        });
        assert_eq!(patch.enable, Some(true));
        assert_eq!(patch.respect_rules, Some(true));
        assert_eq!(patch.enhanced_mode.as_deref(), Some("redir-host"));
        assert!(!patch.clear_enhanced_mode);
        assert_eq!(patch.fake_ip_filter_mode.as_deref(), Some("rule"));
    }

    #[test]
    fn unmapped_mapping_mode_clears_the_key() {
        let patch = dns_patch_from_settings(DnsSettingsPatch {
            enhanced_mode: Some(DnsEnhancedMode::Unmapped),
            ..DnsSettingsPatch::default()
        });
        assert!(patch.clear_enhanced_mode);
        assert!(patch.enhanced_mode.is_none());
    }

    #[test]
    fn switch_toggle_patch_derives_from_current_value() {
        let current = DnsCoreSwitches::default();
        let patch = DnsSettingsPatch::toggle(DnsSwitchField::Cache, current);
        let mapped = dns_patch_from_settings(patch);
        assert_eq!(mapped.cache, Some(true));
        assert_eq!(mapped.enable, Some(false));
    }

    #[test]
    fn workbench_lists_and_fallback_policy_map_onto_the_domain_patch() {
        let patch = DnsSettingsPatch {
            nameserver: Some(vec!["https://doh.pub/dns-query".to_owned()]),
            fallback: Some(vec!["tls://1.0.0.1:853".to_owned()]),
            fallback_policy: Some(infiltrator_contract::dns::DnsFallbackPolicy {
                geoip: true,
                geoip_code: "CN".to_owned(),
                trigger_ipcidr: vec!["192.168.0.0/16".to_owned()],
            }),
            fake_ip_filter: Some(vec!["*.lan".to_owned()]),
            proxy_server_nameserver: Some(vec!["tls://223.5.5.5:853".to_owned()]),
            direct_nameserver: Some(vec!["system".to_owned()]),
            ..DnsSettingsPatch::default()
        };
        let mapped = dns_patch_from_settings(patch);
        assert_eq!(
            mapped.nameserver.as_deref(),
            Some(&["https://doh.pub/dns-query".to_owned()][..])
        );
        assert_eq!(
            mapped.fallback.as_deref(),
            Some(&["tls://1.0.0.1:853".to_owned()][..])
        );
        let filter = mapped
            .fallback_filter_partial
            .expect("fallback filter partial merge");
        assert_eq!(filter.geoip, Some(true));
        assert_eq!(filter.geoip_code.as_deref(), Some("CN"));
        assert_eq!(
            filter.ipcidr.as_deref(),
            Some(&["192.168.0.0/16".to_owned()][..])
        );
        assert!(mapped.fallback_filter.is_none());
        assert_eq!(
            mapped.fake_ip_filter.as_deref(),
            Some(&["*.lan".to_owned()][..])
        );
        assert_eq!(
            mapped.proxy_server_nameserver.as_deref(),
            Some(&["tls://223.5.5.5:853".to_owned()][..])
        );
        assert_eq!(
            mapped.direct_nameserver.as_deref(),
            Some(&["system".to_owned()][..])
        );
    }

    #[test]
    fn an_emptied_range_maps_to_the_explicit_clear_flag() {
        let patch = dns_patch_from_settings(DnsSettingsPatch {
            clear_fake_ip_range: true,
            ..DnsSettingsPatch::default()
        });
        assert!(patch.clear_fake_ip_range);
        assert!(patch.fake_ip_range.is_none());
    }
}

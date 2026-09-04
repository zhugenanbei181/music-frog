//! Configuration-file use-cases over the profile store port.

use crate::profile_application::ProfileApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_domain::{dns, fake_ip, proxy_providers, rules, sniffer, tun};
use infiltrator_ports::profile_store::ProfileStore;
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

fn parse_yaml(content: &str) -> Result<Value, Failure> {
    serde_yaml_ng::from_str(content).map_err(config_failure)
}

fn config_failure(error: impl std::fmt::Display) -> Failure {
    Failure::new(ErrorCode::Configuration, error.to_string(), false)
}

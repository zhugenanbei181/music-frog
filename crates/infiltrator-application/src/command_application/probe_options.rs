//! Persist latency parameters before publishing an applied fact; all probes resolve the same values.
use super::CommandApplication;
use crate::proxy_probe_options_projection::{stored_options, validate_options};
use infiltrator_contract::error::Failure;
use infiltrator_contract::proxy_probe_options::ProxyProbeOptions;

impl CommandApplication {
    pub(super) async fn save_probe_options(
        &self,
        options: ProxyProbeOptions,
    ) -> Result<(), Failure> {
        let options = validate_options(options)?;
        self.settings()?
            .update(|settings| {
                settings.runtime_panel.delay_test_url = options.test_url;
                settings.runtime_panel.delay_timeout_ms = options.timeout_ms;
            })
            .await
    }

    pub(super) async fn resolve_probe_options(
        &self,
        url: Option<String>,
        timeout_ms: Option<u32>,
    ) -> Result<ProxyProbeOptions, Failure> {
        let mut options = if url.is_none() || timeout_ms.is_none() {
            match &self.settings {
                Some(settings) => stored_options(&settings.load().await?),
                None => ProxyProbeOptions::default(),
            }
        } else {
            ProxyProbeOptions::default()
        };
        if let Some(url) = url {
            options.test_url = url;
        }
        if let Some(timeout) = timeout_ms {
            options.timeout_ms = timeout;
        }
        validate_options(options)
    }
}

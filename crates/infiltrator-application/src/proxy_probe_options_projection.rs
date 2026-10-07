//! One validation and settings fold shared by all latency-probe entry points.
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_probe_options::{
    DEFAULT_PROBE_TIMEOUT_MS, DEFAULT_PROBE_URL, MAX_PROBE_TIMEOUT_MS, ProxyProbeDraft,
    ProxyProbeOptions, ProxyProbeSettingsSnapshot,
};
use infiltrator_domain::settings::AppSettings;
use url::Url;

fn invalid(message: &str) -> Failure {
    Failure::new(ErrorCode::InvalidInput, message, false)
}

pub fn validate_options(mut options: ProxyProbeOptions) -> Result<ProxyProbeOptions, Failure> {
    options.test_url = options.test_url.trim().into();
    let parsed = Url::parse(&options.test_url)
        .map_err(|_| invalid("probe URL must be an absolute HTTP or HTTPS URL"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(invalid(
            "probe URL must use HTTP or HTTPS and include a host",
        ));
    }
    if !(1..=MAX_PROBE_TIMEOUT_MS).contains(&options.timeout_ms) {
        return Err(invalid(
            "probe timeout must be between 1 and 32767 milliseconds",
        ));
    }
    Ok(options)
}

pub fn parse_draft(draft: &ProxyProbeDraft) -> Result<ProxyProbeOptions, Failure> {
    let url = draft.test_url.trim();
    let timeout = draft.timeout_ms.trim();
    validate_options(ProxyProbeOptions {
        test_url: if url.is_empty() {
            DEFAULT_PROBE_URL.into()
        } else {
            url.into()
        },
        timeout_ms: if timeout.is_empty() {
            DEFAULT_PROBE_TIMEOUT_MS
        } else {
            timeout
                .parse()
                .map_err(|_| invalid("probe timeout must be a whole number of milliseconds"))?
        },
    })
}

pub fn stored_options(settings: &AppSettings) -> ProxyProbeOptions {
    ProxyProbeOptions {
        test_url: settings.runtime_panel.delay_test_url.clone(),
        timeout_ms: settings.runtime_panel.delay_timeout_ms,
    }
}

pub fn project_settings(
    settings: Option<&Result<AppSettings, Failure>>,
) -> ProxyProbeSettingsSnapshot {
    match settings {
        None => ProxyProbeSettingsSnapshot::default(),
        Some(Err(failure)) => ProxyProbeSettingsSnapshot {
            options: None,
            can_persist: false,
            failure: Some(failure.clone()),
        },
        Some(Ok(settings)) => {
            let options = stored_options(settings);
            let failure = validate_options(options.clone()).err();
            ProxyProbeSettingsSnapshot {
                options: Some(options),
                can_persist: true,
                failure,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_preserves_drafts_and_rejects_unsupported_timeout_before_execution() {
        let mut draft = ProxyProbeDraft {
            test_url: " https://probe.example.test/path ".into(),
            timeout_ms: "32767".into(),
        };
        let original = draft.clone();
        assert_eq!(
            parse_draft(&draft).unwrap(),
            ProxyProbeOptions {
                test_url: "https://probe.example.test/path".into(),
                timeout_ms: 32767
            }
        );
        assert_eq!(draft, original);
        for timeout in ["0", "32768", "60000", "-1", "1.5", "later"] {
            draft.timeout_ms = timeout.into();
            assert_eq!(
                parse_draft(&draft).unwrap_err().code,
                ErrorCode::InvalidInput
            );
        }
        draft.timeout_ms = "5000".into();
        for url in [
            "relative/path",
            "file:///tmp/probe",
            "ftp://probe.example.test",
        ] {
            draft.test_url = url.into();
            assert!(parse_draft(&draft).is_err());
        }
        assert_eq!(
            parse_draft(&ProxyProbeDraft::default()).unwrap(),
            ProxyProbeOptions::default()
        );
    }
    #[test]
    fn stored_invalid_values_and_read_failure_never_become_synthetic_applied_defaults() {
        let mut settings = AppSettings::default();
        settings.runtime_panel.delay_timeout_ms = 60000;
        let projected = project_settings(Some(&Ok(settings)));
        assert_eq!(projected.options.unwrap().timeout_ms, 60000);
        assert!(projected.failure.is_some());
        let failure = Failure::new(ErrorCode::Storage, "settings unavailable", true);
        let projected = project_settings(Some(&Err(failure.clone())));
        assert_eq!(projected.options, None);
        assert_eq!(projected.failure, Some(failure));
        assert!(!projected.can_persist);
    }
}

//! Application seam for public IP and GeoIP probe read model.

pub type PublicIpFactTuple = (
    String,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<u32>,
    Option<String>,
);

use infiltrator_contract::public_ip::{PublicIpProbeSnapshot, PublicIpProbeStatus};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};

#[derive(Clone, Copy, Debug, Default)]
pub struct PublicIpApplication;

impl PublicIpApplication {
    /// Projects public IP probe facts into the canonical shared snapshot.
    pub fn project(
        &self,
        core: &CoreSnapshot,
        ip_fact: Option<&Result<PublicIpFactTuple, String>>,
    ) -> PublicIpProbeSnapshot {
        let revision = core.revision.max(1);
        if !matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        ) {
            return PublicIpProbeSnapshot::unavailable(
                core.generation,
                revision,
                "core is not running; public IP is not probed",
            );
        }
        let Some(fact) = ip_fact else {
            return PublicIpProbeSnapshot::unsupported(
                core.generation,
                revision,
                "public IP probe port is not composed",
            );
        };
        match fact {
            Ok((ip, country, city, isp, asn, provider)) => PublicIpProbeSnapshot {
                generation: core.generation,
                revision,
                status: PublicIpProbeStatus::Ready,
                failure: None,
                ip: Some(ip.clone()),
                country_code: country.clone(),
                city: city.clone(),
                isp: isp.clone(),
                asn: *asn,
                provider: provider.clone(),
                checked_at_epoch_ms: Some(1741300000000),
            },
            Err(err) => PublicIpProbeSnapshot::failed(
                core.generation,
                revision,
                format!("public IP probe failed: {err}"),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreLifecycle;

    fn core(lifecycle: CoreLifecycle) -> CoreSnapshot {
        CoreSnapshot {
            lifecycle,
            generation: 2,
            revision: 3,
            session_token: None,
            proxy_mode: None,
            core_version: None,
            sampled_at_epoch_ms: None,
            failure: None,
            upload_bps: 0.0,
            download_bps: 0.0,
            active_connections: 0,
            memory_bytes: None,
            watchdog: Default::default(),
        }
    }

    #[test]
    fn stopped_core_reports_unavailable() {
        let snapshot = PublicIpApplication.project(&core(CoreLifecycle::Stopped), None);
        assert_eq!(snapshot.status, PublicIpProbeStatus::Unknown);
    }

    #[test]
    fn missing_probe_reports_unsupported() {
        let snapshot = PublicIpApplication.project(&core(CoreLifecycle::Running), None);
        assert_eq!(snapshot.status, PublicIpProbeStatus::Unsupported);
    }

    #[test]
    fn successful_fact_projects_ready() {
        let fact = Ok((
            "1.1.1.1".to_string(),
            Some("US".to_string()),
            Some("Los Angeles".to_string()),
            Some("Cloudflare".to_string()),
            Some(13335),
            Some("ipapi.is".to_string()),
        ));
        let snapshot = PublicIpApplication.project(&core(CoreLifecycle::Running), Some(&fact));
        assert!(snapshot.is_drawable());
        assert_eq!(snapshot.ip.as_deref(), Some("1.1.1.1"));
        assert_eq!(snapshot.country_code.as_deref(), Some("US"));
    }
}

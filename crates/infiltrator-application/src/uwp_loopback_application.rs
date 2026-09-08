//! UWP loopback use-cases over the runtime-neutral host port.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::uwp::{UwpLoopbackAvailability, UwpLoopbackSnapshot};
use infiltrator_ports::error::PortError;
use infiltrator_ports::uwp_loopback::UwpLoopbackPort;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const UWP_SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct UwpLoopbackApplication {
    port: Arc<dyn UwpLoopbackPort>,
    next_revision: Arc<AtomicU64>,
    last_snapshot: Arc<Mutex<Option<(Instant, UwpLoopbackSnapshot)>>>,
}

impl UwpLoopbackApplication {
    pub fn new(port: Arc<dyn UwpLoopbackPort>) -> Self {
        Self {
            port,
            next_revision: Arc::new(AtomicU64::new(1)),
            last_snapshot: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn snapshot(&self) -> UwpLoopbackSnapshot {
        let revision = self.next_revision.fetch_add(1, Ordering::Relaxed);
        let snapshot = match self.port.scan().await {
            Ok(packages) => UwpLoopbackSnapshot::supported(revision, packages),
            Err(PortError::Unsupported { reason, .. }) => {
                UwpLoopbackSnapshot::unsupported(revision, reason)
            }
            Err(error) => UwpLoopbackSnapshot::unavailable(revision, error.to_string()),
        };
        *self.last_snapshot.lock().expect("UWP snapshot cache lock") =
            Some((Instant::now(), snapshot.clone()));
        snapshot
    }

    pub async fn snapshot_cached(&self) -> UwpLoopbackSnapshot {
        if let Some((observed_at, snapshot)) = self
            .last_snapshot
            .lock()
            .expect("UWP snapshot cache lock")
            .as_ref()
            .cloned()
            && observed_at.elapsed() < UWP_SNAPSHOT_CACHE_TTL
        {
            return snapshot;
        }
        self.snapshot().await
    }

    pub async fn set_exempt(
        &self,
        sid: &str,
        exempt: bool,
    ) -> Result<UwpLoopbackSnapshot, Failure> {
        let sid = infiltrator_domain::uwp::validate_app_container_sid(sid)
            .map_err(|message| Failure::new(ErrorCode::InvalidInput, message, false))?;
        self.port
            .set_exempt(sid, exempt)
            .await
            .map_err(Failure::from)?;
        let snapshot = self.snapshot().await;
        verify_package_state(&snapshot, sid, exempt)?;
        Ok(snapshot)
    }

    pub async fn set_all(&self, exempt: bool) -> Result<UwpLoopbackSnapshot, Failure> {
        self.port.set_all(exempt).await.map_err(Failure::from)?;
        let snapshot = self.snapshot().await;
        match &snapshot.availability {
            UwpLoopbackAvailability::Supported => {
                if snapshot
                    .packages
                    .iter()
                    .any(|package| package.loopback_exempt != exempt)
                {
                    return Err(Failure::new(
                        ErrorCode::InvalidState,
                        format!("UWP loopback bulk readback mismatch: requested {exempt}"),
                        true,
                    ));
                }
                Ok(snapshot)
            }
            UwpLoopbackAvailability::Unsupported { reason }
            | UwpLoopbackAvailability::Unavailable { reason } => {
                Err(Failure::new(ErrorCode::Unsupported, reason.clone(), false))
            }
        }
    }
}

fn verify_package_state(
    snapshot: &UwpLoopbackSnapshot,
    sid: &str,
    exempt: bool,
) -> Result<(), Failure> {
    match &snapshot.availability {
        UwpLoopbackAvailability::Supported => {
            let Some(package) = snapshot.packages.iter().find(|package| package.sid == sid) else {
                return Err(Failure::new(
                    ErrorCode::Storage,
                    format!("AppContainer SID {sid} disappeared during readback"),
                    true,
                ));
            };
            if package.loopback_exempt != exempt {
                return Err(Failure::new(
                    ErrorCode::InvalidState,
                    format!(
                        "UWP loopback readback mismatch for {sid}: requested {exempt}, observed {}",
                        package.loopback_exempt
                    ),
                    true,
                ));
            }
            Ok(())
        }
        UwpLoopbackAvailability::Unsupported { reason }
        | UwpLoopbackAvailability::Unavailable { reason } => {
            Err(Failure::new(ErrorCode::Unsupported, reason.clone(), false))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::uwp::UwpPackageSnapshot;
    use infiltrator_ports::error::PortError;
    use std::sync::Mutex;

    #[derive(Default)]
    struct FakePort {
        packages: Mutex<Vec<UwpPackageSnapshot>>,
        ignore_write: bool,
    }

    #[async_trait]
    impl UwpLoopbackPort for FakePort {
        async fn scan(&self) -> Result<Vec<UwpPackageSnapshot>, PortError> {
            Ok(self.packages.lock().expect("UWP packages lock").clone())
        }

        async fn set_exempt(&self, sid: &str, exempt: bool) -> Result<(), PortError> {
            if !self.ignore_write
                && let Some(package) = self
                    .packages
                    .lock()
                    .expect("UWP packages lock")
                    .iter_mut()
                    .find(|package| package.sid == sid)
            {
                package.loopback_exempt = exempt;
            }
            Ok(())
        }

        async fn set_all(&self, exempt: bool) -> Result<(), PortError> {
            if !self.ignore_write {
                for package in self.packages.lock().expect("UWP packages lock").iter_mut() {
                    package.loopback_exempt = exempt;
                }
            }
            Ok(())
        }
    }

    fn fake_port(ignore_write: bool) -> Arc<FakePort> {
        Arc::new(FakePort {
            packages: Mutex::new(vec![UwpPackageSnapshot {
                sid: "S-1-15-2-1001".to_owned(),
                display_name: "Windows Terminal".to_owned(),
                package_family_name: "Microsoft.WindowsTerminal".to_owned(),
                loopback_exempt: false,
            }]),
            ignore_write,
        })
    }

    #[tokio::test]
    async fn application_scans_and_reads_back_single_exemption() {
        let application = UwpLoopbackApplication::new(fake_port(false));
        let snapshot = application
            .set_exempt(" S-1-15-2-1001 ", true)
            .await
            .expect("UWP exemption should read back");
        assert!(snapshot.is_supported());
        assert!(snapshot.packages[0].loopback_exempt);
    }

    #[tokio::test]
    async fn application_rejects_ignored_exemption_write() {
        let application = UwpLoopbackApplication::new(fake_port(true));
        let failure = application
            .set_exempt("S-1-15-2-1001", true)
            .await
            .expect_err("ignored host write must fail closed");
        assert_eq!(failure.code, ErrorCode::InvalidState);
    }
}

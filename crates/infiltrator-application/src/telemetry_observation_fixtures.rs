//! Isolated real telemetry reads: first observation, measured zero and a denied subsequent read.
use crate::core_application::CoreApplication;
use crate::surface_reader::ApplicationSurfaceReader;
use async_trait::async_trait;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_ports::error::PortError;
use infiltrator_ports::overview::{OverviewReader, OverviewSample};
use infiltrator_ports::surface::SurfaceReader;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
#[derive(Default)]
pub struct TelemetryObservationReader {
    pub denied: AtomicBool,
    epoch: AtomicI64,
}

/// Exercise real start, warmup, measured zero and denied read through the same application reader.
pub async fn stale_zero_snapshot(
    core: &CoreApplication,
    reader: &ApplicationSurfaceReader,
    telemetry: &TelemetryObservationReader,
) -> Result<SurfaceSnapshot, Failure> {
    core.execute(CommandIntent::StartCore).await.into_unit()?;
    let unknown = reader.read().await?;
    assert!(unknown.shell_readout.upload_bps.value.is_none());
    let zero = reader.read().await?;
    assert_eq!(zero.shell_readout.upload_bps.value, Some(0.0));
    assert!(zero.shell_readout.upload_bps.current);
    telemetry.denied.store(true, Ordering::Release);
    let failed = reader.read().await?;
    assert_eq!(failed.shell_readout.upload_bps.value, Some(0.0));
    assert!(!failed.shell_readout.upload_bps.current);
    assert!(failed.shell_readout.rate_failure.is_some());
    Ok(failed)
}
#[async_trait]
impl OverviewReader for TelemetryObservationReader {
    async fn sample(&self) -> Result<OverviewSample, PortError> {
        if self.denied.load(Ordering::Acquire) {
            return Err(PortError::PermissionDenied(
                "isolated telemetry read denied".into(),
            ));
        }
        Ok(OverviewSample {
            lifecycle: CoreLifecycle::Running,
            mode: None,
            upload_total: 512,
            download_total: 1024,
            active_connections: 0,
            memory_bytes: None,
            core_version: None,
            sampled_at_epoch_ms: Some(self.epoch.fetch_add(1, Ordering::AcqRel) + 1),
        })
    }
    async fn set_mode(&self, mode: ProxyMode) -> Result<ProxyMode, PortError> {
        Ok(mode)
    }
}

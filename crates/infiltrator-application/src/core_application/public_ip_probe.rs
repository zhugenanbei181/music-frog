use super::*;
use infiltrator_ports::core_snapshot_source::CoreSnapshotSource;
use std::sync::Weak;

pub(super) struct CoreSnapshotSourceHandle {
    pub(super) inner: Weak<Inner>,
}

impl CoreSnapshotSource for CoreSnapshotSourceHandle {
    fn core_snapshot(&self) -> CoreSnapshot {
        let Some(inner) = self.inner.upgrade() else {
            return runtime_projection::snapshot_from_state(
                &CoreState::Idle { generation: 0 },
                0,
                Default::default(),
                None,
                runtime_projection::current_epoch_ms(),
            );
        };
        let mirror = inner.state.read().expect("core state lock");
        let watchdog = inner
            .watchdog
            .lock()
            .expect("core watchdog lock")
            .machine
            .snapshot();
        let identity = inner
            .core_identity
            .read()
            .expect("core identity lock")
            .clone();
        runtime_projection::snapshot_from_state(
            &mirror.state,
            mirror.revision,
            watchdog,
            identity,
            runtime_projection::current_epoch_ms(),
        )
    }
}

pub(super) fn install_public_ip_probe_application(
    application: &CoreApplication,
    owner: PublicIpProbeApplication,
) -> Result<(), Failure> {
    let mut slot = application
        .inner
        .public_ip_probe
        .write()
        .expect("public IP probe application lock");
    if application.inner.closed.load(Ordering::Acquire) {
        return Err(closed_failure());
    }
    if slot.is_some() {
        return Err(Failure::new(
            ErrorCode::InvalidState,
            "public IP probe application is already installed for this Core owner",
            false,
        ));
    }
    *slot = Some(
        owner.with_core_snapshot_source(Arc::new(CoreSnapshotSourceHandle {
            inner: Arc::downgrade(&application.inner),
        })),
    );
    Ok(())
}

pub(super) fn public_ip_probe_application(
    application: &CoreApplication,
) -> Option<PublicIpProbeApplication> {
    application
        .inner
        .public_ip_probe
        .read()
        .expect("public IP probe application lock")
        .clone()
}

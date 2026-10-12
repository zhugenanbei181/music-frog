use super::*;
use async_trait::async_trait;
use infiltrator_contract::version::{
    CoreRelease, CoreReleaseSummary, EngineNeutralReleaseChannel, EngineReleaseChannel,
    InstalledCoreVersion,
};
use infiltrator_ports::application_runtime::{
    ApplicationFuture, ApplicationRuntime, ApplicationSleep, ApplicationTaskAdmission,
    ApplicationTaskId,
};
use infiltrator_ports::core_binary_selection::CoreBinarySelectionPort;
use infiltrator_ports::error::PortError;
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionRollbackRequest,
    VersionUninstallRequest,
};
use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Notify;

#[derive(Clone, Copy)]
enum InstallBehavior {
    Complete,
    Fail,
    WaitForCancellation,
}

struct TestVersionPort {
    engine_id: EngineId,
    install_behavior: InstallBehavior,
    install_started: Arc<Notify>,
    operations: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl VersionPort for TestVersionPort {
    fn engine_id(&self) -> EngineId {
        self.engine_id.clone()
    }

    async fn list_installed(
        &self,
        _engine_id: &EngineId,
    ) -> Result<Vec<InstalledCoreVersion>, PortError> {
        Ok(Vec::new())
    }

    async fn latest(&self, channel: EngineReleaseChannel) -> Result<CoreRelease, PortError> {
        Ok(CoreRelease {
            engine_id: channel.engine_id,
            version: "v1.0.0".to_owned(),
            release_date: "2026-10-01".to_owned(),
        })
    }

    async fn list_releases(
        &self,
        engine_id: &EngineId,
        _limit: usize,
    ) -> Result<Vec<CoreReleaseSummary>, PortError> {
        Ok(vec![CoreReleaseSummary {
            engine_id: engine_id.clone(),
            version: "v1.0.0".to_owned(),
            name: "test release".to_owned(),
            published_at: "2026-10-01".to_owned(),
            prerelease: false,
        }])
    }

    async fn install(&self, request: VersionInstallRequest) -> Result<(), PortError> {
        if request.engine_id != self.engine_id {
            return Err(PortError::Failed("foreign install engine".to_owned()));
        }
        self.operations
            .lock()
            .expect("test operation log lock")
            .push(format!("install:{}", request.version));
        request.progress.progress(VersionDownloadProgress {
            downloaded: 7,
            total: Some(21),
        });
        self.install_started.notify_one();
        match self.install_behavior {
            InstallBehavior::Complete => Ok(()),
            InstallBehavior::Fail => Err(PortError::Network("test download failure".to_owned())),
            InstallBehavior::WaitForCancellation => loop {
                if request.progress.is_cancelled() {
                    return Err(PortError::Rejected(Failure::new(
                        ErrorCode::Canceled,
                        "test install cancelled",
                        false,
                    )));
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            },
        }
    }

    async fn activate(&self, request: VersionActivationRequest) -> Result<(), PortError> {
        if request.engine_id != self.engine_id {
            return Err(PortError::Failed("foreign activation engine".to_owned()));
        }
        self.operations
            .lock()
            .expect("test operation log lock")
            .push(format!("activate:{}", request.version));
        Ok(())
    }

    async fn uninstall(&self, request: VersionUninstallRequest) -> Result<(), PortError> {
        if request.engine_id != self.engine_id {
            return Err(PortError::Failed("foreign uninstall engine".to_owned()));
        }
        self.operations
            .lock()
            .expect("test operation log lock")
            .push(format!("uninstall:{}", request.version));
        Ok(())
    }

    async fn rollback(&self, request: VersionRollbackRequest) -> Result<String, PortError> {
        if request.engine_id != self.engine_id {
            return Err(PortError::Failed("foreign rollback engine".to_owned()));
        }
        self.operations
            .lock()
            .expect("test operation log lock")
            .push("rollback".to_owned());
        Ok("v1.0.0".to_owned())
    }
}

struct TestFactory {
    ports: HashMap<EngineId, Arc<TestVersionPort>>,
    binary_selection_requests: AtomicUsize,
}

struct TokioTestRuntime;

struct AtCapacityRuntime;

impl ApplicationRuntime for TokioTestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn spawn_background(&self, future: ApplicationFuture) -> ApplicationTaskAdmission {
        static NEXT_TASK_ID: AtomicU64 = AtomicU64::new(1);
        let task_id = NEXT_TASK_ID.fetch_add(1, Ordering::SeqCst);
        let Some(task_id) = NonZeroU64::new(task_id) else {
            return ApplicationTaskAdmission::IdExhausted;
        };
        tokio::spawn(future);
        ApplicationTaskAdmission::Scheduled {
            task_id: ApplicationTaskId::new(task_id),
        }
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

impl ApplicationRuntime for AtCapacityRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("test runtime")
            .block_on(future);
    }

    fn spawn_background(&self, _future: ApplicationFuture) -> ApplicationTaskAdmission {
        ApplicationTaskAdmission::AtCapacity {
            occupying_task_id: ApplicationTaskId::new(
                NonZeroU64::new(99).expect("test task ID is non-zero"),
            ),
        }
    }

    fn sleep(&self, duration: Duration) -> ApplicationSleep<'_> {
        Box::pin(tokio::time::sleep(duration))
    }
}

impl EngineVersionBackendFactory for TestFactory {
    fn version_port(&self, engine_id: &EngineId) -> Result<Arc<dyn VersionPort>, PortError> {
        self.ports
            .get(engine_id)
            .cloned()
            .map(|port| port as Arc<dyn VersionPort>)
            .ok_or_else(|| {
                PortError::unsupported(
                    infiltrator_contract::capability::Capability::CoreVersionInstall,
                    "no test version port for the requested engine",
                )
            })
    }

    fn binary_selection_port(
        &self,
        _engine_id: &EngineId,
    ) -> Result<Arc<dyn CoreBinarySelectionPort>, PortError> {
        self.binary_selection_requests
            .fetch_add(1, Ordering::SeqCst);
        Err(PortError::unsupported(
            infiltrator_contract::capability::Capability::CoreVersionInstall,
            "version-management tests do not select a process binary",
        ))
    }
}

fn engine(name: &str) -> EngineId {
    EngineId::new(name).expect("valid engine ID")
}

fn port(engine_id: EngineId, install_behavior: InstallBehavior) -> Arc<TestVersionPort> {
    Arc::new(TestVersionPort {
        engine_id,
        install_behavior,
        install_started: Arc::new(Notify::new()),
        operations: Arc::new(Mutex::new(Vec::new())),
    })
}

fn application(
    ports: impl IntoIterator<Item = Arc<TestVersionPort>>,
) -> CoreVersionManagementApplication {
    application_with_runtime(ports, Arc::new(TokioTestRuntime))
}

fn application_with_runtime(
    ports: impl IntoIterator<Item = Arc<TestVersionPort>>,
    runtime: Arc<dyn ApplicationRuntime>,
) -> CoreVersionManagementApplication {
    let ports = ports
        .into_iter()
        .map(|port| (port.engine_id.clone(), port))
        .collect();
    CoreVersionManagementApplication::new(
        Arc::new(TestFactory {
            ports,
            binary_selection_requests: AtomicUsize::new(0),
        }),
        runtime,
    )
}

async fn wait_for_terminal(
    application: &CoreVersionManagementApplication,
    install_id: CoreVersionInstallId,
) -> CoreVersionInstallState {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if let CoreVersionInstallObservation::Found { state } =
                application.read_install(install_id)
                && !state.is_active()
            {
                return state;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("version install reaches a terminal state")
}

#[tokio::test]
async fn install_progress_cancel_and_terminal_state_are_one_typed_lifecycle() {
    let sing_box = engine("sing-box");
    let port = port(sing_box.clone(), InstallBehavior::WaitForCancellation);
    let install_started = Arc::clone(&port.install_started);
    let application = application([Arc::clone(&port)]);
    let started = install_started.notified();

    let result = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.clone(),
            action: CoreVersionManagementAction::Install {
                version: "v1.14.3".to_owned(),
            },
        })
        .await
        .expect("owner accepts a typed install job");
    let CoreVersionManagementResult::InstallAccepted { receipt, state } = result else {
        panic!("install request returns an install receipt and state");
    };
    assert!(matches!(state, CoreVersionInstallState::Running { .. }));
    started.await;

    let CoreVersionInstallObservation::Found { state } =
        application.read_install(receipt.install_id)
    else {
        panic!("accepted install remains readable");
    };
    assert!(matches!(
        state,
        CoreVersionInstallState::Running {
            progress: Some(VersionDownloadProgress {
                downloaded: 7,
                total: Some(21)
            }),
            ..
        }
    ));

    let duplicate = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.clone(),
            action: CoreVersionManagementAction::Install {
                version: "v1.14.4".to_owned(),
            },
        })
        .await
        .expect_err("the same engine cannot run overlapping installs");
    assert_eq!(duplicate.code, ErrorCode::InvalidState);

    assert_eq!(
        application.cancel_install(receipt.install_id),
        CoreVersionInstallCancelOutcome::Requested {
            install_id: receipt.install_id
        }
    );
    assert_eq!(
        application.cancel_install(receipt.install_id),
        CoreVersionInstallCancelOutcome::AlreadyRequested {
            install_id: receipt.install_id
        }
    );
    assert!(matches!(
        application.read_install(receipt.install_id),
        CoreVersionInstallObservation::Found {
            state: CoreVersionInstallState::CancelRequested { .. }
        }
    ));
    assert!(matches!(
        wait_for_terminal(&application, receipt.install_id).await,
        CoreVersionInstallState::Cancelled { .. }
    ));
    assert_eq!(
        application.cancel_install(receipt.install_id),
        CoreVersionInstallCancelOutcome::AlreadyFinished {
            install_id: receipt.install_id
        }
    );
}

#[tokio::test]
async fn version_operations_are_dispatched_to_the_exact_engine_backend() {
    let mihomo = port(engine("mihomo"), InstallBehavior::Complete);
    let sing_box = port(engine("sing-box"), InstallBehavior::Complete);
    let application = application([Arc::clone(&mihomo), Arc::clone(&sing_box)]);

    let activated = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.engine_id.clone(),
            action: CoreVersionManagementAction::Activate {
                version: "v1.14.3".to_owned(),
            },
        })
        .await
        .expect("sing-box activation reaches its registered backend");
    assert!(matches!(
        activated,
        CoreVersionManagementResult::Activated { engine_id, version }
            if engine_id == sing_box.engine_id && version == "v1.14.3"
    ));

    let latest = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.engine_id.clone(),
            action: CoreVersionManagementAction::Latest {
                channel: EngineNeutralReleaseChannel::Stable,
            },
        })
        .await
        .expect("latest release lookup stays within sing-box");
    assert!(matches!(
        latest,
        CoreVersionManagementResult::Latest { engine_id, release }
            if engine_id == sing_box.engine_id && release.engine_id == sing_box.engine_id
    ));

    let removed = application
        .execute(CoreVersionManagementRequest {
            engine_id: mihomo.engine_id.clone(),
            action: CoreVersionManagementAction::Uninstall {
                version: "v1.19.32".to_owned(),
            },
        })
        .await
        .expect("Mihomo uninstall reaches only the Mihomo backend");
    assert!(matches!(
        removed,
        CoreVersionManagementResult::Uninstalled { engine_id, version }
            if engine_id == mihomo.engine_id && version == "v1.19.32"
    ));

    let installed = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.engine_id.clone(),
            action: CoreVersionManagementAction::ListInstalled,
        })
        .await
        .expect("installed versions are read through the selected engine port");
    assert!(matches!(
        installed,
        CoreVersionManagementResult::InstalledVersions { engine_id, versions }
            if engine_id == sing_box.engine_id && versions.is_empty()
    ));

    let releases = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.engine_id.clone(),
            action: CoreVersionManagementAction::ListReleases { limit: 10 },
        })
        .await
        .expect("release listing is engine-scoped");
    assert!(matches!(
        releases,
        CoreVersionManagementResult::Releases { engine_id, releases }
            if engine_id == sing_box.engine_id && releases.len() == 1
    ));

    let rollback = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.engine_id.clone(),
            action: CoreVersionManagementAction::Rollback,
        })
        .await
        .expect("rollback targets the selected engine");
    assert!(matches!(
        rollback,
        CoreVersionManagementResult::RolledBack { engine_id, selected_version }
            if engine_id == sing_box.engine_id && selected_version == "v1.0.0"
    ));

    assert_eq!(
        sing_box
            .operations
            .lock()
            .expect("operation log")
            .as_slice(),
        ["activate:v1.14.3", "rollback"]
    );
    assert_eq!(
        mihomo.operations.lock().expect("operation log").as_slice(),
        ["uninstall:v1.19.32"]
    );
}

#[tokio::test]
async fn install_can_succeed_and_unknown_engine_fails_without_selecting_a_binary() {
    let sing_box = port(engine("sing-box"), InstallBehavior::Complete);
    let application = application([Arc::clone(&sing_box)]);
    let result = application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.engine_id.clone(),
            action: CoreVersionManagementAction::Install {
                version: "v1.14.3".to_owned(),
            },
        })
        .await
        .expect("owner accepts install");
    let CoreVersionManagementResult::InstallAccepted { receipt, .. } = result else {
        panic!("install returns a typed receipt");
    };
    assert!(matches!(
        wait_for_terminal(&application, receipt.install_id).await,
        CoreVersionInstallState::Completed { .. }
    ));

    let unknown = application
        .execute(CoreVersionManagementRequest {
            engine_id: engine("unregistered-engine"),
            action: CoreVersionManagementAction::ListInstalled,
        })
        .await
        .expect_err("unregistered engines fail closed");
    assert_eq!(unknown.code, ErrorCode::Unsupported);
}

#[tokio::test]
async fn adapter_failure_and_runtime_capacity_are_typed_and_leave_no_phantom_job() {
    let sing_box = engine("sing-box");
    let failing_port = port(sing_box.clone(), InstallBehavior::Fail);
    let failing_application = application([Arc::clone(&failing_port)]);
    let result = failing_application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box.clone(),
            action: CoreVersionManagementAction::Install {
                version: "v1.14.3".to_owned(),
            },
        })
        .await
        .expect("install admission succeeds before the fake download fails");
    let CoreVersionManagementResult::InstallAccepted { receipt, .. } = result else {
        panic!("install returns a receipt before completion");
    };
    assert!(matches!(
        wait_for_terminal(&failing_application, receipt.install_id).await,
        CoreVersionInstallState::Failed { .. }
    ));

    let capacity_port = port(sing_box.clone(), InstallBehavior::WaitForCancellation);
    let capacity_application =
        application_with_runtime([capacity_port], Arc::new(AtCapacityRuntime));
    let failure = capacity_application
        .execute(CoreVersionManagementRequest {
            engine_id: sing_box,
            action: CoreVersionManagementAction::Install {
                version: "v1.14.3".to_owned(),
            },
        })
        .await
        .expect_err("runtime capacity rejects without creating a background job");
    assert_eq!(failure.code, ErrorCode::InvalidState);
    assert_eq!(
        capacity_application.read_install(CoreVersionInstallId::new(1).expect("ID")),
        CoreVersionInstallObservation::NotFound {
            install_id: CoreVersionInstallId::new(1).expect("ID")
        }
    );
}

#[test]
fn install_ids_reject_zero_and_task_reads_are_explicitly_not_found() {
    assert!(CoreVersionInstallId::new(0).is_err());
    let application = application([]);
    let install_id = CoreVersionInstallId::new(7).expect("non-zero install ID");
    assert_eq!(
        application.read_install(install_id),
        CoreVersionInstallObservation::NotFound { install_id }
    );
    assert_eq!(
        application.cancel_install(install_id),
        CoreVersionInstallCancelOutcome::NotFound { install_id }
    );
}

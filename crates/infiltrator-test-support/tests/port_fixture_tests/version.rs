//! Behaviour of the isolated core-version port fixture.

use infiltrator_contract::version::{CoreArtifactVerification, EngineNeutralReleaseChannel};
use infiltrator_ports::version::{
    VersionActivationRequest, VersionInstallRequest, VersionPort, VersionRollbackRequest,
    VersionUninstallRequest,
};
use infiltrator_test_support::version_fixtures::{
    CAPTURE_ACTIVE_VERSION, CAPTURE_ALPHA_VERSION, CAPTURE_ENGINE, CAPTURE_ROLLBACK_VERSION,
    VersionCapturePort, VersionCaptureSink,
};
use lurker_core_contract::identity::EngineId;
use lurker_core_ports::error::PortError;
use std::sync::Arc;

fn owner_engine() -> EngineId {
    EngineId::new(CAPTURE_ENGINE).expect("valid capture engine ID")
}

fn foreign_engine() -> EngineId {
    EngineId::new("sing-box").expect("valid foreign engine ID")
}

#[tokio::test]
async fn the_installed_versions_answer_the_owner_question() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    let installed = port
        .list_installed(&owner_engine())
        .await
        .expect("installed versions");

    assert_eq!(installed.len(), 2);
    assert!(
        installed
            .iter()
            .any(|entry| entry.version == CAPTURE_ACTIVE_VERSION && entry.is_default)
    );
    assert!(
        installed
            .iter()
            .any(|entry| entry.version == CAPTURE_ROLLBACK_VERSION && !entry.is_default)
    );
}

#[tokio::test]
async fn the_rollback_journal_names_the_previous_installed_version() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    let snapshot = port
        .rollback_snapshot(&owner_engine())
        .await
        .expect("rollback snapshot");

    assert_eq!(snapshot.current.as_deref(), Some(CAPTURE_ACTIVE_VERSION));
    assert_eq!(snapshot.target.as_deref(), Some(CAPTURE_ROLLBACK_VERSION));
    assert_eq!(snapshot.history, vec![CAPTURE_ROLLBACK_VERSION.to_owned()]);
}

#[tokio::test]
async fn rollback_selects_the_previous_installed_version() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    let selected = port
        .rollback(VersionRollbackRequest {
            engine_id: owner_engine(),
        })
        .await
        .expect("rollback");

    assert_eq!(selected, CAPTURE_ROLLBACK_VERSION);
    assert_eq!(
        port.installed_versions()
            .into_iter()
            .find(|entry| entry.is_default)
            .map(|entry| entry.version),
        Some(CAPTURE_ROLLBACK_VERSION.to_owned()),
        "the owner readback must show the selected version as active"
    );
}

#[tokio::test]
async fn a_failing_rollback_is_reported_as_a_typed_failure() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    port.fail_rollback("no rollback journal on this host");

    let error = port
        .rollback(VersionRollbackRequest {
            engine_id: owner_engine(),
        })
        .await
        .expect_err("the rollback must fail");

    assert!(matches!(error, PortError::Failed(_)));
}

#[tokio::test]
async fn a_foreign_engine_never_reaches_the_version_port() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);

    let error = port
        .list_installed(&foreign_engine())
        .await
        .expect_err("a foreign engine must be refused");

    assert!(matches!(error, PortError::Rejected(_)));
}

#[tokio::test]
async fn an_unpublished_release_channel_is_a_typed_unsupported() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    let channels = port
        .release_channels(&owner_engine())
        .await
        .expect("release channels");
    assert_eq!(channels.len(), EngineNeutralReleaseChannel::ALL.len());

    let latest = port
        .latest(channels[0].clone())
        .await
        .expect("latest release");
    assert_eq!(latest.version, CAPTURE_ACTIVE_VERSION);
}

#[tokio::test]
async fn a_cancelled_install_is_refused_and_records_no_artifact() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    let sink = VersionCaptureSink::new();
    sink.cancel();

    let error = port
        .install(VersionInstallRequest {
            engine_id: owner_engine(),
            version: "v9.9.9".to_owned(),
            progress: Arc::new(sink),
        })
        .await
        .expect_err("a cancelled install must fail");

    assert!(matches!(error, PortError::Rejected(_)));
    assert!(matches!(
        port.artifact_verification(),
        CoreArtifactVerification::Rejected { .. }
    ));
    assert!(
        !port
            .installed_versions()
            .iter()
            .any(|entry| entry.version == "v9.9.9")
    );
}

#[tokio::test]
async fn an_installed_version_becomes_the_active_one() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);
    let sink = VersionCaptureSink::new();

    port.install(VersionInstallRequest {
        engine_id: owner_engine(),
        version: CAPTURE_ALPHA_VERSION.to_owned(),
        progress: Arc::new(sink),
    })
    .await
    .expect("install");

    port.activate(VersionActivationRequest {
        engine_id: owner_engine(),
        version: CAPTURE_ALPHA_VERSION.to_owned(),
    })
    .await
    .expect("activate");

    let installed = port.installed_versions();
    assert_eq!(
        installed
            .iter()
            .filter(|entry| entry.is_default)
            .map(|entry| entry.version.as_str())
            .collect::<Vec<_>>(),
        vec![CAPTURE_ALPHA_VERSION]
    );
    assert!(matches!(
        port.artifact_verification(),
        CoreArtifactVerification::Verified { .. }
    ));
}

#[tokio::test]
async fn the_active_version_cannot_be_uninstalled() {
    let port = VersionCapturePort::new(CAPTURE_ENGINE);

    let error = port
        .uninstall(VersionUninstallRequest {
            engine_id: owner_engine(),
            version: CAPTURE_ACTIVE_VERSION.to_owned(),
        })
        .await
        .expect_err("the active version must be protected");

    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(port.installed_versions().len(), 2);
}

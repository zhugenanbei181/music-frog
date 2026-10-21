//! Behaviour of the isolated core-process and readiness fixtures.

use infiltrator_test_support::core_process_fixtures::{
    CAPTURE_ENGINE, CAPTURE_INSTANCE, CoreCaptureBehaviour, CoreCaptureProcess,
};
use lurker_core_contract::core_process::CoreProcessStartContext;
use lurker_core_contract::identity::{CoreIdentity, CoreInstanceId, EngineId};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_ports::core_process::{CoreProcess, CoreReadiness};
use lurker_core_ports::error::PortError;

fn capture_identity() -> CoreIdentity {
    CoreIdentity::new(
        EngineId::new(CAPTURE_ENGINE).expect("valid capture engine ID"),
        CoreInstanceId::new(CAPTURE_INSTANCE).expect("valid capture instance ID"),
        None,
    )
}

fn capture_context() -> CoreProcessStartContext {
    CoreProcessStartContext::standalone(capture_identity())
}

#[tokio::test]
async fn a_ready_core_answers_the_readiness_probe() {
    let process = CoreCaptureProcess::new(CAPTURE_ENGINE, CAPTURE_INSTANCE);

    process.start(capture_context()).await.expect("start");

    assert_eq!(
        process.status().await.expect("status"),
        CoreLifecycle::Running
    );
    assert_eq!(process.start_count(), 1);
    assert!(
        process
            .probe()
            .await
            .expect("readiness")
            .starts_with("isolated-core-controller")
    );
    assert!(process.controller_endpoint().is_some());
    assert!(process.pid().await.is_some());
}

#[tokio::test]
async fn a_stalled_core_never_becomes_ready() {
    let process = CoreCaptureProcess::new(CAPTURE_ENGINE, CAPTURE_INSTANCE)
        .with_behaviour(CoreCaptureBehaviour::StalledOnStart);

    process.start(capture_context()).await.expect("start");

    assert_eq!(
        process.status().await.expect("status"),
        CoreLifecycle::Starting
    );
    let error = process.probe().await.expect_err("the probe must fail");
    assert!(matches!(error, PortError::Rejected(_)));
    assert!(process.controller_endpoint().is_none());
}

#[tokio::test]
async fn a_failed_launch_is_a_typed_failure() {
    let process = CoreCaptureProcess::new(CAPTURE_ENGINE, CAPTURE_INSTANCE)
        .with_behaviour(CoreCaptureBehaviour::FailedLaunch);

    let error = process
        .start(capture_context())
        .await
        .expect_err("the launch must fail");

    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(
        process.status().await.expect("status"),
        CoreLifecycle::Stopped
    );
}

#[tokio::test]
async fn a_foreign_start_context_is_refused() {
    let process = CoreCaptureProcess::new(CAPTURE_ENGINE, CAPTURE_INSTANCE);
    let foreign = CoreProcessStartContext::standalone(CoreIdentity::new(
        EngineId::new("sing-box").expect("valid foreign engine ID"),
        CoreInstanceId::new(CAPTURE_INSTANCE).expect("valid capture instance ID"),
        None,
    ));

    let error = process
        .start(foreign)
        .await
        .expect_err("a foreign engine must be refused");

    assert!(matches!(error, PortError::Rejected(_)));
    assert_eq!(process.start_count(), 0);
}

#[tokio::test]
async fn stopping_a_stopped_core_is_a_no_op() {
    let process = CoreCaptureProcess::new(CAPTURE_ENGINE, CAPTURE_INSTANCE);

    process.stop().await.expect("stop");

    assert_eq!(
        process.status().await.expect("status"),
        CoreLifecycle::Stopped
    );
}

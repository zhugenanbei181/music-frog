use infiltrator_contract::core_identity::CoreVersion;
use infiltrator_contract::core_identity::EngineId;
use infiltrator_contract::speedtest::SpeedtestConcurrency;
use infiltrator_contract::speedtest::SpeedtestRunId;
use infiltrator_ports::application_runtime::ApplicationSleep;
use std::thread;
use tokio::task::yield_now;
use tokio::time::timeout;
use super::*;
use crate::outbound_application::OutboundApplication;
use crate::speedtest_application::tests::{
    FakeOutboundPort, ProbeGate, app_settings, id, observation, target,
};
use crate::speedtest_application::{SpeedtestApplication, SpeedtestTaskOutcome};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::core_capabilities::{
    AdapterCapabilityDecision, AdapterRevision, CoreCapabilityEntry, CoreCapabilityMatrix,
    CoreCapabilityScope, CoreOperation, EngineCapabilityDecision, HostCapabilityDecision,
    OutboundOperation,
};
use infiltrator_contract::core_identity::CoreCommandOutcome;
use infiltrator_contract::outbound::probe::OutboundProbeBatchScope;
use infiltrator_contract::speedtest::{
    SpeedtestCancelOutcome, SpeedtestRunState, SpeedtestStartReceipt,
};
use infiltrator_contract::surface::HostKind;
use infiltrator_ports::application_runtime::{
    ApplicationTaskAdmission, ApplicationTaskId, ApplicationTaskShutdown,
};
use infiltrator_ports::core_capability_provider::{
    CoreCapabilityProvider, CoreCapabilityProviderError,
};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;
use tokio::runtime::Builder;
use tokio::time::sleep;

struct SpeedtestCapabilityProvider;

impl CoreCapabilityProvider for SpeedtestCapabilityProvider {
    fn matrix_for(
        &self,
        engine_id: &EngineId,
        core_version: Option<&CoreVersion>,
    ) -> Result<CoreCapabilityMatrix, CoreCapabilityProviderError> {
        let scope = CoreCapabilityScope::new(
            engine_id.clone(),
            core_version.cloned(),
            HostKind::Desktop,
            AdapterRevision::new(1),
        );
        let entries = [CoreOperation::Outbound(OutboundOperation::Probe)]
            .into_iter()
            .map(|operation| {
                CoreCapabilityEntry::new(
                    operation,
                    EngineCapabilityDecision::Supported,
                    HostCapabilityDecision::Supported,
                    AdapterCapabilityDecision::Supported,
                )
            })
            .collect();
        CoreCapabilityMatrix::new(scope, entries).map_err(|error| {
            CoreCapabilityProviderError::MatrixUnavailable {
                reason: error.to_string(),
            }
        })
    }
}

/// Test host runtime: actor commands get short-lived current-thread runtimes,
/// while each admitted background task owns a separate current-thread driver.
struct ActorTestRuntime {
    next_task_id: AtomicU64,
}

impl ApplicationRuntime for ActorTestRuntime {
    fn block_on(&self, future: ApplicationFuture) {
        Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("actor command runtime")
            .block_on(future);
    }

    fn spawn_background(&self, future: ApplicationFuture) -> ApplicationTaskAdmission {
        let value = self.next_task_id.fetch_add(1, Ordering::Relaxed);
        let Some(value) = NonZeroU64::new(value) else {
            return ApplicationTaskAdmission::IdExhausted;
        };
        let task_id = ApplicationTaskId::new(value);
        match thread::Builder::new()
            .name("speedtest-actor-test-task".to_owned())
            .spawn(move || {
                Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("actor background runtime")
                    .block_on(future);
            }) {
            Ok(_) => ApplicationTaskAdmission::Scheduled { task_id },
            Err(_) => ApplicationTaskAdmission::WorkerFailed,
        }
    }

    fn shutdown_background_tasks(&self) -> ApplicationTaskShutdown {
        ApplicationTaskShutdown::Closed
    }

    fn sleep(
        &self,
        duration: Duration,
    ) -> ApplicationSleep<'_> {
        Box::pin(sleep(duration))
    }
}

async fn receive(receiver: Receiver<CoreCommandOutcome>) -> CoreCommandOutcome {
    timeout(Duration::from_secs(3), async move {
        loop {
            match receiver.try_recv() {
                Ok(outcome) => return outcome,
                Err(TryRecvError::Empty) => yield_now().await,
                Err(TryRecvError::Disconnected) => panic!("Core actor dropped command reply"),
            }
        }
    })
    .await
    .expect("Core actor command reply arrives")
}

async fn wait_for_cancelled(
    speedtest: &SpeedtestApplication,
    run_id: SpeedtestRunId,
) {
    timeout(Duration::from_secs(3), async {
        loop {
            if matches!(
                speedtest.snapshot().state,
                SpeedtestRunState::Cancelled { run_id: current, .. } if current == run_id
            ) {
                return;
            }
            yield_now().await;
        }
    })
    .await
    .expect("Core actor publishes the cancelled terminal state");
}

fn started_receipt(outcome: CoreCommandOutcome) -> SpeedtestStartReceipt {
    match outcome.result {
        CommandResult::Produced { output, .. } => match *output {
            CommandOutput::SpeedtestStarted(receipt) => receipt,
            other => panic!("unexpected start output: {other:?}"),
        },
        other => panic!("speedtest start was not accepted: {other:?}"),
    }
}

fn cancelled_outcome(
    outcome: CoreCommandOutcome,
    expected: SpeedtestRunId,
) {
    match outcome.result {
        CommandResult::Produced { output, .. } => match *output {
            CommandOutput::SpeedtestCancelled(SpeedtestCancelOutcome::Requested { run_id }) => {
                assert_eq!(run_id, expected)
            }
            other => panic!("unexpected cancel output: {other:?}"),
        },
        other => panic!("speedtest cancellation was rejected: {other:?}"),
    }
}

fn stale_cancel_outcome(
    outcome: CoreCommandOutcome,
    requested: SpeedtestRunId,
    current: SpeedtestRunId,
) {
    match outcome.result {
        CommandResult::Produced { output, .. } => match *output {
            CommandOutput::SpeedtestCancelled(SpeedtestCancelOutcome::StaleRun {
                requested_run_id,
                current_run_id: Some(current_run_id),
            }) => {
                assert_eq!(requested_run_id, requested);
                assert_eq!(current_run_id, current);
            }
            other => panic!("unexpected stale cancel output: {other:?}"),
        },
        other => panic!("stale speedtest cancellation was rejected: {other:?}"),
    }
}

#[tokio::test]
async fn core_actor_cancels_while_probe_is_blocked_and_fences_old_completion() {
    let runtime = Arc::new(ActorTestRuntime {
        next_task_id: AtomicU64::new(1),
    });
    let process = Arc::new(FakeProcess {
        running: AtomicBool::new(true),
        fail_start: false,
        fail_stop: false,
    });
    let application = CoreApplication::new_with_policy(
        process,
        Arc::new(FakeReadiness {
            endpoint: Ok("http://127.0.0.1:9090".to_owned()),
        }),
        ReadinessPolicy {
            timeout: Duration::from_millis(100),
            poll_interval: Duration::from_millis(1),
        },
        runtime,
    );
    application
        .install_core_capability_application(CoreCapabilityApplication::new(Arc::new(
            SpeedtestCapabilityProvider,
        )))
        .expect("install test capability owner");
    assert!(
        application
            .adopt_if_running()
            .await
            .expect("adopt running test Core")
    );
    let scope = application.snapshot().scope().expect("adopted Core scope");

    let gate = Arc::new(ProbeGate::default());
    let outbound = Arc::new(FakeOutboundPort::gated(
        observation(scope.clone(), 11),
        gate.clone(),
    ));
    let speedtest = SpeedtestApplication::new(app_settings());
    speedtest
        .install_outbound_application(OutboundApplication::new(outbound))
        .expect("install active outbound owner");
    application
        .install_speedtest_application(speedtest.clone())
        .expect("install Core-owned speedtest");
    let client = ScopedCoreCommandClient::new(application.clone(), scope.clone());
    let request_target = target(
        scope.clone(),
        11,
        OutboundProbeBatchScope::One(id("fake-core", "outbound-a")),
        4,
    );

    let (_, start_reply) = client.dispatch_tracked(CommandIntent::StartSpeedtest {
        target: request_target.clone(),
    });
    let receipt = started_receipt(receive(start_reply).await);
    timeout(Duration::from_secs(3), gate.started.notified())
        .await
        .expect("probe reaches its blocked gate");
    assert!(matches!(
        speedtest.snapshot().state,
        SpeedtestRunState::Probing { run_id, .. } if run_id == receipt.run_id
    ));

    let (_, cancel_reply) = client.dispatch_tracked(CommandIntent::CancelSpeedtestRun {
        run_id: receipt.run_id,
    });
    cancelled_outcome(receive(cancel_reply).await, receipt.run_id);
    wait_for_cancelled(&speedtest, receipt.run_id).await;

    let (_, second_start_reply) = client.dispatch_tracked(CommandIntent::StartSpeedtest {
        target: request_target,
    });
    let second_receipt = started_receipt(receive(second_start_reply).await);
    assert!(second_receipt.run_id > receipt.run_id);
    timeout(Duration::from_secs(3), gate.started.notified())
        .await
        .expect("second probe reaches its blocked gate");

    let (_, stale_cancel_reply) = client.dispatch_tracked(CommandIntent::CancelSpeedtestRun {
        run_id: receipt.run_id,
    });
    stale_cancel_outcome(
        receive(stale_cancel_reply).await,
        receipt.run_id,
        second_receipt.run_id,
    );
    assert!(matches!(
        speedtest.snapshot().state,
        SpeedtestRunState::Probing { run_id, .. } if run_id == second_receipt.run_id
    ));

    assert!(
        application
            .inner
            .dispatch_tx
            .send(DispatchedMessage::SpeedtestCompleted {
                scope: scope.clone(),
                run_id: receipt.run_id,
                outcome: SpeedtestTaskOutcome::Completed,
            })
            .is_ok()
    );
    let (_, barrier_reply) = client.dispatch_tracked(CommandIntent::SetSpeedtestConcurrency {
        concurrency: SpeedtestConcurrency::new(2)
            .expect("valid test concurrency"),
    });
    assert!(matches!(
        receive(barrier_reply).await.result,
        CommandResult::Completed { .. }
    ));
    assert!(matches!(
        speedtest.snapshot().state,
        SpeedtestRunState::Probing { run_id, .. } if run_id == second_receipt.run_id
    ));

    let (_, second_cancel_reply) = client.dispatch_tracked(CommandIntent::CancelSpeedtestRun {
        run_id: second_receipt.run_id,
    });
    cancelled_outcome(receive(second_cancel_reply).await, second_receipt.run_id);
    wait_for_cancelled(&speedtest, second_receipt.run_id).await;
    application
        .close()
        .await
        .expect("Core owner closes cleanly");
}

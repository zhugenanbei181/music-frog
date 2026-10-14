//! test-intent: behavior
use super::{FakeProcess, application};
use crate::command_application::handler::{
    CommandFuture, CommandHandler, CommandOutputFuture, ValidatedCoreCommandScope,
};
use crate::core_application::CoreApplication;
use crate::core_application::{
    test_dispatch_at_current_scope, test_execute_result_at_current_scope,
};
use futures_util::poll;
use infiltrator_contract::command::{CommandIntent, CommandResult};
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::profile_document_identity::ProfileId;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::subscription_filter_result::{FilterReport, SubscriptionFilterApplied};
use infiltrator_ports::core_lifecycle::CoreLifecyclePort;
use infiltrator_ports::core_watchdog::{CoreWatchdogPort, WatchdogTick};
use infiltrator_test_support::subscription_filter_fixture::{FIXTURE_DOCUMENT, observation};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::task::Poll;
use std::time::Duration;
use tokio::pin;
use tokio::sync::oneshot;
use tokio::time::timeout;

fn stopped_application() -> CoreApplication {
    application(
        FakeProcess {
            running: AtomicBool::new(false),
            fail_start: false,
            fail_stop: false,
        },
        Ok("http://127.0.0.1:9090".into()),
    )
}

struct TypedFilterHandler(SubscriptionFilterApplied);
impl CommandHandler for TypedFilterHandler {
    fn handle(&self, _: CommandIntent) -> CommandFuture {
        panic!("core must use the typed command output path")
    }
    fn handle_output(&self, _: CommandIntent) -> CommandOutputFuture {
        let applied = self.0.clone();
        Box::pin(async move { Ok(CommandOutput::SubscriptionFilterApplied(applied)) })
    }
    fn handle_output_scoped(
        &self,
        scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        if scope.as_ref().is_some()
            && matches!(intent, CommandIntent::SaveSubscriptionFilter { .. })
        {
            self.handle_output(intent)
        } else {
            Box::pin(async {
                Err(Failure::unsupported(
                    "test fake only handles scoped subscription filter saves",
                ))
            })
        }
    }
}
#[tokio::test]
async fn typed_filter_terminal_retains_request_source_and_statistics_and_rejects_foreign_output() {
    let application = stopped_application();
    let source = observation("main", FIXTURE_DOCUMENT, Default::default())
        .unwrap()
        .workspace_identity;
    let report = FilterReport {
        total_input: 11,
        passed: 7,
        renamed: 2,
        deduplicated: 1,
        ..Default::default()
    };
    let applied = SubscriptionFilterApplied {
        workspace_identity: source.clone(),
        report,
    };
    application.install_command_handler(Arc::new(TypedFilterHandler(applied.clone())));
    let result = test_execute_result_at_current_scope(
        &application,
        CommandIntent::SaveSubscriptionFilter {
            workspace_identity: source.clone(),
            filter: Default::default(),
        },
    )
    .await;
    match result {
        CommandResult::Produced { request_id, output } => {
            assert!(request_id.0 > 0);
            assert_eq!(
                *output,
                CommandOutput::SubscriptionFilterApplied(applied.clone())
            );
        }
        other => panic!("full typed terminal is required: {other:?}"),
    }
    let mut foreign = applied;
    foreign.workspace_identity.document.profile_id = ProfileId::new("other").unwrap();
    application.install_command_handler(Arc::new(TypedFilterHandler(foreign)));
    assert!(
        matches!(test_execute_result_at_current_scope(&application, CommandIntent::SaveSubscriptionFilter { workspace_identity: source, filter: Default::default() }).await, CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::InvalidState)
    );
}

struct LifecycleTransaction(Arc<CoreApplication>);
impl CommandHandler for LifecycleTransaction {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        assert!(matches!(intent, CommandIntent::SwitchProfile { .. }));
        let application = self.0.clone();
        Box::pin(async move {
            assert_eq!(
                CoreLifecyclePort::start(application.as_ref())
                    .await
                    .map_err(Failure::from)?,
                1
            );
            assert_eq!(
                CoreLifecyclePort::restart(application.as_ref())
                    .await
                    .map_err(Failure::from)?,
                2
            );
            CoreLifecyclePort::stop(application.as_ref())
                .await
                .map_err(Failure::from)
        })
    }
    fn handle_output_scoped(
        &self,
        scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        if scope.as_ref().is_some() && matches!(intent, CommandIntent::SwitchProfile { .. }) {
            let handled = self.handle(intent);
            Box::pin(async move { handled.await.map(|()| CommandOutput::Unit) })
        } else {
            Box::pin(async {
                Err(Failure::unsupported(
                    "test fake only handles scoped profile switches",
                ))
            })
        }
    }
}

#[tokio::test]
async fn compound_command_can_start_restart_and_stop_its_own_core_without_deadlock() {
    let application = Arc::new(stopped_application());
    let handler = Arc::new(LifecycleTransaction(application.clone()));
    application.install_command_handler(handler.clone());
    let result = timeout(
        Duration::from_secs(2),
        test_execute_result_at_current_scope(
            &application,
            CommandIntent::SwitchProfile {
                profile_id: "transaction-fixture".into(),
            },
        ),
    )
    .await
    .expect("nested lifecycle transaction must terminate");
    assert!(matches!(result, CommandResult::Completed { .. }));
    assert_eq!(application.snapshot().lifecycle, CoreLifecycle::Stopped);
    assert_eq!(application.generation(), 2);
    application.close().await.unwrap();
    assert_eq!(
        Arc::strong_count(&handler),
        1,
        "closing must release the installed facade cycle"
    );
    let weak = Arc::downgrade(&application);
    drop(handler);
    drop(application);
    assert!(weak.upgrade().is_none());
}

struct BlockingHandler {
    gate: Mutex<Option<oneshot::Receiver<()>>>,
    calls: Mutex<Vec<CommandIntent>>,
    nested: Option<Arc<CoreApplication>>,
}
impl CommandHandler for BlockingHandler {
    fn handle(&self, intent: CommandIntent) -> CommandFuture {
        self.calls.lock().unwrap().push(intent);
        let gate = self.gate.lock().unwrap().take();
        let nested = self.nested.clone();
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
            if let Some(application) = nested {
                CoreLifecyclePort::start(application.as_ref())
                    .await
                    .map_err(Failure::from)?;
            }
            Ok(())
        })
    }
    fn handle_output_scoped(
        &self,
        scope: ValidatedCoreCommandScope,
        intent: CommandIntent,
    ) -> CommandOutputFuture {
        if scope.as_ref().is_some() && matches!(intent, CommandIntent::SwitchProfile { .. }) {
            let handled = self.handle(intent);
            Box::pin(async move { handled.await.map(|()| CommandOutput::Unit) })
        } else {
            Box::pin(async {
                Err(Failure::unsupported(
                    "test fake only handles scoped profile switches",
                ))
            })
        }
    }
}

#[tokio::test]
async fn command_admission_serializes_business_work_without_holding_the_lifecycle_gate() {
    let application = stopped_application();
    let (release, gate) = oneshot::channel();
    let handler = Arc::new(BlockingHandler {
        gate: Mutex::new(Some(gate)),
        calls: Mutex::new(vec![]),
        nested: None,
    });
    application.install_command_handler(handler.clone());
    let first_intent = CommandIntent::SwitchProfile {
        profile_id: "first".into(),
    };
    let second_intent = CommandIntent::SwitchProfile {
        profile_id: "second".into(),
    };
    let first = test_execute_result_at_current_scope(&application, first_intent.clone());
    let second = test_execute_result_at_current_scope(&application, second_intent.clone());
    pin!(first, second);
    assert!(matches!(poll!(first.as_mut()), Poll::Pending));
    assert!(matches!(poll!(second.as_mut()), Poll::Pending));
    assert_eq!(*handler.calls.lock().unwrap(), vec![first_intent.clone()]);
    release.send(()).unwrap();
    assert!(matches!(first.await, CommandResult::Completed { .. }));
    assert!(matches!(second.await, CommandResult::Completed { .. }));
    assert_eq!(
        *handler.calls.lock().unwrap(),
        vec![first_intent, second_intent]
    );
}

#[tokio::test]
async fn closing_finishes_active_work_rejects_new_requests_and_disables_lifecycle_restart() {
    let application = Arc::new(stopped_application());
    let (release, gate) = oneshot::channel();
    let handler = Arc::new(BlockingHandler {
        gate: Mutex::new(Some(gate)),
        calls: Mutex::new(vec![]),
        nested: Some(application.clone()),
    });
    application.install_command_handler(handler.clone());
    let active_intent = CommandIntent::SwitchProfile {
        profile_id: "active".into(),
    };
    let active = test_execute_result_at_current_scope(&application, active_intent.clone());
    let close = application.close();
    pin!(active, close);
    assert!(matches!(poll!(active.as_mut()), Poll::Pending));
    assert!(matches!(poll!(close.as_mut()), Poll::Pending));
    let (id, terminal) = test_dispatch_at_current_scope(&application, CommandIntent::StartCore);
    assert!(
        matches!(terminal.recv().unwrap().result, CommandResult::Rejected { request_id, failure }
        if request_id == id && failure.code == ErrorCode::Canceled)
    );
    release.send(()).unwrap();
    assert!(matches!(active.await, CommandResult::Completed { .. }));
    close.await.unwrap();
    assert_eq!(*handler.calls.lock().unwrap(), vec![active_intent]);
    assert_eq!(Arc::strong_count(&handler), 1);
    assert!(
        CoreLifecyclePort::start(application.as_ref())
            .await
            .is_err()
    );
    assert!(
        matches!(test_execute_result_at_current_scope(&application, CommandIntent::RestartCore).await,
        CommandResult::Rejected { failure, .. } if failure.code == ErrorCode::Canceled)
    );
    assert_eq!(
        application.watchdog_tick().await.unwrap(),
        WatchdogTick::Healthy
    );
    assert_eq!(application.snapshot().lifecycle, CoreLifecycle::Stopped);
    assert_eq!(application.generation(), 1);
    application.close().await.unwrap();
}

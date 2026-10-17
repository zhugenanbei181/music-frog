use super::*;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tokio::sync::oneshot::channel;

#[test]
fn background_scheduler_has_one_slot_and_closes_after_its_task_leaves() {
    let runtime = Arc::new(TokioApplicationRuntime::new().expect("host runtime builds"));
    let (entered_tx, entered_rx) = mpsc::channel();
    let (release_tx, release_rx) = channel();
    let scheduled = runtime.spawn_background(Box::pin(async move {
        let _ = entered_tx.send(());
        let _ = release_rx.await;
    }));
    let ApplicationTaskAdmission::Scheduled { task_id } = scheduled else {
        panic!("first task must acquire the background slot: {scheduled:?}");
    };
    entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("background worker starts admitted task");
    assert_eq!(
        runtime.spawn_background(Box::pin(async {})),
        ApplicationTaskAdmission::AtCapacity {
            occupying_task_id: task_id,
        }
    );

    let shutdown_runtime = Arc::clone(&runtime);
    let (shutdown_tx, shutdown_rx) = mpsc::channel();
    let (shutdown_entered_tx, shutdown_entered_rx) = mpsc::channel();
    let shutdown_thread = thread::spawn(move || {
        let _ = shutdown_entered_tx.send(());
        let result = shutdown_runtime.shutdown_background_tasks();
        let _ = shutdown_tx.send(result);
    });
    shutdown_entered_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("shutdown caller starts");
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if matches!(
            *runtime
                .background_state
                .0
                .lock()
                .expect("background scheduler state lock"),
            BackgroundSchedulerState::Closing
        ) {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "shutdown enters its typed closing state"
        );
        thread::yield_now();
    }
    let _ = release_tx.send(());
    assert_eq!(
        shutdown_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("shutdown waits for task completion"),
        ApplicationTaskShutdown::Closed
    );
    shutdown_thread.join().expect("shutdown thread exits");
    assert_eq!(
        runtime.spawn_background(Box::pin(async {})),
        ApplicationTaskAdmission::Closed
    );
    assert_eq!(
        runtime.shutdown_background_tasks(),
        ApplicationTaskShutdown::AlreadyClosed
    );
}

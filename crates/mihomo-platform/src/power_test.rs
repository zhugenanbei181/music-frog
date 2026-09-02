use super::*;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::time::sleep;

#[tokio::test]
async fn test_power_event_variants_and_predicates() {
    let suspend = PowerEvent::Suspend;
    let resume = PowerEvent::Resume;
    let battery = PowerEvent::BatteryLow;
    let ac = PowerEvent::AcPowerChanged;
    let sleep_ev = PowerEvent::Sleep;
    let wake_ev = PowerEvent::Wake;
    let gap_ev = PowerEvent::TimerGapDetected { gap_ms: 4000 };
    let hibernate = PowerEvent::Hibernate;
    let screen_lock = PowerEvent::ScreenLocked;
    let screen_unlock = PowerEvent::ScreenUnlocked;

    assert!(suspend.is_suspend());
    assert!(sleep_ev.is_suspend());
    assert!(hibernate.is_suspend());
    assert!(!resume.is_suspend());

    assert!(resume.is_resume());
    assert!(wake_ev.is_resume());
    assert!(gap_ev.is_resume());
    assert!(!battery.is_resume());
    assert!(!ac.is_resume());
    assert!(!screen_lock.is_resume());
    assert!(!screen_unlock.is_resume());

    assert!(resume.requires_connection_reset());
    assert!(suspend.requires_connection_reset());
    assert!(!battery.requires_connection_reset());

    // Serde roundtrip
    let json = serde_json::to_string(&battery).unwrap();
    let parsed: PowerEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, PowerEvent::BatteryLow);
}

#[tokio::test]
async fn test_power_event_watcher_broadcast_and_emit() {
    let watcher = PowerEventWatcher::new();
    let mut rx = watcher.start();

    watcher.emit(PowerEvent::Resume).unwrap();
    let event = rx.recv().await.unwrap();
    assert_eq!(event, PowerEvent::Resume);

    watcher.emit(PowerEvent::BatteryLow).unwrap();
    let event2 = rx.recv().await.unwrap();
    assert_eq!(event2, PowerEvent::BatteryLow);

    watcher.emit(PowerEvent::AcPowerChanged).unwrap();
    let event3 = rx.recv().await.unwrap();
    assert_eq!(event3, PowerEvent::AcPowerChanged);

    watcher.stop();
}

#[tokio::test]
async fn test_self_healing_probe_success() {
    let probe_called = Arc::new(AtomicU32::new(0));
    let recovery_called = Arc::new(AtomicU32::new(0));

    let probe_called_clone = probe_called.clone();
    let probe_fn: ProbeFn = Arc::new(move || {
        probe_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { true })
    });

    let recovery_called_clone = recovery_called.clone();
    let recovery_fn: RecoveryFn = Arc::new(move || {
        recovery_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    });

    let controller = SelfHealingController::new(probe_fn, recovery_fn);
    let mut trigger_rx = controller.subscribe_triggers();
    let (tx, rx) = broadcast::channel(16);
    let (stop_tx, stop_rx) = watch::channel(false);

    let controller_clone = Arc::new(controller);
    let controller_task = controller_clone.clone();

    let handle = tokio::spawn(async move {
        controller_task.run(rx, stop_rx).await;
    });

    tx.send(PowerEvent::Resume).unwrap();
    sleep(Duration::from_millis(50)).await;

    assert_eq!(probe_called.load(Ordering::SeqCst), 1);
    assert_eq!(recovery_called.load(Ordering::SeqCst), 0);
    assert!(trigger_rx.try_recv().is_err());

    let stats = controller_clone.get_stats().await;
    assert_eq!(stats.resumes_detected, 1);
    assert_eq!(stats.probes_succeeded, 1);

    stop_tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn test_self_healing_with_connection_reset_hook() {
    let reset_called = Arc::new(AtomicU32::new(0));
    let reset_called_clone = reset_called.clone();
    let reset_fn: ResetConnectionsFn = Arc::new(move || {
        reset_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    });

    let probe_fn: ProbeFn = Arc::new(|| Box::pin(async { true }));
    let recovery_fn: RecoveryFn = Arc::new(|| Box::pin(async {}));

    let controller =
        SelfHealingController::new(probe_fn, recovery_fn).with_connection_reset(reset_fn);

    let (tx, rx) = broadcast::channel(16);
    let (stop_tx, stop_rx) = watch::channel(false);

    let controller_arc = Arc::new(controller);
    let controller_task = controller_arc.clone();

    let handle = tokio::spawn(async move {
        controller_task.run(rx, stop_rx).await;
    });

    tx.send(PowerEvent::Wake).unwrap();
    sleep(Duration::from_millis(50)).await;

    assert_eq!(reset_called.load(Ordering::SeqCst), 1);
    let stats = controller_arc.get_stats().await;
    assert_eq!(stats.resets_triggered, 1);

    stop_tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn test_self_healing_probe_failure_emits_trigger() {
    let probe_called = Arc::new(AtomicU32::new(0));
    let recovery_called = Arc::new(AtomicU32::new(0));

    let probe_called_clone = probe_called.clone();
    let probe_fn: ProbeFn = Arc::new(move || {
        probe_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { false })
    });

    let recovery_called_clone = recovery_called.clone();
    let recovery_fn: RecoveryFn = Arc::new(move || {
        recovery_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    });

    let controller = SelfHealingController::new(probe_fn, recovery_fn);
    let mut trigger_rx = controller.subscribe_triggers();
    let (tx, rx) = broadcast::channel(16);
    let (stop_tx, stop_rx) = watch::channel(false);

    let controller_clone = Arc::new(controller);
    let controller_task = controller_clone.clone();

    let handle = tokio::spawn(async move {
        controller_task.run(rx, stop_rx).await;
    });

    tx.send(PowerEvent::Resume).unwrap();
    sleep(Duration::from_millis(50)).await;

    assert_eq!(probe_called.load(Ordering::SeqCst), 1);
    assert_eq!(recovery_called.load(Ordering::SeqCst), 1);

    let trigger = trigger_rx.recv().await.unwrap();
    assert!(trigger.reason.contains("unresponsive"));
    assert!(trigger.timestamp_secs > 0);

    stop_tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn test_self_healing_probe_timeout() {
    let probe_called = Arc::new(AtomicU32::new(0));
    let recovery_called = Arc::new(AtomicU32::new(0));

    let probe_called_clone = probe_called.clone();
    let probe_fn: ProbeFn = Arc::new(move || {
        probe_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {
            sleep(Duration::from_millis(150)).await;
            true
        })
    });

    let recovery_called_clone = recovery_called.clone();
    let recovery_fn: RecoveryFn = Arc::new(move || {
        recovery_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    });

    let controller =
        SelfHealingController::new_with_timeout(probe_fn, recovery_fn, Duration::from_millis(40));
    let mut trigger_rx = controller.subscribe_triggers();
    let (tx, rx) = broadcast::channel(16);
    let (stop_tx, stop_rx) = watch::channel(false);

    let handle = tokio::spawn(async move {
        controller.run(rx, stop_rx).await;
    });

    sleep(Duration::from_millis(10)).await;
    tx.send(PowerEvent::Wake).unwrap();

    sleep(Duration::from_millis(120)).await;

    assert_eq!(probe_called.load(Ordering::SeqCst), 1);
    assert_eq!(recovery_called.load(Ordering::SeqCst), 1);

    let trigger = trigger_rx.recv().await.unwrap();
    assert!(trigger.reason.contains("unresponsive"));

    stop_tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn test_debounce_behavior() {
    let probe_called = Arc::new(AtomicU32::new(0));
    let recovery_called = Arc::new(AtomicU32::new(0));

    let probe_called_clone = probe_called.clone();
    let probe_fn: ProbeFn = Arc::new(move || {
        probe_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { true })
    });

    let recovery_called_clone = recovery_called.clone();
    let recovery_fn: RecoveryFn = Arc::new(move || {
        recovery_called_clone.fetch_add(1, Ordering::SeqCst);
        Box::pin(async {})
    });

    // Use 100ms debounce for deterministic testing
    let controller = SelfHealingController::new_with_config(
        probe_fn,
        recovery_fn,
        Duration::from_millis(100),
        Duration::from_millis(50),
    );
    let (tx, rx) = broadcast::channel(16);
    let (stop_tx, stop_rx) = watch::channel(false);

    let controller_clone = Arc::new(controller);
    let controller_task = controller_clone.clone();

    let handle = tokio::spawn(async move {
        controller_task.run(rx, stop_rx).await;
    });

    // Burst send multiple resume events
    tx.send(PowerEvent::Resume).unwrap();
    tx.send(PowerEvent::Wake).unwrap();
    tx.send(PowerEvent::TimerGapDetected { gap_ms: 3500 })
        .unwrap();

    sleep(Duration::from_millis(30)).await;

    // Only 1 probe should have executed due to debounce
    assert_eq!(probe_called.load(Ordering::SeqCst), 1);

    // Sleep past debounce window (100ms)
    sleep(Duration::from_millis(120)).await;

    // Send another resume event
    tx.send(PowerEvent::Resume).unwrap();
    sleep(Duration::from_millis(30)).await;

    // Should now be probed again
    assert_eq!(probe_called.load(Ordering::SeqCst), 2);

    stop_tx.send(true).unwrap();
    handle.await.unwrap();
}

#[tokio::test]
async fn test_five_tier_pipeline_happy_path() {
    let rst_called = Arc::new(AtomicU32::new(0));
    let probe_called = Arc::new(AtomicU32::new(0));
    let retest_called = Arc::new(AtomicU32::new(0));
    let reload_called = Arc::new(AtomicU32::new(0));
    let respawn_called = Arc::new(AtomicU32::new(0));

    let r_c = rst_called.clone();
    let p_c = probe_called.clone();
    let ret_c = retest_called.clone();
    let rel_c = reload_called.clone();
    let res_c = respawn_called.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_zombie_purge(Arc::new(move || {
            r_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }))
        .with_fake_ip_probe(Arc::new(move || {
            p_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(true) })
        }))
        .with_node_retest(Arc::new(move || {
            ret_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(5) }) // 5 alive nodes
        }))
        .with_config_reload(Arc::new(move || {
            rel_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }))
        .with_process_respawn(Arc::new(move || {
            res_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("System wake test").await;

    assert!(report.success);
    assert!(!report.safe_mode_tripped);
    assert_eq!(
        report.highest_tier_reached,
        SelfHealingTier::Tier3NodeDelayRetest
    );
    assert_eq!(rst_called.load(Ordering::SeqCst), 1);
    assert_eq!(probe_called.load(Ordering::SeqCst), 1);
    assert_eq!(retest_called.load(Ordering::SeqCst), 1);
    assert_eq!(reload_called.load(Ordering::SeqCst), 0);
    assert_eq!(respawn_called.load(Ordering::SeqCst), 0);
    assert_eq!(pipeline.consecutive_failures(), 0);
}

#[tokio::test]
async fn test_five_tier_pipeline_fake_ip_probe_failure_escalates_to_tier4() {
    let probe_called = Arc::new(AtomicU32::new(0));
    let reload_called = Arc::new(AtomicU32::new(0));
    let respawn_called = Arc::new(AtomicU32::new(0));

    let p_c = probe_called.clone();
    let rel_c = reload_called.clone();
    let res_c = respawn_called.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_fake_ip_probe(Arc::new(move || {
            let call = p_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if call == 0 {
                    Ok(false) // First probe fails
                } else {
                    Ok(true) // Probe after reload succeeds
                }
            })
        }))
        .with_config_reload(Arc::new(move || {
            rel_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }))
        .with_process_respawn(Arc::new(move || {
            res_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("Wake with dead Fake-IP").await;

    assert!(report.success);
    assert_eq!(
        report.highest_tier_reached,
        SelfHealingTier::Tier4ConfigReload
    );
    assert_eq!(probe_called.load(Ordering::SeqCst), 2);
    assert_eq!(reload_called.load(Ordering::SeqCst), 1);
    assert_eq!(respawn_called.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn test_five_tier_pipeline_node_retest_zero_escalates_to_tier4() {
    let retest_called = Arc::new(AtomicU32::new(0));
    let reload_called = Arc::new(AtomicU32::new(0));

    let ret_c = retest_called.clone();
    let rel_c = reload_called.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_fake_ip_probe(Arc::new(|| Box::pin(async { Ok(true) })))
        .with_node_retest(Arc::new(move || {
            ret_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(0) }) // 0 alive nodes
        }))
        .with_config_reload(Arc::new(move || {
            rel_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("Wake with all nodes dead").await;

    assert!(report.success);
    assert_eq!(
        report.highest_tier_reached,
        SelfHealingTier::Tier4ConfigReload
    );
    assert_eq!(retest_called.load(Ordering::SeqCst), 1);
    assert_eq!(reload_called.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_five_tier_pipeline_tier4_failure_escalates_to_tier5() {
    let reload_called = Arc::new(AtomicU32::new(0));
    let respawn_called = Arc::new(AtomicU32::new(0));

    let rel_c = reload_called.clone();
    let res_c = respawn_called.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_fake_ip_probe(Arc::new(|| Box::pin(async { Ok(false) })))
        .with_config_reload(Arc::new(move || {
            rel_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Err("Config reload API connection refused".to_string()) })
        }))
        .with_process_respawn(Arc::new(move || {
            res_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    let report = pipeline.execute("Wake with dead daemon process").await;

    assert!(report.success);
    assert_eq!(
        report.highest_tier_reached,
        SelfHealingTier::Tier5ProcessRespawnAndSafeMode
    );
    assert_eq!(reload_called.load(Ordering::SeqCst), 1);
    assert_eq!(respawn_called.load(Ordering::SeqCst), 1);
    assert_eq!(pipeline.consecutive_failures(), 1);
}

#[tokio::test]
async fn test_five_tier_pipeline_safe_mode_trip_after_consecutive_failures() {
    let respawn_called = Arc::new(AtomicU32::new(0));
    let res_c = respawn_called.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_safe_mode_threshold(3)
        .with_fake_ip_probe(Arc::new(|| Box::pin(async { Ok(false) })))
        .with_config_reload(Arc::new(|| Box::pin(async { Err("Failed".to_string()) })))
        .with_process_respawn(Arc::new(move || {
            res_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }));

    // Attempt 1
    let r1 = pipeline.execute("Wake 1").await;
    assert!(r1.success);
    assert!(!r1.safe_mode_tripped);
    assert_eq!(pipeline.consecutive_failures(), 1);

    // Attempt 2
    let r2 = pipeline.execute("Wake 2").await;
    assert!(r2.success);
    assert!(!r2.safe_mode_tripped);
    assert_eq!(pipeline.consecutive_failures(), 2);

    // Attempt 3 -> trips safe mode
    let r3 = pipeline.execute("Wake 3").await;
    assert!(!r3.success);
    assert!(r3.safe_mode_tripped);
    assert_eq!(pipeline.consecutive_failures(), 3);
    assert_eq!(respawn_called.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_self_healing_controller_with_five_tier_pipeline() {
    let rst_called = Arc::new(AtomicU32::new(0));
    let probe_called = Arc::new(AtomicU32::new(0));

    let r_c = rst_called.clone();
    let p_c = probe_called.clone();

    let pipeline = SelfHealingPipeline::new()
        .with_zombie_purge(Arc::new(move || {
            r_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }))
        .with_fake_ip_probe(Arc::new(move || {
            p_c.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(true) })
        }));

    let controller = SelfHealingController::new(
        Arc::new(|| Box::pin(async { true })),
        Arc::new(|| Box::pin(async {})),
    )
    .with_pipeline(pipeline);

    let mut report_rx = controller.subscribe_reports();
    let (tx, rx) = broadcast::channel(16);
    let (stop_tx, stop_rx) = watch::channel(false);

    let controller_arc = Arc::new(controller);
    let controller_task = controller_arc.clone();

    let handle = tokio::spawn(async move {
        controller_task.run(rx, stop_rx).await;
    });

    tx.send(PowerEvent::Resume).unwrap();
    sleep(Duration::from_millis(50)).await;

    let report = report_rx.recv().await.unwrap();
    assert!(report.success);
    assert_eq!(rst_called.load(Ordering::SeqCst), 1);
    assert_eq!(probe_called.load(Ordering::SeqCst), 1);

    let stats = controller_arc.get_stats().await;
    assert_eq!(stats.resumes_detected, 1);
    assert_eq!(stats.zombie_purges_executed, 1);
    assert_eq!(stats.fake_ip_probes_succeeded, 1);

    stop_tx.send(true).unwrap();
    handle.await.unwrap();
}

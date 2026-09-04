use super::*;
use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

#[test]
fn test_new_report_and_full_report() {
    init_process_timer();
    let report = CrashReporter::new_report(
        "Test panic",
        "v1.0.0",
        Some("Stack trace line 1\nStack trace line 2"),
    );
    assert!(report.timestamp_secs > 0);
    assert_eq!(report.os_info, std::env::consts::OS);
    assert_eq!(report.panic_reason, "Test panic");
    assert_eq!(report.client_version, "v1.0.0");
    assert_eq!(report.core_version, Some("v1.0.0".to_string()));
    assert_eq!(
        report.backtrace_summary.as_deref(),
        Some("Stack trace line 1\nStack trace line 2")
    );

    let full_report = CrashReporter::new_full_report(
        "Fatal error",
        "v1.2.3",
        Some("core-v2.0"),
        120,
        Some("SIGTERM"),
        None,
    );
    assert_eq!(full_report.core_version, Some("core-v2.0".to_string()));
    assert_eq!(full_report.uptime_secs, 120);
    assert_eq!(full_report.fatal_signal, Some("SIGTERM".to_string()));

    let fp = full_report.fingerprint();
    assert!(!fp.is_empty());
}

#[test]
fn test_new_signal_report() {
    let sig_report = CrashReporter::new_signal_report("SIGSEGV", "v1.0.0", Some("core-v1"));
    assert_eq!(sig_report.fatal_signal, Some("SIGSEGV".to_string()));
    assert!(sig_report.panic_reason.contains("SIGSEGV"));
}

#[test]
fn test_sanitize_report_tokens_and_secrets() {
    let mut report = CrashReporter::new_report(
        "Failed with token Bearer abcdef123456, bearer xyz_789, token=secretToken123, password=myPassword! and apiKey=key456",
        "v1.0.0",
        Some("Error in https://admin:supersecret@example.com/api?token=abc"),
    );
    CrashReporter::sanitize_report(&mut report);

    assert!(report.sanitized);
    assert!(!report.panic_reason.contains("abcdef123456"));
    assert!(!report.panic_reason.contains("xyz_789"));
    assert!(!report.panic_reason.contains("secretToken123"));
    assert!(!report.panic_reason.contains("myPassword!"));
    assert!(!report.panic_reason.contains("key456"));
    assert!(report.panic_reason.contains("<REDACTED_TOKEN>"));
    assert!(report.panic_reason.contains("<REDACTED_SECRET>"));

    let bt = report.backtrace_summary.unwrap();
    assert!(!bt.contains("supersecret"));
    assert!(bt.contains("<REDACTED_USER>:<REDACTED_PASS>@"));
}

#[test]
fn test_sanitize_paths_and_jwt() {
    let mut report = CrashReporter::new_report(
        "File not found: /home/username/.config/mihomo and C:\\Users\\Admin\\AppData and /Users/developer/Library with eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
        "v1.0.0",
        None,
    );
    CrashReporter::sanitize_report(&mut report);
    assert!(report.panic_reason.contains("<REDACTED_HOME>"));
    assert!(report.panic_reason.contains("<REDACTED_JWT>"));
}

#[test]
fn test_serialization_roundtrip() {
    let report =
        CrashReporter::new_full_report("Test panic", "v1.0.0", Some("v1.0.0"), 42, None, None);
    let serialized = CrashReporter::serialize_report(&report).unwrap();
    let deserialized = CrashReporter::parse_report(&serialized).unwrap();
    assert_eq!(report, deserialized);
}

#[test]
fn test_save_crash_dump_creates_file_and_rotates() {
    let temp_dir = tempfile::tempdir().unwrap();
    let report = CrashReporter::new_report("Panic test in save", "v1.0.0", None);

    let saved_path = CrashReporter::save_crash_dump(&report, Some(temp_dir.path())).unwrap();

    assert!(saved_path.exists());
    assert!(
        saved_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("crash_")
    );
    assert!(
        saved_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".json")
    );

    let content = std::fs::read_to_string(&saved_path).unwrap();
    let parsed = CrashReporter::parse_report(&content).unwrap();
    assert_eq!(parsed.panic_reason, "Panic test in save");
    assert!(parsed.sanitized);

    // Rotation check
    let reports_dir = temp_dir.path().join("crash_reports");
    let rotated = CrashReporter::rotate_crash_dumps(&reports_dir, 0).unwrap();
    assert_eq!(rotated, 1);
}

#[test]
fn test_clean_exit_hook_registration_and_execution() {
    CleanExitHook::reset_for_tests();

    let proxy_restored = Arc::new(AtomicU32::new(0));
    let tun_restored = Arc::new(AtomicU32::new(0));

    let proxy_clone = proxy_restored.clone();
    CleanExitHook::register_proxy_restore(move || {
        proxy_clone.fetch_add(1, Ordering::SeqCst);
    });

    let tun_clone = tun_restored.clone();
    CleanExitHook::register_tun_route_restore(move || {
        tun_clone.fetch_add(1, Ordering::SeqCst);
    });

    CleanExitHook::register_cleanup("faulty_hook", || {
        panic!("Intentional test panic in cleanup");
    });

    assert!(!CleanExitHook::is_cleanup_performed());

    CleanExitHook::run_emergency_cleanup();

    assert!(CleanExitHook::is_cleanup_performed());
    assert_eq!(proxy_restored.load(Ordering::SeqCst), 1);
    assert_eq!(tun_restored.load(Ordering::SeqCst), 1);

    CleanExitHook::reset_for_tests();
}

#[test]
fn test_dns_crash_watchdog_lifecycle_and_orphaned_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    // Create active sentinel for a dead PID (e.g. PID 99999999)
    let dead_pid = 99999999;
    let sentinel = DnsStateSentinel::new(
        dead_pid,
        vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()],
        vec!["127.0.0.1".to_string()],
        Some("127.0.0.1:7890".to_string()),
    );

    let path = DnsCrashWatchdog::write_sentinel(home, &sentinel).unwrap();
    assert!(path.exists());

    let read = DnsCrashWatchdog::read_sentinel(home).unwrap().unwrap();
    assert_eq!(read.daemon_pid, dead_pid);

    // Touch heartbeat
    DnsCrashWatchdog::touch_heartbeat(home).unwrap();

    // Should detect orphaned state because PID 99999999 is not alive
    let orphaned = DnsCrashWatchdog::check_orphaned_state(home);
    assert!(orphaned.is_some());

    let dns_restored = Arc::new(AtomicU32::new(0));
    let proxy_restored = Arc::new(AtomicU32::new(0));

    let d_clone = dns_restored.clone();
    let p_clone = proxy_restored.clone();

    let recovery = DnsCrashWatchdog::recover_orphaned_state(
        home,
        move |dns| {
            assert_eq!(dns, &["1.1.1.1", "8.8.8.8"]);
            d_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
        move |proxy| {
            assert_eq!(proxy, Some("127.0.0.1:7890"));
            p_clone.fetch_add(1, Ordering::SeqCst);
            Ok(())
        },
    );

    assert!(recovery.is_some());
    assert_eq!(dns_restored.load(Ordering::SeqCst), 1);
    assert_eq!(proxy_restored.load(Ordering::SeqCst), 1);

    // Sentinel should be removed after recovery
    assert!(DnsCrashWatchdog::read_sentinel(home).unwrap().is_none());
}

#[test]
fn test_dns_state_sentinel_builder_and_expiration() {
    let mut sentinel = DnsStateSentinel::new(
        12345,
        vec!["1.1.1.1".to_string()],
        vec!["127.0.0.1".to_string()],
        Some("127.0.0.1:7890".to_string()),
    )
    .with_interface("eth0")
    .with_heartbeat_timeout(10)
    .with_current_proxy(Some("127.0.0.1:7890".to_string()))
    .with_meta("key1", "val1");

    assert_eq!(sentinel.daemon_pid, 12345);
    assert_eq!(sentinel.interface_name.as_deref(), Some("eth0"));
    assert_eq!(sentinel.heartbeat_timeout_secs, 10);
    assert_eq!(
        sentinel.system_proxy_current.as_deref(),
        Some("127.0.0.1:7890")
    );
    assert_eq!(
        sentinel.metadata.get("key1").map(String::as_str),
        Some("val1")
    );
    assert!(sentinel.is_active);

    let now = sentinel.heartbeat_secs + 5;
    assert!(!sentinel.is_heartbeat_expired(now));

    let expired_now = sentinel.heartbeat_secs + 15;
    assert!(sentinel.is_heartbeat_expired(expired_now));

    sentinel.touch_heartbeat();
    assert!(!sentinel.is_heartbeat_expired(sentinel.heartbeat_secs + 5));

    sentinel.mark_inactive();
    assert!(!sentinel.is_active);
    assert!(!sentinel.is_heartbeat_expired(sentinel.heartbeat_secs + 100));
}

#[tokio::test]
async fn test_standalone_dns_watchdog_loop_and_commands() {
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path();

    let dead_pid = 99999998;
    let sentinel = DnsStateSentinel::new(
        dead_pid,
        vec!["8.8.8.8".to_string(), "8.8.4.4".to_string()],
        vec!["127.0.0.1".to_string()],
        None,
    )
    .with_interface("wlan0");

    DnsCrashWatchdog::write_sentinel(home, &sentinel).unwrap();

    let watchdog = StandaloneDnsWatchdog::new(home.to_path_buf())
        .with_poll_interval(Duration::from_millis(50))
        .with_heartbeat_timeout(5);

    let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
    let dns_called = Arc::new(AtomicU32::new(0));
    let proxy_called = Arc::new(AtomicU32::new(0));

    let d_clone = dns_called.clone();
    let p_clone = proxy_called.clone();

    let handle = tokio::spawn(async move {
        watchdog
            .run_loop(
                move |_dns| {
                    d_clone.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
                move |_proxy| {
                    p_clone.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                },
                stop_rx,
            )
            .await
    });

    tokio::time::sleep(Duration::from_millis(150)).await;
    stop_tx.send(true).unwrap();

    let recoveries = handle.await.unwrap();
    assert_eq!(recoveries, 1);
    assert_eq!(dns_called.load(Ordering::SeqCst), 1);

    // Test platform commands generator
    let cmds = StandaloneDnsWatchdog::generate_platform_dns_restore_commands(
        Some("eth0"),
        &["1.1.1.1".to_string(), "1.0.0.1".to_string()],
    );
    assert!(!cmds.is_empty());
}

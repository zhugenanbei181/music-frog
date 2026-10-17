use super::process;
use infiltrator_platform::TEST_LOCK;
#[cfg(all(test, target_os = "linux"))]
use infiltrator_platform::paths::clear_home_dir_override;
#[cfg(all(test, target_os = "linux"))]
use infiltrator_platform::paths::set_home_dir_override;
use lurker_core_ports::core_process::CoreProcess;
#[cfg(all(test, target_os = "linux"))]
use std::env::{current_exe, split_paths, var_os};
#[cfg(all(test, target_os = "linux"))]
use std::fs::Permissions;
use std::fs::canonicalize;
#[cfg(all(test, target_os = "linux"))]
#[cfg(target_os = "linux")]
use std::fs::read_link;
#[cfg(all(test, target_os = "linux"))]
use std::fs::set_permissions;
#[cfg(all(test, target_os = "linux"))]
use std::fs::write;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
#[cfg(all(test, target_os = "linux"))]
use std::process::id;
use std::process::{Child, Command};
#[cfg(all(test, target_os = "linux"))]
use std::ptr::null_mut;
#[cfg(all(test, target_os = "linux"))]
use std::time;
use tokio::task::yield_now;
use tokio::time::sleep;
use tokio::time::timeout;

/// Happy path: with a correct parent pid, the pre_exec closure arms
/// PR_SET_PDEATHSIG successfully and lets exec proceed, so the spawned
/// `sh` runs to completion with exit code 0.
#[test]
fn parent_death_signal_allows_normal_spawn() {
    let parent = id();
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("exit 0");
    // SAFETY: closure only runs async-signal-safe libc calls in the child.
    unsafe {
        cmd.pre_exec(process::parent_death_signal(parent));
    }
    let status = cmd.status().expect("failed to spawn sh");
    assert!(status.success(), "expected exit 0, got: {status}");
}

/// Race-guard path: with a parent pid that can never match (u32::MAX),
/// the closure must `_exit(1)` inside the child before exec, so `sh` never
/// runs and the child reports exit code 1.
#[test]
fn parent_death_signal_exits_when_parent_pid_mismatches() {
    let impossible_parent = u32::MAX;
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("exit 0");
    // SAFETY: closure only runs async-signal-safe libc calls in the child.
    unsafe {
        cmd.pre_exec(process::parent_death_signal(impossible_parent));
    }
    let status = cmd.status().expect("failed to spawn sh");
    assert_eq!(status.code(), Some(1), "expected child _exit(1): {status}");
}

/// End-to-end: `spawn_daemon` (the real spawn path with the pre_exec
/// wiring) starts a long-running fake core, reports a live pid, and
/// `kill_process` still cleans it up. Runs under the shared TEST_LOCK
/// because `spawn_daemon` resolves the log dir via the global HOME
/// override.
#[tokio::test]
async fn spawn_daemon_runs_fake_core_with_death_signal() {
    use std::os::unix::fs::PermissionsExt;

    let _guard = TEST_LOCK.lock().await;
    let dir = tempfile::tempdir().expect("tempdir");

    // Fake core: a long-running executable so spawn_daemon's liveness
    // checks see a running process.
    let script = dir.path().join("fake-mihomo");
    write(&script, "#!/bin/sh\nwhile :; do sleep 60; done\n").expect("write fake core");
    set_permissions(&script, Permissions::from_mode(0o755)).expect("chmod fake core");

    let config = dir.path().join("config.yaml");
    write(&config, "mixed-port: 7890\n").expect("write config");

    set_home_dir_override(dir.path().to_path_buf());
    let spawned = process::spawn_daemon(&script, &config).await;
    clear_home_dir_override();

    let spawned = spawned.expect("spawn_daemon should succeed");
    assert!(
        process::is_process_alive(spawned.pid),
        "core should be alive"
    );

    process::kill_process(spawned.pid).expect("kill fake core");

    // spawn_daemon intentionally never waits on the child (detached
    // daemon), so the SIGKILLed core lingers as a zombie that sysinfo
    // still counts as alive until this test reaps it. Reaping also proves
    // the kill landed.
    let mut reaped = false;
    for _ in 0..20 {
        // SAFETY: the fake core is a direct child of this test process.
        let rc = unsafe { libc::waitpid(spawned.pid as i32, null_mut(), libc::WNOHANG) };
        if rc == spawned.pid as i32 || rc == -1 {
            reaped = true;
            break;
        }
        sleep(time::Duration::from_millis(100)).await;
    }
    assert!(reaped, "fake core was not reaped after kill");
    assert!(
        !process::is_process_alive(spawned.pid),
        "killed core should no longer be listed as alive"
    );
}

/// The pid file round-trip used by start/stop keeps working alongside the
/// death-signal wiring.
#[tokio::test]
async fn pid_file_round_trip() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pid_file = dir.path().join("mihomo.pid");

    process::write_pid_file(&pid_file, 4242)
        .await
        .expect("write");
    let pid = process::read_pid_file(&pid_file).await.expect("read");
    assert_eq!(pid, 4242);
    process::remove_pid_file(&pid_file).await.expect("remove");
    assert!(process::read_pid_file(&pid_file).await.is_err());
}

/// An assertion failure still reaps the exact child created by this fixture.
struct OwnedTestChild(Child);
impl Drop for OwnedTestChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Resolve the requested executable before spawn and await its actual exec identity.
/// Sampling `/proc/exe` immediately after spawn can observe the pre-exec image.
async fn spawn_test_sleep() -> (OwnedTestChild, PathBuf) {
    let executable = split_paths(&var_os("PATH").expect("test executable path"))
        .map(|directory| directory.join("sleep"))
        .find(|candidate| candidate.is_file())
        .expect("sleep fixture executable");
    let binary = canonicalize(&executable).expect("canonical fixture executable");
    let mut child = OwnedTestChild(
        Command::new(executable)
            .arg("60")
            .spawn()
            .expect("spawn sleep fixture"),
    );
    let pid = child.0.id();
    timeout(time::Duration::from_secs(5), async {
        loop {
            if process::process_matches_binary(pid, &binary) {
                break;
            }
            assert!(
                child.0.try_wait().expect("child status").is_none(),
                "fixture exited before exec readiness"
            );
            yield_now().await;
        }
    })
    .await
    .expect("fixture exec readiness");
    (child, binary)
}

#[tokio::test]
async fn stop_waits_for_owned_process_exit_before_removing_pid_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut child, binary) = spawn_test_sleep().await;
    let pid = child.0.id();
    let pid_file = dir.path().join("mihomo.pid");
    process::write_pid_file(&pid_file, pid)
        .await
        .expect("write process pid");
    let controller = super::ProcessCoreController::with_pid_file(
        binary,
        dir.path().join("config.yaml"),
        pid_file.clone(),
    );
    *controller.owned_pid.lock().expect("owned pid lock") = Some(pid);

    CoreProcess::stop(&controller)
        .await
        .expect("stop waits until the process is no longer alive");

    assert!(!process::is_process_alive(pid));
    assert!(!pid_file.exists());
    assert_eq!(*controller.owned_pid.lock().expect("owned pid lock"), None);
    let _ = child.0.wait();
}

#[tokio::test]
async fn stop_confirms_an_owned_process_that_exited_before_close() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut child, binary) = spawn_test_sleep().await;
    let pid = child.0.id();
    let pid_file = dir.path().join("mihomo.pid");
    process::write_pid_file(&pid_file, pid)
        .await
        .expect("write process pid");
    let controller = super::ProcessCoreController::with_pid_file(
        binary,
        dir.path().join("config.yaml"),
        pid_file.clone(),
    );
    *controller.owned_pid.lock().expect("owned pid lock") = Some(pid);

    child.0.kill().expect("simulate an unexpected process exit");
    let _ = child.0.wait();
    CoreProcess::stop(&controller)
        .await
        .expect("already exited owned process is a terminal stop");

    assert!(!pid_file.exists());
    assert_eq!(*controller.owned_pid.lock().expect("owned pid lock"), None);
}

/// A new host instance must reclaim a live core record left by a crashed
/// instance, but only after verifying that the PID belongs to the expected
/// executable. This is the non-Linux fallback for the kernel death signal.
#[cfg(target_os = "linux")]
#[tokio::test]
async fn start_admission_reclaims_a_live_expected_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut child, binary) = spawn_test_sleep().await;
    let pid = child.0.id();
    let pid_file = dir.path().join("mihomo.pid");
    process::write_pid_file(&pid_file, pid)
        .await
        .expect("write orphan pid");

    let controller = super::ProcessCoreController::with_pid_file(
        binary.clone(),
        dir.path().join("config.yaml"),
        pid_file.clone(),
    );
    let reclaimed = controller
        .reconcile_stale_pid_record()
        .await
        .unwrap_or_else(|error| {
            panic!(
                "cleanup orphan: {error}; expected={binary:?}; actual={:?}",
                read_link(format!("/proc/{pid}/exe"))
            )
        });
    assert_eq!(
        reclaimed,
        super::PidRecordReconciliation::StoppedOrphan { pid }
    );
    let _ = child.0.wait();
    assert!(!pid_file.exists());
    assert!(!process::is_process_alive(pid));
}

#[tokio::test]
async fn start_admission_removes_a_dead_pid_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pid_file = dir.path().join("mihomo.pid");
    process::write_pid_file(&pid_file, u32::MAX)
        .await
        .expect("write stale pid");
    let controller = super::ProcessCoreController::with_pid_file(
        dir.path().join("mihomo"),
        dir.path().join("config.yaml"),
        pid_file.clone(),
    );
    assert_eq!(
        controller
            .reconcile_stale_pid_record()
            .await
            .expect("reconcile stale pid"),
        super::PidRecordReconciliation::RemovedDeadRecord { pid: u32::MAX }
    );
    assert!(!pid_file.exists());
}

#[tokio::test]
async fn start_admission_refuses_an_unrelated_live_process() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (mut child, _) = spawn_test_sleep().await;
    let pid = child.0.id();
    let pid_file = dir.path().join("mihomo.pid");
    process::write_pid_file(&pid_file, pid)
        .await
        .expect("write unrelated pid");
    let controller = super::ProcessCoreController::with_pid_file(
        current_exe().expect("test executable"),
        dir.path().join("config.yaml"),
        pid_file.clone(),
    );
    let stop_error = CoreProcess::stop(&controller)
        .await
        .expect_err("stop must refuse an unrelated process record");
    assert!(stop_error.to_string().contains("unrelated process"));
    assert!(process::is_process_alive(pid));

    let error = controller
        .reconcile_stale_pid_record()
        .await
        .expect_err("unrelated process must be refused");
    assert!(error.to_string().contains("unrelated process"));
    assert!(process::is_process_alive(pid));
    child.0.kill().expect("kill unrelated process");
    let _ = child.0.wait();
    let _ = process::remove_pid_file(&pid_file).await;
}

/// Compile-time guarantee that the pre_exec closure satisfies the
/// `FnMut() + Send + Sync + 'static` bounds required by `CommandExt::pre_exec`.
#[test]
fn parent_death_signal_closure_is_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>(_: &T) {}
    let closure = process::parent_death_signal(id());
    assert_send_sync(&closure);
}

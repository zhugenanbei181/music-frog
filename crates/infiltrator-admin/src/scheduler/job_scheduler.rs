//! Admin host periodic job scheduler (FUNC-006).
//!
//! [`JobScheduler`] owns named periodic jobs that trigger application use
//! cases (subscription updates, sync, and future admin maintenance). The
//! scheduler is host infrastructure; it does not own business state. Each
//! job:
//!
//! - fires immediately on registration and then once per `interval`
//!   (tokio's first interval tick completes at once);
//! - runs sequentially: the loop waits for the next tick only after the
//!   previous run's future completed, so every job is single-flight by
//!   construction — two runs of the same job can never overlap;
//! - shuts down through a [`tokio::sync::watch`] channel instead of a
//!   cancellation-token type (no `tokio-util` dependency).
//!   [`JobScheduler::cancel`] additionally aborts the task handle as a
//!   backstop: jobs are expected to be short, so aborting a run in flight
//!   is an accepted trade-off, and dropping the scheduler closes every
//!   watch channel so remaining loops wind down on their next poll;
//! - exposes its counters through [`JobScheduler::snapshot`].
//!
//! Jobs are keyed by name. Spawning a job whose name is already registered
//! stops the previous one and installs the new one; [`Spawned::replaced`]
//! reports whether that happened.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::MissedTickBehavior;

/// Mutable runtime counters of one registered job.
#[derive(Default)]
struct JobState {
    run_count: u64,
    failure_count: u64,
    last_error: Option<String>,
}

/// Registry entry for one job: shutdown signal, task handle, and counters.
struct JobEntry {
    shutdown_tx: watch::Sender<bool>,
    handle: JoinHandle<()>,
    state: Arc<Mutex<JobState>>,
}

impl JobEntry {
    /// Stop the job: flip the shutdown flag so the loop exits cleanly at
    /// its next poll, then abort the handle so termination is guaranteed
    /// even while a run is in flight.
    fn stop(self) {
        let _ = self.shutdown_tx.send(true);
        self.handle.abort();
    }
}

/// Point-in-time status of one registered job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSnapshot {
    /// Registered job name.
    pub name: String,
    /// Always `true` for entries reachable from the registry: a canceled
    /// job's entry is removed eagerly, so presence implies active. The
    /// field is kept so a future "stopping" intermediate state can be
    /// reported without changing the shape of this struct.
    pub active: bool,
    /// Number of runs started, including runs that later failed.
    pub run_count: u64,
    /// Number of runs that ended with `Err`.
    pub failure_count: u64,
    /// Error message of the most recent failed run; cleared on success.
    pub last_error: Option<String>,
}

/// Result of [`JobScheduler::spawn_job`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spawned {
    /// `true` when an existing job of the same name was stopped and
    /// replaced by the new registration.
    pub replaced: bool,
}

/// Registry of named periodic jobs.
///
/// All methods take `&self`: the registry is guarded by a
/// `std::sync::Mutex` and each job's counters by a per-job mutex, so a
/// scheduler is cheaply shareable and never holds a lock across an `.await`.
pub struct JobScheduler {
    jobs: Mutex<HashMap<String, JobEntry>>,
}

impl JobScheduler {
    /// Create an empty scheduler.
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
        }
    }

    /// Register and start a periodic job.
    ///
    /// The first run happens immediately (tokio's first interval tick
    /// completes at once), afterwards once per `interval`. Runs of one job
    /// are strictly sequential — the loop starts waiting for the next tick
    /// only after the previous run's future completed — which makes every
    /// job single-flight by construction. A slow run therefore delays the
    /// next one instead of piling up; missed ticks are delayed rather than
    /// burst-replayed ([`MissedTickBehavior::Delay`]).
    ///
    /// Spawning a job whose name is already registered stops the previous
    /// one and reports `replaced = true`.
    pub fn spawn_job<F, Fut>(&self, name: &str, interval: Duration, job: F) -> Spawned
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<(), String>> + Send + 'static,
    {
        debug_assert!(interval > Duration::ZERO, "job interval must be non-zero");
        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let state = Arc::new(Mutex::new(JobState::default()));
        let job_name = name.to_string();
        let loop_state = Arc::clone(&state);

        let handle = tokio::spawn(async move {
            // Fully qualified call: the `interval` parameter shadows the
            // `tokio::time::interval` constructor inside this function.
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    // Resolves when the flag flips to `true`; the `Err` case
                    // (all senders dropped, e.g. scheduler dropped) ends the
                    // loop the same way.
                    _ = shutdown_rx.wait_for(|running| *running) => break,
                    _ = ticker.tick() => {}
                }
                // Count the run as started, then release the lock before
                // awaiting the job future.
                loop_state
                    .lock()
                    .expect("scheduler job state lock")
                    .run_count += 1;
                let outcome = job().await;
                let mut state = loop_state.lock().expect("scheduler job state lock");
                match outcome {
                    Ok(()) => state.last_error = None,
                    Err(err) => {
                        log::warn!(target: "scheduler", "periodic job `{job_name}` failed: {err}");
                        state.failure_count += 1;
                        state.last_error = Some(err);
                    }
                }
            }
        });

        let previous = self.jobs.lock().expect("scheduler registry lock").insert(
            name.to_string(),
            JobEntry {
                shutdown_tx,
                handle,
                state,
            },
        );
        let replaced = previous.is_some();
        if let Some(previous) = previous {
            previous.stop();
            log::info!(target: "scheduler", "replaced active periodic job `{name}`");
        }
        Spawned { replaced }
    }

    /// Stop the named job: flip its shutdown flag, abort its task, and
    /// remove it from the registry. Returns whether a job of that name was
    /// registered.
    pub fn cancel(&self, name: &str) -> bool {
        match self
            .jobs
            .lock()
            .expect("scheduler registry lock")
            .remove(name)
        {
            Some(entry) => {
                entry.stop();
                true
            }
            None => false,
        }
    }

    /// Stop every registered job and empty the registry.
    pub fn cancel_all(&self) {
        let mut jobs = self.jobs.lock().expect("scheduler registry lock");
        for entry in jobs.drain().map(|(_, entry)| entry) {
            entry.stop();
        }
    }

    /// Snapshots of all registered jobs, sorted by job name.
    pub fn snapshot(&self) -> Vec<JobSnapshot> {
        let jobs = self.jobs.lock().expect("scheduler registry lock");
        let mut snapshots: Vec<JobSnapshot> = jobs
            .iter()
            .map(|(name, entry)| {
                let state = entry.state.lock().expect("scheduler job state lock");
                JobSnapshot {
                    name: name.clone(),
                    active: true,
                    run_count: state.run_count,
                    failure_count: state.failure_count,
                    last_error: state.last_error.clone(),
                }
            })
            .collect();
        snapshots.sort_by(|left, right| left.name.cmp(&right.name));
        snapshots
    }

    /// Whether a job of this name is currently registered.
    #[cfg(test)]
    pub fn is_active(&self, name: &str) -> bool {
        self.jobs
            .lock()
            .expect("scheduler registry lock")
            .contains_key(name)
    }
}

impl Default for JobScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// Upper bound for waiting on job progress before a test fails. The
    /// crate's tokio does not enable `test-util`, so tokio's paused clock
    /// (`start_paused`) is unavailable; tests run against real time with
    /// small intervals and poll with a deadline instead of pinning ticks.
    const TIMEOUT: Duration = Duration::from_secs(5);

    /// Spawn a job that bumps `counter` on every run and always succeeds.
    fn spawn_counter(
        scheduler: &JobScheduler,
        name: &str,
        interval: Duration,
        counter: Arc<AtomicU64>,
    ) -> Spawned {
        scheduler.spawn_job(name, interval, move || {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
    }

    /// Wait until `counter` reaches `target`.
    async fn wait_until_reaches(counter: &AtomicU64, target: u64) {
        let deadline = Instant::now() + TIMEOUT;
        while counter.load(Ordering::SeqCst) < target {
            assert!(
                Instant::now() < deadline,
                "job did not reach {target} runs within {TIMEOUT:?}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Wait until the snapshot of job `name` satisfies `predicate` and
    /// return that snapshot.
    async fn wait_for_snapshot(
        scheduler: &JobScheduler,
        name: &str,
        predicate: impl Fn(&JobSnapshot) -> bool,
    ) -> JobSnapshot {
        let deadline = Instant::now() + TIMEOUT;
        loop {
            if let Some(snap) = scheduler
                .snapshot()
                .into_iter()
                .find(|snap| snap.name == name && predicate(snap))
            {
                return snap;
            }
            assert!(
                Instant::now() < deadline,
                "job `{name}` did not reach the expected state within {TIMEOUT:?}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    #[tokio::test]
    async fn job_runs_immediately_then_once_per_interval() {
        let scheduler = JobScheduler::new();
        let runs = Arc::new(AtomicU64::new(0));
        spawn_counter(
            &scheduler,
            "counter",
            Duration::from_millis(10),
            Arc::clone(&runs),
        );

        // The first tick completes immediately: the spawned task is polled
        // before this 1ms sleep can fire.
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert_eq!(runs.load(Ordering::SeqCst), 1);

        // Several more ticks land afterwards, one per interval.
        wait_until_reaches(&runs, 4).await;

        let snaps = scheduler.snapshot();
        assert_eq!(snaps.len(), 1);
        assert!(snaps[0].active);
        assert!(snaps[0].run_count >= 4);
        assert_eq!(snaps[0].failure_count, 0);
        assert_eq!(snaps[0].last_error, None);
    }

    #[tokio::test]
    async fn cancel_stops_further_runs() {
        let scheduler = JobScheduler::new();
        let runs = Arc::new(AtomicU64::new(0));
        spawn_counter(
            &scheduler,
            "doomed",
            Duration::from_millis(10),
            Arc::clone(&runs),
        );

        wait_until_reaches(&runs, 2).await;

        assert!(scheduler.cancel("doomed"));
        assert!(!scheduler.is_active("doomed"));
        assert!(scheduler.snapshot().is_empty());

        // Wait several intervals; a live job would have run again by now.
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            runs.load(Ordering::SeqCst),
            2,
            "canceled job must not run again"
        );
    }

    #[tokio::test]
    async fn failures_are_counted_and_recorded() {
        let scheduler = JobScheduler::new();
        scheduler.spawn_job("bad", Duration::from_millis(10), || async {
            Err("boom".to_string())
        });

        let snap = wait_for_snapshot(&scheduler, "bad", |s| s.failure_count >= 3).await;
        assert!(snap.run_count >= snap.failure_count);
        assert_eq!(snap.last_error.as_deref(), Some("boom"));
        assert!(snap.active);
    }

    #[tokio::test]
    async fn same_name_spawn_replaces_previous_job() {
        let scheduler = JobScheduler::new();
        let runs_a = Arc::new(AtomicU64::new(0));
        let spawned_a = spawn_counter(
            &scheduler,
            "dup",
            Duration::from_millis(10),
            Arc::clone(&runs_a),
        );
        assert!(!spawned_a.replaced);

        // Let job A complete its immediate first run.
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert_eq!(runs_a.load(Ordering::SeqCst), 1);

        let runs_b = Arc::new(AtomicU64::new(0));
        let spawned_b = spawn_counter(
            &scheduler,
            "dup",
            Duration::from_millis(20),
            Arc::clone(&runs_b),
        );
        assert!(spawned_b.replaced);
        assert!(scheduler.is_active("dup"));

        // B keeps ticking while A stays frozen at its single run.
        wait_until_reaches(&runs_b, 2).await;
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            runs_a.load(Ordering::SeqCst),
            1,
            "replaced job A must stay stopped"
        );
        assert!(runs_b.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn cancel_unknown_name_is_reported_and_registry_stays_empty() {
        let scheduler = JobScheduler::new();
        assert!(!scheduler.cancel("ghost"));
        assert!(!scheduler.is_active("ghost"));
        assert!(scheduler.snapshot().is_empty());
    }

    #[tokio::test]
    async fn snapshot_lists_entries_sorted_by_name_with_fields() {
        let scheduler = JobScheduler::new();
        // Long intervals: after the immediate first run the next tick is
        // 500ms away, so the counters are stable while the test inspects
        // them (the whole test finishes far earlier).
        scheduler.spawn_job("zeta", Duration::from_millis(500), || async {
            Err("nope".to_string())
        });
        spawn_counter(
            &scheduler,
            "middle",
            Duration::from_millis(500),
            Arc::new(AtomicU64::new(0)),
        );
        spawn_counter(
            &scheduler,
            "alpha",
            Duration::from_millis(500),
            Arc::new(AtomicU64::new(0)),
        );

        // Every job completes its immediate first run.
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(scheduler.cancel("middle"));

        assert_eq!(
            scheduler.snapshot(),
            vec![
                JobSnapshot {
                    name: "alpha".to_string(),
                    active: true,
                    run_count: 1,
                    failure_count: 0,
                    last_error: None,
                },
                JobSnapshot {
                    name: "zeta".to_string(),
                    active: true,
                    run_count: 1,
                    failure_count: 1,
                    last_error: Some("nope".to_string()),
                },
            ]
        );
    }
}

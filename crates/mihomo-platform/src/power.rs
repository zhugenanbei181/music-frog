use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, watch};
use tokio::time::{self, Instant};

/// Power event representing sleep, wake, or significant timer gaps indicating possible suspension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PowerEvent {
    Sleep,
    Wake,
    TimerGapDetected { gap_ms: u64 },
}

/// Watches for system power events using a heuristic timer gap detection method.
pub struct PowerEventWatcher {
    sender: broadcast::Sender<PowerEvent>,
    stop_tx: watch::Sender<bool>,
    stop_rx: watch::Receiver<bool>,
}

impl Default for PowerEventWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl PowerEventWatcher {
    /// Creates a new PowerEventWatcher.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);
        Self {
            sender,
            stop_tx,
            stop_rx,
        }
    }

    /// Starts the watcher background task and returns a receiver for power events.
    pub fn start(&self) -> broadcast::Receiver<PowerEvent> {
        let rx = self.sender.subscribe();
        let sender = self.sender.clone();
        let mut stop_rx = self.stop_rx.clone();

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_millis(1000));
            interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
            interval.tick().await; // First tick resolves immediately

            let mut last_tick = Instant::now();

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = Instant::now();
                        let gap = now.duration_since(last_tick).as_millis() as u64;

                        // Expected ~1000ms. If it's > 3000ms, it's likely a sleep/wake cycle or heavy pause.
                        if gap > 3000 {
                            let _ = sender.send(PowerEvent::Sleep);
                            let _ = sender.send(PowerEvent::TimerGapDetected { gap_ms: gap });
                            let _ = sender.send(PowerEvent::Wake);
                        }

                        last_tick = now;
                    }
                    _ = stop_rx.changed() => {
                        if *stop_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        rx
    }

    /// Stops the watcher background task.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(true);
    }
}

pub type ProbeFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = bool> + Send>> + Send + Sync>;
pub type RecoveryFn = Arc<dyn Fn() -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// Controls self-healing by listening to power events and triggering probes/recovery.
pub struct SelfHealingController {
    probe_fn: ProbeFn,
    recovery_fn: RecoveryFn,
    probe_timeout: Duration,
}

impl SelfHealingController {
    /// Creates a new SelfHealingController with default 2s probe timeout.
    pub fn new(probe_fn: ProbeFn, recovery_fn: RecoveryFn) -> Self {
        Self::new_with_timeout(probe_fn, recovery_fn, Duration::from_secs(2))
    }

    /// Creates a new SelfHealingController with a custom probe timeout.
    pub fn new_with_timeout(
        probe_fn: ProbeFn,
        recovery_fn: RecoveryFn,
        probe_timeout: Duration,
    ) -> Self {
        Self {
            probe_fn,
            recovery_fn,
            probe_timeout,
        }
    }

    /// Runs the controller background task, listening for events.
    pub async fn run(
        &self,
        mut watcher_rx: broadcast::Receiver<PowerEvent>,
        mut stop_rx: watch::Receiver<bool>,
    ) {
        let mut last_wake = Instant::now() - Duration::from_secs(60); // Initialize in the past to allow immediate trigger

        loop {
            tokio::select! {
                result = watcher_rx.recv() => {
                    match result {
                        Ok(PowerEvent::Wake) | Ok(PowerEvent::TimerGapDetected { .. }) => {
                            let now = Instant::now();
                            // Debounce: 5 seconds to avoid rapid duplicate triggers
                            if now.duration_since(last_wake) < Duration::from_secs(5) {
                                continue;
                            }
                            last_wake = now;

                            let probe = (self.probe_fn)();
                            
                            // Timeout maps to `false` (probe failed).
                            let probe_success =
                                time::timeout(self.probe_timeout, probe).await.unwrap_or_default();

                            if !probe_success {
                                (self.recovery_fn)().await;
                            }
                        }
                        Ok(PowerEvent::Sleep) => {}
                        Err(_) => {} // E.g., lagged or channel closed
                    }
                }
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_power_event_watcher_broadcast() {
        let watcher = PowerEventWatcher::new();
        let mut rx = watcher.start();

        // Simulate sending an event directly since timing is hard to test deterministically
        watcher.sender.send(PowerEvent::Wake).unwrap();

        let event = rx.recv().await.unwrap();
        assert_eq!(event, PowerEvent::Wake);

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
            Box::pin(async {  })
        });

        let controller = SelfHealingController::new(probe_fn, recovery_fn);
        let (tx, rx) = broadcast::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);

        let controller_clone = Arc::new(controller);
        let controller_task = controller_clone.clone();
        
        let handle = tokio::spawn(async move {
            controller_task.run(rx, stop_rx).await;
        });

        // Trigger Wake event
        tx.send(PowerEvent::Wake).unwrap();
        sleep(Duration::from_millis(100)).await;

        assert_eq!(probe_called.load(Ordering::SeqCst), 1);
        assert_eq!(recovery_called.load(Ordering::SeqCst), 0); // Should not recover on success

        stop_tx.send(true).unwrap();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_self_healing_probe_failure() {
        let probe_called = Arc::new(AtomicU32::new(0));
        let recovery_called = Arc::new(AtomicU32::new(0));

        let probe_called_clone = probe_called.clone();
        let probe_fn: ProbeFn = Arc::new(move || {
            probe_called_clone.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { false }) // Returns false, indicating failure
        });

        let recovery_called_clone = recovery_called.clone();
        let recovery_fn: RecoveryFn = Arc::new(move || {
            recovery_called_clone.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {  })
        });

        let controller = SelfHealingController::new(probe_fn, recovery_fn);
        let (tx, rx) = broadcast::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);

        let controller_clone = Arc::new(controller);
        let controller_task = controller_clone.clone();

        let handle = tokio::spawn(async move {
            controller_task.run(rx, stop_rx).await;
        });

        // Trigger Wake event
        tx.send(PowerEvent::Wake).unwrap();
        sleep(Duration::from_millis(100)).await;

        assert_eq!(probe_called.load(Ordering::SeqCst), 1);
        assert_eq!(recovery_called.load(Ordering::SeqCst), 1); // Should recover on failure

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
                sleep(Duration::from_millis(150)).await; // Exceeds 40ms timeout
                true
            })
        });

        let recovery_called_clone = recovery_called.clone();
        let recovery_fn: RecoveryFn = Arc::new(move || {
            recovery_called_clone.fetch_add(1, Ordering::SeqCst);
            Box::pin(async {  })
        });

        let controller = SelfHealingController::new_with_timeout(
            probe_fn,
            recovery_fn,
            Duration::from_millis(40),
        );
        let (tx, rx) = broadcast::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);

        let handle = tokio::spawn(async move {
            controller.run(rx, stop_rx).await;
        });

        sleep(Duration::from_millis(10)).await;
        tx.send(PowerEvent::Wake).unwrap();

        // Wait for probe to start (10ms), timeout (40ms) and recovery to execute
        sleep(Duration::from_millis(100)).await;

        assert_eq!(probe_called.load(Ordering::SeqCst), 1);
        assert_eq!(recovery_called.load(Ordering::SeqCst), 1); // Timeout triggers recovery

        stop_tx.send(true).unwrap();
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_debounce() {
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
            Box::pin(async {  })
        });

        let controller = SelfHealingController::new(probe_fn, recovery_fn);
        let (tx, rx) = broadcast::channel(16);
        let (stop_tx, stop_rx) = watch::channel(false);

        let controller_clone = Arc::new(controller);
        let controller_task = controller_clone.clone();

        let handle = tokio::spawn(async move {
            controller_task.run(rx, stop_rx).await;
        });

        tokio::time::pause();

        // Send multiple wake events quickly
        tx.send(PowerEvent::Wake).unwrap();
        tx.send(PowerEvent::TimerGapDetected { gap_ms: 3500 }).unwrap();
        tx.send(PowerEvent::Wake).unwrap();

        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        // Probe should only be called once due to 5s debounce
        assert_eq!(probe_called.load(Ordering::SeqCst), 1);
        assert_eq!(recovery_called.load(Ordering::SeqCst), 0);

        // Advance past debounce window
        tokio::time::advance(Duration::from_secs(6)).await;

        // Send another wake event
        tx.send(PowerEvent::Wake).unwrap();
        
        tokio::time::advance(Duration::from_millis(100)).await;
        tokio::task::yield_now().await;

        // Probe should be called again
        assert_eq!(probe_called.load(Ordering::SeqCst), 2);

        stop_tx.send(true).unwrap();
        handle.await.unwrap();
    }
}

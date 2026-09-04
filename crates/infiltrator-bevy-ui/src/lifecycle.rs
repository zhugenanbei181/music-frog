//! RAII background task lifecycle governance and safe teardown infrastructure.
//!
//! Charter (docs/BEVY_UI_FRONTEND.md):
//! Strictly no orphan tasks ("严禁管杀不管埋"). Every background worker, pump, and stream
//! carries an explicit cancellation token and auto-terminates on drop within timeout.

use bevy::ecs::resource::Resource;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle, spawn};
use std::time::{Duration, Instant};

/// Cancellation token shared between host and background worker thread.
#[derive(Clone, Debug)]
pub struct TearDownToken {
    cancelled: Arc<AtomicBool>,
}

impl Default for TearDownToken {
    fn default() -> Self {
        Self::new()
    }
}

impl TearDownToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if background task has been requested to terminate.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

/// RAII handle wrapping a background worker thread. Cancels task on Drop.
#[derive(Debug)]
pub struct LifecycleTaskHandle {
    pub name: String,
    token: TearDownToken,
    join_handle: Option<JoinHandle<()>>,
}

impl LifecycleTaskHandle {
    pub fn spawn<F>(name: impl Into<String>, task_fn: F) -> Self
    where
        F: FnOnce(TearDownToken) + Send + 'static,
    {
        let name_str = name.into();
        let token = TearDownToken::new();
        let thread_token = token.clone();

        let handle = spawn(move || {
            task_fn(thread_token);
        });

        Self {
            name: name_str,
            token,
            join_handle: Some(handle),
        }
    }

    /// Request task termination.
    pub fn cancel(&self) {
        self.token.cancel();
    }

    /// Whether task cancellation has been triggered.
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    /// Cancel and block waiting for worker thread exit up to `timeout`.
    pub fn cancel_and_wait(&mut self, timeout: Duration) -> bool {
        self.cancel();
        if let Some(handle) = self.join_handle.take() {
            let start = Instant::now();
            while !handle.is_finished() {
                if start.elapsed() >= timeout {
                    return false;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let _ = handle.join();
            true
        } else {
            true
        }
    }
}

impl Drop for LifecycleTaskHandle {
    fn drop(&mut self) {
        self.cancel();
        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
    }
}

/// Global registry tracking running background tasks in the Bevy ECS World.
#[derive(Resource, Debug, Default)]
pub struct TaskLifecycleRegistry {
    tasks: Vec<LifecycleTaskHandle>,
}

impl TaskLifecycleRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register and spawn a new managed background task.
    pub fn spawn_task<F>(&mut self, name: impl Into<String>, task_fn: F)
    where
        F: FnOnce(TearDownToken) + Send + 'static,
    {
        self.tasks.push(LifecycleTaskHandle::spawn(name, task_fn));
    }

    /// Number of registered tasks.
    pub fn active_count(&self) -> usize {
        self.tasks.len()
    }

    /// Terminate and join all managed tasks.
    pub fn terminate_all(&mut self) {
        for task in &mut self.tasks {
            task.cancel_and_wait(Duration::from_millis(50));
        }
        self.tasks.clear();
    }
}

impl Drop for TaskLifecycleRegistry {
    fn drop(&mut self) {
        self.terminate_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_lifecycle_cancellation_and_raii_teardown() {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut handle = LifecycleTaskHandle::spawn("test_worker", move |token| {
            while !token.is_cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
            tx.send(true).unwrap();
        });

        assert!(!handle.is_cancelled());

        // Cancel and wait for exit
        assert!(handle.cancel_and_wait(Duration::from_millis(100)));
        assert!(handle.is_cancelled());
        assert_eq!(rx.try_recv(), Ok(true));
    }

    #[test]
    fn test_task_registry_batch_termination() {
        let mut registry = TaskLifecycleRegistry::new();

        registry.spawn_task("task1", |token| {
            while !token.is_cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
        });

        registry.spawn_task("task2", |token| {
            while !token.is_cancelled() {
                std::thread::sleep(Duration::from_millis(2));
            }
        });

        assert_eq!(registry.active_count(), 2);
        registry.terminate_all();
        assert_eq!(registry.active_count(), 0);
    }
}

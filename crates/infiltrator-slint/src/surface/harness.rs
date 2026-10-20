//! Headless Slint platform and the harness that drives the real callbacks.
//!
//! The harness owns a real [`AppWindow`] with the product callbacks registered,
//! so every gesture below runs the same code the running product runs. It
//! reports the owner actions the surface dispatched and the typed failure the
//! surface raised, plus the owner-visible projection the surface wrote.
//!
//! The harness must be created and used on one thread: Slint components are
//! bound to the thread that created them.

use super::{FailureObserver, install_failure_observer};
use crate::callbacks;
use crate::lumen_projection;
use crate::{AppWindow, ObservationRow, SlintAction, SurfaceSnapshot, WorkerRequest};
use lurker_core_contract::error::Failure;
use slint::Model;
use slint::platform;
use slint::platform::software_renderer;
use slint::platform::{EventLoopProxy, Platform, PlatformError, WindowAdapter};
use std::cell::{Cell, RefCell};
use std::mem;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;

/// A queued event loop callback that the harness runs on demand.
type QueuedEvent = Box<dyn FnOnce() + Send>;

/// Events the surface scheduled on the event loop, tagged with the thread that
/// scheduled them so every harness only runs the events of its own window.
static QUEUED_EVENTS: Mutex<Vec<(thread::ThreadId, QueuedEvent)>> = Mutex::new(Vec::new());

thread_local! {
    /// Whether this thread already installed the headless Slint platform.
    static PLATFORM_INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// The outcome of one driven surface gesture.
#[derive(Debug)]
pub enum SurfaceOutcome {
    /// The surface dispatched these owner actions. An empty list means the
    /// gesture was handled by the surface without an owner command.
    Dispatched(Vec<SlintAction>),
    /// The surface refused the gesture. The typed failure was projected to the
    /// owner-visible notice.
    Rejected {
        /// The typed failure the surface raised.
        failure: Failure,
        /// Owner actions the surface dispatched before refusing.
        actions: Vec<SlintAction>,
    },
}

/// A headless Slint surface that drives the real product callbacks.
pub struct SurfaceHarness {
    window: AppWindow,
    requests: Receiver<WorkerRequest>,
    latest: Arc<Mutex<Option<SurfaceSnapshot>>>,
    failures: Rc<RefCell<Vec<Failure>>>,
    previous_observer: Option<FailureObserver>,
}

impl SurfaceHarness {
    /// Creates a headless surface with the real callbacks registered.
    ///
    /// `snapshot` is the owner snapshot the surface observes; `None` models an
    /// owner that has not published its first snapshot yet.
    pub fn new(snapshot: Option<SurfaceSnapshot>) -> Result<Self, PlatformError> {
        install_headless_platform();
        let window = AppWindow::new()?;
        let (requests, receiver) = channel::<WorkerRequest>();
        let latest = Arc::new(Mutex::new(snapshot));
        callbacks::register_callbacks(&window, requests, Arc::clone(&latest));
        let failures = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::clone(&failures);
        let observer: FailureObserver = Rc::new(move |failure: Failure| {
            observed.borrow_mut().push(failure);
        });
        let previous_observer = install_failure_observer(observer);
        Ok(Self {
            window,
            requests: receiver,
            latest,
            failures,
            previous_observer,
        })
    }

    /// Publishes an owner snapshot to the surface, exactly as the owner worker
    /// does when the shared owner hands over a new snapshot.
    pub fn publish_snapshot(&self, snapshot: SurfaceSnapshot) {
        if let Ok(mut latest) = self.latest.lock() {
            *latest = Some(snapshot.clone());
        }
        lumen_projection::project_snapshot(&self.window, &snapshot, None);
    }

    /// Drives the real `navigate` callback.
    pub fn navigate(&self, page: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_navigate(page.into()))
    }

    /// Drives the real `set-route-mode` callback.
    pub fn set_route_mode(&self, mode: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_route_mode(mode.into()))
    }

    /// Drives the real `set-network-mode` callback.
    pub fn set_network_mode(&self, mode: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_network_mode(mode.into()))
    }

    /// Drives the real `select-proxy` callback.
    pub fn select_proxy(&self, group: &str, node: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_select_proxy(group.into(), node.into()))
    }

    /// Drives the real `test-proxy` callback.
    pub fn test_proxy(&self, group: &str, node: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_test_proxy(group.into(), node.into()))
    }

    /// Drives the real `search-proxy-nodes` callback.
    pub fn search_proxy_nodes(&self, query: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_search_proxy_nodes(query.into()))
    }

    /// Drives the real `set-proxy-sort` callback.
    pub fn set_proxy_sort(&self, sort: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_proxy_sort(sort.into()))
    }

    /// Drives the real `close-connection` callback.
    pub fn close_connection(&self, id: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_close_connection(id.into()))
    }

    /// Drives the real `set-connection-view` callback.
    pub fn set_connection_view(&self, show_closed: bool) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_connection_view(show_closed))
    }

    /// Drives the real `set-connection-sort` callback.
    ///
    /// The widget binds the selected sort to the window property before it
    /// calls the callback, so the harness does the same.
    pub fn set_connection_sort(&self, sort: &str) -> SurfaceOutcome {
        let window = &self.window;
        window.set_connection_sort(sort.into());
        self.drive(|| window.invoke_set_connection_sort(sort.into()))
    }

    /// Drives the real `set-log-level` callback.
    ///
    /// The widget binds the selected level to the window property before it
    /// calls the callback, so the harness does the same.
    pub fn set_log_level(&self, level: &str) -> SurfaceOutcome {
        let window = &self.window;
        window.set_log_level(level.into());
        self.drive(|| window.invoke_set_log_level(level.into()))
    }

    /// Drives the real `search-logs` callback.
    ///
    /// The widget binds the typed text to the window property before it calls
    /// the callback, so the harness does the same.
    pub fn search_logs(&self, query: &str) -> SurfaceOutcome {
        let window = &self.window;
        window.set_log_search(query.into());
        self.drive(|| window.invoke_search_logs(query.into()))
    }

    /// Drives the real `set-rule-enabled` callback.
    pub fn set_rule_enabled(&self, index: &str, enabled: bool) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_rule_enabled(index.into(), enabled))
    }

    /// Drives the real `update-provider` callback.
    pub fn update_provider(&self, name: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_update_provider(name.into()))
    }

    /// Drives the real `set-theme-mode` callback.
    pub fn set_theme_mode(&self, mode: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_theme_mode(mode.into()))
    }

    /// Drives the real `set-theme-color` callback.
    pub fn set_theme_color(&self, hex: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_theme_color(hex.into()))
    }

    /// Drives the real `set-application-language` callback.
    pub fn set_application_language(&self, language: &str) -> SurfaceOutcome {
        let window = &self.window;
        self.drive(|| window.invoke_set_application_language(language.into()))
    }

    /// The owner-visible notice the surface last projected.
    pub fn notice(&self) -> String {
        self.window.get_notice().to_string()
    }

    /// The projected Core lifecycle label.
    pub fn runtime_status(&self) -> String {
        self.window.get_runtime_status().to_string()
    }

    /// The projected routing mode label.
    pub fn route_mode(&self) -> String {
        self.window.get_route_mode().to_string()
    }

    /// The projected network mode label.
    pub fn network_mode(&self) -> String {
        self.window.get_network_mode().to_string()
    }

    /// The warning the surface projects when the owner publishes a bounded
    /// window of observations.
    pub fn snapshot_window_warning(&self) -> String {
        self.window.get_snapshot_window_warning().to_string()
    }

    /// The projected connection rows.
    pub fn connection_rows(&self) -> Vec<ObservationRow> {
        model_rows(&self.window.get_connections())
    }

    /// The projected log rows.
    pub fn log_rows(&self) -> Vec<ObservationRow> {
        model_rows(&self.window.get_logs())
    }

    /// The projected rule rows.
    pub fn rule_rows(&self) -> Vec<ObservationRow> {
        model_rows(&self.window.get_rules())
    }

    /// The projected proxy node rows.
    pub fn proxy_node_rows(&self) -> Vec<ObservationRow> {
        model_rows(&self.window.get_proxy_nodes())
    }

    /// The accent colour draft the surface is editing.
    pub fn theme_color_code(&self) -> String {
        self.window.get_theme_color_code().to_string()
    }

    /// The accent colour error the surface projected.
    pub fn theme_color_error(&self) -> String {
        self.window.get_theme_color_error().to_string()
    }

    /// The rule the surface marked as pending.
    pub fn rule_pending_index(&self) -> String {
        self.window.get_rule_pending_index().to_string()
    }

    /// The connection the surface marked as pending.
    pub fn connection_breaking_id(&self) -> String {
        self.window.get_connection_breaking_id().to_string()
    }

    /// The proxy group the surface is probing.
    pub fn proxy_testing_group(&self) -> String {
        self.window.get_proxy_testing_group().to_string()
    }

    /// The proxy node the surface is probing.
    pub fn proxy_testing_node(&self) -> String {
        self.window.get_proxy_testing_node().to_string()
    }

    /// Runs one gesture and reports what the surface did with it.
    fn drive(&self, gesture: impl FnOnce()) -> SurfaceOutcome {
        self.failures.borrow_mut().clear();
        gesture();
        self.drain_events();
        let actions = self.take_actions();
        let failure = self.failures.borrow_mut().pop();
        match failure {
            Some(failure) => SurfaceOutcome::Rejected { failure, actions },
            None => SurfaceOutcome::Dispatched(actions),
        }
    }

    /// Runs the events the surface scheduled on the event loop of this thread.
    fn drain_events(&self) {
        let thread_id = thread::current().id();
        let events = {
            let mut queue = QUEUED_EVENTS.lock().expect("surface event queue");
            let scheduled = mem::take(&mut *queue);
            let mut mine = Vec::new();
            for (owner, event) in scheduled {
                if owner == thread_id {
                    mine.push(event);
                } else {
                    queue.push((owner, event));
                }
            }
            mine
        };
        for event in events {
            event();
        }
    }

    /// Takes the owner actions the surface dispatched to the owner worker.
    fn take_actions(&self) -> Vec<SlintAction> {
        let mut actions = Vec::new();
        while let Ok(request) = self.requests.try_recv() {
            if let WorkerRequest::Action(action) = request {
                actions.push(action);
            }
        }
        actions
    }
}

impl Drop for SurfaceHarness {
    fn drop(&mut self) {
        if let Some(previous) = self.previous_observer.take() {
            install_failure_observer(previous);
        }
    }
}

/// Reads every row of a projected Slint model.
fn model_rows(model: &slint::ModelRc<ObservationRow>) -> Vec<ObservationRow> {
    (0..model.row_count())
        .filter_map(|row| model.row_data(row))
        .collect()
}

/// Installs the headless Slint platform for this thread.
///
/// The Slint event loop proxy is a process wide slot, so only the first thread
/// provides it; every later thread installs a platform without one. Each
/// thread still owns its own Slint context and its own window.
fn install_headless_platform() {
    if PLATFORM_INSTALLED.with(Cell::get) {
        return;
    }
    static INSTALL_LOCK: Mutex<()> = Mutex::new(());
    static PROXY_PROVIDED: AtomicBool = AtomicBool::new(false);
    let _guard = INSTALL_LOCK.lock().expect("surface platform lock");
    if PLATFORM_INSTALLED.with(Cell::get) {
        return;
    }
    let provide_proxy = !PROXY_PROVIDED.swap(true, Ordering::AcqRel);
    let _ = platform::set_platform(Box::new(HeadlessPlatform { provide_proxy }));
    PLATFORM_INSTALLED.with(|installed| installed.set(true));
}

/// A Slint platform that renders into memory and never opens a window.
struct HeadlessPlatform {
    provide_proxy: bool,
}

impl Platform for HeadlessPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(software_renderer::MinimalSoftwareWindow::new(
            software_renderer::RepaintBufferType::NewBuffer,
        ))
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn EventLoopProxy>> {
        self.provide_proxy
            .then(|| Box::new(QueueEventLoopProxy) as Box<dyn EventLoopProxy>)
    }
}

/// An event loop proxy that parks the scheduled work until a harness runs it.
struct QueueEventLoopProxy;

impl EventLoopProxy for QueueEventLoopProxy {
    fn quit_event_loop(&self) -> Result<(), slint::EventLoopError> {
        Ok(())
    }

    fn invoke_from_event_loop(&self, event: QueuedEvent) -> Result<(), slint::EventLoopError> {
        let owner = thread::current().id();
        QUEUED_EVENTS
            .lock()
            .expect("surface event queue")
            .push((owner, event));
        Ok(())
    }
}

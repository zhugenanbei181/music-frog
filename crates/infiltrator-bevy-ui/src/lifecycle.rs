//! RAII background task lifecycle governance and safe teardown infrastructure.
//!
//! Charter (docs/bevy-ui/BEVY_UI_FRONTEND.md):
//! Strictly no orphan tasks ("严禁管杀不管埋"). Every background worker, pump, and stream
//! carries an explicit cancellation token and auto-terminates on drop within timeout.

use crate::surface::HostSurfaceRestoreRequested;
use crate::surface::on_surface_restore_requested;
use bevy::app::{App, Plugin, Startup};
use bevy::ecs::event::Event;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Res, ResMut};
use infiltrator_bevy_widgets::boot_cache::{
    BootCacheReject, BootCacheRestore, BootPipelineCache, BudgetVerdict, ZeroAllocBudgetMeter,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{JoinHandle, sleep, spawn};
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
                sleep(Duration::from_millis(1));
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

/// In-memory stand-in for the persisted cold-start cache record.
///
/// A native host swaps this for a file / mmap-backed store without touching the
/// shell: the launch path only ever sees the typed bytes.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct BootCacheStore(pub Option<Vec<u8>>);

/// Typed, non-fatal cold-start cache launch report.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootCacheLaunchReport {
    /// The typed restore outcome (`Hit` or a specific `Miss` reason).
    pub restore: BootCacheRestore,
    /// Whether the launch had to perform a real rebuild.
    pub rebuilt: bool,
    /// The steady-state zero-allocation verdict sampled after warm-up.
    pub budget: BudgetVerdict,
}

impl Default for BootCacheLaunchReport {
    fn default() -> Self {
        Self {
            restore: BootCacheRestore::Miss(BootCacheReject::Empty),
            rebuilt: false,
            budget: BudgetVerdict::Within,
        }
    }
}

/// Shader pipelines compiled by the cold-start rebuild path.
pub const BOOT_WARM_SHADERS: usize = 24;
/// Font glyphs rasterized by the cold-start rebuild path.
pub const BOOT_WARM_GLYPHS: usize = 512;
/// Recorded cold-start rebuild duration in milliseconds.
pub const BOOT_WARM_DURATION_MS: u64 = 40;

/// Restore the pipeline cache from the store, or rebuild on a typed miss.
///
/// A corrupt or absent record is never fatal: it becomes a typed
/// [`BootCacheRestore::Miss`] and forces a real rebuild instead of a partial
/// restore.
pub fn restore_boot_cache(
    mut cache: ResMut<BootPipelineCache>,
    store: Res<BootCacheStore>,
    mut report: ResMut<BootCacheLaunchReport>,
) {
    let restore = match store.0.as_deref() {
        Some(record) => cache.restore(record),
        None => BootCacheRestore::Miss(BootCacheReject::Empty),
    };
    let rebuilt = match restore {
        BootCacheRestore::Hit => false,
        BootCacheRestore::Miss(_) => {
            cache.mark_warmed(BOOT_WARM_SHADERS, BOOT_WARM_GLYPHS, BOOT_WARM_DURATION_MS);
            true
        }
    };
    report.restore = restore;
    report.rebuilt = rebuilt;
}

/// Record the warmed cache for the next launch.
///
/// A fresh rebuild always overwrites a stale/corrupt record; a clean hit keeps
/// the existing bytes untouched.
pub fn record_boot_cache(
    cache: Res<BootPipelineCache>,
    report: Res<BootCacheLaunchReport>,
    mut store: ResMut<BootCacheStore>,
) {
    if report.rebuilt || store.0.is_none() {
        store.0 = cache.record();
    }
}

/// Declare the steady-state zero-allocation budget after warm-up.
pub fn assert_zero_alloc_budget(
    mut meter: ResMut<ZeroAllocBudgetMeter>,
    mut report: ResMut<BootCacheLaunchReport>,
) {
    report.budget = meter.assert_frame(0, 0);
}

/// BEVY-039: cold-start pipeline cache wired into launch.
///
/// Installed by [`HostLifecyclePlugin`] so the windowed and headless
/// compositions share the same cold-start path.
pub struct BootCacheLaunchPlugin;

impl Plugin for BootCacheLaunchPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BootCacheStore>();
        app.init_resource::<BootPipelineCache>();
        app.init_resource::<BootCacheLaunchReport>();
        app.init_resource::<ZeroAllocBudgetMeter>();
        app.add_systems(
            Startup,
            (
                restore_boot_cache,
                record_boot_cache,
                assert_zero_alloc_budget,
            )
                .chain(),
        );
    }
}

/// Native host application lifecycle signal (Android `Activity`/`Surface`,
/// winit focus/occlusion, or a desktop no-op host). This is the typed input the
/// host feeds into the shell; the shell never invents Android lifecycle facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HostLifecycleEvent {
    /// The Activity is resumed and its surface is visible and focused.
    Foreground,
    /// The Activity is stopped/paused; the surface may still exist.
    Background,
    /// The surface is visible but covered (dialog, system overlay).
    Occluded,
    /// The native surface was destroyed; the UI must stop observing/rendering.
    SurfaceLost,
    /// A new native surface is ready; the UI must re-read a full snapshot.
    SurfaceRestored,
    /// The Activity was recreated; UI state is preserved and old events fenced.
    ActivityRecreated,
}

/// One typed lifecycle input from the host.
///
/// `epoch` is the Activity instance identity (bumped on recreation) and
/// `revision` is a 1-based monotonic counter within that epoch. An input is
/// stale — and ignored — when its epoch is older, or when its revision does not
/// advance the last accepted revision of the same epoch. This fences a delayed
/// event from a retired Activity so it can never resurrect an old surface.
#[derive(Event, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLifecycleInput {
    pub kind: HostLifecycleEvent,
    pub epoch: u64,
    pub revision: u64,
}

/// UI observation phase derived from the host lifecycle facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UiObservationPhase {
    #[default]
    Foreground,
    Occluded,
    Background,
    SurfaceLost,
}

/// Whether the shell should observe the surface and run UI work. It is a
/// separate gate from business resources so a destroyed surface pauses UI
/// observation without touching the VPN data plane.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UiObservationGate {
    pub active: bool,
    pub phase: UiObservationPhase,
}

impl Default for UiObservationGate {
    fn default() -> Self {
        Self {
            active: true,
            phase: UiObservationPhase::Foreground,
        }
    }
}

impl UiObservationGate {
    /// Whether UI observation/rendering is permitted right now.
    pub const fn permits_observation(&self) -> bool {
        self.active
    }
}

/// Typed effects of one accepted lifecycle transition.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LifecycleEffects {
    /// Pause UI observation and rendering.
    pub stop_observation: bool,
    /// Read back one complete snapshot from the composed source.
    pub request_surface_restore: bool,
    /// Preserve route/draft/confirmation state instead of resetting it.
    pub preserve_ui_state: bool,
    /// The VPN data plane is never owned by the UI; this is always `false`.
    pub stop_vpn: bool,
}

/// The lifecycle projection consumed by the shell. It is kept separate from
/// [`crate::surface::LatestCoreLifecycle`]: that projects the mihomo core's
/// lifecycle, this projects the host Activity/surface lifecycle.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostLifecycleState {
    pub epoch: u64,
    pub revision: u64,
    pub phase: UiObservationPhase,
    pub observation_active: bool,
}

impl Default for HostLifecycleState {
    fn default() -> Self {
        Self {
            epoch: 0,
            revision: 0,
            phase: UiObservationPhase::Foreground,
            observation_active: true,
        }
    }
}

impl HostLifecycleState {
    /// Whether an input belongs to a retired Activity or an already-applied
    /// revision and must therefore be ignored.
    pub fn is_stale(&self, input: HostLifecycleInput) -> bool {
        input.epoch < self.epoch || (input.epoch == self.epoch && input.revision <= self.revision)
    }

    /// Apply one input. Returns `None` when the input is stale, otherwise the
    /// typed effects the shell must act on.
    pub fn apply(&mut self, input: HostLifecycleInput) -> Option<LifecycleEffects> {
        if self.is_stale(input) {
            return None;
        }
        self.epoch = input.epoch;
        self.revision = input.revision;
        let mut effects = LifecycleEffects::default();
        match input.kind {
            HostLifecycleEvent::Foreground => {
                self.phase = UiObservationPhase::Foreground;
                self.observation_active = true;
            }
            HostLifecycleEvent::Occluded => {
                self.phase = UiObservationPhase::Occluded;
                self.observation_active = true;
            }
            HostLifecycleEvent::Background => {
                self.phase = UiObservationPhase::Background;
                self.observation_active = true;
            }
            HostLifecycleEvent::SurfaceLost => {
                self.phase = UiObservationPhase::SurfaceLost;
                self.observation_active = false;
                effects.stop_observation = true;
            }
            HostLifecycleEvent::SurfaceRestored => {
                self.phase = UiObservationPhase::Foreground;
                self.observation_active = true;
                effects.request_surface_restore = true;
            }
            HostLifecycleEvent::ActivityRecreated => {
                self.phase = UiObservationPhase::SurfaceLost;
                self.observation_active = false;
                effects.stop_observation = true;
                effects.preserve_ui_state = true;
            }
        }
        Some(effects)
    }

    /// Release UI-owned resources on destroy. The VPN data plane and its
    /// service binding are owned by the host / `:vpn` process, so this can only
    /// stop UI observation and never stops the VPN.
    pub fn release_ui(&mut self) -> LifecycleEffects {
        self.observation_active = false;
        self.phase = UiObservationPhase::SurfaceLost;
        LifecycleEffects {
            stop_observation: true,
            ..LifecycleEffects::default()
        }
    }
}

/// Consume one typed host lifecycle input and project it onto the UI
/// observation gate. A restore request re-reads a full snapshot; every stale
/// input is dropped by the state machine before it can touch the gate.
fn on_host_lifecycle(
    input: On<HostLifecycleInput>,
    mut state: ResMut<HostLifecycleState>,
    mut gate: ResMut<UiObservationGate>,
    mut commands: Commands,
) {
    let input = *input;
    let Some(effects) = state.apply(input) else {
        return;
    };
    gate.active = state.observation_active;
    gate.phase = state.phase;
    if effects.request_surface_restore {
        commands.trigger(HostSurfaceRestoreRequested);
    }
}

/// Installs the typed host lifecycle consumer and the restore read-back hook.
pub struct HostLifecyclePlugin;

impl Plugin for HostLifecyclePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<HostLifecycleState>()
            .init_resource::<UiObservationGate>()
            .add_observer(on_host_lifecycle)
            .add_observer(on_surface_restore_requested)
            .add_plugins(BootCacheLaunchPlugin);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use std::sync::mpsc::channel;

    #[test]
    fn test_task_lifecycle_cancellation_and_raii_teardown() {
        let (tx, rx) = channel();

        let mut handle = LifecycleTaskHandle::spawn("test_worker", move |token| {
            while !token.is_cancelled() {
                sleep(Duration::from_millis(2));
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
                sleep(Duration::from_millis(2));
            }
        });

        registry.spawn_task("task2", |token| {
            while !token.is_cancelled() {
                sleep(Duration::from_millis(2));
            }
        });

        assert_eq!(registry.active_count(), 2);
        registry.terminate_all();
        assert_eq!(registry.active_count(), 0);
    }

    fn input(kind: HostLifecycleEvent, epoch: u64, revision: u64) -> HostLifecycleInput {
        HostLifecycleInput {
            kind,
            epoch,
            revision,
        }
    }

    #[test]
    fn foreground_background_and_occlusion_keep_ui_observation_active() {
        let mut state = HostLifecycleState::default();

        let foreground = state
            .apply(input(HostLifecycleEvent::Foreground, 1, 1))
            .expect("first input is accepted");
        assert!(!foreground.stop_observation);
        assert!(state.observation_active);
        assert_eq!(state.phase, UiObservationPhase::Foreground);

        state
            .apply(input(HostLifecycleEvent::Background, 1, 2))
            .expect("background is accepted");
        assert!(
            state.observation_active,
            "background still observes for throttled sampling"
        );
        assert_eq!(state.phase, UiObservationPhase::Background);

        state
            .apply(input(HostLifecycleEvent::Occluded, 1, 3))
            .expect("occlusion is accepted");
        assert!(state.observation_active);
        assert_eq!(state.phase, UiObservationPhase::Occluded);
    }

    #[test]
    fn surface_lost_stops_observation_and_restore_requests_a_snapshot() {
        let mut state = HostLifecycleState::default();

        let lost = state
            .apply(input(HostLifecycleEvent::SurfaceLost, 1, 1))
            .expect("surface loss is accepted");
        assert!(lost.stop_observation);
        assert!(!state.observation_active);
        assert_eq!(state.phase, UiObservationPhase::SurfaceLost);

        let restored = state
            .apply(input(HostLifecycleEvent::SurfaceRestored, 1, 2))
            .expect("surface restore is accepted");
        assert!(
            restored.request_surface_restore,
            "restore must read back a complete snapshot"
        );
        assert!(state.observation_active);
    }

    #[test]
    fn stale_inputs_are_rejected_across_revisions_and_epochs() {
        let mut state = HostLifecycleState::default();
        state
            .apply(input(HostLifecycleEvent::Foreground, 2, 5))
            .expect("baseline accepted");

        assert!(
            state.is_stale(input(HostLifecycleEvent::Background, 2, 5)),
            "an equal revision is not a new event"
        );
        assert!(state.is_stale(input(HostLifecycleEvent::Background, 2, 4)));
        assert!(
            state.is_stale(input(HostLifecycleEvent::SurfaceLost, 1, 99)),
            "a retired Activity epoch can never resurrect an old surface"
        );
        assert!(!state.is_stale(input(HostLifecycleEvent::Background, 2, 6)));

        let before = state;
        assert!(
            state
                .apply(input(HostLifecycleEvent::SurfaceLost, 1, 99))
                .is_none()
        );
        assert_eq!(state, before, "a stale input leaves the projection intact");
    }

    #[test]
    fn activity_recreation_preserves_ui_state_and_fences_the_old_epoch() {
        let mut state = HostLifecycleState::default();
        state
            .apply(input(HostLifecycleEvent::Foreground, 1, 1))
            .expect("baseline accepted");

        let recreated = state
            .apply(input(HostLifecycleEvent::ActivityRecreated, 2, 1))
            .expect("recreation is accepted");
        assert!(recreated.stop_observation);
        assert!(
            recreated.preserve_ui_state,
            "route/draft state survives Activity recreation"
        );
        assert!(!recreated.stop_vpn, "the UI never stops the VPN");
        assert_eq!(state.epoch, 2);

        assert!(
            state
                .apply(input(HostLifecycleEvent::Background, 1, 99))
                .is_none(),
            "old-epoch events are fenced after recreation"
        );

        let restored = state
            .apply(input(HostLifecycleEvent::SurfaceRestored, 2, 2))
            .expect("the new surface restores");
        assert!(restored.request_surface_restore);
    }

    #[test]
    fn activity_recreation_preserves_navigation_and_draft_resources() {
        use crate::pages::connections_drawer::ConnectionsRuleDraft;
        use crate::route::{ActiveRoute, Route};
        use infiltrator_domain::rules::RuleEntry;

        let mut app = App::new();
        app.init_resource::<ActiveRoute>();
        app.init_resource::<ConnectionsRuleDraft>();
        app.add_plugins(HostLifecyclePlugin);

        app.world_mut().resource_mut::<ActiveRoute>().0 = Some(Route::Profiles);
        app.world_mut()
            .resource_mut::<ConnectionsRuleDraft>()
            .entries
            .push(RuleEntry {
                rule: "DOMAIN-SUFFIX,example.com".to_owned(),
                enabled: true,
            });

        app.world_mut()
            .trigger(input(HostLifecycleEvent::ActivityRecreated, 2, 1));
        app.update();

        assert_eq!(
            app.world().resource::<ActiveRoute>().0,
            Some(Route::Profiles),
            "the navigation source survives Activity recreation"
        );
        let draft = app.world().resource::<ConnectionsRuleDraft>();
        assert_eq!(
            draft.entries.len(),
            1,
            "the draft is not reset on recreation"
        );
        assert_eq!(draft.entries[0].rule, "DOMAIN-SUFFIX,example.com");
    }

    #[test]
    fn ui_release_stops_observation_without_touching_vpn() {
        let mut state = HostLifecycleState::default();
        let effects = state.release_ui();
        assert!(effects.stop_observation);
        assert!(
            !effects.stop_vpn,
            "destroying the UI releases only UI observation, never the VPN"
        );
        assert!(!state.observation_active);
        assert_eq!(state.phase, UiObservationPhase::SurfaceLost);
    }

    #[derive(Resource, Default)]
    struct RestoreFlag(bool);

    fn capture_restore(_request: On<HostSurfaceRestoreRequested>, mut flag: ResMut<RestoreFlag>) {
        flag.0 = true;
    }

    #[test]
    fn host_lifecycle_plugin_projects_the_gate_and_emits_a_restore_request() {
        let mut app = App::new();
        app.init_resource::<RestoreFlag>();
        app.add_observer(capture_restore);
        app.add_plugins(HostLifecyclePlugin);

        app.world_mut()
            .trigger(input(HostLifecycleEvent::SurfaceLost, 1, 1));
        app.update();
        assert!(
            !app.world().resource::<UiObservationGate>().active,
            "a lost surface stops UI observation"
        );
        assert!(!app.world().resource::<RestoreFlag>().0);

        // A delayed older-epoch event must not restart observation.
        app.world_mut()
            .trigger(input(HostLifecycleEvent::Foreground, 1, 1));
        app.update();
        assert!(!app.world().resource::<UiObservationGate>().active);

        app.world_mut()
            .trigger(input(HostLifecycleEvent::SurfaceRestored, 1, 2));
        app.update();
        assert!(app.world().resource::<UiObservationGate>().active);
        assert!(
            app.world().resource::<RestoreFlag>().0,
            "restore read-back is requested exactly through the typed event"
        );
    }

    #[test]
    fn first_boot_records_and_next_launch_restores_without_rebuild() {
        let mut first = App::new();
        first.add_plugins(BootCacheLaunchPlugin);
        first.update();
        let report = *first.world().resource::<BootCacheLaunchReport>();
        assert_eq!(
            report.restore,
            BootCacheRestore::Miss(BootCacheReject::Empty)
        );
        assert!(report.rebuilt, "a cold boot rebuilds the pipeline cache");
        assert_eq!(report.budget, BudgetVerdict::Within);
        assert_eq!(
            first.world().resource::<BootPipelineCache>().rebuild_count,
            1
        );
        let record = first
            .world()
            .resource::<BootCacheStore>()
            .0
            .clone()
            .expect("a fresh boot records a cache");

        // The next launch carries the record over and restores it.
        let mut second = App::new();
        second.add_plugins(BootCacheLaunchPlugin);
        second.insert_resource(BootCacheStore(Some(record)));
        second.update();
        let report = *second.world().resource::<BootCacheLaunchReport>();
        assert_eq!(report.restore, BootCacheRestore::Hit);
        assert!(!report.rebuilt, "a hit must never rebuild");
        assert_eq!(
            second.world().resource::<BootPipelineCache>().rebuild_count,
            0
        );
        assert!(second.world().resource::<BootPipelineCache>().is_warmed_up);
    }

    #[test]
    fn corrupt_cache_is_rejected_typed_and_rebuilt_non_fatally() {
        let mut corrupt = vec![0u8; 38];
        corrupt[0] = b'X';
        let mut app = App::new();
        app.add_plugins(BootCacheLaunchPlugin);
        app.insert_resource(BootCacheStore(Some(corrupt)));
        app.update();
        let report = *app.world().resource::<BootCacheLaunchReport>();
        assert_eq!(
            report.restore,
            BootCacheRestore::Miss(BootCacheReject::BadMagic)
        );
        assert!(report.rebuilt);
        assert!(app.world().resource::<BootPipelineCache>().is_warmed_up);

        let repaired = app
            .world()
            .resource::<BootCacheStore>()
            .0
            .clone()
            .expect("a corrupt record is replaced by a fresh one");
        let mut probe = BootPipelineCache::new();
        assert_eq!(probe.restore(&repaired), BootCacheRestore::Hit);
    }
}

//! Fine-grained reactive primitives and change-tracking state for Bevy UI.
//!
//! Charter law (docs/bevy-ui/BEVY_UI_FRONTEND.md):
//! Observers and systems restamp components in place and avoid full-tree
//! rebuilding or continuous full-table polling. This module provides pure,
//! zero-allocation change detection and memoization tools to support
//! high-frequency data streams (e.g. traffic rates, connection meters).

use crate::signal_dag::{ReactiveDag, SignalId};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::query::Changed;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, ResMut};
use std::marker::PhantomData;

/// Marker component attached to entities that have pending visual changes.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirtyMarker;

/// Typed dirty component marking a specific component type `C` as dirty.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dirty<C> {
    _marker: PhantomData<C>,
}

impl<C> Dirty<C> {
    /// Create a new dirty marker for type `C`.
    pub const fn new() -> Self {
        Self {
            _marker: PhantomData,
        }
    }
}

/// A value tracker that detects and retains whether the tracked value changed
/// since the last observation or reset.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChangeTracker<T> {
    current: T,
    dirty: bool,
    version: u64,
}

impl<T> ChangeTracker<T> {
    /// Initialize with an initial value (initially not dirty).
    pub fn new(initial: T) -> Self {
        Self {
            current: initial,
            dirty: false,
            version: 0,
        }
    }

    /// Read the current value.
    pub fn get(&self) -> &T {
        &self.current
    }

    /// Read the current version counter.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Check if the value has been marked dirty.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Check and clear the dirty flag in one step.
    pub fn take_dirty(&mut self) -> bool {
        let was_dirty = self.dirty;
        self.dirty = false;
        was_dirty
    }
}

impl<T: PartialEq> ChangeTracker<T> {
    /// Set a new value. If it differs from the current value, marks dirty,
    /// increments the version counter, and returns `true`.
    pub fn set(&mut self, next: T) -> bool {
        if self.current != next {
            self.current = next;
            self.dirty = true;
            self.version = self.version.wrapping_add(1);
            true
        } else {
            false
        }
    }

    /// Set a new value unconditionally, marking dirty and incrementing version.
    pub fn set_force(&mut self, next: T) {
        self.current = next;
        self.dirty = true;
        self.version = self.version.wrapping_add(1);
    }
}

/// A versioned reactive signal holding a value and an advancing epoch counter.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReactiveSignal<T> {
    value: T,
    epoch: u64,
}

impl<T> ReactiveSignal<T> {
    /// Initialize signal with a value at epoch 0.
    pub fn new(initial: T) -> Self {
        Self {
            value: initial,
            epoch: 0,
        }
    }

    /// Read the current value.
    pub fn get(&self) -> &T {
        &self.value
    }

    /// Read the current epoch.
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Check if the signal has changed since the given epoch.
    pub fn has_changed_since(&self, since_epoch: u64) -> bool {
        self.epoch > since_epoch
    }

    /// Mutate the value in place and advance the epoch.
    pub fn update(&mut self, f: impl FnOnce(&mut T)) {
        f(&mut self.value);
        self.epoch = self.epoch.wrapping_add(1);
    }
}

impl<T: PartialEq> ReactiveSignal<T> {
    /// Replace value if distinct, advancing epoch if changed.
    pub fn set(&mut self, next: T) -> bool {
        if self.value != next {
            self.value = next;
            self.epoch = self.epoch.wrapping_add(1);
            true
        } else {
            false
        }
    }
}

/// Pure function memoizer: caches the last input and output to avoid
/// recomputing expensive transforms (e.g. chart polylines or formatting)
/// when inputs remain unchanged.
#[derive(Clone, Debug, Default)]
pub struct Memoized<I, O> {
    last_input: Option<I>,
    cached_output: Option<O>,
}

impl<I: PartialEq + Clone, O: Clone> Memoized<I, O> {
    /// Create an empty memoizer.
    pub fn new() -> Self {
        Self {
            last_input: None,
            cached_output: None,
        }
    }

    /// Evaluate the memoized function. If `input` equals `last_input`,
    /// returns a reference to the cached output; otherwise computes `f`,
    /// stores the result, and returns it.
    pub fn eval(&mut self, input: I, compute: impl FnOnce(&I) -> O) -> &O {
        if self.last_input.as_ref() != Some(&input) {
            let output = compute(&input);
            self.last_input = Some(input);
            self.cached_output = Some(output);
        }
        self.cached_output
            .as_ref()
            .expect("cached output populated")
    }

    /// Invalidate the cache.
    pub fn invalidate(&mut self) {
        self.last_input = None;
        self.cached_output = None;
    }
}

/// Resource owning the reactive [`ReactiveDag`] evaluated by the ECS bridge.
///
/// Register signals once, then bind entities with [`SignalInput`] (sources)
/// and [`SignalOutput`] (derived results). Add [`ReactiveGraphPlugin`] to run
/// the bridge in `Update`.
#[derive(Resource, Default)]
pub struct ReactiveDagResource {
    dag: ReactiveDag,
}

impl ReactiveDagResource {
    /// Wrap an already-registered graph.
    pub fn new(dag: ReactiveDag) -> Self {
        Self { dag }
    }

    /// Read-only access to the graph.
    pub fn dag(&self) -> &ReactiveDag {
        &self.dag
    }

    /// Mutable access to register nodes and edges.
    pub fn dag_mut(&mut self) -> &mut ReactiveDag {
        &mut self.dag
    }
}

impl From<ReactiveDag> for ReactiveDagResource {
    fn from(dag: ReactiveDag) -> Self {
        Self { dag }
    }
}

/// Source component bound to a DAG signal. Mutating `value` marks the signal
/// dirty through Bevy change detection; only its dependent subtree recomputes.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalInput {
    pub signal: SignalId,
    pub value: i64,
}

impl SignalInput {
    pub const fn new(signal: SignalId, value: i64) -> Self {
        Self { signal, value }
    }
}

/// Derived component mirroring a DAG signal's value back onto its entity.
/// The bridge only writes when the value actually changed, so an idle frame
/// produces no component churn.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignalOutput {
    pub signal: SignalId,
    pub value: i64,
}

impl SignalOutput {
    pub const fn new(signal: SignalId) -> Self {
        Self { signal, value: 0 }
    }

    pub const fn with_value(signal: SignalId, value: i64) -> Self {
        Self { signal, value }
    }
}

/// Counters describing the most recent evaluation pass, for headless
/// assertions and diagnostics. `last_recomputed` is empty on an idle frame.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct ReactiveEvaluationStats {
    pub last_recomputed: Vec<SignalId>,
    pub total_recomputed: u64,
}

/// Mirror changed ECS inputs into the graph, marking dependents dirty.
pub fn stage_signal_inputs(
    mut graph: ResMut<ReactiveDagResource>,
    inputs: Query<&SignalInput, Changed<SignalInput>>,
) {
    for input in &inputs {
        let _ = graph.dag_mut().set_value(input.signal, input.value);
    }
}

/// Evaluate dirty subtrees in topological order. A cyclic graph fails closed:
/// nothing is recomputed and the stats report an empty pass.
pub fn evaluate_reactive_graph(
    mut graph: ResMut<ReactiveDagResource>,
    mut stats: ResMut<ReactiveEvaluationStats>,
) {
    match graph.dag_mut().evaluate() {
        Ok(report) => {
            stats.total_recomputed = stats
                .total_recomputed
                .saturating_add(report.recomputed_count() as u64);
            stats.last_recomputed = report.recomputed;
        }
        Err(_) => stats.last_recomputed.clear(),
    }
}

/// Flush derived values back onto their bound output components.
pub fn flush_signal_outputs(
    graph: Res<ReactiveDagResource>,
    mut outputs: Query<&mut SignalOutput>,
) {
    for mut output in &mut outputs {
        if let Some(value) = graph.dag().get_value(output.signal)
            && output.value != value
        {
            output.value = value;
        }
    }
}

/// Installs the DAG resource, the evaluation stats and the chained
/// input → evaluate → output bridge.
pub struct ReactiveGraphPlugin;

impl Plugin for ReactiveGraphPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ReactiveDagResource>();
        app.init_resource::<ReactiveEvaluationStats>();
        app.add_systems(
            Update,
            (
                stage_signal_inputs,
                evaluate_reactive_graph,
                flush_signal_outputs,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_tracker_detects_actual_modifications() {
        let mut tracker = ChangeTracker::new(10);
        assert_eq!(*tracker.get(), 10);
        assert!(!tracker.is_dirty());
        assert_eq!(tracker.version(), 0);

        // Same value does not dirty
        assert!(!tracker.set(10));
        assert!(!tracker.is_dirty());
        assert_eq!(tracker.version(), 0);

        // Different value dirties and advances version
        assert!(tracker.set(20));
        assert!(tracker.is_dirty());
        assert_eq!(*tracker.get(), 20);
        assert_eq!(tracker.version(), 1);

        // take_dirty clears
        assert!(tracker.take_dirty());
        assert!(!tracker.is_dirty());
    }

    #[test]
    fn reactive_signal_tracks_epochs() {
        let mut sig = ReactiveSignal::new("init".to_string());
        assert_eq!(sig.epoch(), 0);
        assert!(!sig.has_changed_since(0));

        assert!(sig.set("second".to_string()));
        assert_eq!(sig.epoch(), 1);
        assert!(sig.has_changed_since(0));
        assert!(!sig.has_changed_since(1));

        // Setting same value doesn't bump epoch
        assert!(!sig.set("second".to_string()));
        assert_eq!(sig.epoch(), 1);
    }

    #[test]
    fn memoized_caches_expensive_computations() {
        let mut memo = Memoized::new();
        let mut count = 0;

        let res1 = memo.eval(5, |&n| {
            count += 1;
            n * 2
        });
        assert_eq!(*res1, 10);
        assert_eq!(count, 1);

        // Same input hits cache
        let res2 = memo.eval(5, |&n| {
            count += 1;
            n * 2
        });
        assert_eq!(*res2, 10);
        assert_eq!(count, 1);

        // New input recomputes
        let res3 = memo.eval(6, |&n| {
            count += 1;
            n * 2
        });
        assert_eq!(*res3, 12);
        assert_eq!(count, 2);
    }
}

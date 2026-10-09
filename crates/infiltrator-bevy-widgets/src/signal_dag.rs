//! Reactive computation graph DAG (Directed Acyclic Graph) and Micro-Frontend AST sanitizer.
//!
//! [`ReactiveDag`] is the widget layer's fine-grained dependency evaluator: a
//! source signal changes, dirtiness propagates only to its transitive
//! dependents, and [`ReactiveDag::evaluate`] recomputes exactly that dirty
//! subtree in topological order. Cycles are rejected fail-closed — edge
//! insertion rolls back and evaluation returns [`DagError::CycleDetected`]
//! instead of panicking or looping.
//!
//! The ECS bridge that turns Bevy change detection into these graph updates
//! lives in [`crate::reactive`] (`ReactiveGraphPlugin`).

use std::collections::{BTreeSet, HashMap, HashSet};
use std::error::Error;
use std::fmt;

/// Unique identifier for a node in the reactive DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SignalId(pub usize);

/// Whether a node is an externally written source or a derived computation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalKind {
    /// A source signal whose value is written from outside the graph.
    Source,
    /// A derived signal whose value is produced by an attached computation.
    Derived,
}

/// A node in the reactive signal computation graph.
#[derive(Clone, Debug)]
pub struct SignalNode {
    pub id: SignalId,
    pub value: i64,
    /// Upstream dependencies, in the order their values are passed to a
    /// derived node's computation.
    pub dependencies: Vec<SignalId>,
    pub dependents: HashSet<SignalId>,
    pub kind: SignalKind,
    /// Set when the node or one of its dependencies changed and the node has
    /// not yet been re-evaluated.
    pub dirty: bool,
}

/// Failure modes of graph construction and evaluation. The graph never panics
/// on malformed input; callers decide how to surface these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DagError {
    /// A referenced signal was never registered.
    UnknownSignal(SignalId),
    /// The requested edge would close a cycle, so it was not added.
    CycleDetected,
}

impl fmt::Display for DagError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DagError::UnknownSignal(id) => write!(formatter, "unknown signal {:?}", id),
            DagError::CycleDetected => formatter.write_str("cycle detected in reactive signal DAG"),
        }
    }
}

impl Error for DagError {}

/// Outcome of one evaluation pass: the exact nodes recomputed, in topological
/// order. Empty when nothing was dirty.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluationReport {
    pub recomputed: Vec<SignalId>,
}

impl EvaluationReport {
    /// Number of nodes recomputed in this pass.
    pub fn recomputed_count(&self) -> usize {
        self.recomputed.len()
    }

    /// Whether this pass recomputed nothing.
    pub fn is_empty(&self) -> bool {
        self.recomputed.is_empty()
    }
}

/// A derived node's computation: given the dependency values (in dependency
/// order), produce the new value.
type ComputeFn = Box<dyn Fn(&[i64]) -> i64 + Send + Sync>;

/// Reactive Directed Acyclic Graph tracking signals and derived computations.
///
/// Registration is explicit (`create_signal` / `create_computed` / `connect`);
/// evaluation is pull-based (`evaluate`) and bounded to dirty subtrees.
#[derive(Default)]
pub struct ReactiveDag {
    nodes: HashMap<SignalId, SignalNode>,
    computes: HashMap<SignalId, ComputeFn>,
    next_id: usize,
    total_recomputations: u64,
}

impl fmt::Debug for ReactiveDag {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReactiveDag")
            .field("node_count", &self.nodes.len())
            .field("next_id", &self.next_id)
            .field("total_recomputations", &self.total_recomputations)
            .finish()
    }
}

impl ReactiveDag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of nodes currently marked dirty.
    pub fn dirty_count(&self) -> usize {
        self.nodes.values().filter(|node| node.dirty).count()
    }

    /// Total nodes recomputed across every [`Self::evaluate`] call. Monotonic;
    /// lets callers prove work is bounded to dirty subtrees.
    pub fn total_recomputations(&self) -> u64 {
        self.total_recomputations
    }

    fn allocate_id(&mut self) -> SignalId {
        let id = SignalId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Register a new source signal node with an initial value.
    pub fn create_signal(&mut self, initial_value: i64) -> SignalId {
        let id = self.allocate_id();
        self.nodes.insert(
            id,
            SignalNode {
                id,
                value: initial_value,
                dependencies: Vec::new(),
                dependents: HashSet::new(),
                kind: SignalKind::Source,
                dirty: false,
            },
        );
        id
    }

    /// Register a derived signal node that depends on upstream signals but has
    /// no attached computation yet (attach one with [`Self::set_compute`]).
    pub fn create_derived(&mut self, dependencies: &[SignalId], initial_value: i64) -> SignalId {
        let id = self.allocate_id();
        let mut deps = Vec::new();
        for &dep in dependencies {
            if !deps.contains(&dep) {
                deps.push(dep);
            }
            if let Some(upstream) = self.nodes.get_mut(&dep) {
                upstream.dependents.insert(id);
            }
        }

        self.nodes.insert(
            id,
            SignalNode {
                id,
                value: initial_value,
                dependencies: deps,
                dependents: HashSet::new(),
                kind: SignalKind::Derived,
                dirty: false,
            },
        );
        id
    }

    /// Register a derived node with its computation. The initial value is
    /// computed eagerly from the current dependency values.
    pub fn create_computed(
        &mut self,
        dependencies: &[SignalId],
        compute: impl Fn(&[i64]) -> i64 + Send + Sync + 'static,
    ) -> Result<SignalId, DagError> {
        let mut deps = Vec::new();
        for &dep in dependencies {
            if !self.nodes.contains_key(&dep) {
                return Err(DagError::UnknownSignal(dep));
            }
            if !deps.contains(&dep) {
                deps.push(dep);
            }
        }

        let inputs: Vec<i64> = deps
            .iter()
            .filter_map(|dep| self.nodes.get(dep).map(|node| node.value))
            .collect();
        let value = compute(&inputs);

        let id = self.allocate_id();
        for &dep in &deps {
            if let Some(upstream) = self.nodes.get_mut(&dep) {
                upstream.dependents.insert(id);
            }
        }
        self.nodes.insert(
            id,
            SignalNode {
                id,
                value,
                dependencies: deps,
                dependents: HashSet::new(),
                kind: SignalKind::Derived,
                dirty: false,
            },
        );
        self.computes.insert(id, Box::new(compute));
        Ok(id)
    }

    /// Attach or replace the computation of an existing node, marking it dirty.
    pub fn set_compute(
        &mut self,
        id: SignalId,
        compute: impl Fn(&[i64]) -> i64 + Send + Sync + 'static,
    ) -> Result<(), DagError> {
        if !self.nodes.contains_key(&id) {
            return Err(DagError::UnknownSignal(id));
        }
        self.computes.insert(id, Box::new(compute));
        if let Some(node) = self.nodes.get_mut(&id) {
            node.kind = SignalKind::Derived;
            node.dirty = true;
        }
        Ok(())
    }

    /// Add a dependency edge `from -> to`. Fail-closed: if the edge would close
    /// a cycle it is not added and [`DagError::CycleDetected`] is returned.
    pub fn connect(&mut self, from: SignalId, to: SignalId) -> Result<(), DagError> {
        if !self.nodes.contains_key(&from) {
            return Err(DagError::UnknownSignal(from));
        }
        if !self.nodes.contains_key(&to) {
            return Err(DagError::UnknownSignal(to));
        }

        let already_linked = self
            .nodes
            .get(&to)
            .map(|node| node.dependencies.contains(&from))
            .unwrap_or(false);
        if let Some(node) = self.nodes.get_mut(&to)
            && !already_linked
        {
            node.dependencies.push(from);
        }
        if let Some(node) = self.nodes.get_mut(&from) {
            node.dependents.insert(to);
        }

        match self.topological_sort() {
            Ok(_) => {
                if let Some(node) = self.nodes.get_mut(&to) {
                    node.dirty = true;
                }
                Ok(())
            }
            Err(error) => {
                if !already_linked {
                    if let Some(node) = self.nodes.get_mut(&to) {
                        node.dependencies.retain(|dep| *dep != from);
                    }
                    if let Some(node) = self.nodes.get_mut(&from) {
                        node.dependents.remove(&to);
                    }
                }
                Err(error)
            }
        }
    }

    /// Update a source signal value, mark it and every transitive dependent
    /// dirty, and return the affected dependents in topological order.
    pub fn update_signal(&mut self, id: SignalId, new_value: i64) -> Vec<SignalId> {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.value = new_value;
            node.dirty = true;
        } else {
            return Vec::new();
        }
        self.mark_dependents_dirty(id)
    }

    /// Fallible form of [`Self::update_signal`] used by the ECS bridge.
    pub fn set_value(&mut self, id: SignalId, new_value: i64) -> Result<Vec<SignalId>, DagError> {
        if !self.nodes.contains_key(&id) {
            return Err(DagError::UnknownSignal(id));
        }
        Ok(self.update_signal(id, new_value))
    }

    /// Mark a node and its transitive dependents dirty without changing values,
    /// returning the affected dependents in topological order.
    pub fn mark_dirty(&mut self, id: SignalId) -> Result<Vec<SignalId>, DagError> {
        if !self.nodes.contains_key(&id) {
            return Err(DagError::UnknownSignal(id));
        }
        if let Some(node) = self.nodes.get_mut(&id) {
            node.dirty = true;
        }
        Ok(self.mark_dependents_dirty(id))
    }

    fn mark_dependents_dirty(&mut self, id: SignalId) -> Vec<SignalId> {
        let mut affected: HashSet<SignalId> = HashSet::new();
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let Some(node) = self.nodes.get(&current) else {
                continue;
            };
            for &dependent in &node.dependents {
                if affected.insert(dependent) {
                    stack.push(dependent);
                }
            }
        }
        for &node_id in &affected {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.dirty = true;
            }
        }
        self.order_subset(&affected)
    }

    /// Recompute every dirty node in topological order. Returns the recomputed
    /// nodes; empty when nothing changed. A cyclic graph fails closed with
    /// [`DagError::CycleDetected`] and leaves all values untouched.
    pub fn evaluate(&mut self) -> Result<EvaluationReport, DagError> {
        let order = self.topological_sort()?;
        let mut recomputed = Vec::new();
        for id in order {
            let is_dirty = self.nodes.get(&id).map(|node| node.dirty).unwrap_or(false);
            if !is_dirty {
                continue;
            }
            let inputs = self.dependency_values(id);
            if let Some(compute) = self.computes.get(&id) {
                let next = compute(&inputs);
                if let Some(node) = self.nodes.get_mut(&id) {
                    node.value = next;
                }
                recomputed.push(id);
            }
            if let Some(node) = self.nodes.get_mut(&id) {
                node.dirty = false;
            }
        }
        self.total_recomputations = self
            .total_recomputations
            .saturating_add(recomputed.len() as u64);
        Ok(EvaluationReport { recomputed })
    }

    pub fn get_value(&self, id: SignalId) -> Option<i64> {
        self.nodes.get(&id).map(|node| node.value)
    }

    /// Check whether the current signal graph contains any circular dependencies.
    pub fn has_cycle(&self) -> bool {
        self.topological_sort().is_err()
    }

    /// Computes a valid topological evaluation order for all signals using
    /// Kahn's algorithm. Returns `Err` if a cycle is present.
    pub fn topological_sort(&self) -> Result<Vec<SignalId>, DagError> {
        let mut in_degree: HashMap<SignalId, usize> = HashMap::with_capacity(self.nodes.len());
        for (&id, node) in &self.nodes {
            let resolved = node
                .dependencies
                .iter()
                .filter(|dep| self.nodes.contains_key(dep))
                .count();
            in_degree.insert(id, resolved);
        }

        let mut ready: BTreeSet<SignalId> = in_degree
            .iter()
            .filter(|&(_, &degree)| degree == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut sorted = Vec::with_capacity(self.nodes.len());
        while let Some(current) = ready.pop_first() {
            sorted.push(current);
            if let Some(node) = self.nodes.get(&current) {
                for &dependent in &node.dependents {
                    if let Some(degree) = in_degree.get_mut(&dependent) {
                        *degree = degree.saturating_sub(1);
                        if *degree == 0 {
                            ready.insert(dependent);
                        }
                    }
                }
            }
        }

        if sorted.len() == self.nodes.len() {
            Ok(sorted)
        } else {
            Err(DagError::CycleDetected)
        }
    }

    /// Read values of all direct dependencies for a signal, in dependency order.
    pub fn get_dependency_values(&self, id: SignalId) -> Vec<i64> {
        self.dependency_values(id)
    }

    fn dependency_values(&self, id: SignalId) -> Vec<i64> {
        self.nodes
            .get(&id)
            .map(|node| {
                node.dependencies
                    .iter()
                    .filter_map(|dep| self.nodes.get(dep).map(|upstream| upstream.value))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn order_subset(&self, subset: &HashSet<SignalId>) -> Vec<SignalId> {
        match self.topological_sort() {
            Ok(order) => order.into_iter().filter(|id| subset.contains(id)).collect(),
            Err(_) => {
                let mut fallback: Vec<SignalId> = subset.iter().copied().collect();
                fallback.sort();
                fallback
            }
        }
    }
}

/// Abstract syntax tree node descriptor for sandboxed plugin widgets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PluginWidgetAst {
    Container {
        direction_column: bool,
        children: Vec<PluginWidgetAst>,
    },
    Label {
        text: String,
        is_bold: bool,
    },
    StatCard {
        title: String,
        value: String,
    },
}

impl PluginWidgetAst {
    /// Validate that the AST tree adheres to max depth and whitelist security constraints.
    pub fn validate_and_sanitize(&self, max_depth: usize) -> bool {
        if max_depth == 0 {
            return false;
        }
        match self {
            PluginWidgetAst::Container { children, .. } => {
                if children.len() > 64 {
                    return false;
                }
                children
                    .iter()
                    .all(|c| c.validate_and_sanitize(max_depth - 1))
            }
            PluginWidgetAst::Label { text, .. } => text.len() <= 1024,
            PluginWidgetAst::StatCard { title, value } => title.len() <= 128 && value.len() <= 128,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dag_topological_sort_and_cycle_detection() {
        let mut dag = ReactiveDag::new();

        let s1 = dag.create_signal(10);
        let s2 = dag.create_signal(20);

        // d1 depends on s1, s2
        let d1 = dag.create_derived(&[s1, s2], 30);
        // d2 depends on d1
        let d2 = dag.create_derived(&[d1], 60);

        assert!(!dag.has_cycle());

        let order = dag.topological_sort().expect("valid sort");
        assert_eq!(order.len(), 4);

        // In order, s1 and s2 must precede d1, and d1 must precede d2
        let pos_s1 = order.iter().position(|&x| x == s1).unwrap();
        let pos_d1 = order.iter().position(|&x| x == d1).unwrap();
        let pos_d2 = order.iter().position(|&x| x == d2).unwrap();

        assert!(pos_s1 < pos_d1);
        assert!(pos_d1 < pos_d2);

        // Update s1 -> returns dirty downstream signals [d1, d2]
        let dirty = dag.update_signal(s1, 15);
        assert!(dirty.contains(&d1));
        assert!(dirty.contains(&d2));
    }

    #[test]
    fn topological_order_is_deterministic_and_edge_respecting() {
        let mut dag = ReactiveDag::new();
        let a = dag.create_signal(1);
        let b = dag.create_signal(2);
        let c = dag
            .create_computed(&[a, b], |values| values[0] + values[1])
            .unwrap();
        let d = dag.create_computed(&[c], |values| values[0] * 10).unwrap();

        let first = dag.topological_sort().expect("acyclic");
        let second = dag.topological_sort().expect("acyclic");
        assert_eq!(first, second, "topological order must be deterministic");

        let position = |id: SignalId| first.iter().position(|&x| x == id).unwrap();
        assert!(position(a) < position(c));
        assert!(position(b) < position(c));
        assert!(position(c) < position(d));
        assert_eq!(dag.get_value(d), Some(30));
    }

    #[test]
    fn cycle_is_rejected_fail_closed_without_mutating_graph() {
        let mut dag = ReactiveDag::new();
        let a = dag.create_signal(1);
        let b = dag.create_computed(&[a], |values| values[0] + 1).unwrap();

        // b already depends on a; closing a -> b -> a must fail and roll back.
        assert_eq!(dag.connect(b, a), Err(DagError::CycleDetected));
        assert!(!dag.has_cycle());
        assert!(!dag.nodes.get(&a).unwrap().dependencies.contains(&b));

        // A self-loop is likewise rejected rather than hanging or panicking.
        assert_eq!(dag.connect(a, a), Err(DagError::CycleDetected));
        assert!(!dag.has_cycle());

        // Unknown signals surface as a typed error, never a panic.
        assert_eq!(
            dag.connect(SignalId(999), a),
            Err(DagError::UnknownSignal(SignalId(999)))
        );
    }

    #[test]
    fn evaluate_recomputes_only_the_dirty_subtree() {
        let mut dag = ReactiveDag::new();
        let source_a = dag.create_signal(1);
        let source_b = dag.create_signal(1);
        let branch_a = dag
            .create_computed(&[source_a], |values| values[0] + 10)
            .unwrap();
        let branch_b = dag
            .create_computed(&[source_b], |values| values[0] + 20)
            .unwrap();
        let leaf = dag
            .create_computed(&[branch_a], |values| values[0] * 2)
            .unwrap();

        assert_eq!(dag.get_value(branch_a), Some(11));
        assert_eq!(dag.get_value(branch_b), Some(21));
        assert_eq!(dag.get_value(leaf), Some(22));

        dag.set_value(source_a, 5).expect("source exists");
        assert_eq!(dag.dirty_count(), 3, "source plus its two dependents");

        let report = dag.evaluate().expect("acyclic");
        assert_eq!(report.recomputed, vec![branch_a, leaf]);
        assert!(!report.recomputed.contains(&branch_b));
        assert_eq!(dag.get_value(branch_a), Some(15));
        assert_eq!(dag.get_value(leaf), Some(30));
        assert_eq!(dag.get_value(branch_b), Some(21), "cold branch untouched");
        assert_eq!(dag.dirty_count(), 0);
    }

    #[test]
    fn evaluate_is_a_noop_when_nothing_changed() {
        let mut dag = ReactiveDag::new();
        let source = dag.create_signal(2);
        let derived = dag
            .create_computed(&[source], |values| values[0] * 3)
            .unwrap();

        dag.set_value(source, 4).expect("source exists");
        let first = dag.evaluate().expect("acyclic");
        assert_eq!(first.recomputed, vec![derived]);
        assert_eq!(dag.get_value(derived), Some(12));

        let second = dag.evaluate().expect("acyclic");
        assert!(second.is_empty(), "clean graph must not recompute");
        assert_eq!(dag.total_recomputations(), 1);
    }

    #[test]
    fn hundred_cycle_churn_stays_bounded_to_the_dirty_subtree() {
        let mut dag = ReactiveDag::new();
        let hot = dag.create_signal(0);
        let mut chain = Vec::new();
        let mut upstream = hot;
        for _ in 0..100 {
            upstream = dag
                .create_computed(&[upstream], |values| values[0] + 1)
                .expect("acyclic chain");
            chain.push(upstream);
        }

        // An independent branch that must never be recomputed by hot churn.
        let cold_source = dag.create_signal(0);
        let cold = dag
            .create_computed(&[cold_source], |values| values[0] - 1)
            .unwrap();

        let mut recomputed_total = 0usize;
        for tick in 1..=100i64 {
            dag.set_value(hot, tick).expect("source exists");
            let report = dag.evaluate().expect("acyclic");
            assert_eq!(
                report.recomputed_count(),
                100,
                "only the hot chain is dirty"
            );
            assert!(!report.recomputed.contains(&cold));
            recomputed_total += report.recomputed_count();
        }

        assert_eq!(recomputed_total, 10_000);
        assert_eq!(dag.total_recomputations(), 10_000);
        assert_eq!(
            dag.get_value(chain[99]),
            Some(200),
            "chain head + 100 ticks"
        );
        assert_eq!(dag.get_value(cold), Some(-1), "cold branch untouched");

        // A frame with no input change does no work at all.
        let idle = dag.evaluate().expect("acyclic");
        assert!(idle.is_empty());
        assert_eq!(dag.total_recomputations(), 10_000);
    }
}

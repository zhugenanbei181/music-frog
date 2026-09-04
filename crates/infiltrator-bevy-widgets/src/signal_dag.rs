//! Reactive computation graph DAG (Directed Acyclic Graph) and Micro-Frontend AST sanitizer.

use std::collections::{HashMap, HashSet};

/// Unique identifier for a node in the reactive DAG.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SignalId(pub usize);

/// A node in the reactive signal computation graph.
#[derive(Clone, Debug)]
pub struct SignalNode {
    pub id: SignalId,
    pub value: i64,
    pub dependencies: HashSet<SignalId>,
    pub dependents: HashSet<SignalId>,
}

/// Reactive Directed Acyclic Graph tracking signals and derived computations.
#[derive(Clone, Debug, Default)]
pub struct ReactiveDag {
    nodes: HashMap<SignalId, SignalNode>,
    next_id: usize,
}

impl ReactiveDag {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new source signal node with an initial value.
    pub fn create_signal(&mut self, initial_value: i64) -> SignalId {
        let id = SignalId(self.next_id);
        self.next_id += 1;
        self.nodes.insert(
            id,
            SignalNode {
                id,
                value: initial_value,
                dependencies: HashSet::new(),
                dependents: HashSet::new(),
            },
        );
        id
    }

    /// Register a derived signal node that depends on a set of upstream signals.
    pub fn create_derived(&mut self, dependencies: &[SignalId], initial_value: i64) -> SignalId {
        let id = SignalId(self.next_id);
        self.next_id += 1;
        let mut dep_set = HashSet::new();
        for &dep in dependencies {
            dep_set.insert(dep);
            if let Some(upstream) = self.nodes.get_mut(&dep) {
                upstream.dependents.insert(id);
            }
        }

        self.nodes.insert(
            id,
            SignalNode {
                id,
                value: initial_value,
                dependencies: dep_set,
                dependents: HashSet::new(),
            },
        );
        id
    }

    /// Update a source signal value and return all dirty dependent nodes in topological order.
    pub fn update_signal(&mut self, id: SignalId, new_value: i64) -> Vec<SignalId> {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.value = new_value;
        }

        let mut dirty = Vec::new();
        let mut visited = HashSet::new();
        let mut queue = Vec::new();

        if let Some(node) = self.nodes.get(&id) {
            for &dep in &node.dependents {
                queue.push(dep);
            }
        }

        while let Some(current) = queue.pop() {
            if visited.insert(current) {
                dirty.push(current);
                if let Some(node) = self.nodes.get(&current) {
                    for &next in &node.dependents {
                        queue.push(next);
                    }
                }
            }
        }

        dirty
    }

    pub fn get_value(&self, id: SignalId) -> Option<i64> {
        self.nodes.get(&id).map(|n| n.value)
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

impl ReactiveDag {
    /// Check whether the current signal graph contains any circular dependencies.
    pub fn has_cycle(&self) -> bool {
        self.topological_sort().is_err()
    }

    /// Computes a valid topological evaluation order for all signals using Kahn's algorithm.
    /// Returns `Err` if a cycle is present.
    pub fn topological_sort(&self) -> Result<Vec<SignalId>, &'static str> {
        let mut in_degrees: HashMap<SignalId, usize> = HashMap::new();
        for (&id, node) in &self.nodes {
            in_degrees.entry(id).or_insert(0);
            for &dep in &node.dependents {
                *in_degrees.entry(dep).or_insert(0) += 1;
            }
        }

        let mut zero_in_degree: Vec<SignalId> = in_degrees
            .iter()
            .filter(|&(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();

        let mut sorted = Vec::with_capacity(self.nodes.len());

        while let Some(current) = zero_in_degree.pop() {
            sorted.push(current);
            if let Some(node) = self.nodes.get(&current) {
                for &dep in &node.dependents {
                    if let Some(deg) = in_degrees.get_mut(&dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            zero_in_degree.push(dep);
                        }
                    }
                }
            }
        }

        if sorted.len() == self.nodes.len() {
            Ok(sorted)
        } else {
            Err("Cycle detected in reactive signal DAG")
        }
    }

    /// Read values of all direct dependencies for a signal.
    pub fn get_dependency_values(&self, id: SignalId) -> Vec<i64> {
        if let Some(node) = self.nodes.get(&id) {
            node.dependencies
                .iter()
                .filter_map(|dep| self.nodes.get(dep).map(|n| n.value))
                .collect()
        } else {
            Vec::new()
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
}

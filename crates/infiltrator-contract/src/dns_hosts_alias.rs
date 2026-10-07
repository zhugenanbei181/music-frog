//! Alias lookup follows the locked kernel's exact, single-label and suffix wildcard precedence.
use super::dns_hosts::is_hosts_ip;
use std::collections::{BTreeMap, BTreeSet, HashSet};
#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    key: Option<String>,
}
impl Node {
    fn insert(&mut self, labels: &[&str], key: &str) -> Option<String> {
        let mut current = self;
        for label in labels.iter().rev() {
            current = current.children.entry((*label).into()).or_default();
        }
        current.key.replace(key.into())
    }
    fn search(&self, labels: &[&str]) -> Option<&str> {
        let Some((label, remaining)) = labels.split_last() else {
            return self.key.as_deref();
        };
        for candidate in [*label, "*"] {
            if let Some(child) = self.children.get(candidate)
                && let Some(key) = child.search(remaining)
            {
                return Some(key);
            }
        }
        self.children.get("").and_then(|child| child.key.as_deref())
    }
}
pub(super) fn alias_issues(groups: &BTreeMap<String, Vec<&str>>) -> (Vec<String>, Vec<String>) {
    let mut root = Node::default();
    let mut conflicts = BTreeSet::new();
    let mut insert = |root: &mut Node, labels: &[&str], key: &str| {
        if let Some(previous) = root.insert(labels, key) {
            let old: BTreeSet<_> = groups[&previous].iter().collect();
            let current: BTreeSet<_> = groups[key].iter().collect();
            if old != current {
                conflicts.insert(previous);
                conflicts.insert(key.to_owned());
            }
        }
    };
    for key in groups.keys() {
        let mut labels: Vec<_> = key.split('.').collect();
        if labels.first() == Some(&"+") {
            insert(&mut root, &labels[1..], key);
            labels[0] = "";
        }
        insert(&mut root, &labels, key);
    }
    let mut cycles = Vec::new();
    for start in groups.keys() {
        let mut key = start.as_str();
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(key) {
                cycles.push(start.clone());
                break;
            }
            let Some(values) = groups.get(key) else {
                break;
            };
            if values.len() != 1 || values[0] == "lan" || is_hosts_ip(values[0]) {
                break;
            }
            let alias = values[0].trim_matches('.').to_lowercase();
            let labels: Vec<_> = alias.split('.').collect();
            let Some(next) = root.search(&labels) else {
                break;
            };
            key = next;
        }
    }
    (cycles, conflicts.into_iter().collect())
}

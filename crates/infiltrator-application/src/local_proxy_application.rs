//! User-owned local proxy collection application service (FLPARITY-037).
//!
//! Manages user-owned proxy nodes separate from subscription profiles, ensuring
//! sensitive secrets never leak into snapshot readbacks, and protecting mutations
//! with revision fencing and duplicate detection.

use infiltrator_contract::local_proxy::{
    AddLocalProxyCommand, AddLocalProxyReceipt, LocalProxyCollection, LocalProxyNode,
    LocalProxySnapshot, RemoveLocalProxyCommand, RemoveLocalProxyReceipt,
};
use infiltrator_ports::local_proxy::LocalProxyStorePort;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct State {
    collection: LocalProxyCollection,
}

/// Application service for managing user-owned local proxy configurations.
#[derive(Clone)]
pub struct LocalProxyApplication {
    store: Option<Arc<dyn LocalProxyStorePort>>,
    state: Arc<Mutex<State>>,
}

impl LocalProxyApplication {
    /// Create a new application instance with optional persistent store.
    pub fn new(store: Option<Arc<dyn LocalProxyStorePort>>) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(State {
                collection: LocalProxyCollection::default(),
            })),
        }
    }

    /// Initialize with an existing collection.
    pub fn with_initial_collection(
        store: Option<Arc<dyn LocalProxyStorePort>>,
        collection: LocalProxyCollection,
    ) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(State { collection })),
        }
    }

    /// Return a safe surface snapshot with all secrets redacted.
    pub fn snapshot(&self) -> LocalProxySnapshot {
        let state = self.state.lock().expect("local proxy state lock");
        let summaries: Vec<_> = state
            .collection
            .nodes
            .iter()
            .map(LocalProxyNode::to_summary)
            .collect();
        LocalProxySnapshot {
            total_count: summaries.len(),
            nodes: summaries,
            revision: state.collection.revision,
            last_updated_epoch_ms: state.collection.last_updated_epoch_ms,
        }
    }

    /// Add a proxy node to the local collection with validation, deduplication, and revision check.
    pub fn add_node(&self, cmd: AddLocalProxyCommand) -> AddLocalProxyReceipt {
        let name = cmd.node.name.trim();
        if name.is_empty() {
            return AddLocalProxyReceipt::InvalidNode {
                reason: "节点名称不能为空".to_owned(),
            };
        }

        let server = cmd.node.server.trim();
        if server.is_empty() {
            return AddLocalProxyReceipt::InvalidNode {
                reason: "服务器地址不能为空".to_owned(),
            };
        }

        if cmd.node.port == 0 {
            return AddLocalProxyReceipt::InvalidNode {
                reason: "端口号必须大于 0".to_owned(),
            };
        }

        let mut state = self.state.lock().expect("local proxy state lock");

        if let Some(expected) = cmd.expected_revision {
            if expected != state.collection.revision {
                return AddLocalProxyReceipt::StaleRevision {
                    expected,
                    actual: state.collection.revision,
                };
            }
        }

        for existing in &state.collection.nodes {
            if existing.id == cmd.node.id {
                return AddLocalProxyReceipt::DuplicateNode {
                    existing_id: existing.id.clone(),
                    name: existing.name.clone(),
                };
            }
            if existing.name == cmd.node.name {
                return AddLocalProxyReceipt::DuplicateNode {
                    existing_id: existing.id.clone(),
                    name: existing.name.clone(),
                };
            }
            if existing.server.eq_ignore_ascii_case(&cmd.node.server)
                && existing.port == cmd.node.port
                && existing.protocol == cmd.node.protocol
            {
                return AddLocalProxyReceipt::DuplicateNode {
                    existing_id: existing.id.clone(),
                    name: existing.name.clone(),
                };
            }
        }

        let now_ms = current_epoch_ms();
        let mut node = cmd.node;
        node.added_at_epoch_ms = now_ms;
        let node_id = node.id.clone();
        let node_name = node.name.clone();

        state.collection.nodes.push(node);
        state.collection.revision += 1;
        state.collection.last_updated_epoch_ms = now_ms;
        let new_revision = state.collection.revision;

        AddLocalProxyReceipt::Added {
            id: node_id,
            name: node_name,
            revision: new_revision,
        }
    }

    /// Remove a proxy node by ID with revision check.
    pub fn remove_node(&self, cmd: RemoveLocalProxyCommand) -> RemoveLocalProxyReceipt {
        let mut state = self.state.lock().expect("local proxy state lock");

        if let Some(expected) = cmd.expected_revision {
            if expected != state.collection.revision {
                return RemoveLocalProxyReceipt::StaleRevision {
                    expected,
                    actual: state.collection.revision,
                };
            }
        }

        if let Some(pos) = state
            .collection
            .nodes
            .iter()
            .position(|n| n.id == cmd.node_id)
        {
            state.collection.nodes.remove(pos);
            state.collection.revision += 1;
            state.collection.last_updated_epoch_ms = current_epoch_ms();
            RemoveLocalProxyReceipt::Removed {
                id: cmd.node_id,
                revision: state.collection.revision,
            }
        } else {
            RemoveLocalProxyReceipt::NotFound { id: cmd.node_id }
        }
    }

    /// Clear all local proxy nodes, returning the new revision.
    pub fn clear(&self) -> u64 {
        let mut state = self.state.lock().expect("local proxy state lock");
        state.collection.nodes.clear();
        state.collection.revision += 1;
        state.collection.last_updated_epoch_ms = current_epoch_ms();
        state.collection.revision
    }

    /// Return the full unredacted nodes for Core engine configuration assembly.
    pub fn get_unredacted_nodes(&self) -> Vec<LocalProxyNode> {
        let state = self.state.lock().expect("local proxy state lock");
        state.collection.nodes.clone()
    }

    /// Total count of local proxy nodes.
    pub fn len(&self) -> usize {
        let state = self.state.lock().expect("local proxy state lock");
        state.collection.nodes.len()
    }

    /// Check whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Persist the current local proxy collection to persistent storage if a store is configured.
    pub async fn persist(&self) -> Result<(), PortError> {
        if let Some(ref store) = self.store {
            let collection = {
                let state = self.state.lock().expect("local proxy state lock");
                state.collection.clone()
            };
            store.save_collection(&collection).await?;
        }
        Ok(())
    }

    /// Reload the local proxy collection from persistent storage if a store is configured.
    pub async fn reload(&self) -> Result<u64, PortError> {
        if let Some(ref store) = self.store {
            let loaded = store.load_collection().await?;
            let mut state = self.state.lock().expect("local proxy state lock");
            state.collection = loaded;
            return Ok(state.collection.revision);
        }
        let state = self.state.lock().expect("local proxy state lock");
        Ok(state.collection.revision)
    }

    /// Access the underlying store port if configured.
    pub fn store(&self) -> Option<&Arc<dyn LocalProxyStorePort>> {
        self.store.as_ref()
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::local_proxy::LocalProxyProtocol;

    fn sample_node(id: &str, name: &str, port: u16) -> LocalProxyNode {
        LocalProxyNode {
            id: id.into(),
            name: name.into(),
            protocol: LocalProxyProtocol::Shadowsocks,
            server: "192.168.1.100".into(),
            port,
            secret: Some("secret123".into()),
            cipher: Some("aes-256-gcm".into()),
            network: Some("tcp".into()),
            tls: false,
            sni: None,
            skip_cert_verify: false,
            source_profile_id: Some("sub-profile-1".into()),
            added_at_epoch_ms: 0,
        }
    }

    #[test]
    fn add_valid_node_succeeds_and_snapshot_is_redacted() {
        let app = LocalProxyApplication::new(None);
        let receipt = app.add_node(AddLocalProxyCommand {
            node: sample_node("n1", "Node 1", 8388),
            expected_revision: None,
        });

        match receipt {
            AddLocalProxyReceipt::Added { id, name, revision } => {
                assert_eq!(id, "n1");
                assert_eq!(name, "Node 1");
                assert_eq!(revision, 1);
            }
            other => panic!("expected Added, got {other:?}"),
        }

        let snap = app.snapshot();
        assert_eq!(snap.total_count, 1);
        assert_eq!(snap.revision, 1);
        assert_eq!(snap.nodes[0].id, "n1");
        assert!(snap.nodes[0].has_secret);

        let unredacted = app.get_unredacted_nodes();
        assert_eq!(unredacted[0].secret, Some("secret123".into()));
    }

    #[test]
    fn duplicate_node_is_rejected() {
        let app = LocalProxyApplication::new(None);
        let res1 = app.add_node(AddLocalProxyCommand {
            node: sample_node("n1", "Node 1", 8388),
            expected_revision: None,
        });
        assert!(matches!(res1, AddLocalProxyReceipt::Added { .. }));

        // Duplicate by ID
        let res2 = app.add_node(AddLocalProxyCommand {
            node: sample_node("n1", "Node 2", 9000),
            expected_revision: None,
        });
        assert!(matches!(res2, AddLocalProxyReceipt::DuplicateNode { .. }));

        // Duplicate by Name
        let res3 = app.add_node(AddLocalProxyCommand {
            node: sample_node("n2", "Node 1", 9001),
            expected_revision: None,
        });
        assert!(matches!(res3, AddLocalProxyReceipt::DuplicateNode { .. }));

        // Duplicate by server:port:protocol
        let res4 = app.add_node(AddLocalProxyCommand {
            node: sample_node("n3", "Node 3", 8388),
            expected_revision: None,
        });
        assert!(matches!(res4, AddLocalProxyReceipt::DuplicateNode { .. }));
    }

    #[test]
    fn invalid_node_is_rejected() {
        let app = LocalProxyApplication::new(None);
        let mut node = sample_node("n1", "", 8388);
        let res = app.add_node(AddLocalProxyCommand {
            node,
            expected_revision: None,
        });
        assert!(matches!(res, AddLocalProxyReceipt::InvalidNode { .. }));

        node = sample_node("n1", "Valid Name", 0);
        let res2 = app.add_node(AddLocalProxyCommand {
            node,
            expected_revision: None,
        });
        assert!(matches!(res2, AddLocalProxyReceipt::InvalidNode { .. }));
    }

    #[test]
    fn stale_revision_is_rejected() {
        let app = LocalProxyApplication::new(None);
        app.add_node(AddLocalProxyCommand {
            node: sample_node("n1", "Node 1", 8388),
            expected_revision: None,
        });

        let res = app.add_node(AddLocalProxyCommand {
            node: sample_node("n2", "Node 2", 8389),
            expected_revision: Some(0), // expected 0, but current is 1
        });
        match res {
            AddLocalProxyReceipt::StaleRevision { expected, actual } => {
                assert_eq!(expected, 0);
                assert_eq!(actual, 1);
            }
            other => panic!("expected StaleRevision, got {other:?}"),
        }
    }

    #[test]
    fn remove_node_workflow() {
        let app = LocalProxyApplication::new(None);
        app.add_node(AddLocalProxyCommand {
            node: sample_node("n1", "Node 1", 8388),
            expected_revision: None,
        });

        let not_found = app.remove_node(RemoveLocalProxyCommand {
            node_id: "non-existent".into(),
            expected_revision: None,
        });
        assert!(matches!(
            not_found,
            RemoveLocalProxyReceipt::NotFound { .. }
        ));

        let removed = app.remove_node(RemoveLocalProxyCommand {
            node_id: "n1".into(),
            expected_revision: Some(1),
        });
        assert!(matches!(
            removed,
            RemoveLocalProxyReceipt::Removed { revision: 2, .. }
        ));
        assert_eq!(app.len(), 0);
    }

    struct MockStore {
        saved: Mutex<LocalProxyCollection>,
    }

    #[async_trait::async_trait]
    impl LocalProxyStorePort for MockStore {
        async fn load_collection(&self) -> Result<LocalProxyCollection, PortError> {
            Ok(self.saved.lock().unwrap().clone())
        }

        async fn save_collection(
            &self,
            collection: &LocalProxyCollection,
        ) -> Result<(), PortError> {
            *self.saved.lock().unwrap() = collection.clone();
            Ok(())
        }
    }

    #[tokio::test]
    async fn persist_and_reload_with_mock_store() {
        let store = Arc::new(MockStore {
            saved: Mutex::new(LocalProxyCollection::default()),
        });
        let app = LocalProxyApplication::new(Some(store.clone()));
        app.add_node(AddLocalProxyCommand {
            node: sample_node("n1", "Node 1", 8388),
            expected_revision: None,
        });

        app.persist().await.expect("persist should succeed");
        assert_eq!(store.saved.lock().unwrap().nodes.len(), 1);

        let app2 = LocalProxyApplication::new(Some(store));
        assert_eq!(app2.len(), 0);
        let rev = app2.reload().await.expect("reload should succeed");
        assert_eq!(rev, 1);
        assert_eq!(app2.len(), 1);
        assert_eq!(app2.snapshot().nodes[0].id, "n1");
    }
}

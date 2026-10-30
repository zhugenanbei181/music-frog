//! User-owned general override application service (FLPARITY-038).
//!
//! Manages ordered, enabled/disabled general overrides with application phase routing,
//! revision fencing, backend engine compatibility, and atomic collection persistence.

use infiltrator_contract::general_override::{
    GeneralOverrideApplyReceipt, GeneralOverrideBackendCompatibility, GeneralOverrideCollection,
    GeneralOverrideItem, GeneralOverrideMutationReceipt, GeneralOverridePhase,
    GeneralOverrideSnapshot, RemoveGeneralOverrideCommand, ReorderGeneralOverridesCommand,
    ToggleGeneralOverrideCommand, UpsertGeneralOverrideCommand,
};
use infiltrator_ports::general_override::GeneralOverrideStorePort;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

struct State {
    collection: GeneralOverrideCollection,
}

/// Application service for managing user-owned general overrides.
#[derive(Clone)]
pub struct GeneralOverrideApplication {
    store: Option<Arc<dyn GeneralOverrideStorePort>>,
    state: Arc<Mutex<State>>,
}

impl GeneralOverrideApplication {
    /// Create a new application instance with optional persistent store.
    pub fn new(store: Option<Arc<dyn GeneralOverrideStorePort>>) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(State {
                collection: GeneralOverrideCollection::default(),
            })),
        }
    }

    /// Initialize with an existing collection.
    pub fn with_initial_collection(
        store: Option<Arc<dyn GeneralOverrideStorePort>>,
        collection: GeneralOverrideCollection,
    ) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(State { collection })),
        }
    }

    /// Return a safe surface snapshot with full item summaries.
    pub fn snapshot(&self) -> GeneralOverrideSnapshot {
        let state = self.state.lock().expect("general override state lock");
        let items: Vec<_> = state
            .collection
            .items
            .iter()
            .map(GeneralOverrideItem::to_summary)
            .collect();
        let enabled_count = items.iter().filter(|i| i.enabled).count();
        GeneralOverrideSnapshot {
            items,
            total_count: state.collection.items.len(),
            enabled_count,
            revision: state.collection.revision,
            last_applied_epoch_ms: state.collection.last_applied_epoch_ms,
        }
    }

    /// Insert or update a general override item with revision fencing and duplicate name detection.
    pub fn upsert(&self, cmd: UpsertGeneralOverrideCommand) -> GeneralOverrideMutationReceipt {
        if cmd.item.name.trim().is_empty() {
            return GeneralOverrideMutationReceipt::InvalidContent {
                reason: "覆写名称不能为空".to_owned(),
            };
        }

        if cmd.item.content.trim().is_empty() {
            return GeneralOverrideMutationReceipt::InvalidContent {
                reason: "覆写内容不能为空".to_owned(),
            };
        }

        let mut state = self.state.lock().expect("general override state lock");

        if let Some(expected) = cmd.expected_revision {
            if expected != state.collection.revision {
                return GeneralOverrideMutationReceipt::StaleRevision {
                    expected,
                    actual: state.collection.revision,
                };
            }
        }

        for existing in &state.collection.items {
            if existing.id == cmd.item.id {
                continue;
            }
            if existing.name == cmd.item.name {
                return GeneralOverrideMutationReceipt::DuplicateName {
                    name: cmd.item.name.clone(),
                };
            }
        }

        let now_ms = current_epoch_ms();
        let mut item = cmd.item;
        item.updated_at_epoch_ms = now_ms;

        if let Some(pos) = state.collection.items.iter().position(|i| i.id == item.id) {
            state.collection.items[pos] = item;
        } else {
            state.collection.items.push(item);
        }
        state.collection.revision += 1;

        let id = state
            .collection
            .items
            .last()
            .map(|i| i.id.clone())
            .unwrap_or_default();
        GeneralOverrideMutationReceipt::Success {
            id,
            revision: state.collection.revision,
        }
    }

    /// Toggle an override item enabled/disabled with revision fencing.
    pub fn toggle(&self, cmd: ToggleGeneralOverrideCommand) -> GeneralOverrideMutationReceipt {
        let mut state = self.state.lock().expect("general override state lock");

        if let Some(expected) = cmd.expected_revision {
            if expected != state.collection.revision {
                return GeneralOverrideMutationReceipt::StaleRevision {
                    expected,
                    actual: state.collection.revision,
                };
            }
        }

        if let Some(item) = state.collection.items.iter_mut().find(|i| i.id == cmd.id) {
            item.enabled = cmd.enabled;
            item.updated_at_epoch_ms = current_epoch_ms();
            state.collection.revision += 1;
            GeneralOverrideMutationReceipt::Success {
                id: cmd.id,
                revision: state.collection.revision,
            }
        } else {
            GeneralOverrideMutationReceipt::NotFound { id: cmd.id }
        }
    }

    /// Reorder general overrides by ID list with revision fencing.
    pub fn reorder(&self, cmd: ReorderGeneralOverridesCommand) -> GeneralOverrideMutationReceipt {
        let mut state = self.state.lock().expect("general override state lock");

        if let Some(expected) = cmd.expected_revision {
            if expected != state.collection.revision {
                return GeneralOverrideMutationReceipt::StaleRevision {
                    expected,
                    actual: state.collection.revision,
                };
            }
        }

        // Check that all provided IDs are present in the collection and lengths match.
        if cmd.ordered_ids.len() != state.collection.items.len() {
            return GeneralOverrideMutationReceipt::InvalidContent {
                reason: "重排 ID 列表长度与覆写项数量不一致".to_owned(),
            };
        }

        let mut new_items = Vec::with_capacity(state.collection.items.len());
        for id in &cmd.ordered_ids {
            if let Some(item) = state.collection.items.iter().find(|i| &i.id == id) {
                new_items.push(item.clone());
            } else {
                return GeneralOverrideMutationReceipt::NotFound { id: id.clone() };
            }
        }

        state.collection.items = new_items;
        for (idx, item) in state.collection.items.iter_mut().enumerate() {
            item.order = idx;
        }
        state.collection.revision += 1;
        GeneralOverrideMutationReceipt::Reordered {
            revision: state.collection.revision,
        }
    }

    /// Remove an override item with revision fencing.
    pub fn remove(&self, cmd: RemoveGeneralOverrideCommand) -> GeneralOverrideMutationReceipt {
        let mut state = self.state.lock().expect("general override state lock");

        if let Some(expected) = cmd.expected_revision {
            if expected != state.collection.revision {
                return GeneralOverrideMutationReceipt::StaleRevision {
                    expected,
                    actual: state.collection.revision,
                };
            }
        }

        if let Some(pos) = state.collection.items.iter().position(|i| i.id == cmd.id) {
            state.collection.items.remove(pos);
            state.collection.revision += 1;
            GeneralOverrideMutationReceipt::Success {
                id: cmd.id,
                revision: state.collection.revision,
            }
        } else {
            GeneralOverrideMutationReceipt::NotFound { id: cmd.id }
        }
    }

    /// Apply the enabled general overrides of the requested phase against the given backend engine.
    ///
    /// Compatibility and content parsing are validated before applying. The receipt reflects
    /// the applied count, unsupported backends, and parse failures in typed form.
    pub fn apply(&self, phase: GeneralOverridePhase, engine: &str) -> GeneralOverrideApplyReceipt {
        let mut state = self.state.lock().expect("general override state lock");

        let eligible: Vec<GeneralOverrideItem> = state
            .collection
            .items
            .iter()
            .filter(|i| i.enabled && i.phase == phase)
            .cloned()
            .collect();

        if eligible.is_empty() {
            return GeneralOverrideApplyReceipt::SkippedNoOverrides { phase };
        }

        for item in &eligible {
            match &item.backend_compatibility {
                GeneralOverrideBackendCompatibility::Universal => {}
                GeneralOverrideBackendCompatibility::MihomoOnly { feature } => {
                    if !engine.eq_ignore_ascii_case("mihomo") {
                        return GeneralOverrideApplyReceipt::BackendUnsupported {
                            engine: engine.to_owned(),
                            feature: feature.clone(),
                        };
                    }
                }
                GeneralOverrideBackendCompatibility::SingBoxOnly { feature } => {
                    if !engine.eq_ignore_ascii_case("singbox")
                        && !engine.eq_ignore_ascii_case("sing-box")
                    {
                        return GeneralOverrideApplyReceipt::BackendUnsupported {
                            engine: engine.to_owned(),
                            feature: feature.clone(),
                        };
                    }
                }
                GeneralOverrideBackendCompatibility::Incompatible { reason } => {
                    return GeneralOverrideApplyReceipt::ParseFailure {
                        id: item.id.clone(),
                        reason: reason.clone(),
                    };
                }
            }
        }

        // Validate YAML/JSON parse for each item content.
        for item in &eligible {
            let parsed: Result<serde_yaml_ng::Value, _> = serde_yaml_ng::from_str(&item.content);
            if parsed.is_err() {
                return GeneralOverrideApplyReceipt::ParseFailure {
                    id: item.id.clone(),
                    reason: "覆写内容不是合法的 YAML 或 JSON 配置".to_owned(),
                };
            }
        }

        state.collection.revision += 1;
        state.collection.last_applied_epoch_ms = current_epoch_ms();
        GeneralOverrideApplyReceipt::Applied {
            applied_count: eligible.len(),
            revision: state.collection.revision,
            phase,
        }
    }

    /// Total count of general overrides.
    pub fn len(&self) -> usize {
        let state = self.state.lock().expect("general override state lock");
        state.collection.items.len()
    }

    /// Check whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Access the underlying store port if configured.
    pub fn store(&self) -> Option<&Arc<dyn GeneralOverrideStorePort>> {
        self.store.as_ref()
    }

    /// Persist the current general override collection to persistent storage if a store is configured.
    pub async fn persist(&self) -> Result<(), PortError> {
        if let Some(ref store) = self.store {
            let collection = {
                let state = self.state.lock().expect("general override state lock");
                state.collection.clone()
            };
            store.save_collection(&collection).await?;
        }
        Ok(())
    }

    /// Reload the general override collection from persistent storage if a store is configured.
    pub async fn reload(&self) -> Result<u64, PortError> {
        if let Some(ref store) = self.store {
            let loaded = store.load_collection().await?;
            let mut state = self.state.lock().expect("general override state lock");
            state.collection = loaded;
            return Ok(state.collection.revision);
        }
        let state = self.state.lock().expect("general override state lock");
        Ok(state.collection.revision)
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

    fn sample_item(id: &str, name: &str, content: &str, order: usize) -> GeneralOverrideItem {
        GeneralOverrideItem {
            id: id.into(),
            name: name.into(),
            content: content.into(),
            enabled: true,
            order,
            phase: GeneralOverridePhase::PreEngineLaunch,
            backend_compatibility: GeneralOverrideBackendCompatibility::Universal,
            updated_at_epoch_ms: 0,
        }
    }

    #[test]
    fn upsert_add_and_update() {
        let app = GeneralOverrideApplication::new(None);

        let receipt = app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });
        assert!(matches!(
            receipt,
            GeneralOverrideMutationReceipt::Success { revision: 1, .. }
        ));
        assert_eq!(app.len(), 1);

        // Update with same ID
        let receipt2 = app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7891", 0),
            expected_revision: Some(1),
        });
        assert!(matches!(
            receipt2,
            GeneralOverrideMutationReceipt::Success { revision: 2, .. }
        ));
        assert_eq!(app.len(), 1);
    }

    #[test]
    fn upsert_rejects_duplicate_name_and_invalid_input() {
        let app = GeneralOverrideApplication::new(None);
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });

        let dup = app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("b", "Override A", "port: 7891", 1),
            expected_revision: None,
        });
        assert!(matches!(
            dup,
            GeneralOverrideMutationReceipt::DuplicateName { .. }
        ));

        let empty_name = app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("b", "  ", "port: 7891", 1),
            expected_revision: None,
        });
        assert!(matches!(
            empty_name,
            GeneralOverrideMutationReceipt::InvalidContent { .. }
        ));

        let empty_content = app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("b", "Override B", "   ", 1),
            expected_revision: None,
        });
        assert!(matches!(
            empty_content,
            GeneralOverrideMutationReceipt::InvalidContent { .. }
        ));
    }

    #[test]
    fn upsert_rejects_stale_revision() {
        let app = GeneralOverrideApplication::new(None);
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });

        let stale = app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("b", "Override B", "port: 7891", 1),
            expected_revision: Some(0),
        });
        match stale {
            GeneralOverrideMutationReceipt::StaleRevision { expected, actual } => {
                assert_eq!(expected, 0);
                assert_eq!(actual, 1);
            }
            other => panic!("expected StaleRevision, got {other:?}"),
        }
    }

    #[test]
    fn toggle_and_remove() {
        let app = GeneralOverrideApplication::new(None);
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });

        let toggled = app.toggle(ToggleGeneralOverrideCommand {
            id: "a".into(),
            enabled: false,
            expected_revision: Some(1),
        });
        assert!(matches!(
            toggled,
            GeneralOverrideMutationReceipt::Success { revision: 2, .. }
        ));

        let not_found = app.toggle(ToggleGeneralOverrideCommand {
            id: "zz".into(),
            enabled: true,
            expected_revision: None,
        });
        assert!(matches!(
            not_found,
            GeneralOverrideMutationReceipt::NotFound { .. }
        ));

        let removed = app.remove(RemoveGeneralOverrideCommand {
            id: "a".into(),
            expected_revision: Some(2),
        });
        assert!(matches!(
            removed,
            GeneralOverrideMutationReceipt::Success { revision: 3, .. }
        ));
        assert_eq!(app.len(), 0);
    }

    #[test]
    fn reorder() {
        let app = GeneralOverrideApplication::new(None);
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("b", "Override B", "port: 7891", 1),
            expected_revision: None,
        });

        let reordered = app.reorder(ReorderGeneralOverridesCommand {
            ordered_ids: vec!["b".into(), "a".into()],
            expected_revision: Some(2),
        });
        assert!(matches!(
            reordered,
            GeneralOverrideMutationReceipt::Reordered { revision: 3 }
        ));

        let snap = app.snapshot();
        assert_eq!(snap.items[0].id, "b");
        assert_eq!(snap.items[0].order, 0);
        assert_eq!(snap.items[1].id, "a");
        assert_eq!(snap.items[1].order, 1);

        // Reorder with invalid length
        let invalid = app.reorder(ReorderGeneralOverridesCommand {
            ordered_ids: vec!["a".into()],
            expected_revision: None,
        });
        assert!(matches!(
            invalid,
            GeneralOverrideMutationReceipt::InvalidContent { .. }
        ));
    }

    #[test]
    fn apply_with_empty_collection() {
        let app = GeneralOverrideApplication::new(None);
        let receipt = app.apply(GeneralOverridePhase::PreEngineLaunch, "mihomo");
        assert!(matches!(
            receipt,
            GeneralOverrideApplyReceipt::SkippedNoOverrides { .. }
        ));
    }

    #[test]
    fn apply_with_universal_item() {
        let app = GeneralOverrideApplication::new(None);
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });

        let receipt = app.apply(GeneralOverridePhase::PreEngineLaunch, "mihomo");
        match receipt {
            GeneralOverrideApplyReceipt::Applied {
                applied_count,
                revision,
                ..
            } => {
                assert_eq!(applied_count, 1);
                assert_eq!(revision, 2);
            }
            other => panic!("expected Applied, got {other:?}"),
        }
    }

    #[test]
    fn apply_with_backend_unsupported() {
        let app = GeneralOverrideApplication::new(None);
        let mut item = sample_item("a", "Mihomo Sniffer", "sniffer: true", 0);
        item.backend_compatibility = GeneralOverrideBackendCompatibility::MihomoOnly {
            feature: "sniffer".into(),
        };
        app.upsert(UpsertGeneralOverrideCommand {
            item,
            expected_revision: None,
        });

        let receipt = app.apply(GeneralOverridePhase::PreEngineLaunch, "singbox");
        assert!(matches!(
            receipt,
            GeneralOverrideApplyReceipt::BackendUnsupported { .. }
        ));
    }

    #[test]
    fn apply_with_parse_failure() {
        let app = GeneralOverrideApplication::new(None);
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Bad YAML", "key: [unclosed: value, {nested", 0),
            expected_revision: None,
        });

        let receipt = app.apply(GeneralOverridePhase::PreEngineLaunch, "mihomo");
        assert!(matches!(
            receipt,
            GeneralOverrideApplyReceipt::ParseFailure { .. }
        ));
    }

    #[test]
    fn apply_filters_by_phase_and_enabled() {
        let app = GeneralOverrideApplication::new(None);
        let mut pre = sample_item("pre", "Pre Item", "port: 7890", 0);
        pre.phase = GeneralOverridePhase::PreEngineLaunch;
        let mut post = sample_item("post", "Post Item", "rules: []", 1);
        post.phase = GeneralOverridePhase::PostConfigLoad;
        let mut disabled = sample_item("disabled", "Disabled Item", "port: 7891", 2);
        disabled.enabled = false;

        app.upsert(UpsertGeneralOverrideCommand {
            item: pre,
            expected_revision: None,
        });
        app.upsert(UpsertGeneralOverrideCommand {
            item: post,
            expected_revision: None,
        });
        app.upsert(UpsertGeneralOverrideCommand {
            item: disabled,
            expected_revision: None,
        });

        let receipt = app.apply(GeneralOverridePhase::PostConfigLoad, "mihomo");
        match receipt {
            GeneralOverrideApplyReceipt::Applied { applied_count, .. } => {
                assert_eq!(applied_count, 1);
            }
            other => panic!("expected Applied, got {other:?}"),
        }
    }

    struct MockStore {
        saved: Mutex<GeneralOverrideCollection>,
    }

    #[async_trait::async_trait]
    impl GeneralOverrideStorePort for MockStore {
        async fn load_collection(&self) -> Result<GeneralOverrideCollection, PortError> {
            Ok(self.saved.lock().unwrap().clone())
        }

        async fn save_collection(
            &self,
            collection: &GeneralOverrideCollection,
        ) -> Result<(), PortError> {
            *self.saved.lock().unwrap() = collection.clone();
            Ok(())
        }
    }

    #[tokio::test]
    async fn persist_and_reload_with_mock_store() {
        let store = Arc::new(MockStore {
            saved: Mutex::new(GeneralOverrideCollection::default()),
        });
        let app = GeneralOverrideApplication::new(Some(store.clone()));
        app.upsert(UpsertGeneralOverrideCommand {
            item: sample_item("a", "Override A", "port: 7890", 0),
            expected_revision: None,
        });

        app.persist().await.expect("persist should succeed");
        assert_eq!(store.saved.lock().unwrap().items.len(), 1);

        let app2 = GeneralOverrideApplication::new(Some(store));
        assert_eq!(app2.len(), 0);
        let rev = app2.reload().await.expect("reload should succeed");
        assert_eq!(rev, 1);
        assert_eq!(app2.len(), 1);
    }
}

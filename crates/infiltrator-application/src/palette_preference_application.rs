//! User-owned palette preference application service (FLPARITY-036).
//!
//! Owns the seed color and palette style as a draft-then-save flow: a draft
//! can be updated and cancelled freely, saving promotes it with a revision
//! bump, and `reload` is the restart readback from the persistent store.
//! No UI surface invents a seed, a style or a second copy of this state.

use infiltrator_contract::palette_preference::PalettePreference;
use infiltrator_contract::palette_preference::PalettePreferenceMutationReceipt;
use infiltrator_contract::palette_preference::PalettePreferenceSnapshot;
use infiltrator_contract::palette_preference::PaletteStylePreference;
use infiltrator_contract::palette_preference::SavePalettePreferenceCommand;
use infiltrator_contract::palette_preference::SeedColorSource;
use infiltrator_contract::palette_preference::UpdatePalettePreferenceDraftCommand;
use infiltrator_ports::palette_preference::PalettePreferenceStorePort;
use lurker_core_ports::error::PortError;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

struct State {
    saved: PalettePreference,
    draft: Option<(SeedColorSource, PaletteStylePreference)>,
}

/// Application service for the user-owned palette preference.
#[derive(Clone)]
pub struct PalettePreferenceApplication {
    store: Option<Arc<dyn PalettePreferenceStorePort>>,
    state: Arc<Mutex<State>>,
}

impl PalettePreferenceApplication {
    /// Create a new application instance with an optional persistent store.
    pub fn new(store: Option<Arc<dyn PalettePreferenceStorePort>>) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(State {
                saved: PalettePreference::default(),
                draft: None,
            })),
        }
    }

    /// Initialize with an already-saved preference (host startup path).
    pub fn with_initial_preference(
        store: Option<Arc<dyn PalettePreferenceStorePort>>,
        saved: PalettePreference,
    ) -> Self {
        Self {
            store,
            state: Arc::new(Mutex::new(State { saved, draft: None })),
        }
    }

    /// The surface-safe snapshot of the current preference state.
    pub fn snapshot(&self) -> PalettePreferenceSnapshot {
        let state = self.state.lock().expect("palette preference state lock");
        state.saved.to_snapshot(state.draft.is_some())
    }

    /// Update the unsaved draft with revision fencing and seed validation.
    pub fn update_draft(
        &self,
        cmd: UpdatePalettePreferenceDraftCommand,
    ) -> PalettePreferenceMutationReceipt {
        if let Err(reason) = cmd.seed.validate() {
            return PalettePreferenceMutationReceipt::InvalidSeed { reason };
        }
        let mut state = self.state.lock().expect("palette preference state lock");
        if cmd.revision != state.saved.revision {
            return PalettePreferenceMutationReceipt::RevisionStale {
                current_revision: state.saved.revision,
            };
        }
        state.draft = Some((cmd.seed, cmd.style));
        PalettePreferenceMutationReceipt::Applied {
            revision: state.saved.revision,
        }
    }

    /// Cancel the unsaved draft; the saved preference stays untouched.
    pub fn cancel_draft(&self) -> PalettePreferenceMutationReceipt {
        let mut state = self.state.lock().expect("palette preference state lock");
        state.draft = None;
        PalettePreferenceMutationReceipt::Applied {
            revision: state.saved.revision,
        }
    }

    /// Promote the draft to the saved preference and bump the revision.
    pub fn save(&self, cmd: SavePalettePreferenceCommand) -> PalettePreferenceMutationReceipt {
        let mut state = self.state.lock().expect("palette preference state lock");
        if cmd.revision != state.saved.revision {
            return PalettePreferenceMutationReceipt::RevisionStale {
                current_revision: state.saved.revision,
            };
        }
        let Some((seed, style)) = state.draft else {
            return PalettePreferenceMutationReceipt::Applied {
                revision: state.saved.revision,
            };
        };
        if let Err(reason) = seed.validate() {
            return PalettePreferenceMutationReceipt::InvalidSeed { reason };
        }
        state.saved.revision += 1;
        state.saved.seed = seed;
        state.saved.style = style;
        state.saved.last_saved_epoch_ms = current_epoch_ms();
        state.draft = None;
        PalettePreferenceMutationReceipt::Applied {
            revision: state.saved.revision,
        }
    }

    /// Persist the saved preference to the store (restart-safe write path).
    pub async fn persist(&self) -> Result<(), PortError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(());
        };
        let saved = self
            .state
            .lock()
            .expect("palette preference state lock")
            .saved;
        store.save_preference(&saved).await
    }

    /// Restart readback: replace the in-memory saved preference from the store.
    pub async fn reload(&self) -> Result<u64, PortError> {
        let Some(store) = self.store.as_ref() else {
            return Ok(0);
        };
        let loaded = store.load_preference().await?;
        let revision = loaded.revision;
        let mut state = self.state.lock().expect("palette preference state lock");
        state.saved = loaded;
        state.draft = None;
        Ok(revision)
    }

    /// The store behind this application, if one is attached.
    pub fn store(&self) -> Option<&Arc<dyn PalettePreferenceStorePort>> {
        self.store.as_ref()
    }
}

fn current_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft_cmd(
        seed: SeedColorSource,
        style: PaletteStylePreference,
        revision: u64,
    ) -> UpdatePalettePreferenceDraftCommand {
        UpdatePalettePreferenceDraftCommand {
            revision,
            seed,
            style,
        }
    }

    #[test]
    fn default_preference_follows_the_theme() {
        let app = PalettePreferenceApplication::new(None);
        let snapshot = app.snapshot();
        assert_eq!(snapshot.seed, SeedColorSource::FollowTheme);
        assert_eq!(snapshot.style, PaletteStylePreference::TonalSpot);
        assert!(!snapshot.has_unsaved_draft);
    }

    #[test]
    fn draft_update_cancel_roundtrip() {
        let app = PalettePreferenceApplication::new(None);
        let receipt = app.update_draft(draft_cmd(
            SeedColorSource::Custom { rgb: 0x33691E },
            PaletteStylePreference::Vibrant,
            0,
        ));
        assert!(matches!(
            receipt,
            PalettePreferenceMutationReceipt::Applied { revision: 0 }
        ));
        assert!(app.snapshot().has_unsaved_draft);

        let receipt = app.cancel_draft();
        assert!(matches!(
            receipt,
            PalettePreferenceMutationReceipt::Applied { revision: 0 }
        ));
        let snapshot = app.snapshot();
        assert!(!snapshot.has_unsaved_draft);
        assert_eq!(snapshot.seed, SeedColorSource::FollowTheme);
    }

    #[test]
    fn invalid_seed_is_rejected() {
        let app = PalettePreferenceApplication::new(None);
        let receipt = app.update_draft(draft_cmd(
            SeedColorSource::Custom { rgb: 0x01FF_FFFF },
            PaletteStylePreference::Vibrant,
            0,
        ));
        assert!(matches!(
            receipt,
            PalettePreferenceMutationReceipt::InvalidSeed { .. }
        ));
        assert!(!app.snapshot().has_unsaved_draft);
    }

    #[test]
    fn stale_revision_is_rejected_with_the_current_revision() {
        let app = PalettePreferenceApplication::new(None);
        let receipt = app.update_draft(draft_cmd(
            SeedColorSource::Custom { rgb: 0x33691E },
            PaletteStylePreference::Vibrant,
            7,
        ));
        assert!(matches!(
            receipt,
            PalettePreferenceMutationReceipt::RevisionStale {
                current_revision: 0
            }
        ));
    }

    #[test]
    fn save_promotes_the_draft_and_bumps_the_revision() {
        let app = PalettePreferenceApplication::new(None);
        app.update_draft(draft_cmd(
            SeedColorSource::Custom { rgb: 0x1976D2 },
            PaletteStylePreference::Expressive,
            0,
        ));
        let receipt = app.save(SavePalettePreferenceCommand { revision: 0 });
        assert!(matches!(
            receipt,
            PalettePreferenceMutationReceipt::Applied { revision: 1 }
        ));
        let snapshot = app.snapshot();
        assert_eq!(snapshot.seed, SeedColorSource::Custom { rgb: 0x1976D2 });
        assert_eq!(snapshot.style, PaletteStylePreference::Expressive);
        assert_eq!(snapshot.revision, 1);
        assert!(!snapshot.has_unsaved_draft);
        assert!(snapshot.last_saved_epoch_ms > 0);
    }

    #[test]
    fn save_without_a_draft_is_a_no_op() {
        let app = PalettePreferenceApplication::new(None);
        let receipt = app.save(SavePalettePreferenceCommand { revision: 0 });
        assert!(matches!(
            receipt,
            PalettePreferenceMutationReceipt::Applied { revision: 0 }
        ));
        assert_eq!(app.snapshot().revision, 0);
    }

    struct MockStore {
        saved: Mutex<Option<PalettePreference>>,
    }

    impl MockStore {
        fn new() -> Self {
            Self {
                saved: Mutex::new(None),
            }
        }
    }

    #[async_trait::async_trait]
    impl PalettePreferenceStorePort for MockStore {
        async fn load_preference(&self) -> Result<PalettePreference, PortError> {
            self.saved
                .lock()
                .expect("mock store lock")
                .ok_or(PortError::NotFound("palette preference".to_owned()))
        }

        async fn save_preference(&self, preference: &PalettePreference) -> Result<(), PortError> {
            *self.saved.lock().expect("mock store lock") = Some(*preference);
            Ok(())
        }
    }

    #[tokio::test]
    async fn persist_and_reload_roundtrip_is_the_restart_readback() {
        let store = Arc::new(MockStore::new());
        let app = PalettePreferenceApplication::new(Some(
            store.clone() as Arc<dyn PalettePreferenceStorePort>
        ));
        app.update_draft(draft_cmd(
            SeedColorSource::Custom { rgb: 0xF9A825 },
            PaletteStylePreference::Spritz,
            0,
        ));
        app.save(SavePalettePreferenceCommand { revision: 0 });
        app.persist().await.expect("persist must succeed");

        // A fresh application reads the persisted preference back (restart).
        let restarted = PalettePreferenceApplication::new(Some(
            store.clone() as Arc<dyn PalettePreferenceStorePort>
        ));
        let revision = restarted.reload().await.expect("reload must succeed");
        assert_eq!(revision, 1);
        let snapshot = restarted.snapshot();
        assert_eq!(snapshot.seed, SeedColorSource::Custom { rgb: 0xF9A825 });
        assert_eq!(snapshot.style, PaletteStylePreference::Spritz);
        assert_eq!(snapshot.revision, 1);
    }

    #[tokio::test]
    async fn reload_without_a_store_reports_zero() {
        let app = PalettePreferenceApplication::new(None);
        assert_eq!(app.reload().await.expect("no store reload"), 0);
    }
}

//! Headless tests for the shared Overview layout and sync conflict commands.
//!
//! Both commands used to answer "unsupported"; the shared application now
//! dispatches them to their owner and answers with the owner's typed readback.

use super::*;
use crate::sync_conflict_test_support::{FakeProfileStore, FakeSyncPort};
use async_trait::async_trait;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::overview_layout::{
    OverviewCardKind, OverviewCardMoveDirection, OverviewLayoutSnapshot,
};
use infiltrator_contract::sync::{SyncConflict, SyncConflictResolution};
use infiltrator_domain::settings::AppSettings;
use infiltrator_ports::settings_store::SettingsStore;
use lurker_core_ports::error::PortError;
use std::sync::{Arc, Mutex};

/// The fake settings owner: it reports the configuration root the conflict
/// copies live in.
#[derive(Default)]
struct FakeSettingsStore {
    settings: Mutex<AppSettings>,
}

impl FakeSettingsStore {
    fn with_configs_dir(configs_dir: &str) -> Self {
        let store = Self::default();
        {
            let mut settings = store.settings.lock().expect("settings lock");
            settings.configs_dir = Some(configs_dir.to_owned());
        }
        store
    }
}

#[async_trait]
impl SettingsStore for FakeSettingsStore {
    async fn load(&self) -> Result<AppSettings, PortError> {
        Ok(self.settings.lock().expect("settings lock").clone())
    }

    async fn load_hydrated(&self) -> Result<AppSettings, PortError> {
        self.load().await
    }

    async fn save(&self, settings: &AppSettings) -> Result<(), PortError> {
        *self.settings.lock().expect("settings lock") = settings.clone();
        Ok(())
    }
}

fn layout_application() -> CommandApplication {
    CommandApplication::new().with_overview_layout(OverviewLayoutApplication::new())
}

#[tokio::test]
async fn a_layout_move_is_dispatched_to_the_owner_and_answers_with_its_order() {
    let application = layout_application();
    let output = application
        .execute_output(CommandIntent::MoveOverviewCard {
            kind: OverviewCardKind::Traffic,
            direction: OverviewCardMoveDirection::Up,
        })
        .await
        .expect("the layout owner answers");

    let snapshot = output.into_overview_layout().expect("layout readback");
    assert_eq!(snapshot.order[0], OverviewCardKind::Traffic);
    assert_eq!(snapshot.order[1], OverviewCardKind::RoutingIntentSegment);
    assert!(snapshot.is_customized);
}

#[tokio::test]
async fn a_layout_reorder_is_dispatched_to_the_owner_and_answers_with_its_order() {
    let application = layout_application();
    let order = vec![OverviewCardKind::Quota, OverviewCardKind::Traffic];
    let output = application
        .execute_output(CommandIntent::ReorderOverviewCards {
            order: order.clone(),
        })
        .await
        .expect("the layout owner answers");

    assert_eq!(
        output
            .into_overview_layout()
            .expect("layout readback")
            .order,
        order
    );
}

#[tokio::test]
async fn a_layout_reset_restores_the_canonical_order() {
    let application = layout_application();
    application
        .execute_output(CommandIntent::MoveOverviewCard {
            kind: OverviewCardKind::Traffic,
            direction: OverviewCardMoveDirection::Up,
        })
        .await
        .expect("the layout owner answers");

    let output = application
        .execute_output(CommandIntent::ResetOverviewCardOrder)
        .await
        .expect("the layout owner answers");
    let snapshot = output.into_overview_layout().expect("layout readback");
    assert_eq!(snapshot.order, OverviewCardKind::DEFAULT_ORDER.to_vec());
    assert!(!snapshot.is_customized);
}

#[tokio::test]
async fn a_layout_command_without_the_owner_is_refused_with_a_typed_failure() {
    let application = CommandApplication::new();
    let failure = application
        .execute_output(CommandIntent::MoveOverviewCard {
            kind: OverviewCardKind::Traffic,
            direction: OverviewCardMoveDirection::Up,
        })
        .await
        .expect_err("no layout owner is composed");

    assert_eq!(failure.code, ErrorCode::NotReady);
}

#[tokio::test]
async fn an_invalid_layout_order_is_refused_with_a_typed_failure() {
    let application = layout_application();
    let failure = application
        .execute_output(CommandIntent::ReorderOverviewCards {
            order: vec![OverviewCardKind::Traffic, OverviewCardKind::Traffic],
        })
        .await
        .expect_err("a repeated card is not an order");

    assert_eq!(failure.code, ErrorCode::InvalidInput);
}

fn conflict_application(
    port: Arc<FakeSyncPort>,
    store: Arc<FakeProfileStore>,
) -> CommandApplication {
    CommandApplication::new()
        .with_sync(SyncApplication::new(port))
        .with_profile(ProfileApplication::new(store))
        .with_settings(SettingsApplication::new(Arc::new(
            FakeSettingsStore::with_configs_dir("/fake/configs"),
        )))
}

#[tokio::test]
async fn keeping_the_local_configuration_answers_with_the_resolution_receipt() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let application = conflict_application(Arc::clone(&port), Arc::clone(&store));

    let output = application
        .execute_output(CommandIntent::ResolveConflictKeepLocal)
        .await
        .expect("the sync owner resolves its conflict");
    let receipt = output
        .into_sync_conflict_resolution()
        .expect("conflict resolution receipt");
    assert_eq!(receipt.resolution, SyncConflictResolution::KeepLocal);
    assert_eq!(receipt.conflict.profile, "main");
    // The owner's state really changed.
    assert!(port.discarded());
    assert_eq!(store.saves(), 0);
}

#[tokio::test]
async fn taking_the_remote_configuration_answers_with_the_resolution_receipt() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let application = conflict_application(Arc::clone(&port), Arc::clone(&store));

    let output = application
        .execute_output(CommandIntent::ResolveConflictTakeRemote)
        .await
        .expect("the sync owner resolves its conflict");
    let receipt = output
        .into_sync_conflict_resolution()
        .expect("conflict resolution receipt");
    assert_eq!(receipt.resolution, SyncConflictResolution::TakeRemote);
    // The local profile really holds the remote content.
    assert_eq!(store.content(), "mode: rule\n");
    assert_eq!(store.saves(), 1);
}

#[tokio::test]
async fn a_conflict_resolution_without_a_pending_conflict_is_refused() {
    let port = Arc::new(FakeSyncPort::without_conflict());
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let application = conflict_application(Arc::clone(&port), Arc::clone(&store));

    let failure = application
        .execute_output(CommandIntent::ResolveConflictKeepLocal)
        .await
        .expect_err("no conflict is pending");
    assert_eq!(failure.code, ErrorCode::InvalidState);
}

#[tokio::test]
async fn a_stale_conflict_answer_cannot_resolve_the_next_conflict() {
    let port = Arc::new(FakeSyncPort::with_conflict("mode: rule\n"));
    port.publishing_successor(SyncConflict {
        profile: "other".to_owned(),
        remote_path: "/fake/configs/other.remote-conflict.yaml".to_owned(),
    });
    let store = Arc::new(FakeProfileStore::with_content("mode: direct\n"));
    let application = conflict_application(Arc::clone(&port), Arc::clone(&store));

    let failure = application
        .execute_output(CommandIntent::ResolveConflictKeepLocal)
        .await
        .expect_err("the owner moved on to another conflict");
    assert_eq!(failure.code, ErrorCode::NotReady);
    assert!(failure.retryable);
}

#[tokio::test]
async fn a_conflict_command_without_the_sync_owner_is_refused_with_a_typed_failure() {
    let application = CommandApplication::new();
    let failure = application
        .execute_output(CommandIntent::ResolveConflictKeepLocal)
        .await
        .expect_err("no sync owner is composed");

    assert_eq!(failure.code, ErrorCode::NotReady);
}

#[tokio::test]
async fn the_typed_outputs_match_their_intents() {
    let layout = CommandOutput::OverviewLayoutApplied(OverviewLayoutSnapshot::default());
    assert!(
        layout
            .validate_for(&CommandIntent::ResetOverviewCardOrder)
            .is_ok()
    );
    assert!(
        layout
            .validate_for(&CommandIntent::ResolveConflictKeepLocal)
            .is_err()
    );
}

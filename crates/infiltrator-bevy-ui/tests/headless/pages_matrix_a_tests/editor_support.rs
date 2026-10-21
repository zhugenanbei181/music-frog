//! Coherent editor document/sidecar observations for native matrix scenarios.
use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::world::World;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::ui::Checked;
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use infiltrator_bevy_ui::pages::profiles::{ProfileItem, ProfilesProjection};
use infiltrator_bevy_ui::pages::profiles_aggregator::{
    AggregatorSourceToggle, AggregatorSwitch, AggregatorSwitchKind,
};
use infiltrator_bevy_ui::pages::profiles_editor_panes::ProfileEditorPane;
use infiltrator_contract::aggregator::{
    AggregationDraft, AggregationRenameRule, AggregationTemplate,
};
use infiltrator_contract::profile_document::{DocumentInspection, ProfileDocumentSnapshot};
use infiltrator_contract::profile_editor_read::{ProfileEditorReadSnapshot, ProfileReadStatus};
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::snapshot_history::{SnapshotEntry, SnapshotHistorySnapshot};
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_test_support::subscription_filter_fixture::{self, FIXTURE_DOCUMENT, observation};
use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_contract::profile_protection::ProfileWriteProtection;

pub(super) fn snapshot_history_fixture() -> SnapshotHistorySnapshot {
    SnapshotHistorySnapshot {
        profile: "main".to_owned(),
        entries: vec![
            SnapshotEntry {
                id: "/fake/configs/main-history/snap-002.yaml".to_owned(),
                file_name: "1750000100000-cafebabe.yaml".to_owned(),
                timestamp_millis: 1_750_000_100_000,
                sha256: "cafebabe00112233445566778899aabbccddeeff00112233445566778899aabb"
                    .to_owned(),
                is_newest: true,
                is_duplicate: false,
            },
            SnapshotEntry {
                id: "/fake/configs/main-history/snap-001.yaml".to_owned(),
                file_name: "1750000000000-deadbeef.yaml".to_owned(),
                timestamp_millis: 1_750_000_000_000,
                sha256: "deadbeef00112233445566778899aabbccddeeff00112233445566778899aabb"
                    .to_owned(),
                is_newest: false,
                is_duplicate: true,
            },
        ],
        keep_limit: 20,
        pending_prune: 1,
        duplicate_entries: 1,
        last_prune: None,
    }
}

pub(super) fn snapshot_diff_fixture() -> YamlAstDiffSnapshot {
    YamlAstDiffSnapshot::demo_fixture().with_source_path("/fake/configs/main-history/snap-001.yaml")
}

pub(super) fn snapshot_diff_page_projection(
    diff: Option<YamlAstDiffSnapshot>,
) -> ProfilesProjection {
    ProfilesProjection {
        profiles: vec![ProfileItem {
            id: "main".to_owned(),
            name: "主力高速订阅 (Primary VIP)".to_owned(),
            url: "https://subscribe.musicfrog.io/main".to_owned(),
            updated_at: "2026-09-22 08:30".to_owned(),
            upload_bytes: Some(0),
            download_bytes: Some(0),
            total_bytes: Some(0),
            is_active: true,
            user_agent: String::new(),
            insecure_skip_verify: false,
            etag: None,
            last_modified: None,
            has_backup: false,
            cron_expression: None,
            auto_update_enabled: false,
            update_interval_hours: None,
            next_update: None,
            auto_reload_core: true,
            filter: Default::default(),
            filter_source: observation("main", FIXTURE_DOCUMENT, Default::default())
                .map(|obs| obs.workspace_identity),
            write_protection: ProfileWriteProtection::RemoteSubscription,
            failure: None,
        }],
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
        yaml_ast_diff: diff,
        snapshot_history: None,
        apply_transaction: None,
        editor_read: Default::default(),
        profile_document: None,
        profile_options: None,
        script_sandbox: None,
        script_export: None,
    }
}

pub(super) fn aggregation_page_projection() -> ProfilesProjection {
    ProfilesProjection {
        profiles: vec![],
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: Some(super::aggregation_preview_report()),
        aggregation_templates: vec![AggregationTemplate {
            name: "已保存模板".to_owned(),
            draft: AggregationDraft {
                source_profiles: vec!["Primary VIP".to_owned()],
                target_name: "Template-Target".to_owned(),
                deduplicate: false,
                deduplicate_names: true,
                geo_cluster: false,
                generate_groups: true,
                remove_emojis: false,
                rename_rules: vec![AggregationRenameRule {
                    pattern: "-广告$".to_owned(),
                    replacement: String::new(),
                }],
                custom_groups: Vec::new(),
                availability_precheck: false,
                activate_after_create: true,
            },
            updated_at: "2026-09-22T11:00:00+00:00".to_owned(),
        }],
        aggregation_templates_available: true,
        yaml_ast_diff: None,
        snapshot_history: None,
        apply_transaction: None,
        editor_read: Default::default(),
        profile_document: None,
        profile_options: None,
        script_sandbox: None,
        script_export: None,
    }
}

pub(super) fn set_source_checked(app: &mut App, name: &str, checked: bool) {
    let child = {
        let mut query = app
            .world_mut()
            .query::<(&AggregatorSourceToggle, &Children)>();
        let mut found = None;
        for (toggle, children) in query.iter(app.world()) {
            if toggle.1 == name {
                found = children.iter().next().copied();
                break;
            }
        }
        found.expect("source toggle row")
    };
    let mut entity = app.world_mut().entity_mut(child);
    if checked {
        entity.insert(Checked);
    } else {
        entity.remove::<Checked>();
    }
}

pub(super) fn set_switch_checked(app: &mut App, kind: AggregatorSwitchKind, checked: bool) {
    let child = {
        let mut query = app.world_mut().query::<(&AggregatorSwitch, &Children)>();
        let mut found = None;
        for (switch, children) in query.iter(app.world()) {
            if switch.0 == kind {
                found = children.iter().next().copied();
                break;
            }
        }
        found.expect("switch row")
    };
    let mut entity = app.world_mut().entity_mut(child);
    if checked {
        entity.insert(Checked);
    } else {
        entity.remove::<Checked>();
    }
}

pub(super) fn keyboard_press(logical_key: Key, text: Option<&str>) -> KeyboardInput {
    KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key,
        state: ButtonState::Pressed,
        text: text.map(|value| value.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

pub(super) fn subtree_text_count(world: &World, root: Entity) -> usize {
    let mut count = usize::from(world.get::<Text>(root).is_some());
    if let Some(children) = world.get::<Children>(root) {
        for child in children.iter() {
            count += subtree_text_count(world, *child);
        }
    }
    count
}

pub(super) fn snippet_button_entity(app: &mut App, index: usize) -> Entity {
    use infiltrator_bevy_ui::pages::profiles_editor::ProfileEditorSnippetButton;
    let mut query = app
        .world_mut()
        .query::<(Entity, &ProfileEditorSnippetButton)>();
    query
        .iter(app.world())
        .find(|(_, button)| button.index == index)
        .map(|(entity, _)| entity)
        .expect("snippet button")
}

pub(super) fn pane_button_entity(app: &mut App, pane: ProfileEditorPane) -> Entity {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::ProfileEditorPaneButton;
    let mut query = app
        .world_mut()
        .query::<(Entity, &ProfileEditorPaneButton)>();
    query
        .iter(app.world())
        .find(|(_, button)| button.pane == pane)
        .map(|(entity, _)| entity)
        .expect("pane switch button")
}

pub(super) fn pane_area_display(app: &mut App, pane: ProfileEditorPane) -> Display {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::ProfileEditorPaneArea;
    let mut query = app.world_mut().query::<(&ProfileEditorPaneArea, &Node)>();
    query
        .iter(app.world())
        .find(|(area, _)| area.pane == pane)
        .map(|(_, node)| node.display)
        .expect("pane area node")
}

pub(super) fn editor_page_projection(
    protection: ProfileWriteProtection,
    content: &str,
) -> ProfilesProjection {
    let mut projection = snapshot_diff_page_projection(None);
    let workspace_identity =
        subscription_filter_fixture::observation("main", content, Default::default())
            .unwrap()
            .workspace_identity;
    let document = ProfileDocumentSnapshot::new(workspace_identity.clone(), content, protection)
        .with_inspection(DocumentInspection::Inspected(DocumentValidation::Valid));
    projection.editor_read = ProfileEditorReadSnapshot {
        profile: Some("main".into()),
        observed_workspace_identity: Some(workspace_identity.clone()),
        verified_workspace_identity: Some(workspace_identity),
        document: ProfileReadStatus::Ready,
        ..Default::default()
    };
    projection.profile_document = Some(document);
    projection.snapshot_history = Some(snapshot_history_fixture());
    projection
}

pub(super) fn editor_options_page_projection(
    content: &str,
    options: Option<ProfileOptionsSnapshot>,
) -> ProfilesProjection {
    let mut projection = editor_page_projection(ProfileWriteProtection::Editable, content);
    if let Some(options) = &options {
        assert_eq!(
            options.workspace_identity.document.revision,
            subscription_filter_fixture::observation("main", content, Default::default())
                .unwrap()
                .workspace_identity
                .document
                .revision,
            "native editor facts must describe the supplied document bytes"
        );
        projection.editor_read.observed_workspace_identity =
            Some(options.workspace_identity.clone());
        projection.editor_read.verified_workspace_identity =
            Some(options.workspace_identity.clone());
        projection.editor_read.options = ProfileReadStatus::Ready;
        if let Some(document) = &mut projection.profile_document {
            document.workspace_identity = options.workspace_identity.clone();
        }
    }
    projection.profile_options = options;
    projection
}

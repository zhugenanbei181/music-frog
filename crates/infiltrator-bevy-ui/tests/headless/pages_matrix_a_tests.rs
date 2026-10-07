//! Headless integration tests for Page Matrix A (Proxies, Profiles, Connections, Logs, Rules):
//! - Page mounting under ContentSlot
//! - Button activation triggering typed UiCommand submission to CommandSink
//! - In-place subtree restamp on XxxProjectionUpdated events
//! - Empty lists, boundary conditions, and defensive rendering.

#[path = "pages_matrix_a_tests/editor_support.rs"]
mod editor_support;
#[path = "pages_matrix_a_tests/filter_support.rs"]
mod filter_support;
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::query::With;
use bevy::ecs::world::World;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::ui::prelude::{Display, Node};
use bevy::ui::widget::Text;
use bevy::ui::{Checked, ScrollPosition};
use bevy::ui_widgets::Activate;
use editor_support::{editor_options_page_projection, editor_page_projection};
use filter_support::{filter_field_entity, filter_field_text};
use infiltrator_application::logical_rule_projection::project_logical_rule;
use infiltrator_application::rule_source_identity::rule_workspace;
use infiltrator_application::rule_tracer_application::RuleTracerApplication;
use infiltrator_application::subscription_filter_fixture::{FIXTURE_DOCUMENT, observation};
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommand, UiCommandSink};
use infiltrator_bevy_ui::pages::connections::*;
use infiltrator_bevy_ui::pages::connections_confirm::{
    CloseAllConfirmationAction, CloseAllConfirmationRoot,
};
use infiltrator_bevy_ui::pages::connections_drawer::*;
use infiltrator_bevy_ui::pages::connections_idle::*;
use infiltrator_bevy_ui::pages::connections_view::*;
use infiltrator_bevy_ui::pages::logs::*;
use infiltrator_bevy_ui::pages::profiles::*;
use infiltrator_bevy_ui::pages::profiles_aggregator::{
    AddAggregatorCustomGroupButton, AggregatorCustomGroupKeywordsField,
    AggregatorCustomGroupNameField, AggregatorNameField, AggregatorRenamesField,
    AggregatorSourceToggle, AggregatorSwitch, AggregatorSwitchKind, AggregatorTemplateNameField,
    DeleteAggregationTemplateButton, PreviewAggregationButton, ReAggregateTemplateButton,
    SaveAggregatedProfileButton, SaveAggregationTemplateButton, UseAggregationTemplateButton,
};
use infiltrator_bevy_ui::pages::profiles_diff::{
    RefreshSnapshotDiffButton, RollbackSnapshotButton, SnapshotDiffModeButton,
};
use infiltrator_bevy_ui::pages::profiles_editor_panes::ProfileEditorPane;
use infiltrator_bevy_ui::pages::profiles_import::{
    ChooseLocalFileButton, ImportLocalFileButton, ProfilesImportRoot,
    RestoreSubscriptionBackupButton, SaveUserAgentButton, SubscriptionBackupStatus,
    SubscriptionInsecureToggle, SubscriptionUserAgentField,
};
use infiltrator_bevy_ui::pages::profiles_import_channels::{
    ImportClipboardSubscriptionButton, ImportLocalPathField, ImportLocalSubscriptionButton,
    ImportSubscriptionNameField, ImportSubscriptionUrlButton, ImportSubscriptionUrlField,
    SaveSubscriptionFilterButton, SubscriptionFilterIncludeField,
};
use infiltrator_bevy_ui::pages::profiles_subscription_policy::{
    SaveSubscriptionAutoReloadButton, SaveSubscriptionPolicyButton, SubscriptionAutoReloadToggle,
    SubscriptionPolicyCronField, SubscriptionPolicyIntervalField,
};
use infiltrator_bevy_ui::pages::proxies::*;
use infiltrator_bevy_ui::pages::rules::*;
use infiltrator_bevy_ui::pages::rules_builder::{
    AddCustomRuleButton, InjectGamePresetsButton, RuleBuilderSelection, RulePayloadField,
    RuleTargetField, RuleTypeChip, RulesBuilderState,
};
use infiltrator_bevy_ui::pages::rules_edit::{
    RuleMoveDownButton, RuleMoveUpButton, RuleToggleButton,
};
use infiltrator_bevy_ui::pages::rules_mrs::{RulesMrsRoot, UnpackRuleProviderButton};
use infiltrator_bevy_ui::pages::rules_projection::{RuleTypeBadge, RuleTypeText};
use infiltrator_bevy_ui::pages::rules_subrules::{
    RulesSubRuleState, SubRuleConditionRow, SubRuleInsertButton, SubRuleOperatorChip,
    SubRulePresetButton, SubRulePreviewLine, SubRuleRemoveConditionButton, SubRuleTargetField,
};
use infiltrator_bevy_ui::pages::rules_tracer::{
    ApplyTracerRuleOverrideButton, SimulateRuleTraceButton, TracerOverrideTargetField,
    TracerQueryField, TracerSourceIpField,
};
use infiltrator_bevy_ui::pages::rules_view::{
    RuleRow, RuleSearchField, RulesListScrollArea, RulesPageIndicator, RulesPageNextButton,
    RulesViewState, RulesWindowRows,
};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::projection::{OverviewProjection, OverviewSource, SourceKind};
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_ui::surface::{SurfaceSource, overview_projection};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::gesture::SwipeToActionItem;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::state::{TextFieldInput, TextFieldState};
use infiltrator_contract::aggregator::{
    AggregationCustomGroup, AggregationDraft, AggregationRenameRule, AggregationReport,
    AggregationTemplate,
};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::mrs_acceleration::{
    MrsAccelerationSnapshot, MrsBehaviorKind, MrsCompressionKind, MrsItemSnapshot,
};
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::rule_hit_audit::{RuleDeadEntry, RuleDeadReason, RuleHitAuditSnapshot};
use infiltrator_contract::rule_tracer::TracerRuleOverride;
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;
use infiltrator_contract::subscription_filter_form::FilterField;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_tracer::{RuleOverridePort, RuleWorkspace};
use std::sync::Arc;
use std::sync::Mutex;

use crate::support::*;

fn setup_matrix_a_app(sink: Arc<DemoCommandSink>) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::default());
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    app
}

/// Test-controlled shared surface source: the router mounts exactly this
/// snapshot, so a page can be exercised with a hand-built read model.
struct StaticRulesSurface {
    snapshot: SurfaceSnapshot,
}

impl OverviewSource for StaticRulesSurface {
    fn current(&self) -> OverviewProjection {
        overview_projection(&self.snapshot)
    }

    fn kind(&self) -> SourceKind {
        SourceKind::LiveCore
    }
}

impl SurfaceSource for StaticRulesSurface {
    fn surface_snapshot(&self) -> SurfaceSnapshot {
        self.snapshot.clone()
    }
}

#[path = "pages_matrix_a_tests/rules_fixture.rs"]
mod rules_fixture;

fn navigate_to(app: &mut App, route: Route) -> (Entity, Entity) {
    app.world_mut().commands().trigger(RouteChanged(route));
    app.update();
    let slot = content_slot(app.world_mut());
    let (root, mounted_route) = page_root(app.world_mut());
    assert_eq!(mounted_route, route);
    let parent = app
        .world()
        .get::<ChildOf>(root)
        .expect("page root parent")
        .0;
    assert_eq!(parent, slot, "page root is parented under ContentSlot");
    (root, slot)
}
// 1. Proxies Page Tests
// 2. Profiles Page Tests
// 3. Connections Page Tests

fn confirmation_action(app: &mut App, action: CloseAllConfirmationAction) -> Entity {
    app.world_mut()
        .query::<(Entity, &CloseAllConfirmationAction)>()
        .iter(app.world())
        .filter(|(_, value)| **value == action)
        .map(|(entity, _)| entity)
        .last()
        .expect("modal action")
}

/// DUAL-13-12: the render order of the flat rows, read from the container's
/// children in layout order. Each row mounts as a card wrapper around its
/// marked node, so the wrapper's subtree is walked for the row marker.
fn connection_row_order(app: &mut App) -> Vec<usize> {
    let container = app
        .world_mut()
        .query_filtered::<Entity, With<ConnRowsContainer>>()
        .single(app.world())
        .expect("rows container");
    let children: Vec<Entity> = app
        .world()
        .get::<Children>(container)
        .expect("rows container children")
        .iter()
        .copied()
        .collect();
    children
        .iter()
        .map(|child| subtree_row_index(app.world(), *child).unwrap_or(usize::MAX))
        .collect()
}

fn subtree_row_index(world: &World, root: Entity) -> Option<usize> {
    if let Some(row) = world.get::<ConnectionRow>(root) {
        return Some(row.0);
    }
    let children = world.get::<Children>(root)?;
    children
        .iter()
        .find_map(|child| subtree_row_index(world, *child))
}
// 4. Logs Page Tests
// 5. Rules Page Tests

/// DUAL-12-08: an in-memory host apply capability for the Bevy headless test.
struct FakeRuleOverridePort {
    rules: Mutex<Vec<RuleEntry>>,
}

#[async_trait::async_trait]
impl RuleOverridePort for FakeRuleOverridePort {
    async fn load_rule_workspace(&self) -> Result<RuleWorkspace, PortError> {
        Ok(tracer_test_workspace(
            &self.rules.lock().expect("rules lock"),
        ))
    }

    async fn compare_and_apply_rules(
        &self,
        expected: &RuleWorkspace,
        entries: &[RuleEntry],
    ) -> Result<(), PortError> {
        let mut rules = self.rules.lock().expect("rules lock");
        if tracer_test_workspace(&rules).source != expected.source || *rules != expected.rules {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "Source changed",
                true,
            )));
        }
        *rules = entries.to_vec();
        Ok(())
    }
}

fn tracer_test_workspace(rules: &[RuleEntry]) -> RuleWorkspace {
    let raw: Vec<&str> = rules.iter().map(|entry| entry.rule.as_str()).collect();
    let yaml = format!(
        "proxy-groups:\n  - name: PROXY\n    type: select\n    proxies: [DIRECT]\nrules: {}\n",
        serde_json::to_string(&raw).expect("rules flow sequence")
    );
    let mut workspace = rule_workspace("test.yaml".to_owned(), &yaml).expect("test workspace");
    workspace.rules = rules.to_vec();
    workspace
}

fn rules_mrs_test_item(name: &str, behavior: MrsBehaviorKind, rule_count: u32) -> MrsItemSnapshot {
    MrsItemSnapshot {
        name: name.to_owned(),
        behavior,
        format_version: 1,
        compression: MrsCompressionKind::None,
        rule_count,
        payload_size_bytes: 128,
        file_size_bytes: 192,
        sha256_digest: Some("deadbeefcafebabe0123456789abcdef".to_owned()),
        crc32_checksum: Some(1),
        is_mmap_accelerated: true,
        is_valid: true,
        description: String::new(),
        updated_at: "2026-09-06 12:00".to_owned(),
        source_url: None,
        unpack_supported: true,
    }
}

/// A projection with `total` generated rules, for window/paging tests.
fn rules_projection_with(total: usize) -> RulesProjection {
    let mut projection = RulesProjection::demo();
    projection.total_rules = total;
    projection.rules = (0..total)
        .map(|index| RuleItem {
            edit_id: None,
            raw: format!("DOMAIN,host-{index}.example,DIRECT"),
            source_ip: false,
            no_resolve: false,
            failure: None,
            id: index + 1,
            rule_type: "DOMAIN".to_owned(),
            payload: format!("host-{index}.example"),
            proxy: "DIRECT".to_owned(),
            hit_count: Some(0),
            is_enabled: true,
            last_hit_secs: None,
            is_shadowed: false,
            shadow_reason: None,
        })
        .collect();
    projection
}

/// The mounted rules keyword field (the wrapper's text-field child).
fn rules_search_field(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<&Children, With<RuleSearchField>>()
        .single(app.world())
        .expect("search field wrapper")
        .iter()
        .copied()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("search text field")
}

fn mounted_rule_rows(app: &mut App) -> Vec<usize> {
    let mut rows = app.world_mut().query::<&RuleRow>();
    let mut indices: Vec<usize> = rows.iter(app.world()).map(|row| row.0).collect();
    indices.sort_unstable();
    indices
}

fn activate(app: &mut App, entity: Entity) {
    app.world_mut().commands().trigger(Activate { entity });
    app.update();
}

fn payload_field_entity(app: &mut App) -> Entity {
    let children: Vec<Entity> = app
        .world_mut()
        .query_filtered::<&Children, With<RulePayloadField>>()
        .single(app.world())
        .expect("payload wrapper")
        .iter()
        .copied()
        .collect();
    children
        .into_iter()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("payload text field")
}

fn target_field_entity(app: &mut App) -> Entity {
    let children: Vec<Entity> = app
        .world_mut()
        .query_filtered::<&Children, With<RuleTargetField>>()
        .single(app.world())
        .expect("target wrapper")
        .iter()
        .copied()
        .collect();
    children
        .into_iter()
        .find(|child| app.world().get::<TextField>(*child).is_some())
        .expect("target text field")
}

/// Count mounted entities carrying a marker component.
fn count_with<T: Component>(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<T>>()
        .iter(app.world())
        .count()
}

// ---- DUAL-07-02/04/12: subscription fetch options dual surface ---------------

fn subscription_fetch_projection() -> ProfilesProjection {
    ProfilesProjection {
        auto_update_interval_hours: 12,
        updating: false,
        aggregation: None,
        aggregation_templates: Vec::new(),
        aggregation_templates_available: true,
        yaml_ast_diff: None,
        snapshot_history: None,
        apply_transaction: None,
        editor_read: Default::default(),
        profile_document: None,
        profile_options: None,
        script_sandbox: None,
        script_export: None,
        profiles: vec![ProfileItem {
            id: "sub-fetch".to_owned(),
            name: "抓取选项订阅".to_owned(),
            url: "https://fetch.example/sub".to_owned(),
            updated_at: "2026-09-22 09:00".to_owned(),
            upload_bytes: Some(0),
            download_bytes: Some(0),
            total_bytes: Some(0),
            is_active: true,
            user_agent: "ClashVerge/2.0".to_owned(),
            insecure_skip_verify: true,
            etag: Some("\"fetch-etag\"".to_owned()),
            last_modified: Some("Tue, 22 Sep 2026 09:00:00 GMT".to_owned()),
            has_backup: true,
            cron_expression: Some("0 */6 * * *".to_owned()),
            auto_update_enabled: true,
            update_interval_hours: Some(6),
            next_update: None,
            auto_reload_core: true,
            filter_source: observation(
                "sub-fetch",
                FIXTURE_DOCUMENT,
                SubscriptionFilterDraft {
                    include: "香港".into(),
                    exclude: "广告".into(),
                    ..Default::default()
                },
            )
            .map(|o| o.source),
            filter: SubscriptionFilterDraft {
                include: "香港".to_owned(),
                exclude: "广告".to_owned(),
                ..Default::default()
            },
            write_protection: ProfileWriteProtection::RemoteSubscription,
        }],
    }
}

/// Overwrite the single-marker text field's state in a headless app.
fn set_marker_text<M: Component>(app: &mut App, value: &str) {
    let field = {
        let mut wrappers = app.world_mut().query_filtered::<&Children, With<M>>();
        *wrappers
            .single(app.world())
            .expect("field wrapper")
            .iter()
            .next()
            .expect("text field child")
    };
    app.world_mut()
        .get_mut::<TextField>(field)
        .expect("text field state")
        .0
        .apply(TextFieldInput::SetText(value.to_owned()));
}

fn marker_entity<M: Component>(app: &mut App) -> Entity {
    let mut query = app.world_mut().query_filtered::<Entity, With<M>>();
    query.single(app.world()).expect("marker entity")
}

// ---- DUAL-07-03/08: cron schedule + node cleaning pipeline dual surface ------

// ---- DUAL-07-11/13: batch update + safe backup dual surface -------------------

// ---- DUAL-07-09/14: subscription policy + auto-reload + delete --------------

// ---- DUAL-08: aggregator wizard + shared preview dual surface ---------------

/// DUAL-08: a report carrying real region clusters and a master cascade.
fn aggregation_preview_report() -> AggregationReport {
    use infiltrator_contract::aggregator::{
        AggregationReport, GeneratedGroupSnapshot, RegionalClusterSnapshot,
    };
    AggregationReport {
        draft: AggregationDraft {
            source_profiles: vec![
                "主力高速订阅 (Primary VIP)".to_owned(),
                "备用容灾线路 (Backup Anycast)".to_owned(),
                "局域网调试配置 (LAN Lab)".to_owned(),
            ],
            target_name: "Merged-All".to_owned(),
            deduplicate: true,
            deduplicate_names: true,
            geo_cluster: true,
            generate_groups: true,
            remove_emojis: true,
            rename_rules: vec![AggregationRenameRule {
                pattern: "-广告$".to_owned(),
                replacement: String::new(),
            }],
            custom_groups: vec![AggregationCustomGroup {
                name: "流媒体专用".to_owned(),
                group_type: "select".to_owned(),
                member_keywords: vec!["香港".to_owned()],
            }],
            availability_precheck: true,
            activate_after_create: false,
        },
        source_count: 3,
        missing_sources: vec![],
        input_nodes: 32,
        total_nodes: 30,
        duplicates_removed: 2,
        renamed_nodes: 30,
        rule_renamed_nodes: 4,
        invalid_nodes_removed: 2,
        invalid_node_samples: vec!["广告节点: trojan: password is required".to_owned()],
        regions: vec![
            RegionalClusterSnapshot {
                iso: "HK".to_owned(),
                label: "香港".to_owned(),
                flag: "🇭🇰".to_owned(),
                group_name: "香港自动测速".to_owned(),
                node_names: vec!["香港 01".to_owned(), "香港 02".to_owned()],
            },
            RegionalClusterSnapshot {
                iso: "JP".to_owned(),
                label: "日本".to_owned(),
                flag: "🇯🇵".to_owned(),
                group_name: "日本自动测速".to_owned(),
                node_names: vec!["东京 01".to_owned()],
            },
        ],
        groups: vec![
            GeneratedGroupSnapshot {
                name: "🚀 节点选择".to_owned(),
                group_type: "select".to_owned(),
                is_master: true,
                is_custom: false,
                members: vec!["♻️ 自动选择".to_owned(), "香港自动测速".to_owned()],
            },
            GeneratedGroupSnapshot {
                name: "香港自动测速".to_owned(),
                group_type: "url-test".to_owned(),
                is_master: false,
                is_custom: false,
                members: vec!["香港 01".to_owned()],
            },
            GeneratedGroupSnapshot {
                name: "流媒体专用".to_owned(),
                group_type: "select".to_owned(),
                is_master: false,
                is_custom: true,
                members: vec!["香港 01".to_owned(), "香港 02".to_owned()],
            },
        ],
        yaml: "port: 7890\nproxies: []\nproxy-groups:\n  - name: 🚀 节点选择\n".to_owned(),
        generated_at: "2026-09-22T10:00:00+00:00".to_owned(),
    }
}

fn aggregation_page_projection() -> ProfilesProjection {
    ProfilesProjection {
        profiles: vec![],
        auto_update_interval_hours: 0,
        updating: false,
        aggregation: Some(aggregation_preview_report()),
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

/// Flip the `Checked` state of one source row by profile name.
fn set_source_checked(app: &mut App, name: &str, checked: bool) {
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

/// Flip the `Checked` state of one named wizard switch.
fn set_switch_checked(app: &mut App, kind: AggregatorSwitchKind, checked: bool) {
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

// ---- DUAL-09-08/09/12: snapshot diff, rollback and protection chips --------

fn snapshot_diff_fixture() -> YamlAstDiffSnapshot {
    YamlAstDiffSnapshot::demo_fixture().with_source_path("/fake/configs/main-history/snap-001.yaml")
}

fn snapshot_diff_page_projection(diff: Option<YamlAstDiffSnapshot>) -> ProfilesProjection {
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
                .map(|o| o.source),
            write_protection: ProfileWriteProtection::RemoteSubscription,
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

// ---- DUAL-09-03/05/06/07/11/14: history controls + the document editor -----

fn snapshot_history_fixture() -> SnapshotHistorySnapshot {
    use infiltrator_contract::snapshot_history::{SnapshotEntry, SnapshotHistorySnapshot};
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

fn keyboard_press(logical_key: Key, text: Option<&str>) -> KeyboardInput {
    KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key,
        state: ButtonState::Pressed,
        text: text.map(|value| value.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

// ---- DUAL-09-02/04/13: shared viewport, gutter, snippets -------------------

/// Every `Text` entity in a subtree (the bounded-render evidence needs a
/// count, not just a presence check).
fn subtree_text_count(world: &World, root: Entity) -> usize {
    let mut count = usize::from(world.get::<Text>(root).is_some());
    if let Some(children) = world.get::<Children>(root) {
        for child in children.iter() {
            count += subtree_text_count(world, *child);
        }
    }
    count
}

fn snippet_button_entity(app: &mut App, index: usize) -> Entity {
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

// ---- DUAL-09-14: Mixin / Filter panes --------------------------------------

fn pane_button_entity(app: &mut App, pane: ProfileEditorPane) -> Entity {
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

fn pane_area_display(app: &mut App, pane: ProfileEditorPane) -> Display {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::ProfileEditorPaneArea;
    let mut query = app.world_mut().query::<(&ProfileEditorPaneArea, &Node)>();
    query
        .iter(app.world())
        .find(|(area, _)| area.pane == pane)
        .map(|(_, node)| node.display)
        .expect("pane area node")
}

// ---- DUAL-10-08/10/11: shared Mixin studio (preflight + toggles + cascade) --

#[path = "pages_matrix_a_tests/connections.rs"]
mod connections;
#[path = "pages_matrix_a_tests/connections_confirmation.rs"]
mod connections_confirmation;
#[path = "pages_matrix_a_tests/editor_filter_native.rs"]
mod editor_filter_native;
#[path = "pages_matrix_a_tests/filter_form.rs"]
mod filter_form;
#[path = "pages_matrix_a_tests/logs.rs"]
mod logs;
#[path = "pages_matrix_a_tests/operation_panels.rs"]
mod operation_panels;
#[path = "pages_matrix_a_tests/profiles_activate.rs"]
mod profiles_activate;
#[path = "pages_matrix_a_tests/profiles_aggregator.rs"]
mod profiles_aggregator;
#[path = "pages_matrix_a_tests/profiles_auto.rs"]
mod profiles_auto;
#[path = "pages_matrix_a_tests/profiles_delete.rs"]
mod profiles_delete;
#[path = "pages_matrix_a_tests/profiles_editor.rs"]
mod profiles_editor;

#[path = "pages_matrix_a_tests/profiles_editor_locale.rs"]
mod profiles_editor_locale;
#[path = "pages_matrix_a_tests/profiles_empty.rs"]
mod profiles_empty;
#[path = "pages_matrix_a_tests/profiles_fetch.rs"]
mod profiles_fetch;
#[path = "pages_matrix_a_tests/profiles_filter.rs"]
mod profiles_filter;
#[path = "pages_matrix_a_tests/profiles_import.rs"]
mod profiles_import;
#[path = "pages_matrix_a_tests/profiles_mixin.rs"]
mod profiles_mixin;
#[path = "pages_matrix_a_tests/profiles_page.rs"]
mod profiles_page;
#[path = "pages_matrix_a_tests/profiles_projection.rs"]
mod profiles_projection;
#[path = "pages_matrix_a_tests/profiles_restore.rs"]
mod profiles_restore;
#[path = "pages_matrix_a_tests/profiles_save.rs"]
mod profiles_save;
#[path = "pages_matrix_a_tests/profiles_schedule.rs"]
mod profiles_schedule;
#[path = "pages_matrix_a_tests/profiles_script.rs"]
mod profiles_script;
#[path = "pages_matrix_a_tests/profiles_snapshot.rs"]
mod profiles_snapshot;
#[path = "pages_matrix_a_tests/profiles_update.rs"]
mod profiles_update;
#[path = "pages_matrix_a_tests/proxies.rs"]
mod proxies;
#[path = "pages_matrix_a_tests/rules_add.rs"]
mod rules_add;
#[path = "pages_matrix_a_tests/rules_empty.rs"]
mod rules_empty;
#[path = "pages_matrix_a_tests/rules_game.rs"]
mod rules_game;
#[path = "pages_matrix_a_tests/rules_geo.rs"]
mod rules_geo;
#[path = "pages_matrix_a_tests/rules_hit.rs"]
mod rules_hit;
#[path = "pages_matrix_a_tests/rules_json.rs"]
mod rules_json;
#[path = "pages_matrix_a_tests/rules_matrix.rs"]
mod rules_matrix;
#[path = "pages_matrix_a_tests/rules_mrs.rs"]
mod rules_mrs;
#[path = "pages_matrix_a_tests/rules_page.rs"]
mod rules_page;
#[path = "pages_matrix_a_tests/rules_pagination.rs"]
mod rules_pagination;
#[path = "pages_matrix_a_tests/rules_projection.rs"]
mod rules_projection;
#[path = "pages_matrix_a_tests/rules_provider.rs"]
mod rules_provider;
#[path = "pages_matrix_a_tests/rules_refresh.rs"]
mod rules_refresh;
#[path = "pages_matrix_a_tests/rules_search.rs"]
mod rules_search;
#[path = "pages_matrix_a_tests/rules_subrules.rs"]
mod rules_subrules;
#[path = "pages_matrix_a_tests/rules_tab.rs"]
mod rules_tab;
#[path = "pages_matrix_a_tests/rules_toggle.rs"]
mod rules_toggle;
#[path = "pages_matrix_a_tests/rules_tracer.rs"]
mod rules_tracer;
#[path = "pages_matrix_a_tests/rules_truncation.rs"]
mod rules_truncation;
#[path = "pages_matrix_a_tests/rules_type.rs"]
mod rules_type;
#[path = "pages_matrix_a_tests/rules_window.rs"]
mod rules_window;
#[path = "pages_matrix_a_tests/script.rs"]
mod script;

#[path = "pages_matrix_a_tests/connection_inspection.rs"]
mod connection_inspection;

#[path = "pages_matrix_a_tests/connections_grouping.rs"]
mod connections_grouping;

#[path = "pages_matrix_a_tests/connection_search.rs"]
mod connection_search;

#[path = "pages_matrix_a_tests/log_search.rs"]
mod log_search;

#[path = "pages_matrix_a_tests/log_follow.rs"]
mod log_follow;

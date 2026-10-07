//! The Profiles page (配置订阅): subscription management, profile metadata,
//! traffic quotas, manual refresh, and active profile switching.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProfilesLine`],
//! [`ProfileNameText`], [`ProfileTimeText`], [`ProfileTrafficText`], [`ProfileStatusText`]).
//! [`ProfilesPagePlugin`] registers [`apply_profiles_projection`] and action observers
//! once at product assembly. When [`ProfilesProjectionUpdated`]
//! fires, texts and active states restamp in place without tree rebuilds.

#[path = "profiles_query_access.rs"]
pub mod query_access;
use self::query_access::ProfileProjectionTargets;
use infiltrator_composition::demo_identities::{BACKUP_PROFILE, LOCAL_PROFILE, PRIMARY_PROFILE};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::business_panel::{PanelKind, launcher_scene, profiles_panels};
use crate::pages::profiles_editor_copy::replay_profile_protection;
use crate::pages::profiles_filter_dedup::FilterDedupPlugin;
use crate::pages::profiles_snapshot_copy;
use crate::pages::profiles_subscription_copy::sync_status_copy;
use infiltrator_application::profile_metadata_projection;
use infiltrator_bevy_widgets::localization::UiLocale;

use crate::pages::profiles::cards::{header_card_scene, profile_card_scene};
use crate::pages::profiles_aggregator::sync_aggregation_preview;
use crate::pages::profiles_aggregator_wizard::{
    AggregatorComposerState, on_add_aggregator_custom_group, on_aggregation_checkbox_changed,
    on_clear_aggregator_custom_groups, on_delete_aggregation_template, on_preview_aggregation,
    on_reaggregate_template, on_save_aggregated_profile, on_save_aggregation_template,
    on_use_aggregation_template,
};
use crate::pages::profiles_diff::{
    SnapshotDiffViewState, on_refresh_snapshot_diff, on_rollback_snapshot_activated,
    on_snapshot_diff_mode_activated, sync_snapshot_diff,
};
use crate::pages::profiles_diff_history::{
    on_backup_snapshot_activated, on_prune_snapshots_activated, on_refresh_snapshot_history,
    on_snapshot_history_entry_activated, on_snapshot_history_restore_activated,
    on_snapshot_prune_keep_activated,
};
use crate::pages::profiles_editor::profile_editor_scene;
use crate::pages::profiles_import::{
    on_restore_subscription_backup, on_save_subscription_fetch_settings,
    profiles_import_card_scene, sync_subscription_fetch_controls,
};
use crate::pages::profiles_import_channels::{
    on_import_clipboard_subscription, on_import_local_subscription, on_import_subscription_url,
    on_save_subscription_filter,
};
use crate::pages::profiles_script_scene::workbench_scene;
use crate::pages::profiles_script_workbench::ScriptWorkbenchPlugin;
use crate::pages::profiles_subscription_policy::{
    on_save_subscription_auto_reload, on_save_subscription_policy, replay_policy_copy,
    subscription_policy_card_scene, sync_subscription_policy_controls,
};
use crate::route::{PageRoot, Route};
use bevy::app::{App, Plugin, Update};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{FlexDirection, Node, Overflow, Val, percent, px};
/// Root marker on the Profiles page scene.
use bevy::ui_widgets::{Activate, ScrollArea};
use infiltrator_application::subscription_filter_fixture;
use infiltrator_bevy_widgets::gesture::{PullToRefreshState, pull_to_refresh_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::aggregator::{AggregationReport, AggregationTemplate};
use infiltrator_contract::apply_transaction::ApplyTransactionSnapshot;
use infiltrator_contract::error::Failure;
use infiltrator_contract::profile_document::ProfileDocumentSnapshot;
use infiltrator_contract::profile_editor_read::ProfileEditorReadSnapshot;
use infiltrator_contract::profile_options::ProfileOptionsSnapshot;
use infiltrator_contract::profile_protection::ProfileWriteProtection;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::script_export::ScriptExportSnapshot;
use infiltrator_contract::script_sandbox::ScriptSandboxSnapshot;
use infiltrator_contract::snapshot_history::SnapshotHistorySnapshot;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot;
pub mod cards;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ProfilesPageRoot;

/// Marker for text lines updated by the projection observer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfilesLine(pub ProfilesLineKind);

/// Different text lines on the profiles page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProfilesLineKind {
    /// Overview summary: total count and active profile name.
    #[default]
    Summary,
    /// Auto update interval text.
    AutoUpdate,
}

/// Marker for a specific profile card's name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileNameText(pub usize);

/// Marker for a specific profile card's traffic usage text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileTrafficText(pub usize);

/// Marker for a specific profile card's update time text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileTimeText(pub usize);

/// Marker for a specific profile card's active status button text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileStatusText(pub usize);

/// DUAL-09-12: a profile card's write-protection chip. It is restamped by the
/// diff sync system from the shared projection.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileProtectionText(pub usize);

/// Marker for a specific profile card's update-schedule line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileScheduleText(pub usize);

/// Marker and target information for the delete profile button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct DeleteProfileButton {
    pub profile_id: String,
    pub profile_idx: usize,
}

/// Marker and target information for the activate profile button.
#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ActivateProfileButton {
    pub profile_id: String,
    pub profile_idx: usize,
}

/// DUAL-07-11: one-click refresh of every subscription profile on the toolbar.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpdateAllSubscriptionsButton;

/// DUAL-07-05: per-profile "update now" button. Drives the shared
/// `CommandIntent::UpdateProfile`, which applies the application-level
/// retry/backoff and single-flight guard.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpdateProfileButton(pub usize);

/// A single subscription profile snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileItem {
    pub id: String,
    pub name: String,
    pub url: String,
    pub updated_at: String,
    pub upload_bytes: Option<u64>,
    pub download_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub is_active: bool,
    /// Per-profile conditional-request User-Agent (empty = provider default).
    pub user_agent: String,
    /// Per-profile TLS certificate-skip preference.
    pub insecure_skip_verify: bool,
    /// Cached `ETag` validator from the last successful download.
    pub etag: Option<String>,
    /// Cached `Last-Modified` validator from the last successful download.
    pub last_modified: Option<String>,
    /// DUAL-07-13: a transient pre-save `.bak` copy exists and can be restored.
    pub has_backup: bool,
    /// DUAL-07-03: the profile's cron schedule (empty = interval/manual).
    pub cron_expression: Option<String>,
    /// DUAL-07-14: whether the profile participates in scheduled updates.
    pub auto_update_enabled: bool,
    /// DUAL-07-14: the fixed update interval in hours (`None` = cron/manual).
    pub update_interval_hours: Option<u32>,
    /// DUAL-07-14: the next scheduled update instant, RFC3339, when known.
    pub next_update: Option<String>,
    /// DUAL-07-09: reload the running core after this profile's update.
    pub auto_reload_core: bool,
    /// DUAL-07-08: the stored node-keyword filter draft.
    pub filter: SubscriptionFilterDraft,
    pub filter_source: Result<ProfileSourceIdentity, Failure>,
    /// DUAL-09-12: the same write classification the Iced editor and the
    /// application guard use.
    pub write_protection: ProfileWriteProtection,
}

/// Snapshot of the Profiles domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfilesProjection {
    pub profiles: Vec<ProfileItem>,
    pub auto_update_interval_hours: u32,
    pub updating: bool,
    /// DUAL-08: the last shared aggregation preview, or `None` when no draft
    /// has been previewed yet. The surface never builds this locally.
    pub aggregation: Option<AggregationReport>,
    /// DUAL-08-13: persisted aggregation template library.
    pub aggregation_templates: Vec<AggregationTemplate>,
    /// DUAL-08-13: `false` when the host store keeps no template sidecar.
    pub aggregation_templates_available: bool,
    /// DUAL-09-08: the shared snapshot-vs-current AST diff. The surface renders
    /// it directly and never fabricates rows.
    pub yaml_ast_diff: Option<YamlAstDiffSnapshot>,
    /// DUAL-09-06/07: the shared snapshot history (entries + shared prune view).
    pub snapshot_history: Option<SnapshotHistorySnapshot>,
    /// DUAL-09-11: the host core's typed apply-transaction outcome.
    pub apply_transaction: Option<ApplyTransactionSnapshot>,
    /// DUAL-09-03/14: the stored document the editor card renders.
    pub profile_document: Option<ProfileDocumentSnapshot>,
    /// DUAL-09-14: the stored Mixin overlay + filter draft the editor panes
    /// edit; published by the shared sidecar use-case.
    pub profile_options: Option<ProfileOptionsSnapshot>,
    pub editor_read: ProfileEditorReadSnapshot,
    /// DUAL-10-05/14: the shared script-sandbox read model the console renders.
    /// It is the exact projection Iced produced and published; `None` before a
    /// run (never a fabricated execution).
    pub script_sandbox: Option<ScriptSandboxSnapshot>,
    /// DUAL-10-12: the shared export projection (real file name/bytes/checksum
    /// and the typed host outcome) the console card renders. The export action
    /// runs in Iced through the shared application.
    pub script_export: Option<ScriptExportSnapshot>,
}

impl ProfilesProjection {
    /// Believable demo fixture for the Profiles page.
    pub fn demo() -> Self {
        Self {
            auto_update_interval_hours: 6,
            updating: false,
            aggregation: None,
            aggregation_templates: Vec::new(),
            aggregation_templates_available: true,
            snapshot_history: None,
            apply_transaction: None,
            profile_document: None,
            profile_options: None,
            editor_read: Default::default(),
            profiles: vec![
                ProfileItem {
                    id: "sub-1".to_owned(),
                    name: PRIMARY_PROFILE.to_owned(),
                    url: "https://subscribe.musicfrog.io/api/v1/client/subscribe?token=demo_sub_1"
                        .to_owned(),
                    updated_at: "2026-09-02 08:30".to_owned(),
                    upload_bytes: Some(1_250_000_000),
                    download_bytes: Some(48_600_000_000),
                    total_bytes: Some(200_000_000_000),
                    is_active: true,
                    user_agent: "Clash.Meta/1.18.0".to_owned(),
                    insecure_skip_verify: false,
                    etag: Some("\"etag-sub-1\"".to_owned()),
                    last_modified: Some("Tue, 02 Sep 2026 08:30:00 GMT".to_owned()),
                    has_backup: true,
                    cron_expression: Some("0 */6 * * *".to_owned()),
                    auto_update_enabled: true,
                    update_interval_hours: Some(24),
                    next_update: Some("2026-09-22T12:00:00+00:00".to_owned()),
                    auto_reload_core: true,
                    filter: Default::default(),
                    filter_source: subscription_filter_fixture::observation(
                        "sub-1",
                        subscription_filter_fixture::FIXTURE_DOCUMENT,
                        Default::default(),
                    )
                    .map(|o| o.source),
                    write_protection: ProfileWriteProtection::RemoteSubscription,
                },
                ProfileItem {
                    id: "sub-2".to_owned(),
                    name: BACKUP_PROFILE.to_owned(),
                    url: "https://backup.musicfrog.io/clash/config.yaml".to_owned(),
                    updated_at: "2026-09-01 12:00".to_owned(),
                    upload_bytes: Some(120_000_000),
                    download_bytes: Some(2_400_000_000),
                    total_bytes: Some(100_000_000_000),
                    is_active: false,
                    user_agent: String::new(),
                    insecure_skip_verify: true,
                    etag: None,
                    last_modified: None,
                    has_backup: false,
                    cron_expression: None,
                    auto_update_enabled: true,
                    update_interval_hours: Some(6),
                    next_update: Some("2026-09-22T14:00:00+00:00".to_owned()),
                    auto_reload_core: true,
                    filter: Default::default(),
                    filter_source: subscription_filter_fixture::observation(
                        "sub-2",
                        subscription_filter_fixture::FIXTURE_DOCUMENT,
                        Default::default(),
                    )
                    .map(|o| o.source),
                    write_protection: ProfileWriteProtection::RemoteSubscription,
                },
                ProfileItem {
                    id: "sub-3".to_owned(),
                    name: LOCAL_PROFILE.to_owned(),
                    url: "http://192.168.1.100:8080/profile.yaml".to_owned(),
                    updated_at: "2026-08-28 15:45".to_owned(),
                    upload_bytes: Some(10_000_000),
                    download_bytes: Some(50_000_000),
                    total_bytes: Some(0),
                    is_active: false,
                    user_agent: "Shadowrocket/2.2.20".to_owned(),
                    insecure_skip_verify: false,
                    etag: Some("\"etag-sub-3\"".to_owned()),
                    last_modified: Some("Fri, 28 Aug 2026 15:45:00 GMT".to_owned()),
                    has_backup: false,
                    cron_expression: None,
                    auto_update_enabled: false,
                    update_interval_hours: None,
                    next_update: None,
                    auto_reload_core: false,
                    filter: Default::default(),
                    filter_source: subscription_filter_fixture::observation(
                        "sub-3",
                        subscription_filter_fixture::FIXTURE_DOCUMENT,
                        Default::default(),
                    )
                    .map(|o| o.source),
                    write_protection: ProfileWriteProtection::Editable,
                },
            ],
            yaml_ast_diff: None,
            script_sandbox: None,
            script_export: None,
        }
    }

    /// Active profile name.
    pub fn active_profile_name(&self) -> Option<&str> {
        self.profiles
            .iter()
            .find(|profile| profile.is_active)
            .map(|profile| profile.name.as_str())
    }
}

fn auto_update_summary(projection: &ProfilesProjection, locale: &str) -> String {
    profile_metadata_projection::schedule_overview(
        projection.profiles.iter().map(|profile| {
            (
                profile.auto_update_enabled,
                profile.cron_expression.as_deref(),
                profile.update_interval_hours,
            )
        }),
        locale,
    )
}
fn profile_schedule_summary(profile: &ProfileItem, locale: &str) -> String {
    profile_metadata_projection::schedule(
        profile.auto_update_enabled,
        profile.cron_expression.as_deref(),
        profile.update_interval_hours,
        profile.next_update.as_deref(),
        locale,
    )
}

/// The typed event dispatched when profiles data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct ProfilesProjectionUpdated(pub ProfilesProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastProfilesProjection(pub Option<ProfilesProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn profiles_page(projection: &ProfilesProjection, palette: &UiPalette) -> impl Scene + use<> {
    let panels = profiles_panels(projection, palette);
    let summary = profile_metadata_projection::summary(
        projection.profiles.len(),
        projection.active_profile_name(),
        UiLocale::default().code(),
    );
    let auto_update = auto_update_summary(projection, UiLocale::default().code());

    let profile_scenes: Vec<Box<dyn Scene>> = projection
        .profiles
        .iter()
        .enumerate()
        .map(|(idx, p)| Box::new(profile_card_scene(idx, p, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
            Node {
                width: percent(100),
                min_width: px(0.0),
                max_width: percent(100),
                height: percent(100),
                min_height: px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S16),
                overflow: Overflow::clip(),
            }
            PageRoot(Route::Profiles)
            ProfilesPageRoot
            Children [
                Node {
                    width: percent(100), height: percent(100),
                    min_height: px(0.0), flex_shrink: 1.0,
                    flex_direction: FlexDirection::Column,
                    row_gap: px(space::S16), overflow: Overflow::scroll_y(),
                }
                ScrollArea
                Children [
                @{ pull_to_refresh_scene(&PullToRefreshState::default(), palette) }
                --
                @{ header_card_scene(summary, auto_update, palette) }
                --
                @{ profiles_import_card_scene(projection, palette) }
                --
                @{ subscription_policy_card_scene(projection, palette) }
                --
                @{ launcher_scene(PanelKind::Aggregator, palette) }
                --
                @{ launcher_scene(PanelKind::SnapshotDiff, palette) }
                --
                @{ profile_editor_scene(projection, palette) }
                --
                @{ workbench_scene(palette) }
                --
                { profile_scenes }
                ]
                --
                { panels }
            ]
    }
}

// ---- Plugin assembly and native observers -----------------------------------------------

/// Registers this page once during product assembly; mounting never resets its draft.
#[derive(Default)]
pub struct ProfilesPagePlugin;

impl Plugin for ProfilesPagePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LastProfilesProjection>();
        app.add_plugins(FilterDedupPlugin);
        app.init_resource::<AggregatorComposerState>();
        app.init_resource::<LastProfilesProjection>();
        app.add_observer(apply_profiles_projection);
        app.add_systems(
            Update,
            (
                replay_profiles_projection,
                replay_policy_copy,
                replay_profile_protection,
                profiles_snapshot_copy::replay,
            ),
        );
        app.add_observer(on_profiles_action_activated);
        app.add_observer(on_update_profile_activated);
        app.add_observer(on_update_all_subscriptions_activated);
        app.add_observer(on_delete_profile_activated);
        app.add_observer(sync_subscription_fetch_controls);
        app.add_observer(sync_status_copy);

        app.add_observer(on_save_subscription_fetch_settings);
        app.add_observer(on_save_subscription_filter);
        app.add_observer(on_restore_subscription_backup);
        app.add_observer(on_import_subscription_url);
        app.add_observer(on_import_local_subscription);
        app.add_observer(on_import_clipboard_subscription);
        app.add_observer(sync_subscription_policy_controls);
        app.add_observer(on_save_subscription_policy);
        app.add_observer(on_save_subscription_auto_reload);
        app.add_observer(sync_aggregation_preview);
        app.add_observer(on_preview_aggregation);
        app.add_observer(on_save_aggregated_profile);
        app.add_observer(on_add_aggregator_custom_group);
        app.add_observer(on_aggregation_checkbox_changed);
        app.add_observer(on_clear_aggregator_custom_groups);
        app.add_observer(on_save_aggregation_template);
        app.add_observer(on_use_aggregation_template);
        app.add_observer(on_reaggregate_template);
        app.add_observer(on_delete_aggregation_template);
        app.init_resource::<SnapshotDiffViewState>();
        app.add_observer(sync_snapshot_diff);
        app.add_observer(on_refresh_snapshot_diff);
        app.add_observer(on_snapshot_diff_mode_activated);
        app.add_observer(on_rollback_snapshot_activated);
        app.add_observer(on_backup_snapshot_activated);
        app.add_observer(on_refresh_snapshot_history);
        app.add_observer(on_snapshot_prune_keep_activated);
        app.add_observer(on_prune_snapshots_activated);
        app.add_observer(on_snapshot_history_entry_activated);
        app.add_observer(on_snapshot_history_restore_activated);
        // DUAL-10-05/14: the shared script-sandbox console body.
        app.add_plugins(ScriptWorkbenchPlugin);
        // DUAL-09-03/14: the document editor owns its own plugin
        // (`ProfilesEditorPlugin`): keyboard seam, observers and body rebuild.
    }
}

/// DUAL-07-11: route the toolbar "update all" click into the shared command bus.
pub(crate) fn on_update_all_subscriptions_activated(
    activate: On<Activate>,
    buttons: Query<(), With<UpdateAllSubscriptionsButton>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_ok() {
        handle.submit(UiCommand::UpdateAllSubscriptions);
    }
}

pub(crate) fn on_profiles_action_activated(
    activate: On<Activate>,
    buttons: Query<&ActivateProfileButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if let Ok(btn) = buttons.get(activate.entity) {
        handle.submit(UiCommand::ActivateProfile {
            id: btn.profile_id.clone(),
        });
    }
}

/// DUAL-07-05: route a per-profile "update now" click into the shared command
/// bus; retry/backoff and the single-flight guard are the application's job.
pub(crate) fn on_update_profile_activated(
    activate: On<Activate>,
    buttons: Query<&UpdateProfileButton>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let Some(profile) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.profiles.get(button.0))
    else {
        return;
    };
    handle.submit(UiCommand::UpdateProfile {
        id: profile.id.clone(),
    });
}

/// DUAL-07-14: route a per-profile "delete" click into the shared command bus;
/// the application owns the real deletion (document + options sidecar). The
/// active profile is never deleted from this surface — exactly like the Iced
/// card, which only offers the action for inactive profiles.
pub(crate) fn on_delete_profile_activated(
    activate: On<Activate>,
    buttons: Query<&DeleteProfileButton>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let profile = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(|projection| projection.profiles.get(button.profile_idx));
    let profile_id = profile
        .map(|profile| profile.id.clone())
        .unwrap_or_else(|| button.profile_id.clone());
    if profile.is_some_and(|profile| profile.is_active) {
        return;
    }
    handle.submit(UiCommand::DeleteProfile { id: profile_id });
}

pub(crate) fn apply_profiles_projection(
    update: On<ProfilesProjectionUpdated>,
    mut last: ResMut<LastProfilesProjection>,
) {
    last.0 = Some(update.0.clone());
}

pub(crate) fn replay_profiles_projection(
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    last: Res<LastProfilesProjection>,
    targets: ProfileProjectionTargets,
) {
    let ProfileProjectionTargets {
        mut lines,
        mut names,
        mut times,
        mut traffics,
        mut statuses,
        mut schedules,
        mut buttons,
        mut delete_buttons,
    } = targets;

    let Some(projection) = last.0.as_ref() else {
        return;
    };

    for (mut text, line) in &mut lines {
        match line.0 {
            ProfilesLineKind::Summary => {
                text.0 = profile_metadata_projection::summary(
                    projection.profiles.len(),
                    projection.active_profile_name(),
                    locale.code(),
                );
            }
            ProfilesLineKind::AutoUpdate => {
                text.0 = auto_update_summary(projection, locale.code());
            }
        }
    }

    for (mut text, marker) in &mut names {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile.name.clone();
        }
    }

    for (mut text, marker) in &mut times {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile_metadata_projection::updated(&profile.updated_at, locale.code());
        }
    }

    for (mut text, marker) in &mut traffics {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile_metadata_projection::traffic_caption(
                &profile_metadata_projection::traffic(
                    profile.upload_bytes,
                    profile.download_bytes,
                    profile.total_bytes,
                ),
                locale.code(),
            );
        }
    }

    for (mut text, marker) in &mut statuses {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile_metadata_projection::active_status(profile.is_active, locale.code());
        }
    }

    for (mut text, marker) in &mut schedules {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile_schedule_summary(profile, locale.code());
        }
    }

    for (mut bg, mut visual, mut btn) in &mut buttons {
        if let Some(profile) = projection.profiles.get(btn.profile_idx) {
            btn.profile_id = profile.id.clone();
            visual.0 = profile.is_active;
            bg.0 = if profile.is_active {
                palette.success
            } else {
                palette.surface_elevated
            };
        }
    }

    // Keep every delete button bound to the profile it currently renders so a
    // list reorder can never delete the wrong profile; the active profile's
    // action reads as disabled because this surface never deletes it.
    for (mut button, mut visual) in &mut delete_buttons {
        if let Some(profile) = projection.profiles.get(button.profile_idx) {
            button.profile_id = profile.id.clone();
            visual.0 = !profile.is_active;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_profiles_fixture() {
        let proj = ProfilesProjection::demo();
        assert_eq!(proj.profiles.len(), 3);
        assert_eq!(proj.active_profile_name(), Some("Primary VIP"));
        assert_eq!(proj.auto_update_interval_hours, 6);
        assert_eq!(proj.profiles[0].id, "sub-1");
        assert_eq!(proj.profiles[0].name, "Primary VIP");
        assert_eq!(proj.profiles[0].total_bytes, Some(200_000_000_000));
        assert_eq!(proj.profiles[0].upload_bytes, Some(1_250_000_000));
        assert_eq!(proj.profiles[0].download_bytes, Some(48_600_000_000));
        assert!(proj.profiles[0].is_active);
    }
}

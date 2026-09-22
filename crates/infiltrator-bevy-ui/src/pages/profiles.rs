//! The Profiles page (配置订阅): subscription management, profile metadata,
//! traffic quotas, manual refresh, and active profile switching.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProfilesLine`],
//! [`ProfileNameText`], [`ProfileTimeText`], [`ProfileTrafficText`], [`ProfileStatusText`]).
//! The page self-registers [`apply_profiles_projection`] and action observers
//! once per world via [`ProfilesPageRoot`]. When [`ProfilesProjectionUpdated`]
//! fires, texts and active states restamp in place without tree rebuilds.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::overview::format_byte_count;
use crate::route::{PageRoot, Route};

/// Root marker on the Profiles page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(on_insert = bind_profiles_page)]
pub struct ProfilesPageRoot;

/// Once-per-world guard preventing duplicate observer registration.
#[derive(Resource)]
struct ProfilesPageBound;

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
    pub upload_bytes: u64,
    pub download_bytes: u64,
    pub total_bytes: u64,
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
    pub filter: infiltrator_contract::subscription_import::SubscriptionFilterDraft,
    /// DUAL-09-12: the same write classification the Iced editor and the
    /// application guard use.
    pub write_protection: infiltrator_contract::profile_protection::ProfileWriteProtection,
}

/// Snapshot of the Profiles domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfilesProjection {
    pub profiles: Vec<ProfileItem>,
    pub auto_update_interval_hours: u32,
    pub updating: bool,
    /// DUAL-08: the last shared aggregation preview, or `None` when no draft
    /// has been previewed yet. The surface never builds this locally.
    pub aggregation: Option<infiltrator_contract::aggregator::AggregationReport>,
    /// DUAL-09-08: the shared snapshot-vs-current AST diff. The surface renders
    /// it directly and never fabricates rows.
    pub yaml_ast_diff: Option<infiltrator_contract::yaml_ast_diff::YamlAstDiffSnapshot>,
}

impl ProfilesProjection {
    /// Believable demo fixture for the Profiles page.
    pub fn demo() -> Self {
        Self {
            auto_update_interval_hours: 6,
            updating: false,
            aggregation: None,
            profiles: vec![
                ProfileItem {
                    id: "sub-1".to_owned(),
                    name: "主力高速订阅 (Primary VIP)".to_owned(),
                    url: "https://subscribe.musicfrog.io/api/v1/client/subscribe?token=demo_sub_1"
                        .to_owned(),
                    updated_at: "2026-09-02 08:30".to_owned(),
                    upload_bytes: 1_250_000_000,
                    download_bytes: 48_600_000_000,
                    total_bytes: 200_000_000_000,
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
                    write_protection:
                        infiltrator_contract::profile_protection::ProfileWriteProtection::RemoteSubscription,
                },
                ProfileItem {
                    id: "sub-2".to_owned(),
                    name: "备用容灾线路 (Backup Anycast)".to_owned(),
                    url: "https://backup.musicfrog.io/clash/config.yaml".to_owned(),
                    updated_at: "2026-09-01 12:00".to_owned(),
                    upload_bytes: 120_000_000,
                    download_bytes: 2_400_000_000,
                    total_bytes: 100_000_000_000,
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
                    write_protection:
                        infiltrator_contract::profile_protection::ProfileWriteProtection::RemoteSubscription,
                },
                ProfileItem {
                    id: "sub-3".to_owned(),
                    name: "局域网调试配置 (LAN Lab)".to_owned(),
                    url: "http://192.168.1.100:8080/profile.yaml".to_owned(),
                    updated_at: "2026-08-28 15:45".to_owned(),
                    upload_bytes: 10_000_000,
                    download_bytes: 50_000_000,
                    total_bytes: 0,
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
                    write_protection:
                        infiltrator_contract::profile_protection::ProfileWriteProtection::Editable,
                },
            ],
            yaml_ast_diff: None,
        }
    }

    /// Active profile name.
    pub fn active_profile_name(&self) -> &str {
        self.profiles
            .iter()
            .find(|p| p.is_active)
            .map(|p| p.name.as_str())
            .unwrap_or("无活动配置")
    }
}

/// DUAL-07-14: the header summary is derived from the real profile list, never
/// from a page-level constant: a "every N hours" claim only appears when a
/// fixed interval is actually configured, and cron-only sets say so.
pub fn auto_update_summary(projection: &ProfilesProjection) -> String {
    let enabled = projection
        .profiles
        .iter()
        .filter(|profile| profile.auto_update_enabled)
        .count();
    if enabled == 0 {
        return "自动更新: 未启用".to_owned();
    }
    if projection.auto_update_interval_hours > 0 {
        format!(
            "自动更新: {} 个订阅已启用 · 最短周期 {} 小时",
            enabled, projection.auto_update_interval_hours
        )
    } else {
        format!("自动更新: {} 个订阅已启用 · 按 Cron 计划", enabled)
    }
}

/// DUAL-07-14: one card line describing a profile's real update cadence,
/// including the next scheduled run when the shared snapshot knows it.
pub fn profile_schedule_summary(profile: &ProfileItem) -> String {
    let cadence = if !profile.auto_update_enabled {
        "定时计划: 手动".to_owned()
    } else {
        match profile.cron_expression.as_deref() {
            Some(cron) if !cron.trim().is_empty() => format!("定时计划: Cron `{cron}`"),
            _ => match profile.update_interval_hours {
                Some(hours) => format!("定时计划: 每 {hours} 小时"),
                None => "定时计划: 未设置周期".to_owned(),
            },
        }
    };
    match profile.next_update.as_deref() {
        Some(next) if !next.trim().is_empty() => format!("{cadence} · 下次更新 {next}"),
        _ => cadence,
    }
}

/// The typed event dispatched when profiles data updates.
#[derive(Event, Clone, Debug, PartialEq)]
pub struct ProfilesProjectionUpdated(pub ProfilesProjection);

/// Last projection resource for theme replay.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct LastProfilesProjection(pub Option<ProfilesProjection>);

// ---- Scene constructors ---------------------------------------------------

pub fn profiles_page(projection: &ProfilesProjection, palette: &UiPalette) -> impl Scene + use<> {
    let summary = format!(
        "配置订阅 · 共 {} 个配置 (当前生效: {})",
        projection.profiles.len(),
        projection.active_profile_name()
    );
    let auto_update = auto_update_summary(projection);

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
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Profiles)
        ProfilesPageRoot
        Children [
            ( { header_card_scene(summary, auto_update, palette) } ),
            ( { crate::pages::profiles_import::profiles_import_card_scene(projection, palette) } ),
            ( { crate::pages::profiles_subscription_policy::subscription_policy_card_scene(projection, palette) } ),
            ( { crate::pages::profiles_aggregator::profiles_aggregator_scene(projection, palette) } ),
            ( { crate::pages::profiles_diff::snapshot_diff_scene(projection, palette) } ),
            ( { crate::pages::profiles_script::script_sandbox_scene(palette) } ),
            { profile_scenes },
        ]
    }
}

fn header_card_scene(
    summary: String,
    auto_update: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    header_a11y.set_label("配置订阅概览");

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
            }
            template_value(AccessibilityNode(header_a11y))
            Children [
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S12),
                    }
                    Children [
                        ( { icon_tile_scene(IconId::FileText, 36.0, palette) } ),
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                ( Text(summary) ProfilesLine(ProfilesLineKind::Summary) TextRole(Role::Heading) ),
                                ( Text(auto_update) ProfilesLine(ProfilesLineKind::AutoUpdate) TextRole(Role::Caption) ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            UpdateAllSubscriptionsButton
                            Children [
                                ( Text({ "一键更新全部订阅".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            Button
                            Children [
                                ( Text({ "导入订阅链接".to_owned() }) TextRole(Role::BodyStrong) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

fn profile_card_scene(
    idx: usize,
    profile: &ProfileItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = profile.name.clone();
    let url = profile.url.clone();
    let updated = format!("更新于: {}", profile.updated_at);
    let used_bytes = profile.upload_bytes + profile.download_bytes;
    let traffic_str = if profile.total_bytes > 0 {
        format!(
            "已用: {} / 总计: {}",
            format_byte_count(used_bytes),
            format_byte_count(profile.total_bytes)
        )
    } else {
        format!("已用: {} / 无限制", format_byte_count(used_bytes))
    };

    let status_str = if profile.is_active {
        "当前生效中".to_owned()
    } else {
        "点击启用".to_owned()
    };
    let schedule_str = profile_schedule_summary(profile);
    let btn_bg = if profile.is_active {
        palette.success
    } else {
        palette.surface_elevated
    };

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(space::S16),
            }
            Children [
                (
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::S4),
                    }
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( Text(name) ProfileNameText(idx) TextRole(Role::BodyStrong) ),
                                ( Text(updated) ProfileTimeText(idx) TextRole(Role::Caption) ),
                            ]
                        ),
                        ( Text(url) TextRole(Role::Caption) ),
                        (
                            Text({ profile.write_protection.label_zh().to_owned() })
                            ProfileProtectionText(idx)
                            TextRole(Role::Caption)
                        ),
                        ( Text(traffic_str) ProfileTrafficText(idx) TextRole(Role::Mono) ),
                        ( Text(schedule_str) ProfileScheduleText(idx) TextRole(Role::Caption) ),
                    ]
                ),
                (
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::S8),
                    }
                    Children [
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            UpdateProfileButton(idx)
                            Children [
                                ( Text({ "立即更新".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ btn_bg })
                            ControlVisual({ profile.is_active })
                            ActivateProfileButton {
                                profile_id: { profile.id.clone() },
                                profile_idx: { idx },
                            }
                            Button
                            Children [
                                ( Text(status_str) ProfileStatusText(idx) TextRole(Role::Body) ),
                            ]
                        ),
                        (
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            ControlVisual({ !profile.is_active })
                            Button
                            DeleteProfileButton {
                                profile_id: { profile.id.clone() },
                                profile_idx: { idx },
                            }
                            Children [
                                ( Text({ "删除配置".to_owned() }) TextRole(Role::Body) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

// ---- Observer & Update Hook -----------------------------------------------

fn bind_profiles_page(mut world: DeferredWorld<'_>, _context: HookContext) {
    if world.get_resource::<ProfilesPageBound>().is_some() {
        return;
    }
    let mut commands = world.commands();
    commands.insert_resource(ProfilesPageBound);
    commands.init_resource::<LastProfilesProjection>();
    commands.add_observer(apply_profiles_projection);
    commands.add_observer(on_profiles_action_activated);
    commands.add_observer(on_update_profile_activated);
    commands.add_observer(on_update_all_subscriptions_activated);
    commands.add_observer(on_delete_profile_activated);
    commands.add_observer(crate::pages::profiles_import::sync_subscription_fetch_controls);
    commands
        .add_observer(crate::pages::profiles_import_channels::sync_subscription_filter_controls);
    commands.add_observer(crate::pages::profiles_import::on_save_subscription_fetch_settings);
    commands.add_observer(crate::pages::profiles_import_channels::on_save_subscription_filter);
    commands.add_observer(crate::pages::profiles_import::on_restore_subscription_backup);
    commands.add_observer(crate::pages::profiles_import_channels::on_import_subscription_url);
    commands.add_observer(crate::pages::profiles_import_channels::on_import_local_subscription);
    commands.add_observer(crate::pages::profiles_import_channels::on_import_clipboard_subscription);
    commands.add_observer(
        crate::pages::profiles_subscription_policy::sync_subscription_policy_controls,
    );
    commands.add_observer(crate::pages::profiles_subscription_policy::on_save_subscription_policy);
    commands
        .add_observer(crate::pages::profiles_subscription_policy::on_save_subscription_auto_reload);
    commands.add_observer(crate::pages::profiles_aggregator::sync_aggregation_preview);
    commands.add_observer(crate::pages::profiles_aggregator::on_preview_aggregation);
    commands.add_observer(crate::pages::profiles_aggregator::on_save_aggregated_profile);
    commands.init_resource::<crate::pages::profiles_diff::SnapshotDiffViewState>();
    commands.add_observer(crate::pages::profiles_diff::sync_snapshot_diff);
    commands.add_observer(crate::pages::profiles_diff::on_refresh_snapshot_diff);
    commands.add_observer(crate::pages::profiles_diff::on_snapshot_diff_mode_activated);
    commands.add_observer(crate::pages::profiles_diff::on_rollback_snapshot_activated);
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

#[allow(clippy::type_complexity)]
#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_profiles_projection(
    update: On<ProfilesProjectionUpdated>,
    palette: Res<UiPalette>,
    mut last: Option<ResMut<LastProfilesProjection>>,
    mut lines: Query<
        (&mut Text, &ProfilesLine),
        (
            With<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTimeText>,
            Without<ProfileTrafficText>,
            Without<ProfileStatusText>,
        ),
    >,
    mut names: Query<
        (&mut Text, &ProfileNameText),
        (
            With<ProfileNameText>,
            Without<ProfilesLine>,
            Without<ProfileTimeText>,
            Without<ProfileTrafficText>,
            Without<ProfileStatusText>,
        ),
    >,
    mut times: Query<
        (&mut Text, &ProfileTimeText),
        (
            With<ProfileTimeText>,
            Without<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTrafficText>,
            Without<ProfileStatusText>,
        ),
    >,
    mut traffics: Query<
        (&mut Text, &ProfileTrafficText),
        (
            With<ProfileTrafficText>,
            Without<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTimeText>,
            Without<ProfileStatusText>,
        ),
    >,
    mut statuses: Query<
        (&mut Text, &ProfileStatusText),
        (
            With<ProfileStatusText>,
            Without<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTimeText>,
            Without<ProfileTrafficText>,
            Without<ProfileScheduleText>,
        ),
    >,
    mut schedules: Query<
        (&mut Text, &ProfileScheduleText),
        (
            With<ProfileScheduleText>,
            Without<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTimeText>,
            Without<ProfileTrafficText>,
            Without<ProfileStatusText>,
        ),
    >,
    mut buttons: Query<
        (
            &mut BackgroundColor,
            &mut ControlVisual,
            &mut ActivateProfileButton,
        ),
        Without<DeleteProfileButton>,
    >,
    mut delete_buttons: Query<
        (&mut DeleteProfileButton, &mut ControlVisual),
        Without<ActivateProfileButton>,
    >,
) {
    let projection = &update.0;

    for (mut text, line) in &mut lines {
        match line.0 {
            ProfilesLineKind::Summary => {
                text.0 = format!(
                    "配置订阅 · 共 {} 个配置 (当前生效: {})",
                    projection.profiles.len(),
                    projection.active_profile_name()
                );
            }
            ProfilesLineKind::AutoUpdate => {
                text.0 = auto_update_summary(projection);
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
            text.0 = format!("更新于: {}", profile.updated_at);
        }
    }

    for (mut text, marker) in &mut traffics {
        if let Some(profile) = projection.profiles.get(marker.0) {
            let used = profile.upload_bytes + profile.download_bytes;
            text.0 = if profile.total_bytes > 0 {
                format!(
                    "已用: {} / 总计: {}",
                    format_byte_count(used),
                    format_byte_count(profile.total_bytes)
                )
            } else {
                format!("已用: {} / 无限制", format_byte_count(used))
            };
        }
    }

    for (mut text, marker) in &mut statuses {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = if profile.is_active {
                "当前生效中".to_owned()
            } else {
                "点击启用".to_owned()
            };
        }
    }

    for (mut text, marker) in &mut schedules {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = profile_schedule_summary(profile);
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

    if let Some(ref mut last_proj) = last {
        last_proj.0 = Some(projection.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_profiles_fixture() {
        let proj = ProfilesProjection::demo();
        assert_eq!(proj.profiles.len(), 3);
        assert_eq!(proj.active_profile_name(), "主力高速订阅 (Primary VIP)");
        assert_eq!(proj.auto_update_interval_hours, 6);
        assert_eq!(proj.profiles[0].id, "sub-1");
        assert_eq!(proj.profiles[0].name, "主力高速订阅 (Primary VIP)");
        assert_eq!(proj.profiles[0].total_bytes, 200_000_000_000);
        assert_eq!(proj.profiles[0].upload_bytes, 1_250_000_000);
        assert_eq!(proj.profiles[0].download_bytes, 48_600_000_000);
        assert!(proj.profiles[0].is_active);
    }
}

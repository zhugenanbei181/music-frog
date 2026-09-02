//! The Profiles page (配置订阅): subscription list, remote import,
//! auto-update settings, quota/traffic inspection, and profile switching.
//!
//! **Update seam**: mutable nodes carry typed markers ([`ProfilesLine`],
//! [`ProfileCardMarker`]). The page self-registers
//! [`apply_profiles_projection`] once per world via [`ProfilesPageRoot`].

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::lifecycle::HookContext;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, ResMut};
use bevy::ecs::world::DeferredWorld;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

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
    /// Overview summary: total profiles count and active profile name.
    #[default]
    Summary,
    /// Auto update interval text.
    AutoUpdate,
}

/// Marker for a specific profile card's name text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileNameText(pub usize);

/// Marker for a specific profile card's traffic info text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileTrafficText(pub usize);

/// Marker for a specific profile card's update time text.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileTimeText(pub usize);

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
}

/// Snapshot of the Profiles domain.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfilesProjection {
    pub profiles: Vec<ProfileItem>,
    pub auto_update_interval_hours: u32,
    pub updating: bool,
}

impl ProfilesProjection {
    /// Believable demo fixture for the Profiles page.
    pub fn demo() -> Self {
        Self {
            auto_update_interval_hours: 24,
            updating: false,
            profiles: vec![
                ProfileItem {
                    id: "sub-1".to_owned(),
                    name: "主力高速订阅 (Primary VIP)".to_owned(),
                    url: "https://sub.example.com/api/v1/client/subscribe?token=***".to_owned(),
                    updated_at: "2026-09-02 08:30".to_owned(),
                    upload_bytes: 1_400_000_000,
                    download_bytes: 48_500_000_000,
                    total_bytes: 200_000_000_000,
                    is_active: true,
                },
                ProfileItem {
                    id: "sub-2".to_owned(),
                    name: "备用容灾线路 (Backup Direct)".to_owned(),
                    url: "https://backup.example.org/clash/config.yaml".to_owned(),
                    updated_at: "2026-09-01 12:00".to_owned(),
                    upload_bytes: 120_000_000,
                    download_bytes: 4_200_000_000,
                    total_bytes: 100_000_000_000,
                    is_active: false,
                },
                ProfileItem {
                    id: "sub-3".to_owned(),
                    name: "本地开发分流配置 (Local Dev)".to_owned(),
                    url: "file:///etc/musicfrog/profiles/dev_rules.yaml".to_owned(),
                    updated_at: "2026-08-30 19:45".to_owned(),
                    upload_bytes: 0,
                    download_bytes: 0,
                    total_bytes: 0,
                    is_active: false,
                },
            ],
        }
    }

    pub fn active_profile_name(&self) -> &str {
        self.profiles
            .iter()
            .find(|p| p.is_active)
            .map(|p| p.name.as_str())
            .unwrap_or("无活动配置")
    }
}

/// Format traffic usage display string.
pub fn format_traffic_usage(upload: u64, download: u64, total: u64) -> String {
    if total == 0 {
        return "本地无流量限制".to_owned();
    }
    let used = upload + download;
    format!(
        "已用: {} / 总量: {} (剩余: {})",
        format_byte_count(used),
        format_byte_count(total),
        format_byte_count(total.saturating_sub(used))
    )
}

/// The typed event dispatched when profile data updates.
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
    let auto_update = format!(
        "自动更新周期: 每 {} 小时",
        projection.auto_update_interval_hours
    );

    let profile_scenes: Vec<Box<dyn Scene>> = projection
        .profiles
        .iter()
        .enumerate()
        .map(|(idx, item)| Box::new(profile_card_scene(idx, item, palette)) as Box<dyn Scene>)
        .collect();

    bsn! {
        Node {
            width: percent(100),
            height: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S16),
            overflow: Overflow::scroll_y(),
        }
        PageRoot(Route::Profiles)
        ProfilesPageRoot
        Children [
            ( { header_card_scene(summary, auto_update, palette) } ),
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
                            BackgroundColor({ palette.accent })
                            Button
                            Children [
                                ( Text({ "导入订阅".to_owned() }) TextRole(Role::BodyStrong) ),
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
                            Button
                            Children [
                                ( Text({ "更新全部".to_owned() }) TextRole(Role::Body) ),
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
    let time_str = format!("更新时间: {}", profile.updated_at);
    let traffic_str = format_traffic_usage(
        profile.upload_bytes,
        profile.download_bytes,
        profile.total_bytes,
    );
    let status_str = if profile.is_active {
        "当前生效中".to_owned()
    } else {
        "点击启用".to_owned()
    };
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
                                ( Text(time_str) ProfileTimeText(idx) TextRole(Role::Caption) ),
                            ]
                        ),
                        ( Text(url) TextRole(Role::Caption) ),
                        ( Text(traffic_str) ProfileTrafficText(idx) TextRole(Role::Mono) ),
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
                            BackgroundColor({ btn_bg })
                            ControlVisual({ profile.is_active })
                            Button
                            Children [
                                ( Text(status_str) TextRole(Role::Body) ),
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
    commands.add_observer(apply_profiles_projection);
}

#[allow(clippy::type_complexity)]
pub(crate) fn apply_profiles_projection(
    update: On<ProfilesProjectionUpdated>,
    mut last: Option<ResMut<LastProfilesProjection>>,
    mut lines: Query<
        (&mut Text, &ProfilesLine),
        (
            With<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTimeText>,
            Without<ProfileTrafficText>,
        ),
    >,
    mut names: Query<
        (&mut Text, &ProfileNameText),
        (
            With<ProfileNameText>,
            Without<ProfilesLine>,
            Without<ProfileTimeText>,
            Without<ProfileTrafficText>,
        ),
    >,
    mut times: Query<
        (&mut Text, &ProfileTimeText),
        (
            With<ProfileTimeText>,
            Without<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTrafficText>,
        ),
    >,
    mut traffics: Query<
        (&mut Text, &ProfileTrafficText),
        (
            With<ProfileTrafficText>,
            Without<ProfilesLine>,
            Without<ProfileNameText>,
            Without<ProfileTimeText>,
        ),
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
                text.0 = format!(
                    "自动更新周期: 每 {} 小时",
                    projection.auto_update_interval_hours
                );
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
            text.0 = format!("更新时间: {}", profile.updated_at);
        }
    }

    for (mut text, marker) in &mut traffics {
        if let Some(profile) = projection.profiles.get(marker.0) {
            text.0 = format_traffic_usage(
                profile.upload_bytes,
                profile.download_bytes,
                profile.total_bytes,
            );
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
    fn format_traffic_usage_calc() {
        let text = format_traffic_usage(1_000_000_000, 4_000_000_000, 10_000_000_000);
        assert!(text.contains("已用"));
        assert!(text.contains("总量"));

        let zero = format_traffic_usage(0, 0, 0);
        assert_eq!(zero, "本地无流量限制");
    }

    #[test]
    fn demo_profiles_fixture() {
        let proj = ProfilesProjection::demo();
        assert_eq!(proj.profiles.len(), 3);
        assert_eq!(proj.active_profile_name(), "主力高速订阅 (Primary VIP)");
    }
}

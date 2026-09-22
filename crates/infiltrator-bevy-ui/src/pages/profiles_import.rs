//! Local profile import & subscription User-Agent / conditional-fetch
//! configuration scene component.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::Checked;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    PositionType, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button, Checkbox};
use infiltrator_bevy_widgets::checkbox::checkbox_scene;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{
    LastProfilesProjection, ProfilesProjection, ProfilesProjectionUpdated,
};
use crate::pages::profiles_import_channels::{
    ImportClipboardSubscriptionButton, ImportLocalPathField, ImportLocalSubscriptionButton,
    ImportSubscriptionNameField, ImportSubscriptionUrlButton, ImportSubscriptionUrlField,
    SaveSubscriptionFilterButton, SubscriptionFilterDedupToggle, SubscriptionFilterExcludeField,
    SubscriptionFilterExcludeTypesField, SubscriptionFilterIncludeField,
    SubscriptionFilterRenamesField, SubscriptionFilterStatus, SubscriptionScheduleStatus,
};

/// Marker for the profiles import card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfilesImportRoot;

/// Marker for choosing local file button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChooseLocalFileButton;

/// Marker for importing local file button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportLocalFileButton;

/// Marker for the per-profile User-Agent text field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionUserAgentField;

/// Marker for the per-profile insecure-TLS checkbox parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionInsecureToggle;

/// Marker for the conditional-request cache status line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionConditionalStatus;

/// Marker for saving the current profile's User-Agent / insecure-TLS settings.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveUserAgentButton;

/// DUAL-07-13: status line for the selected profile's safe pre-save backup.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionBackupStatus;

/// DUAL-07-13: restore the selected profile's transient pre-save backup.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RestoreSubscriptionBackupButton;

pub(super) fn selected_profile(
    projection: &ProfilesProjection,
) -> Option<&crate::pages::profiles::ProfileItem> {
    projection
        .profiles
        .iter()
        .find(|profile| profile.is_active)
        .or_else(|| projection.profiles.first())
}

/// Profiles import and subscription fetch-options card scene.
pub fn profiles_import_card_scene(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let user_agent = selected_profile(projection)
        .map(|profile| profile.user_agent.clone())
        .unwrap_or_default();
    let insecure_skip_verify = selected_profile(projection)
        .map(|profile| profile.insecure_skip_verify)
        .unwrap_or(false);
    let conditional = selected_profile(projection).and_then(|profile| {
        profile.etag.as_deref().map(|etag| {
            format!(
                "条件请求已缓存 · ETag: {} · Last-Modified: {}",
                etag,
                profile.last_modified.as_deref().unwrap_or("-")
            )
        })
    });
    let conditional =
        conditional.unwrap_or_else(|| "条件请求：尚无 ETag / Last-Modified 缓存".to_owned());
    let has_backup = selected_profile(projection).is_some_and(|profile| profile.has_backup);
    let backup_status = if has_backup {
        "安全备份已就绪 · 可还原上次写入前的配置".to_owned()
    } else {
        "安全备份：暂无（保存订阅配置时自动生成）".to_owned()
    };
    let filter = selected_profile(projection)
        .map(|profile| profile.filter.clone())
        .unwrap_or_default();
    let filter_status = if filter.is_empty() {
        "清洗管道：未启用".to_owned()
    } else {
        format!(
            "清洗管道：包含 `{}` · 排除 `{}` · 协议排除 `{}` · 重命名 {} · 去重 {}",
            filter.include,
            filter.exclude,
            filter.exclude_types,
            if filter.renames.trim().is_empty() {
                "0 条"
            } else {
                "已配置"
            },
            if filter.dedup_index == 0 {
                "关"
            } else {
                "开"
            }
        )
    };
    let schedule_status = selected_profile(projection)
        .map(|profile| match profile.cron_expression.as_deref() {
            Some(cron) if !cron.trim().is_empty() => format!("定时计划：Cron `{cron}`"),
            _ => "定时计划：按小时周期 / 手动".to_owned(),
        })
        .unwrap_or_else(|| "定时计划：手动".to_owned());
    surface_scene(
        vec![
            // Section 1: "导入本地配置文件 (Import Local Config)" Header
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S4)),
                }
                ProfilesImportRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                            ( Text({ "导入本地配置文件 (Import Local Config)".to_owned() }) TextRole(Role::BodyStrong) ),
                        ]
                    ),
                ]
            }),
            // Section 1: Row with path hint box + ChooseLocalFileButton + ImportLocalFileButton
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            flex_grow: 1.0,
                            min_height: px(palette.control_height_px),
                            align_items: AlignItems::Center,
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            border: UiRect::all(Val::Px(palette.hairline_px)),
                        }
                        BackgroundColor({ palette.window_clear })
                        BorderColor {
                            top: { palette.border },
                            right: { palette.border },
                            bottom: { palette.border },
                            left: { palette.border },
                        }
                        Children [
                            ( Text({ "选择或输入本地配置文件路径 (*.yaml, *.yml)...".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
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
                        ChooseLocalFileButton
                        Children [
                            ( Text({ "选择文件".to_owned() }) TextRole(Role::Body) ),
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
                        ImportLocalFileButton
                        Children [
                            ( Text({ "+ 导入本地文件".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
                        ]
                    ),
                ]
            }),
            // Section 1: Toggle switch "导入后立即激活"
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S6)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.window_clear })
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text({ "导入后立即激活".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text({ "已开启".to_owned() }) TextRole(Role::Caption) TextColor({ palette.success }) ),
                            (
                                Node {
                                    width: px(38.0),
                                    height: px(22.0),
                                    border: UiRect::all(Val::Px(palette.hairline_px)),
                                    border_radius: BorderRadius::all(Val::Px(11.0)),
                                    position_type: PositionType::Relative,
                                    align_items: AlignItems::Center,
                                }
                                BackgroundColor({ palette.accent })
                                BorderColor {
                                    top: { palette.accent },
                                    right: { palette.accent },
                                    bottom: { palette.accent },
                                    left: { palette.accent },
                                }
                                Children [
                                    (
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(18.0),
                                            width: px(16.0),
                                            height: px(16.0),
                                            border_radius: BorderRadius::all(Val::Px(8.0)),
                                        }
                                        BackgroundColor({ palette.on_accent })
                                    ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            // Section 2: "订阅请求设置 (Subscription User-Agent)" Header
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                    ( Text({ "订阅请求设置 (Subscription User-Agent)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            // Section 2: Per-profile User-Agent text field + save button
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            flex_grow: 1.0,
                        }
                        SubscriptionUserAgentField
                        Children [
                            ( { text_field_with_placeholder_scene(user_agent, "Clash.Meta / ClashVerge / Shadowrocket".to_owned(), palette) } ),
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
                        SaveUserAgentButton
                        Children [
                            ( Text({ "保存请求设置".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
                        ]
                    ),
                ]
            }),
            // Section 2: Insecure TLS toggle + conditional-request cache state
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        SubscriptionInsecureToggle
                        Children [
                            ( { checkbox_scene("跳过 TLS 证书校验 (Insecure)".to_owned(), insecure_skip_verify, palette) } ),
                        ]
                    ),
                    ( Text({ conditional.clone() }) SubscriptionConditionalStatus TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                ]
            }),
            // Section 2: Safe pre-save backup status + restore action
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ backup_status }) SubscriptionBackupStatus TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
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
                        RestoreSubscriptionBackupButton
                        Children [
                            ( Text({ "还原安全备份".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                ]
            }),
            // Section 2: Preset UA badges / description
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::top(Val::Px(space::S2)),
                }
                Children [
                    ( Text({ "预设 UA:".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "Clash.Meta".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "ClashVerge".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                    (
                        Node {
                            padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ palette.surface_elevated })
                        Children [
                            ( Text({ "Shadowrocket".to_owned() }) TextRole(Role::Caption) ),
                        ]
                    ),
                ]
            }),
            // Section 3: DUAL-07-08 node-keyword cleaning pipeline.
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( { icon_tile_scene(IconId::Settings, 24.0, palette) } ),
                    ( Text({ "节点清洗管道 (Filter Pipeline)".to_owned() }) TextRole(Role::BodyStrong) ),
                    ( Text({ schedule_status.clone() }) SubscriptionScheduleStatus TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node { width: percent(100) }
                        SubscriptionFilterIncludeField
                        Children [ ( { text_field_with_placeholder_scene(filter.include.clone(), "包含关键字（逗号分隔，支持正则）".to_owned(), palette) } ) ]
                    ),
                    (
                        Node { width: percent(100) }
                        SubscriptionFilterExcludeField
                        Children [ ( { text_field_with_placeholder_scene(filter.exclude.clone(), "排除关键字（逗号分隔，支持正则）".to_owned(), palette) } ) ]
                    ),
                    (
                        Node { width: percent(100) }
                        SubscriptionFilterExcludeTypesField
                        Children [ ( { text_field_with_placeholder_scene(filter.exclude_types.clone(), "排除协议（ss, vmess, trojan...）".to_owned(), palette) } ) ]
                    ),
                    (
                        Node { width: percent(100) }
                        SubscriptionFilterRenamesField
                        Children [ ( { text_field_with_placeholder_scene(filter.renames.clone(), "重命名规则（每行 `模式 => 替换`）".to_owned(), palette) } ) ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        SubscriptionFilterDedupToggle
                        Children [
                            ( { checkbox_scene("跨订阅节点去重（保留首个）".to_owned(), filter.dedup_index != 0, palette) } ),
                        ]
                    ),
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( Text({ filter_status.clone() }) SubscriptionFilterStatus TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
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
                                SaveSubscriptionFilterButton
                                Children [
                                    ( Text({ "应用清洗管道".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            // Section 4: DUAL-07-01 multi-channel import workbench.
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::top(Val::Px(space::S8)),
                }
                Children [
                    ( { icon_tile_scene(IconId::FileText, 24.0, palette) } ),
                    ( Text({ "多渠道导入 (URL / 本地 / 剪贴板)".to_owned() }) TextRole(Role::BodyStrong) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S6),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    (
                        Node { width: percent(100) }
                        ImportSubscriptionNameField
                        Children [ ( { text_field_with_placeholder_scene(String::new(), "新配置名称".to_owned(), palette) } ) ]
                    ),
                    (
                        Node { width: percent(100) }
                        ImportSubscriptionUrlField
                        Children [ ( { text_field_with_placeholder_scene(String::new(), "订阅 URL".to_owned(), palette) } ) ]
                    ),
                    (
                        Node { width: percent(100) }
                        ImportLocalPathField
                        Children [ ( { text_field_with_placeholder_scene(String::new(), "本地文件路径 (*.yaml / *.json / *.txt)".to_owned(), palette) } ) ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S4)),
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
                        ImportSubscriptionUrlButton
                        Children [
                            ( Text({ "从 URL 导入".to_owned() }) TextRole(Role::BodyStrong) TextColor({ palette.on_accent }) ),
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
                        ImportLocalSubscriptionButton
                        Children [
                            ( Text({ "从本地文件导入".to_owned() }) TextRole(Role::Body) ),
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
                        ImportClipboardSubscriptionButton
                        Children [
                            ( Text({ "从剪贴板导入".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                ]
            }),
        ],
        palette,
    )
}

fn conditional_status(profile: Option<&crate::pages::profiles::ProfileItem>) -> String {
    match profile.and_then(|profile| {
        profile.etag.as_deref().map(|etag| {
            format!(
                "条件请求已缓存 · ETag: {} · Last-Modified: {}",
                etag,
                profile.last_modified.as_deref().unwrap_or("-")
            )
        })
    }) {
        Some(text) => text,
        None => "条件请求：尚无 ETag / Last-Modified 缓存".to_owned(),
    }
}

/// Restamp the subscription fetch controls in place when the profiles
/// projection updates: the active profile's User-Agent, insecure-TLS toggle,
/// and cached conditional-request validators.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_subscription_fetch_controls(
    update: On<ProfilesProjectionUpdated>,
    toggles: Query<&Children, With<SubscriptionInsecureToggle>>,
    checkboxes: Query<(Entity, Has<Checked>), With<Checkbox>>,
    fields: Query<&Children, With<SubscriptionUserAgentField>>,
    mut text_fields: Query<&mut TextField>,
    mut status_lines: Query<
        &mut Text,
        (
            With<SubscriptionConditionalStatus>,
            Without<SubscriptionBackupStatus>,
        ),
    >,
    mut backup_lines: Query<
        &mut Text,
        (
            With<SubscriptionBackupStatus>,
            Without<SubscriptionConditionalStatus>,
        ),
    >,
    mut commands: Commands,
) {
    let profile = selected_profile(&update.0);
    let user_agent = profile
        .map(|profile| profile.user_agent.clone())
        .unwrap_or_default();
    let insecure = profile.is_some_and(|profile| profile.insecure_skip_verify);

    for children in &toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != insecure
            {
                if insecure {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }

    for children in &fields {
        for child in children.iter() {
            if let Ok(mut field) = text_fields.get_mut(*child)
                && field.0.text() != user_agent
            {
                field.0.apply(
                    infiltrator_bevy_widgets::text_input::state::TextFieldInput::SetText(
                        user_agent.clone(),
                    ),
                );
            }
        }
    }

    let status = conditional_status(profile);
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }

    let backup = backup_status(profile);
    for mut line in &mut backup_lines {
        line.0 = backup.clone();
    }
}

fn backup_status(profile: Option<&crate::pages::profiles::ProfileItem>) -> String {
    if profile.is_some_and(|profile| profile.has_backup) {
        "安全备份已就绪 · 可还原上次写入前的配置".to_owned()
    } else {
        "安全备份：暂无（保存订阅配置时自动生成）".to_owned()
    }
}

/// DUAL-07-13: restore the selected profile's safe pre-save backup through the
/// shared command bus.
pub(super) fn on_restore_subscription_backup(
    activate: On<Activate>,
    buttons: Query<(), With<RestoreSubscriptionBackupButton>>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(profile) = last
        .as_ref()
        .and_then(|last| last.0.as_ref())
        .and_then(selected_profile)
    else {
        return;
    };
    handle.submit(UiCommand::RestoreSubscriptionBackup {
        id: profile.id.clone(),
    });
}

/// Save the selected profile's User-Agent and insecure-TLS preference through
/// the shared command bus.
#[allow(clippy::too_many_arguments)]
pub(super) fn on_save_subscription_fetch_settings(
    activate: On<Activate>,
    buttons: Query<(), With<SaveUserAgentButton>>,
    last: Option<Res<LastProfilesProjection>>,
    fields: Query<&Children, With<SubscriptionUserAgentField>>,
    text_fields: Query<&TextField>,
    toggles: Query<&Children, With<SubscriptionInsecureToggle>>,
    checkboxes: Query<&Checked>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    let Some(profile) = selected_profile(projection) else {
        return;
    };
    let user_agent = fields
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text())
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned);
    let insecure_skip_verify = toggles
        .iter()
        .flat_map(|children| children.iter())
        .any(|child| checkboxes.get(*child).is_ok());
    handle.submit(UiCommand::SaveSubscriptionFetchSettings {
        profile_id: profile.id.clone(),
        user_agent,
        insecure_skip_verify,
    });
}

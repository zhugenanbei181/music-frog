//! Local profile import & subscription User-Agent / conditional-fetch
//! configuration scene component.

#[path = "profiles_import_query_access.rs"]
pub mod query_access;
use self::query_access::SubscriptionFetchControls;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_field_scene;
use crate::localized_widgets::{localized_checkbox_scene, localized_pill_scene};
use crate::pages::profiles::{
    LastProfilesProjection, ProfileItem, ProfilesProjection, ProfilesProjectionUpdated,
};
use crate::pages::profiles_filter_dedup;
use crate::pages::profiles_filter_form::{FilterFormStatus, FilterText};
use crate::pages::profiles_import_channels::{
    ImportClipboardSubscriptionButton, ImportLocalPathField, ImportLocalSubscriptionButton,
    ImportSubscriptionNameField, ImportSubscriptionUrlButton, ImportSubscriptionUrlField,
    SaveSubscriptionFilterButton, SubscriptionFilterExcludeField,
    SubscriptionFilterExcludeTypesField, SubscriptionFilterIncludeField,
    SubscriptionFilterRenamesField, SubscriptionFilterStatus, SubscriptionScheduleStatus,
};
use crate::pages::profiles_subscription_copy;
use crate::pages::profiles_subscription_copy::SubscriptionStatusKind;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With};
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
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::text_input::{TextField, text_field_with_placeholder_scene};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::subscription_filter_form::FilterField;

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

pub(super) fn selected_profile(projection: &ProfilesProjection) -> Option<&ProfileItem> {
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
    let profile = selected_profile(projection);
    let conditional =
        profiles_subscription_copy::projection(SubscriptionStatusKind::Conditional, profile);
    let backup_status =
        profiles_subscription_copy::projection(SubscriptionStatusKind::Backup, profile);
    let filter_status =
        profiles_subscription_copy::projection(SubscriptionStatusKind::Filter, profile);
    let schedule_status =
        profiles_subscription_copy::projection(SubscriptionStatusKind::Schedule, profile);
    let filter = profile
        .map(|profile| profile.filter.clone())
        .unwrap_or_default();
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    @{ icon_tile_scene(IconId::FileText, 24.0, palette) }
                                    --
                                    LocalizedText::plain("profiles_local_import_title") TextRole(Role::BodyStrong)
                                ]
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
                                    LocalizedText::plain("profiles_local_import_path_hint") TextRole(Role::Caption) TextColor({ palette.ink_dim })
                                ]
                                --
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
                                    LocalizedText::plain("profiles_browse_btn") TextRole(Role::Body)
                                ]
                                --
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
                                    LocalizedText::plain("profiles_local_import_action") TextRole(Role::BodyStrong) TextColor({ palette.on_accent })
                                ]
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    LocalizedText::plain("profiles_import_activate") TextRole(Role::Body)
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    LocalizedText::plain("overview_toggle_enabled") TextRole(Role::Caption) TextColor({ palette.success })
                                    --
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
                                        Node {
                                            position_type: PositionType::Absolute,
                                            left: Val::Px(18.0),
                                            width: px(16.0),
                                            height: px(16.0),
                                            border_radius: BorderRadius::all(Val::Px(8.0)),
                                        }
                                        BackgroundColor({ palette.on_accent })
                                    ]
                                ]
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
                                @{ icon_tile_scene(IconId::Settings, 24.0, palette) }
                                --
                                LocalizedText::plain("profiles_request_settings_title") TextRole(Role::BodyStrong)
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
                                Node {
                                    flex_grow: 1.0,
                                }
                                SubscriptionUserAgentField
                                Children [
                                    @{ text_field_with_placeholder_scene(user_agent, "Clash.Meta / ClashVerge / Shadowrocket".to_owned(), palette) }
                                ]
                                --
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
                                    LocalizedText::plain("profiles_request_settings_save") TextRole(Role::BodyStrong) TextColor({ palette.on_accent })
                                ]
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                SubscriptionInsecureToggle
                                Children [
                                    @{ localized_checkbox_scene(LocalizedText::plain("profiles_skip_tls_verification"), insecure_skip_verify, palette) }
                                ]
                                --
                                LocalizedText::new(conditional.key, conditional.params.clone()) SubscriptionConditionalStatus SubscriptionStatusKind::Conditional TextRole(Role::Caption) TextColor({ palette.ink_dim })
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
                                LocalizedText::new(backup_status.key, backup_status.params.clone()) SubscriptionBackupStatus SubscriptionStatusKind::Backup TextRole(Role::Caption) TextColor({ palette.ink_dim })
                                --
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
                                    LocalizedText::plain("profiles_restore_backup") TextRole(Role::Body)
                                ]
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
                                LocalizedText::plain("profiles_user_agent_presets") TextRole(Role::Caption) TextColor({ palette.ink_dim })
                                --
                                Node {
                                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    Text({ "Clash.Meta".to_owned() }) TextRole(Role::Caption)
                                ]
                                --
                                Node {
                                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    Text({ "ClashVerge".to_owned() }) TextRole(Role::Caption)
                                ]
                                --
                                Node {
                                    padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                }
                                BackgroundColor({ palette.surface_elevated })
                                Children [
                                    Text({ "Shadowrocket".to_owned() }) TextRole(Role::Caption)
                                ]
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
                                @{ icon_tile_scene(IconId::Settings, 24.0, palette) }
                                --
                                LocalizedText::plain("profiles_filter_pipeline_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::new(schedule_status.key, schedule_status.params.clone()) SubscriptionScheduleStatus SubscriptionStatusKind::Schedule TextRole(Role::Caption) TextColor({ palette.ink_dim })
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
                                Node { width: percent(100) }
                                SubscriptionFilterIncludeField
                                Children [
                                    @{ localized_field_scene(filter.include.clone(), LocalizedText::plain("field_filter_include"), palette) } FilterText(FilterField::Include) NativeTextField(0)
                                ]
                                --
                                Node { width: percent(100) }
                                SubscriptionFilterExcludeField
                                Children [
                                    @{ localized_field_scene(filter.exclude.clone(), LocalizedText::plain("field_filter_exclude"), palette) } FilterText(FilterField::Exclude) NativeTextField(1)
                                ]
                                --
                                Node { width: percent(100) }
                                SubscriptionFilterExcludeTypesField
                                Children [
                                    @{ localized_field_scene(filter.exclude_types.clone(), LocalizedText::plain("field_filter_protocols"), palette) } FilterText(FilterField::Protocols) NativeTextField(2)
                                ]
                                --
                                Node { width: percent(100) }
                                SubscriptionFilterRenamesField
                                Children [
                                    @{ localized_field_scene(filter.renames.clone(), LocalizedText::plain("field_filter_renames"), palette) } FilterText(FilterField::Renames) NativeTextField(3)
                                ]
                                --
                                LocalizedText::plain("field_filter_advanced") TextRole(Role::Caption)
                                --
                                Node { width: percent(100) }
                                Children [
                                    @{ localized_field_scene(filter.advanced_policy.clone().unwrap_or_default(), LocalizedText::plain("filter_advanced_ph"), palette) } FilterText(FilterField::Advanced) NativeTextField(4)
                                ]
                                --
                                LocalizedText::plain("filter_advanced_help") TextRole(Role::Caption)
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
                                @{ profiles_filter_dedup::scene(filter.dedup_index, palette) }
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    LocalizedText::new(filter_status.key, filter_status.params.clone()) SubscriptionFilterStatus SubscriptionStatusKind::Filter TextRole(Role::Caption) TextColor({ palette.ink_dim })
                                    --
                                    @{ localized_pill_scene(LocalizedText::plain("profiles_filter_pipeline_apply"), true, palette) }
                                    SaveSubscriptionFilterButton
                                    ButtonDisabled(false)
                                ]
                            ]
            }),
            Box::new(bsn! { FilterFormStatus TextRole(Role::Caption) }),
            // Section 4: DUAL-07-01 multi-channel import workbench.
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                                padding: UiRect::top(Val::Px(space::S8)),
                            }
                            Children [
                                @{ icon_tile_scene(IconId::FileText, 24.0, palette) }
                                --
                                LocalizedText::plain("profiles_import_channels_title") TextRole(Role::BodyStrong)
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
                                Node { width: percent(100) }
                                ImportSubscriptionNameField
                                Children [
                                    @{ localized_field_scene(String::new(), LocalizedText::plain("field_profile_name"), palette) }
                                ]
                                --
                                Node { width: percent(100) }
                                ImportSubscriptionUrlField
                                Children [
                                    @{ localized_field_scene(String::new(), LocalizedText::plain("field_subscription_url"), palette) }
                                ]
                                --
                                Node { width: percent(100) }
                                ImportLocalPathField
                                Children [
                                    @{ localized_field_scene(String::new(), LocalizedText::plain("field_profile_file"), palette) }
                                ]
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
                                    LocalizedText::plain("profiles_import_url_action") TextRole(Role::BodyStrong) TextColor({ palette.on_accent })
                                ]
                                --
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
                                    LocalizedText::plain("profiles_import_file_action") TextRole(Role::Body)
                                ]
                                --
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
                                    LocalizedText::plain("profiles_import_clipboard_action") TextRole(Role::Body)
                                ]
                            ]
            }),
        ],
        palette,
    )
}

/// Restamp the subscription fetch controls in place when the profiles
/// projection updates: the active profile's User-Agent, insecure-TLS toggle,
/// and cached conditional-request validators.
pub(super) fn sync_subscription_fetch_controls(
    update: On<ProfilesProjectionUpdated>,
    toggles: Query<&Children, With<SubscriptionInsecureToggle>>,
    checkboxes: Query<(Entity, Has<Checked>), With<Checkbox>>,
    fields: Query<&Children, With<SubscriptionUserAgentField>>,
    mut text_fields: Query<&mut TextField>,
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
                field.0.apply(TextFieldInput::SetText(user_agent.clone()));
            }
        }
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
pub(super) fn on_save_subscription_fetch_settings(
    activate: On<Activate>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
    targets: SubscriptionFetchControls,
) {
    let SubscriptionFetchControls {
        buttons,
        fields,
        text_fields,
        toggles,
        checkboxes,
    } = targets;

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

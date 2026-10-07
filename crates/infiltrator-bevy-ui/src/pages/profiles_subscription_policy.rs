//! DUAL-07-14: the subscription update-policy card.
//!
//! This is the Bevy twin of the Iced subscription editor: the selected
//! profile's subscription URL, auto-update flag, fixed interval, cron
//! expression, and the DUAL-07-09 post-update core-reload preference. Every
//! control is prefilled from the shared `ProfilesPageSnapshot` projection and
//! submitted through the shared command bus; the surface owns no policy logic.

#[path = "profiles_subscription_policy_query_access.rs"]
pub mod query_access;
use self::query_access::{SubscriptionPolicyControls, SubscriptionPolicyTargets};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::localized_widgets::localized_checkbox_scene;
use crate::localized_widgets::localized_field_scene;
use crate::pages::profiles::{
    LastProfilesProjection, ProfileItem, ProfilesProjection, ProfilesProjectionUpdated,
};
use crate::pages::profiles_import::selected_profile;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryFilter, With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::scene::{Scene, bsn};
use bevy::ui::Checked;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_application::subscription_status_projection::{reload_status, schedule_status};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::text_input::state::TextFieldInput;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::subscription_import::SubscriptionScheduleDraft;

/// Marker for the subscription URL text field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionPolicyUrlField;

/// Marker for the auto-update checkbox parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionAutoUpdateToggle;

/// Marker for the update-interval text field parent (hours).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionPolicyIntervalField;

/// Marker for the cron-expression text field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionPolicyCronField;

/// DUAL-07-09: marker for the post-update core-reload checkbox parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionAutoReloadToggle;

/// Marker for the "save update policy" button (URL / auto-update / interval / cron).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveSubscriptionPolicyButton;

/// DUAL-07-09: marker for the "save core reload preference" button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveSubscriptionAutoReloadButton;

/// Status line: the selected profile's persisted schedule.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionPolicyStatus;

/// DUAL-07-09 status line: the persisted core-reload preference.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionAutoReloadStatus;

fn url_of(profile: Option<&ProfileItem>) -> String {
    profile
        .map(|profile| profile.url.clone())
        .unwrap_or_default()
}

fn interval_of(profile: Option<&ProfileItem>) -> String {
    match profile {
        // An unset interval is only meaningful next to a cron expression; the
        // shared application decides whether the 24h default applies.
        Some(profile) => profile
            .update_interval_hours
            .map(|hours| hours.to_string())
            .unwrap_or_default(),
        None => "24".to_owned(),
    }
}

fn cron_of(profile: Option<&ProfileItem>) -> String {
    profile
        .and_then(|profile| profile.cron_expression.clone())
        .unwrap_or_default()
}

fn auto_update_of(profile: Option<&ProfileItem>) -> bool {
    profile.is_some_and(|profile| profile.auto_update_enabled)
}

fn auto_reload_of(profile: Option<&ProfileItem>) -> bool {
    profile.is_some_and(|profile| profile.auto_reload_core)
}

/// The selected profile's persisted schedule summary.
fn policy_status(profile: Option<&ProfileItem>, locale: &str) -> String {
    schedule_status(
        profile.map(|profile| profile.auto_update_enabled),
        profile.and_then(|profile| profile.cron_expression.as_deref()),
        profile.and_then(|profile| profile.update_interval_hours),
    )
    .render(locale)
}
fn auto_reload_status(profile: Option<&ProfileItem>, locale: &str) -> String {
    reload_status(profile.map(|profile| profile.auto_reload_core)).render(locale)
}

/// Subscription update-policy card scene.
pub fn subscription_policy_card_scene(
    projection: &ProfilesProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let profile = selected_profile(projection);
    let url = url_of(profile);
    let interval = interval_of(profile);
    let cron = cron_of(profile);
    let auto_update = auto_update_of(profile);
    let auto_reload = auto_reload_of(profile);
    let status = policy_status(profile, UiLocale::default().code());
    let reload_status = auto_reload_status(profile, UiLocale::default().code());

    surface_scene(
        vec![
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
                                LocalizedText::plain("profiles_update_policy_title") TextRole(Role::BodyStrong)
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
                                SubscriptionPolicyUrlField
                                Children [
                                    @{ localized_field_scene(url, LocalizedText::plain("field_subscription_url"), palette) } NativeTextField(5)
                                ]
                                --
                                Node { width: percent(100) }
                                SubscriptionPolicyIntervalField
                                Children [
                                    @{ localized_field_scene(interval, LocalizedText::plain("field_subscription_interval"), palette) } NativeTextField(6)
                                ]
                                --
                                Node { width: percent(100) }
                                SubscriptionPolicyCronField
                                Children [
                                    @{ localized_field_scene(cron, LocalizedText::plain("field_subscription_cron"), palette) } NativeTextField(7)
                                ]
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                SubscriptionAutoUpdateToggle
                                Children [
                                    @{ localized_checkbox_scene(LocalizedText::plain("subscription_schedule_enabled"), auto_update, palette) }
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    Text({ status.clone() }) SubscriptionPolicyStatus TextRole(Role::Caption)
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
                                    SaveSubscriptionPolicyButton
                                    Children [
                                        LocalizedText::plain("profiles_update_policy_save") TextRole(Role::BodyStrong)
                                    ]
                                ]
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
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                SubscriptionAutoReloadToggle
                                Children [
                                    @{ localized_checkbox_scene(LocalizedText::plain("subscription_reload_after_update"), auto_reload, palette) }
                                ]
                                --
                                Node {
                                    align_items: AlignItems::Center,
                                    column_gap: Val::Px(space::S8),
                                }
                                Children [
                                    Text({ reload_status.clone() }) SubscriptionAutoReloadStatus TextRole(Role::Caption)
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
                                    SaveSubscriptionAutoReloadButton
                                    Children [
                                        LocalizedText::plain("profiles_reload_policy_save") TextRole(Role::Body)
                                    ]
                                ]
                            ]
            }),
        ],
        palette,
    )
}

/// Restamp the policy controls from the shared projection: the surfaces never
/// keep a second copy of the persisted schedule or reload preference.
pub(super) fn sync_subscription_policy_controls(
    update: On<ProfilesProjectionUpdated>,
    mut commands: Commands,
    targets: SubscriptionPolicyTargets,
) {
    let SubscriptionPolicyTargets {
        urls,
        intervals,
        crons,
        auto_update_toggles,
        auto_reload_toggles,
        checkboxes,
        mut text_fields,
        mut status_lines,
        mut reload_lines,
    } = targets;

    let profile = selected_profile(&update.0);

    macro_rules! restamp {
        ($query:expr, $value:expr) => {
            for children in &$query {
                for child in children.iter() {
                    if let Ok(mut field) = text_fields.get_mut(*child)
                        && field.0.text() != $value
                    {
                        field.0.apply(TextFieldInput::SetText($value.clone()));
                    }
                }
            }
        };
    }
    restamp!(urls, url_of(profile));
    restamp!(intervals, interval_of(profile));
    restamp!(crons, cron_of(profile));

    let auto_update = auto_update_of(profile);
    for children in &auto_update_toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != auto_update
            {
                if auto_update {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }

    let auto_reload = auto_reload_of(profile);
    for children in &auto_reload_toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != auto_reload
            {
                if auto_reload {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }

    let status = policy_status(profile, UiLocale::default().code());
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }
    let reload = auto_reload_status(profile, UiLocale::default().code());
    for mut line in &mut reload_lines {
        line.0 = reload.clone();
    }
}

fn read_text_field<F: QueryFilter>(
    parents: &Query<&Children, F>,
    text_fields: &Query<&TextField>,
) -> Option<String> {
    parents
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_owned())
}

fn checked_toggle<F: QueryFilter>(
    parents: &Query<&Children, F>,
    checkboxes: &Query<&Checked>,
) -> bool {
    parents
        .iter()
        .flat_map(|children| children.iter())
        .any(|child| checkboxes.get(*child).is_ok())
}

/// DUAL-07-14: submit the edited schedule draft through the shared command bus.
/// The application validates the cron expression, the URL/auto-update
/// relationship, and the interval; the surface never pre-validates.
pub(super) fn on_save_subscription_policy(
    activate: On<Activate>,
    last: Option<Res<LastProfilesProjection>>,
    handle: Option<Res<CommandSinkHandle>>,
    targets: SubscriptionPolicyControls,
) {
    let SubscriptionPolicyControls {
        buttons,
        urls,
        intervals,
        crons,
        auto_update_toggles,
        text_fields,
        checkboxes,
    } = targets;

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
    let url = read_text_field(&urls, &text_fields).unwrap_or_default();
    let interval_raw = read_text_field(&intervals, &text_fields).unwrap_or_default();
    let cron_raw = read_text_field(&crons, &text_fields).unwrap_or_default();
    let auto_update_enabled = checked_toggle(&auto_update_toggles, &checkboxes);
    // The interval stays free text: the shared application owns parsing and
    // the typed rejection of a malformed or zero value.
    let update_interval_hours = interval_raw.trim().to_owned();
    let cron_expression = {
        let trimmed = cron_raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_owned())
    };
    handle.submit(UiCommand::UpdateSubscriptionSchedule {
        profile_id: profile.id.clone(),
        draft: SubscriptionScheduleDraft {
            url: url.trim().to_owned(),
            auto_update_enabled,
            update_interval_hours,
            cron_expression,
        },
    });
}

/// DUAL-07-09: submit the edited core-reload preference through the shared
/// command bus. A host without a reload seam rejects `enabled = true` with a
/// typed `Unsupported` failure, and the next projection restamp snaps the
/// checkbox back to the persisted value.
pub(super) fn on_save_subscription_auto_reload(
    activate: On<Activate>,
    buttons: Query<(), With<SaveSubscriptionAutoReloadButton>>,
    last: Option<Res<LastProfilesProjection>>,
    toggles: Query<&Children, With<SubscriptionAutoReloadToggle>>,
    checkboxes: Query<&Checked>,
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
    let enabled = checked_toggle(&toggles, &checkboxes);
    handle.submit(UiCommand::SetSubscriptionAutoReload {
        profile_id: profile.id.clone(),
        enabled,
    });
}

#[derive(QueryFilter)]
pub struct PolicyCopyFilter {
    with_status: With<SubscriptionPolicyStatus>,
}
#[derive(QueryFilter)]
pub struct ReloadCopyFilter {
    with_reload: With<SubscriptionAutoReloadStatus>,
    without_status: Without<SubscriptionPolicyStatus>,
}
pub fn replay_policy_copy(
    last: Res<LastProfilesProjection>,
    locale: Res<UiLocale>,
    mut policies: Query<&mut Text, PolicyCopyFilter>,
    mut reloads: Query<&mut Text, ReloadCopyFilter>,
) {
    let profile = last.0.as_ref().and_then(selected_profile);
    let status = policy_status(profile, locale.code());
    for mut text in &mut policies {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
    let status = auto_reload_status(profile, locale.code());
    for mut text in &mut reloads {
        if text.0 != status {
            text.0 = status.clone();
        }
    }
}

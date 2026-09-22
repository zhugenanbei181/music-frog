//! DUAL-07-01/07-08: Bevy nodes for the multi-channel import workbench and
//! the subscription node-cleaning pipeline editor.
//!
//! Split out of `profiles_import.rs` so each business file stays inside the
//! source line budget; the card scene still composes both halves.

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{Has, With, Without};
use bevy::ecs::system::{Commands, Query, Res};
use bevy::ui::Checked;
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Checkbox};
use infiltrator_bevy_widgets::text_input::TextField;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::profiles::{LastProfilesProjection, ProfilesProjectionUpdated};

use super::profiles_import::selected_profile;

/// DUAL-07-08: node-keyword filter include field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterIncludeField;

/// DUAL-07-08: node-keyword filter exclude field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterExcludeField;

/// DUAL-07-08: node-keyword filter protocol-exclusion field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterExcludeTypesField;

/// DUAL-07-08: node-keyword filter rename-rule field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterRenamesField;

/// DUAL-07-08: filter deduplication checkbox parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterDedupToggle;

/// DUAL-07-08: commit the selected profile's filter through the shared runner.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SaveSubscriptionFilterButton;

/// DUAL-07-08: live summary of the selected profile's stored filter pipeline.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionFilterStatus;

/// DUAL-07-03: the selected profile's schedule (interval or cron expression).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubscriptionScheduleStatus;

/// DUAL-07-01: import workbench name field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportSubscriptionNameField;

/// DUAL-07-01: import workbench remote URL field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportSubscriptionUrlField;

/// DUAL-07-01: import workbench local file path field parent.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportLocalPathField;

/// DUAL-07-01: import the remote URL through the shared import application.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportSubscriptionUrlButton;

/// DUAL-07-01: import the local file through the shared import application.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportLocalSubscriptionButton;

/// DUAL-07-01: import whatever the clipboard holds through the host port.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImportClipboardSubscriptionButton;

/// DUAL-07-08: restamp the filter editor from the selected profile's stored
/// draft whenever the profiles projection updates.
#[allow(clippy::too_many_arguments)]
pub(super) fn sync_subscription_filter_controls(
    update: On<ProfilesProjectionUpdated>,
    include: Query<&Children, With<SubscriptionFilterIncludeField>>,
    exclude: Query<&Children, With<SubscriptionFilterExcludeField>>,
    exclude_types: Query<&Children, With<SubscriptionFilterExcludeTypesField>>,
    renames: Query<&Children, With<SubscriptionFilterRenamesField>>,
    toggles: Query<&Children, With<SubscriptionFilterDedupToggle>>,
    checkboxes: Query<(Entity, Has<Checked>), With<Checkbox>>,
    mut text_fields: Query<&mut TextField>,
    mut status_lines: Query<
        &mut Text,
        (
            With<SubscriptionFilterStatus>,
            Without<SubscriptionScheduleStatus>,
        ),
    >,
    mut schedule_lines: Query<
        &mut Text,
        (
            With<SubscriptionScheduleStatus>,
            Without<SubscriptionFilterStatus>,
        ),
    >,
    mut commands: Commands,
) {
    let profile = selected_profile(&update.0);
    let filter = profile.map(|p| p.filter.clone()).unwrap_or_default();

    macro_rules! restamp {
        ($query:expr, $value:expr) => {
            for children in &$query {
                for child in children.iter() {
                    if let Ok(mut field) = text_fields.get_mut(*child)
                        && field.0.text() != $value
                    {
                        field.0.apply(
                            infiltrator_bevy_widgets::text_input::state::TextFieldInput::SetText(
                                $value.clone(),
                            ),
                        );
                    }
                }
            }
        };
    }
    restamp!(include, filter.include);
    restamp!(exclude, filter.exclude);
    restamp!(exclude_types, filter.exclude_types);
    restamp!(renames, filter.renames);

    let dedup_checked = filter.dedup_index != 0;
    for children in &toggles {
        for child in children.iter() {
            if let Ok((entity, checked)) = checkboxes.get(*child)
                && checked != dedup_checked
            {
                if dedup_checked {
                    commands.entity(entity).insert(Checked);
                } else {
                    commands.entity(entity).remove::<Checked>();
                }
            }
        }
    }

    let status = if filter.is_empty() {
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
            if dedup_checked { "开" } else { "关" }
        )
    };
    for mut line in &mut status_lines {
        line.0 = status.clone();
    }

    let schedule = profile
        .map(|p| match p.cron_expression.as_deref() {
            Some(cron) if !cron.trim().is_empty() => format!("定时计划：Cron `{cron}`"),
            _ => "定时计划：按小时周期 / 手动".to_owned(),
        })
        .unwrap_or_else(|| "定时计划：手动".to_owned());
    for mut line in &mut schedule_lines {
        line.0 = schedule.clone();
    }
}

/// DUAL-07-08: commit the edited filter through the shared command bus.
#[allow(clippy::too_many_arguments)]
pub(super) fn on_save_subscription_filter(
    activate: On<Activate>,
    buttons: Query<(), With<SaveSubscriptionFilterButton>>,
    last: Option<Res<LastProfilesProjection>>,
    include: Query<&Children, With<SubscriptionFilterIncludeField>>,
    exclude: Query<&Children, With<SubscriptionFilterExcludeField>>,
    exclude_types: Query<&Children, With<SubscriptionFilterExcludeTypesField>>,
    renames: Query<&Children, With<SubscriptionFilterRenamesField>>,
    toggles: Query<&Children, With<SubscriptionFilterDedupToggle>>,
    text_fields: Query<&TextField>,
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

    macro_rules! read_field {
        ($query:expr) => {
            $query
                .iter()
                .flat_map(|children| children.iter())
                .find_map(|child| text_fields.get(*child).ok())
                .map(|field| field.0.text().to_string())
                .unwrap_or_default()
        };
    }
    let draft = infiltrator_contract::subscription_import::SubscriptionFilterDraft {
        include: read_field!(include),
        exclude: read_field!(exclude),
        exclude_types: read_field!(exclude_types),
        renames: read_field!(renames),
        dedup_index: if toggles
            .iter()
            .flat_map(|children| children.iter())
            .any(|child| checkboxes.get(*child).is_ok())
        {
            1
        } else {
            0
        },
    };
    handle.submit(UiCommand::SaveSubscriptionFilter {
        profile_id: profile.id.clone(),
        filter: draft,
    });
}

/// DUAL-07-01: read the import workbench fields and submit a channel import.
fn submit_import(
    handle: &CommandSinkHandle,
    name: Query<&Children, With<ImportSubscriptionNameField>>,
    text_fields: &Query<&TextField>,
    source: String,
    channel: infiltrator_contract::subscription_import::SubscriptionImportChannel,
) {
    let profile_name = name
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_string())
        .unwrap_or_default();
    if profile_name.trim().is_empty() {
        return;
    }
    handle.submit(UiCommand::ImportSubscription {
        profile_id: profile_name.trim().to_string(),
        channel,
        source,
    });
}

pub(super) fn on_import_subscription_url(
    activate: On<Activate>,
    buttons: Query<(), With<ImportSubscriptionUrlButton>>,
    name: Query<&Children, With<ImportSubscriptionNameField>>,
    url: Query<&Children, With<ImportSubscriptionUrlField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let source = url
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_string())
        .unwrap_or_default();
    submit_import(
        &handle,
        name,
        &text_fields,
        source,
        infiltrator_contract::subscription_import::SubscriptionImportChannel::Url,
    );
}

pub(super) fn on_import_local_subscription(
    activate: On<Activate>,
    buttons: Query<(), With<ImportLocalSubscriptionButton>>,
    name: Query<&Children, With<ImportSubscriptionNameField>>,
    path: Query<&Children, With<ImportLocalPathField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    let source = path
        .iter()
        .flat_map(|children| children.iter())
        .find_map(|child| text_fields.get(*child).ok())
        .map(|field| field.0.text().to_string())
        .unwrap_or_default();
    submit_import(
        &handle,
        name,
        &text_fields,
        source,
        infiltrator_contract::subscription_import::SubscriptionImportChannel::LocalFile,
    );
}

pub(super) fn on_import_clipboard_subscription(
    activate: On<Activate>,
    buttons: Query<(), With<ImportClipboardSubscriptionButton>>,
    name: Query<&Children, With<ImportSubscriptionNameField>>,
    text_fields: Query<&TextField>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    submit_import(
        &handle,
        name,
        &text_fields,
        String::new(),
        infiltrator_contract::subscription_import::SubscriptionImportChannel::Clipboard,
    );
}

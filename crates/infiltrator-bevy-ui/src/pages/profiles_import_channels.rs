//! DUAL-07-01/07-08: Bevy nodes for the multi-channel import workbench and
//! the subscription node-cleaning pipeline editor.
//!
//! Split out of `profiles_import.rs` so each business file stays inside the
//! source line budget; the card scene still composes both halves.

use crate::command::{CommandSinkHandle, UiCommand};

use crate::pages::profiles_filter_form::{self, FilterFormState, FilterText};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_contract::subscription_import::SubscriptionImportChannel;

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

/// Apply only the shared owner draft, with an actual correlated terminal result.
pub(super) fn on_save_subscription_filter(
    activate: On<Activate>,
    buttons: Query<(), With<SaveSubscriptionFilterButton>>,
    mut state: ResMut<FilterFormState>,
    handle: Option<Res<CommandSinkHandle>>,
    fields: Query<(&FilterText, &TextField)>,
) {
    if buttons.contains(activate.entity) {
        if !state.restore_fields {
            profiles_filter_form::capture(&mut state.editor, &fields);
        }
        profiles_filter_form::submit(&mut state, handle.as_deref());
    }
}

/// DUAL-07-01: read the import workbench fields and submit a channel import.
fn submit_import(
    handle: &CommandSinkHandle,
    name: Query<&Children, With<ImportSubscriptionNameField>>,
    text_fields: &Query<&TextField>,
    source: String,
    channel: SubscriptionImportChannel,
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
        SubscriptionImportChannel::Url,
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
        SubscriptionImportChannel::LocalFile,
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
        SubscriptionImportChannel::Clipboard,
    );
}

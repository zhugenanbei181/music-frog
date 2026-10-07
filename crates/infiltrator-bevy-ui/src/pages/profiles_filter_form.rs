//! Native fields feed the shared editor; fact replay never replaces an unsent draft.
use crate::command::{CommandSinkHandle, UiCommand};
use crate::command_events::CommandExecutedEvent;
use crate::pages::profiles::ProfilesProjectionUpdated;
use crate::pages::profiles_import::selected_profile;
use crate::surface::SurfaceSnapshotUpdated;
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::lifecycle::Insert;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::ui::widget::Text;
use infiltrator_application::subscription_filter_copy::status;
use infiltrator_application::subscription_filter_editor::SubscriptionFilterEditor;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::text_input::TextField;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::command_output::CommandOutput;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::subscription_filter_form::{FilterField, FilterObservation};
use infiltrator_contract::surface_snapshot::PageStatus;

#[derive(Component, Clone, Copy, Default)]
pub struct FilterText(pub FilterField);
#[derive(Component, Clone, Copy, Default)]
#[require(Text)]
pub struct FilterFormStatus;
#[derive(Resource, Clone, Default)]
pub struct FilterFormState {
    pub editor: SubscriptionFilterEditor,
    pub request: Option<(RequestId, u64)>,
    pub restore_fields: bool,
}
pub(super) fn initialize(
    insert: On<Insert<FilterText>>,
    state: Res<FilterFormState>,
    mut fields: Query<(&FilterText, &mut TextField)>,
) {
    if state.editor.source_profile().is_some()
        && let Ok((kind, mut field)) = fields.get_mut(insert.entity)
    {
        field
            .0
            .restore_text(kind.0.value(&state.editor.draft).into());
        field.0.set_disabled(!state.editor.can_edit());
    }
}
pub(super) fn capture(
    editor: &mut SubscriptionFilterEditor,
    fields: &Query<(&FilterText, &TextField)>,
) {
    if !editor.can_edit() {
        return;
    }
    for (kind, field) in fields {
        if !field.0.preedit().is_empty() {
            editor.composing();
        }
        editor.edit(kind.0, field.0.text().into());
    }
}
pub(super) fn receive(
    mut state: ResMut<FilterFormState>,
    fields: Query<(&FilterText, &TextField)>,
) {
    if !state.restore_fields {
        capture(&mut state.editor, &fields);
    }
}
pub(super) fn observe(
    update: On<ProfilesProjectionUpdated>,
    mut state: ResMut<FilterFormState>,
    fields: Query<(&FilterText, &TextField)>,
) {
    if !state.restore_fields {
        capture(&mut state.editor, &fields);
    }
    let profile = selected_profile(&update.0);
    let before = state.editor.draft.clone();
    if let Some(profile) = profile {
        state.editor.observe_profile(
            &profile.id,
            profile
                .filter_source
                .clone()
                .map(|source| FilterObservation {
                    source,
                    filter: profile.filter.clone(),
                }),
        );
    } else {
        state.editor.observe(Ok(None));
    }
    state.restore_fields |= before != state.editor.draft;
}
pub(super) fn observe_read_status(
    event: On<SurfaceSnapshotUpdated>,
    mut state: ResMut<FilterFormState>,
) {
    let failure = match &event.0.pages.profiles.status {
        PageStatus::Unavailable { failure } | PageStatus::Failed { failure } => {
            Some(failure.clone())
        }
        PageStatus::Loading => Some(Failure::new(
            ErrorCode::NotReady,
            "Profile filter read is still pending",
            true,
        )),
        PageStatus::Ready | PageStatus::Empty => None,
    };
    if let Some(failure) = failure {
        state.editor.observe(Err(failure));
    }
}
pub(super) fn replay(
    mut state: ResMut<FilterFormState>,
    mut fields: Query<(&FilterText, &mut TextField, Option<&mut AccessibilityNode>)>,
) {
    let disabled = !state.editor.can_edit();
    for (kind, mut field, accessibility) in &mut fields {
        if state.restore_fields {
            field
                .0
                .restore_text(kind.0.value(&state.editor.draft).into());
        }
        if field.0.is_disabled() != disabled {
            field.0.set_disabled(disabled);
        }
        if let Some(mut node) = accessibility {
            if disabled {
                node.set_disabled();
            } else {
                node.clear_disabled();
            }
        }
    }
    state.restore_fields = false;
}
pub(super) fn render_status(
    state: Res<FilterFormState>,
    locale: Res<UiLocale>,
    mut labels: Query<&mut Text, With<FilterFormStatus>>,
) {
    let value = status(&state.editor, locale.code());
    for mut text in &mut labels {
        if text.0 != value {
            text.0.clone_from(&value);
        }
    }
}
pub(super) fn submit(state: &mut FilterFormState, sink: Option<&CommandSinkHandle>) {
    let pending = match state.editor.begin() {
        Ok(pending) => pending,
        Err(_) => return,
    };
    let command = UiCommand::SaveSubscriptionFilter {
        source: pending.source.source,
        filter: pending.draft,
    };
    if let Some(id) = sink.and_then(|sink| sink.submit_tracked(command)) {
        state.request = Some((id, pending.token));
    } else {
        state.editor.finish(
            pending.token,
            Err(Failure::new(
                ErrorCode::NotReady,
                "The filter command service has no terminal acknowledgment",
                true,
            )),
        );
    }
}
pub(super) fn finish(event: On<CommandExecutedEvent>, mut state: ResMut<FilterFormState>) {
    let Some((id, token)) = state.request else {
        return;
    };
    let Some(pending) = &state.editor.pending else {
        return;
    };
    if event.request_id != id
        || event.command
            != (UiCommand::SaveSubscriptionFilter {
                source: pending.source.source.clone(),
                filter: pending.draft.clone(),
            })
    {
        return;
    }
    let result = event
        .result
        .clone()
        .and_then(CommandOutput::into_subscription_filter);
    if state.editor.finish(token, result) {
        state.request = None;
    }
}

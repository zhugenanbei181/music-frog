//! An independent review surface exposes complete frozen contents and real transaction status.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::snapshot_restore::RestoreAction;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Scrollable, Space, button, column, container, row, text};
use iced::{Element, Length};
use infiltrator_application::snapshot_restore_projection::project_restore;
use infiltrator_shared::locales::{Lang, Localizer};
pub(crate) fn modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let presentation = project_restore(&state.editor.snapshot_restore, &state.shell.lang);
    let body = column![
        text(lang.tr("snapshot_restore_title")).size(17),
        text(presentation.status).size(12).width(Length::Fill),
        container(text(presentation.details).size(12).width(Length::Fill))
            .id(InteractionRegion::RestoreDetails.id()),
        text(presentation.content).size(12).width(Length::Fill)
    ]
    .spacing(12)
    .width(Length::Fill);
    let cancel = button(text(lang.tr("snapshot_restore_cancel")))
        .style(style_ghost)
        .on_press_maybe(
            presentation
                .cancel
                .then_some(Message::SnapshotRestore(RestoreAction::Cancel)),
        );
    let mut actions = row![
        container(cancel).id(InteractionRegion::RestoreCancel.id()),
        Space::new().width(Length::Fill)
    ]
    .spacing(8);
    if presentation.confirm {
        let confirm = button(text(lang.tr("snapshot_restore_confirm")))
            .style(style_accent)
            .on_press(Message::SnapshotRestore(RestoreAction::Confirm));
        actions = actions.push(container(confirm).id(InteractionRegion::RestoreConfirm.id()));
    }
    if presentation.retry {
        actions = actions.push(
            button(text(lang.tr("snapshot_restore_retry")))
                .style(style_accent)
                .on_press(Message::SnapshotRestore(RestoreAction::Retry)),
        );
    }
    let review = container(Scrollable::new(body).height(Length::Shrink))
        .max_height((state.shell.viewport.height_px - 180.0).max(80.0))
        .width(Length::Fill);
    let content = column![review, actions].spacing(16);
    let card = modal_card(
        content.into(),
        (state.shell.viewport.width_px - 48.0).clamp(96.0, 700.0),
    );
    modal_backdrop(
        container(card)
            .id(InteractionRegion::RestoreReview.id())
            .into(),
    )
}

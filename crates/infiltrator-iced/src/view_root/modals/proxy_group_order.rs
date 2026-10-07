//! Independent native order editor replays the shared draft without changing live groups.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::theme::{FONT_SEMIBOLD, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn group_order_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let editor = &state.runtime.group_order_editor;
    let pending = editor.pending.is_some();
    let mut groups = column![].spacing(8);
    for (index, name) in editor.draft.iter().enumerate() {
        groups = groups.push(
            row![
                text(name.clone()).width(Length::Fill),
                button(text("▲")).style(style_ghost).on_press_maybe(
                    (!pending && index > 0).then(|| Message::MoveProxyGroupUp(name.clone()))
                ),
                button(text("▼")).style(style_ghost).on_press_maybe(
                    (!pending && index + 1 < editor.draft.len())
                        .then(|| Message::MoveProxyGroupDown(name.clone()))
                ),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        );
    }
    let failure = editor
        .failure
        .as_ref()
        .or(editor.observation_failure.as_ref())
        .map(|failure| failure.message.as_str())
        .unwrap_or_default();
    let body = container(
        scrollable(groups).height((state.shell.viewport.height_px - 270.0).clamp(90.0, 360.0)),
    )
    .id(InteractionRegion::ProxyGroupOrderList.id());
    let content = column![
        text(lang.tr("proxies_reorder_title"))
            .size(16)
            .font(FONT_SEMIBOLD),
        text(lang.tr("proxies_reorder_description")).size(12),
        body,
        text(failure).size(12).style(|theme: &Theme| text::Style {
            color: Some(tokens(theme).danger)
        }),
        row![
            button(text(lang.tr("modal_cancel")))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelProxyGroupOrder)),
            button(text(lang.tr("proxies_reorder_reset")))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::ResetProxyGroupOrder)),
            Space::new().width(Length::Fill),
            button(text(lang.tr(if pending {
                "core_control_pending"
            } else {
                "proxies_reorder_apply"
            })))
            .style(style_accent)
            .on_press_maybe(editor.can_apply().then_some(Message::ApplyProxyGroupOrder)),
        ]
        .spacing(8)
    ]
    .spacing(12);
    let card =
        container(modal_card(content.into(), 520.0)).id(InteractionRegion::ProxyGroupOrder.id());
    modal_backdrop(card.into())
}

//! A native confirmation becomes a persistent results panel after the shared command terminal.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_danger, style_ghost};
use crate::view::theme;
use crate::view::theme::tokens;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Element, Length, Theme};
use infiltrator_presentation::dns_cache_projection::project_cache;
use infiltrator_shared::locales::{Lang, Localizer};
pub fn cache_modal(state: &AppState) -> Element<'_, Message> {
    let model = &state.diag.dns_cache_actions;
    let view = project_cache(model, &Lang(&state.shell.lang));
    let lang = Lang(&state.shell.lang);
    let status = theme::role_body_text(view.status).style(move |theme: &Theme| text::Style {
        color: Some(if view.error {
            tokens(theme).danger
        } else {
            tokens(theme).text_secondary
        }),
    });
    let mut actions = row![
        container(
            button(theme::role_body_text(lang.tr(if model.is_confirmed() {
                "dns_cache_close"
            } else {
                "dns_cache_cancel"
            })))
            .style(style_ghost)
            .on_press_maybe(view.close.then_some(Message::CancelDnsCacheFlush))
        )
        .id(InteractionRegion::CacheCancel.id()),
        Space::new().width(Length::Fill),
    ]
    .spacing(8);
    if view.confirm {
        actions = actions.push(
            container(
                button(theme::role_body_text(lang.tr("dns_cache_confirm")))
                    .style(style_danger)
                    .on_press(Message::ConfirmDnsCacheFlush),
            )
            .id(InteractionRegion::CacheConfirm.id()),
        );
    }
    if view.retry {
        actions = actions.push(
            button(theme::role_body_text(lang.tr("dns_cache_retry")))
                .style(style_accent)
                .on_press(Message::RetryDnsCacheFlush),
        );
    }
    let content = column![
        theme::role_body_text(lang.tr("dns_cache_title")),
        status,
        theme::role_body_text(view.results_label),
        theme::role_body_text(view.fake_ip),
        theme::role_body_text(view.system),
        actions,
    ]
    .spacing(12);
    modal_backdrop(
        container(modal_card(content.into(), 560.0))
            .id(InteractionRegion::CacheDialog.id())
            .into(),
    )
}

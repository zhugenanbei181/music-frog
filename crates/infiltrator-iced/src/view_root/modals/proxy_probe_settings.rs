//! Native parameter editor renders the neutral draft and exact durable write state.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::theme::{FONT_SEMIBOLD, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn probe_settings_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let editor = &state.runtime.probe_options_editor;
    let pending = editor.pending.is_some();
    let failure = editor
        .failure
        .as_ref()
        .or(editor.validation.as_ref())
        .map(|failure| failure.message.as_str())
        .unwrap_or_default();
    let content = column![
        text(lang.tr("proxy_probe_settings_title"))
            .size(16)
            .font(FONT_SEMIBOLD),
        text(lang.tr("proxy_probe_settings_description")).size(12),
        text(lang.tr("proxies_delay_test_url_label")).size(12),
        text_input("https://example.test/204", &editor.draft.test_url)
            .on_input_maybe((!pending).then_some(Message::UpdateDelayTestUrl))
            .padding(10)
            .width(Length::Fill),
        text(lang.tr("proxies_delay_timeout_label")).size(12),
        container(
            text_input("5000", &editor.draft.timeout_ms)
                .on_input_maybe((!pending).then_some(Message::UpdateDelayTimeoutMs))
                .padding(10)
                .width(Length::Fill)
        )
        .id(InteractionRegion::ProxyProbeTimeout.id()),
        text(failure).size(12).style(|theme: &Theme| text::Style {
            color: Some(tokens(theme).danger)
        }),
        row![
            button(text(lang.tr("modal_cancel")))
                .style(style_ghost)
                .on_press_maybe((!pending).then_some(Message::CancelProxyProbeOptions)),
            Space::new().width(Length::Fill),
            button(text(lang.tr(if pending {
                "proxy_probe_settings_pending"
            } else {
                "proxy_probe_settings_apply"
            })))
            .style(style_accent)
            .on_press_maybe(
                editor
                    .can_apply()
                    .then_some(Message::ApplyProxyProbeOptions)
            ),
        ]
        .spacing(12),
    ]
    .spacing(12);
    let card =
        container(modal_card(content.into(), 520.0)).id(InteractionRegion::ProxyProbeSettings.id());
    modal_backdrop(card.into())
}

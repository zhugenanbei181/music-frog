//! Independent inspection of shared, observed proxy facts and delay history.
use super::card::{modal_backdrop, modal_card};
use super::proxy_history::ProxyHistoryChart;
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Canvas, Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_application::proxy_inspection_projection::{
    field_value, history_listing, history_plot,
};
use infiltrator_contract::proxy_inspection::ProxyDetailField;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn inspect_proxy_modal<'a>(state: &'a AppState, proxy_name: &str) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let Some(detail) = state.proxy_inspection(proxy_name) else {
        return Space::new().into();
    };
    let tr = |key: &str| lang.tr(key).into_owned();
    let mut facts = column![].spacing(8);
    for field in ProxyDetailField::ALL {
        facts = facts.push(
            row![
                text(tr(field.key())).size(12).width(Length::FillPortion(1)),
                text(field_value(&detail, field, &tr))
                    .size(12)
                    .font(MONO)
                    .width(Length::FillPortion(2)),
            ]
            .spacing(12),
        );
    }
    let body = column![
        text(tr("proxy_inspection_history"))
            .size(13)
            .font(FONT_SEMIBOLD),
        container(
            Canvas::new(ProxyHistoryChart(history_plot(&detail)))
                .width(Length::Fill)
                .height(90)
        )
        .width(Length::Fill)
        .padding([0, 16])
        .id(InteractionRegion::ProxyInspectionHistory.id()),
        facts,
        text(tr("proxy_inspection_timing_unavailable"))
            .size(12)
            .style(|theme: &Theme| text::Style {
                color: Some(tokens(theme).text_secondary)
            }),
        text(history_listing(&detail, &tr)).size(12).font(MONO),
    ]
    .spacing(16);
    let failure = state
        .runtime
        .inspection_probe
        .failure
        .as_ref()
        .or(state.runtime.inspection_read.failure.as_ref())
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            if state.runtime.inspection_read.loading {
                tr("proxy_inspection_refreshing")
            } else {
                String::new()
            }
        });
    let body_height = (state.shell.viewport.height_px - 250.0).clamp(100.0, 500.0);
    let header = column![
        text(tr("proxy_inspection_title"))
            .size(16)
            .font(FONT_SEMIBOLD),
        text(detail.name.clone()).size(14).font(FONT_MEDIUM),
    ]
    .spacing(4);
    let actions = row![
        button(text(tr("modal_close")))
            .style(style_ghost)
            .on_press(Message::InspectProxy(None)),
        Space::new().width(Length::Fill),
        button(text(tr(
            if state.runtime.inspection_probe.pending.is_some() {
                "core_control_pending"
            } else {
                "modal_speed_test_now"
            }
        )))
        .style(style_accent)
        .on_press_maybe(
            (detail.can_probe
                && (state.surface.latest().is_none() || state.runtime.inspection_read.can_probe())
                && state.runtime.inspection_probe.pending.is_none()
                && (state.commands.is_some() || state.runtime.runtime.is_some()))
            .then_some(Message::TestInspectedProxy)
        ),
    ]
    .align_y(Alignment::Center)
    .spacing(12);
    let content = column![
        header,
        text(failure).size(12).style(|theme: &Theme| text::Style {
            color: Some(tokens(theme).danger)
        }),
        scrollable(body).height(body_height),
        actions
    ]
    .spacing(12);
    let card =
        container(modal_card(content.into(), 560.0)).id(InteractionRegion::ProxyInspection.id());
    modal_backdrop(card.into())
}

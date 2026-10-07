//! A complete native query modal with result tabs and shared bounded pagination.
use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::dns_query::QueryAction;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::theme::{FONT_SEMIBOLD, MONO, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{
    Space, button, column, container, pick_list, row, scrollable, text, text_input,
};
use iced::{Element, Length, Theme};
use infiltrator_application::dns_observation_projection::DnsObservationTone;
use infiltrator_application::dns_query_actions::QuerySection;
use infiltrator_application::dns_query_projection::project_query;
use infiltrator_contract::dns_query::DnsRecordType;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn query_modal(state: &AppState) -> Element<'_, Message> {
    let model = &state.diag.dns_query;
    let view = project_query(model, &state.shell.lang);
    let lang = Lang(&state.shell.lang);
    let name = text_input(lang.tr("dns_query_name").as_ref(), &model.name)
        .on_input_maybe(
            view.run
                .then_some(|name| Message::DnsQuery(QueryAction::Name(name))),
        )
        .padding([8, 10])
        .size(12)
        .style(form_input_style);
    let kind: Element<'_, Message> = if view.run {
        pick_list(DnsRecordType::ALL, Some(model.record_type), |kind| {
            Message::DnsQuery(QueryAction::RecordType(kind))
        })
        .into()
    } else {
        text(model.record_type.wire()).size(12).into()
    };
    let mut tabs = row![].spacing(8);
    for section in QuerySection::ALL {
        tabs = tabs.push(
            button(text(lang.tr(section.key())).size(12))
                .style(if model.section == section {
                    style_accent
                } else {
                    style_ghost
                })
                .on_press(Message::DnsQuery(QueryAction::Section(section))),
        );
    }
    let status = text(view.status)
        .size(12)
        .style(move |theme: &Theme| text::Style {
            color: Some(match view.tone {
                DnsObservationTone::Danger => tokens(theme).danger,
                DnsObservationTone::Warning => tokens(theme).warning,
                DnsObservationTone::Success => tokens(theme).success,
                DnsObservationTone::Neutral => tokens(theme).text_secondary,
            }),
        });
    let mut actions = row![
        container(
            button(text(lang.tr("dns_query_close")).size(12))
                .style(style_ghost)
                .on_press_maybe(view.close.then_some(Message::DnsQuery(QueryAction::Cancel)))
        )
        .id(InteractionRegion::DnsQueryCancel.id()),
        Space::new().width(Length::Fill),
        container(
            button(
                text(lang.tr(if view.retry {
                    "dns_query_retry"
                } else {
                    "dns_query_run"
                }))
                .size(12)
            )
            .style(style_accent)
            .on_press_maybe(view.run.then_some(Message::DnsQuery(if view.retry {
                QueryAction::Retry
            } else {
                QueryAction::Run
            })))
        )
        .id(InteractionRegion::DnsQueryRun.id()),
    ]
    .spacing(8);
    if view.guide {
        actions = actions.push(
            button(text(lang.tr("dns_query_settings")).size(11))
                .style(style_ghost)
                .on_press(Message::Navigate(Route::Settings)),
        );
    }
    let results = column![
        text(view.provenance).size(11),
        text(view.question).size(12).font(MONO),
        text(view.flags).size(11).font(MONO),
        container(text(view.records).size(12).font(MONO))
            .id(InteractionRegion::DnsQueryRecords.id())
    ]
    .spacing(8);
    let content = column![
        text(lang.tr("dns_query_title"))
            .size(17)
            .font(FONT_SEMIBOLD),
        row![
            container(name)
                .width(Length::Fill)
                .id(InteractionRegion::DnsQueryName.id()),
            kind
        ]
        .spacing(8),
        status,
        container(tabs).id(InteractionRegion::DnsQueryTabs.id()),
        scrollable(results).height(Length::Fill),
        row![
            button(text(lang.tr("dns_query_previous_page")).size(11))
                .style(style_ghost)
                .on_press_maybe(
                    view.previous
                        .then_some(Message::DnsQuery(QueryAction::Previous))
                ),
            text(view.counter).size(11),
            button(text(lang.tr("dns_query_next_page")).size(11))
                .style(style_ghost)
                .on_press_maybe(view.next.then_some(Message::DnsQuery(QueryAction::Next))),
        ]
        .spacing(8),
        actions,
    ]
    .spacing(10)
    .height(Length::Fixed(
        (state.shell.viewport.height_px * 0.8).clamp(300.0, 580.0),
    ));
    modal_backdrop(
        container(modal_card(content.into(), 640.0))
            .id(InteractionRegion::DnsQueryDialog.id())
            .into(),
    )
}

//! Native grouping and query widgets replay the shared selection and copy keys.
use crate::state::AppState;
use crate::state::runtime::ConnectionTab;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::theme;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{button, column, container, row, text_input};
use iced::{Element, Font, Length, Theme};
use infiltrator_domain::connection_view::ConnectionGroupingMode;
use infiltrator_presentation::connection_grouping::grouping_label_key;
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) fn grouping<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let lang = Lang(&state.shell.lang);
    container(
        row(ConnectionGroupingMode::ALL.into_iter().map(|mode| {
            let region = match mode {
                ConnectionGroupingMode::Flat => InteractionRegion::ConnectionGroupFlat,
                ConnectionGroupingMode::ByProcess => InteractionRegion::ConnectionGroupProcess,
                ConnectionGroupingMode::ByHost => InteractionRegion::ConnectionGroupHost,
            };
            container(
                button(theme::role_body_text(lang.tr(grouping_label_key(mode))))
                    .padding([6, 8])
                    .style(if state.diag.connections_model.mode() == mode {
                        style_accent
                    } else {
                        style_ghost
                    })
                    .on_press(Message::SetConnectionGroupingMode(mode)),
            )
            .id(region.id())
            .into()
        }))
        .spacing(4),
    )
    .id(InteractionRegion::ConnectionGroupingControls.id())
    .into()
}

pub(crate) fn search<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let lang = Lang(&state.shell.lang);
    let query = state.runtime.connection_query.as_str();
    let input = container(
        text_input(lang.tr("runtime_conn_filter_placeholder").as_ref(), query)
            .on_input(Message::UpdateRuntimeConnectionFilter)
            .padding([7, 10])
            .width(Length::Fill)
            .style(form_input_style),
    )
    .id(InteractionRegion::ConnectionSearch.id())
    .width(Length::Fill);
    let mut controls = row![input].spacing(4);
    if !query.is_empty() {
        controls = controls.push(
            container(
                button(theme::role_body_text(lang.tr("common_clear")))
                    .padding([7, 8])
                    .style(style_ghost)
                    .on_press(Message::UpdateRuntimeConnectionFilter(String::new())),
            )
            .id(InteractionRegion::ConnectionSearchClear.id()),
        );
    }
    let (matched, total, summary_key) = match state.runtime.connection_tab {
        ConnectionTab::Active => (
            state.diag.connections_model.matched_count(),
            state.diag.connections_model.source_count(),
            state.diag.connections_model.search_summary_key(),
        ),
        ConnectionTab::Closed => (
            state.diag.closed_connections_model.matched_count(),
            state.diag.closed_connections_model.source_count(),
            state.diag.closed_connections_model.search_summary_key(),
        ),
    };
    let matched = matched.to_string();
    let total = total.to_string();
    let summary = interpolate(
        lang.tr(summary_key).as_ref(),
        &[("matched", matched.as_str()), ("total", total.as_str())],
    );
    column![controls.width(Length::Fill), theme::role_body_text(summary)]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

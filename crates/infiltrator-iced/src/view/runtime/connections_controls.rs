//! Native grouping and query widgets replay the shared selection and copy keys.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{Element, Font, Length, Theme};
use infiltrator_application::connection_grouping::grouping_label_key;
use infiltrator_domain::connection_view::ConnectionGroupingMode;
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
                button(text(lang.tr(grouping_label_key(mode))).size(12))
                    .padding([6, 8])
                    .style(if state.diag.connection_groups.mode() == mode {
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
    let filter = state.runtime.runtime_connection_filter.trim();
    let closed = filter.starts_with("tab:closed");
    let query = filter.strip_prefix("tab:closed").unwrap_or(filter).trim();
    let on_input = move |value: String| {
        Message::UpdateRuntimeConnectionFilter(if closed {
            format!("tab:closed {value}")
        } else {
            value
        })
    };
    let input = container(
        text_input(lang.tr("runtime_conn_filter_placeholder").as_ref(), query)
            .on_input(on_input)
            .padding([7, 10])
            .size(12)
            .width(Length::Fill)
            .style(form_input_style),
    )
    .id(InteractionRegion::ConnectionSearch.id())
    .width(Length::Fill);
    let mut controls = row![input].spacing(4);
    if !query.is_empty() {
        controls = controls.push(
            container(
                button(text(lang.tr("common_clear")).size(12))
                    .padding([7, 8])
                    .style(style_ghost)
                    .on_press(Message::UpdateRuntimeConnectionFilter(if closed {
                        "tab:closed".into()
                    } else {
                        String::new()
                    })),
            )
            .id(InteractionRegion::ConnectionSearchClear.id()),
        );
    }
    let owner = &state.diag.connection_groups;
    let matched = owner.matched_count().to_string();
    let total = owner.source_count().to_string();
    let summary = interpolate(
        lang.tr(owner.search_summary_key()).as_ref(),
        &[("matched", matched.as_str()), ("total", total.as_str())],
    );
    column![controls.width(Length::Fill), text(summary).size(11)]
        .spacing(4)
        .width(Length::Fill)
        .into()
}

//! Native query controls reuse the shared regex state; renderer choice is toolkit-local.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::theme;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::widget::{button, column, container, row, text_input};
use iced::{Element, Font, Length, Theme};
use infiltrator_shared::i18n_interpolator::interpolate;
use infiltrator_shared::locales::{Lang, Localizer};

pub(crate) fn search<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let lang = Lang(&state.shell.lang);
    let owner = &state.diag.logs_page;
    let input = container(
        text_input(lang.tr("logs_regex_placeholder").as_ref(), owner.query())
            .on_input(Message::UpdateLogRegexFilter)
            .padding([7, 10])
            .width(Length::Fill)
            .style(form_input_style),
    )
    .id(InteractionRegion::LogsSearch.id())
    .width(Length::Fill);
    let clear = container(
        button(theme::role_body_text(lang.tr("common_clear")))
            .padding([7, 8])
            .style(style_ghost)
            .on_press_maybe(
                (!owner.query().is_empty()).then(|| Message::UpdateLogRegexFilter(String::new())),
            ),
    )
    .id(InteractionRegion::LogsSearchClear.id());
    let summary = interpolate(
        lang.tr(owner.status_key()).as_ref(),
        &[
            ("matched", &owner.matched_count().to_string()),
            ("count", &owner.source_count().to_string()),
        ],
    );
    let mut controls = column![
        row![input, clear].spacing(4),
        theme::role_body_text(summary)
    ]
    .spacing(4)
    .width(Length::Fill);
    if let Some(reason) = owner.invalid_pattern() {
        controls = controls.push(theme::role_body_text(interpolate(
            lang.tr("logs_search_invalid_detail").as_ref(),
            &[("reason", reason)],
        )));
    }
    controls.into()
}

pub(crate) fn follow<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let lang = Lang(&state.shell.lang);
    container(
        button(theme::role_body_text(
            lang.tr(state.diag.logs_page.follow().label_key()),
        ))
        .on_press(Message::ToggleLogFollow),
    )
    .id(InteractionRegion::LogFollow.id())
    .into()
}

pub(crate) fn export<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    container(
        button(theme::role_body_text(
            Lang(&state.shell.lang).tr("logs_export_action"),
        ))
        .style(style_accent)
        .on_press_maybe((!state.diag.log_export.is_open()).then_some(Message::ExportRedactedLogs)),
    )
    .id(InteractionRegion::LogExportLaunch.id())
    .into()
}

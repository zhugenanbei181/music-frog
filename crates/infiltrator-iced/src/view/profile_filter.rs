//! Subscription filter pane for the Editor page: include/exclude keyword
//! regexes, type exclusions, rename rules and the dedup strategy, stored in
//! the profile's options sidecar and re-applied on every subscription update.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{form_input_style, segmented_control};
use crate::view::theme::{self, tokens};
use iced::widget::{Space, column, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

/// The per-profile subscription filter form, rendered inside the Editor
/// page's Filter pane. The context profile is the one open in the editor;
/// saving re-runs the pipeline on that profile and stores the spec in its
/// options sidecar (re-applied on every subscription update).
pub fn filter_pane(state: &AppState) -> Element<'_, Message> {
    let draft = &state.editor.filter_draft;
    let context = state
        .editor
        .editor_path
        .as_ref()
        .and_then(|path| path.file_stem())
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "-".to_string());

    let lang = Lang(&state.shell.lang);
    let dedup_labels = vec![
        lang.tr("filter_dedup_disabled").to_string(),
        lang.tr("filter_dedup_first").to_string(),
        lang.tr("filter_dedup_last").to_string(),
        lang.tr("filter_dedup_index").to_string(),
    ];
    let dedup_control =
        segmented_control(&dedup_labels, draft.dedup_index, Message::UpdateFilterDedup);

    let text_field = |placeholder: &str, value: &str, on_input: fn(String) -> Message| {
        text_input(placeholder, value)
            .on_input(on_input)
            .padding([8, 12])
            .size(13)
            .width(Length::Fill)
            .style(form_input_style)
    };
    let save_row = |context: &str| {
        row![
            text(format!(
                "{}：{context} — {}",
                lang.tr("filter_context_prefix"),
                lang.tr("filter_context_hint")
            ))
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
            Space::new().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
    };

    column![
        text_field(
            lang.tr("filter_include_ph").as_ref(),
            &draft.include,
            Message::UpdateFilterInclude
        ),
        Space::new().height(theme::SP_SM),
        text_field(
            lang.tr("filter_exclude_ph").as_ref(),
            &draft.exclude,
            Message::UpdateFilterExclude
        ),
        Space::new().height(theme::SP_SM),
        text_field(
            lang.tr("filter_types_ph").as_ref(),
            &draft.exclude_types,
            Message::UpdateFilterExcludeTypes
        ),
        Space::new().height(theme::SP_SM),
        text_field(
            lang.tr("filter_renames_ph").as_ref(),
            &draft.renames,
            Message::UpdateFilterRenames,
        ),
        Space::new().height(theme::SP_SM),
        dedup_control,
        Space::new().height(theme::SP_MD),
        save_row(&context),
    ]
    .spacing(theme::SP_SM)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

//! Subscription filter pane for the Editor page: include/exclude keyword
//! regexes, type exclusions, rename rules and the dedup strategy, stored in
//! the profile's options sidecar and re-applied on every subscription update.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{form_input_style, style_accent, style_ghost};
use crate::view::theme;
use crate::view::theme::tokens;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::text::Renderer;
use iced::advanced::widget::Id;
use iced::widget::{Space, button, column, container, row, scrollable, text, text_input};
use iced::{Alignment, Border, Element, Font, Length, Padding, Theme};
use infiltrator_application::subscription_filter_copy::status;
use infiltrator_contract::subscription_import::SubscriptionFilterDedup;
use infiltrator_shared::locales::{Lang, Localizer};

/// The per-profile subscription filter form, rendered inside the Editor
/// page's Filter pane. The context profile is the one open in the editor;
/// saving re-runs the pipeline on that profile and stores the spec in its
/// options sidecar (re-applied on every subscription update).
pub fn filter_pane<'a, R: Renderer<Font = Font> + 'a>(
    state: &'a AppState,
) -> Element<'a, Message, Theme, R> {
    let draft = &state.editor.filter_editor.draft;
    let context = state
        .editor
        .editor_path
        .as_ref()
        .and_then(|path| path.file_stem())
        .and_then(|name| name.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| "-".to_string());

    let lang = Lang(&state.shell.lang);
    let dedup_control = row(SubscriptionFilterDedup::ALL.into_iter().map(|mode| {
        button(text(lang.tr(mode.label_key())).size(12))
            .padding([6, 8])
            .style(if draft.dedup_index == mode.index() {
                style_accent
            } else {
                style_ghost
            })
            .on_press_maybe(
                state
                    .editor
                    .filter_editor
                    .can_edit()
                    .then_some(Message::UpdateFilterDedup(mode.index())),
            )
            .into()
    }))
    .spacing(theme::SP_XS);

    let text_field = |placeholder: &str,
                      value: &str,
                      on_input: fn(String) -> Message,
                      region: InteractionRegion| {
        container(
            text_input(placeholder, value)
                .on_input_maybe(state.editor.filter_editor.can_edit().then_some(on_input))
                .padding([8, 12])
                .size(13)
                .width(Length::Fill)
                .style(form_input_style),
        )
        .id(region.id())
    };
    let save_row = |context: &str| {
        row![
            text(format!(
                "{}：{context} — {}",
                lang.tr("filter_context_prefix"),
                lang.tr("filter_context_hint")
            ))
            .size(11)
            .width(Length::Fill)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_tertiary),
            }),
        ]
        .align_y(Alignment::Center)
    };

    let has_error = state.editor.filter_editor.failure.is_some()
        || state.editor.filter_editor.read_failure().is_some();
    let transaction = container(
        column![
            text(lang.tr("field_filter_advanced")).size(13),
            text_field(
                lang.tr("filter_advanced_ph").as_ref(),
                draft.advanced_policy.as_deref().unwrap_or(""),
                Message::UpdateFilterAdvancedPolicy,
                InteractionRegion::FilterAdvanced,
            ),
            text(lang.tr("filter_advanced_help"))
                .size(11)
                .width(Length::Fill),
            container(
                text(status(&state.editor.filter_editor, lang.0))
                    .size(12)
                    .width(Length::Fill)
            )
            .width(Length::Fill)
            .id(InteractionRegion::FilterStatus.id()),
            row![
                container(
                    button(text(lang.tr("filter_form_discard")).size(12))
                        .padding([6, 8])
                        .on_press_maybe(
                            state
                                .editor
                                .filter_editor
                                .pending
                                .is_none()
                                .then_some(Message::DiscardProfileFilter)
                        )
                )
                .id(InteractionRegion::FilterDiscard.id()),
                container(
                    button(text(lang.tr("editor_apply_filter")).size(12))
                        .padding([6, 8])
                        .style(style_accent)
                        .on_press_maybe(
                            state
                                .editor
                                .filter_editor
                                .can_edit()
                                .then_some(Message::SaveProfileFilter)
                        )
                )
                .id(InteractionRegion::FilterSave.id()),
            ]
            .spacing(theme::SP_SM),
        ]
        .spacing(theme::SP_SM)
        .width(Length::Fill),
    )
    .padding([6, 8])
    .style(move |theme: &Theme| container::Style {
        border: Border {
            width: 1.0,
            radius: 4.0.into(),
            color: if has_error {
                tokens(theme).danger
            } else {
                tokens(theme).card_border
            },
        },
        ..Default::default()
    })
    .id(InteractionRegion::FilterTransaction.id());

    let content = column![
        text_field(
            lang.tr("filter_include_ph").as_ref(),
            &draft.include,
            Message::UpdateFilterInclude,
            InteractionRegion::FilterInclude,
        ),
        Space::new().height(theme::SP_SM),
        text_field(
            lang.tr("filter_exclude_ph").as_ref(),
            &draft.exclude,
            Message::UpdateFilterExclude,
            InteractionRegion::FilterExclude,
        ),
        Space::new().height(theme::SP_SM),
        text_field(
            lang.tr("filter_types_ph").as_ref(),
            &draft.exclude_types,
            Message::UpdateFilterExcludeTypes,
            InteractionRegion::FilterProtocols,
        ),
        Space::new().height(theme::SP_SM),
        text_field(
            lang.tr("filter_renames_ph").as_ref(),
            &draft.renames,
            Message::UpdateFilterRenames,
            InteractionRegion::FilterRenames,
        ),
        Space::new().height(theme::SP_SM),
        dedup_control,
        transaction,
        save_row(&context),
    ]
    .spacing(theme::SP_SM)
    .width(Length::Fill)
    .padding(Padding {
        right: 12.0,
        ..Padding::ZERO
    });
    scrollable(content)
        .id(Id::new("filter-scroll"))
        .height(Length::Fill)
        .into()
}

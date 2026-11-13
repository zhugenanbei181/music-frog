//! Passive controls for the Core-owned Connections view preference.
//!
//! The mode, visible columns and column vocabulary all arrive through the
//! shared projection. This module only turns them into localized widgets and
//! emits typed intents; it never stores a mode, column set or label table.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::checkbox::check_box;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::scrollbar;
use crate::view::theme;
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::widget::Id;
use iced::widget::{Column, Row, button, column, container, row};
use iced::{Element, Length};
use infiltrator_contract::connection_view_preference::{ConnectionViewIntent, ConnectionViewMode};
use infiltrator_presentation::connection_view_projection::{
    CONNECTION_VIEW_MODE_LIST_KEY, CONNECTION_VIEW_MODE_TABLE_KEY,
    CONNECTION_VIEW_RESET_COLUMNS_KEY, CONNECTION_VIEW_TITLE_KEY, ConnectionViewProjection,
    project_connection_column_choices,
};
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ColumnChoiceState {
    pub semantic_id: &'static str,
    pub label_key: &'static str,
    pub visible: bool,
    pub intent: ConnectionViewIntent,
}

/// Project the shared column vocabulary against the one authoritative readback.
pub(crate) fn column_choices(projection: &ConnectionViewProjection) -> Vec<ColumnChoiceState> {
    project_connection_column_choices()
        .into_iter()
        .map(|choice| ColumnChoiceState {
            semantic_id: choice.semantic_id,
            label_key: choice.label_key,
            visible: projection
                .columns
                .iter()
                .any(|column| column.column == choice.column),
            intent: ConnectionViewIntent::ToggleColumn {
                column: choice.column,
            },
        })
        .collect()
}

const fn mode_label_key(mode: ConnectionViewMode) -> &'static str {
    match mode {
        ConnectionViewMode::List => CONNECTION_VIEW_MODE_LIST_KEY,
        ConnectionViewMode::Table => CONNECTION_VIEW_MODE_TABLE_KEY,
    }
}

/// Whether the column chooser is mounted for the given readback.
///
/// The column vocabulary configures the table. In the list morphology no
/// column is rendered, so a chooser there would be a control that cannot
/// change what the user sees.
pub(crate) const fn column_chooser_mounted(mode: ConnectionViewMode) -> bool {
    matches!(mode, ConnectionViewMode::Table)
}

pub(crate) fn view_mode_controls(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let projection = &state.diag.connection_view;
    let modes = ConnectionViewMode::ALL;
    let mode_buttons = modes.into_iter().map(|mode| {
        let region = match mode {
            ConnectionViewMode::List => InteractionRegion::ConnectionViewModeList,
            ConnectionViewMode::Table => InteractionRegion::ConnectionViewModeTable,
        };
        let selected = projection.mode == mode;
        container(
            button(theme::role_body_text(lang.tr(mode_label_key(mode))))
                .padding([6, 10])
                .style(if selected { style_accent } else { style_ghost })
                .on_press(Message::ApplyConnectionViewPreference {
                    intent: ConnectionViewIntent::SetViewMode { mode },
                }),
        )
        .id(region.id())
        .into()
    });

    let mode_row = row(mode_buttons)
        .spacing(theme::SP_XS)
        .align_y(iced::Alignment::Center);
    container(column![mode_row].spacing(theme::SP_XS).width(Length::Fill))
        .id(InteractionRegion::ConnectionViewControls.id())
        .width(Length::Fill)
        .into()
}

/// The column chooser of the table morphology.
///
/// The chooser is its own band, not another column of the filter bar: twenty
/// two labelled columns need room the filter bar does not have, and a band
/// that scrolls sideways keeps the page's vertical budget for the table
/// itself.
pub(crate) fn column_chooser_controls(state: &AppState) -> Option<Element<'_, Message>> {
    let lang = Lang(&state.shell.lang);
    let projection = &state.diag.connection_view;
    if !column_chooser_mounted(projection.mode) {
        return None;
    }

    let choices = column_choices(projection);
    let columns: Vec<Element<'_, Message>> = choices
        .into_iter()
        .map(|choice| {
            let intent = choice.intent.clone();
            container(check_box(
                choice.visible,
                lang.tr(choice.label_key),
                move |_| Message::ApplyConnectionViewPreference {
                    intent: intent.clone(),
                },
            ))
            .id(Id::from(format!(
                "connection-view-column-{}",
                choice.semantic_id
            )))
            .into()
        })
        .collect();
    let reset = container(
        button(theme::role_body_text(
            lang.tr(CONNECTION_VIEW_RESET_COLUMNS_KEY),
        ))
        .padding([6, 10])
        .style(style_ghost)
        .on_press(Message::ApplyConnectionViewPreference {
            intent: ConnectionViewIntent::ResetColumns,
        }),
    )
    .id(InteractionRegion::ConnectionViewResetColumns.id());

    let title = theme::role_body_text(lang.tr(CONNECTION_VIEW_TITLE_KEY));
    let actions: Element<'_, Message> = if state.shell.viewport.tier.is_narrow() {
        Column::from_iter([Element::from(reset)])
            .spacing(theme::SP_XS)
            .width(Length::Fill)
            .into()
    } else {
        Row::from_iter([Element::from(reset)])
            .spacing(theme::SP_SM)
            .align_y(iced::Alignment::Center)
            .width(Length::Fill)
            .into()
    };

    // One row that scrolls sideways: the chooser must never cost the table
    // its own height.
    let column_strip = scrollbar::horizontal_scrollable(
        row(columns)
            .spacing(theme::SP_SM)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill);

    Some(
        container(
            column![title, actions, column_strip]
                .spacing(theme::SP_XS)
                .width(Length::Fill),
        )
        .id(InteractionRegion::ConnectionViewColumns.id())
        .width(Length::Fill)
        .into(),
    )
}

#[cfg(test)]
#[path = "../../../tests/gui/connection_view_controls_tests.rs"]
mod tests;

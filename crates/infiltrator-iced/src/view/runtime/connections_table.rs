//! The Connections page's table morphology.
//!
//! The list answers "what is happening right now"; the table answers "which
//! facts does this user want side by side". Every cell comes from the shared
//! projection, so this module only lays the published columns out: it never
//! re-derives a value, never keeps a column set of its own and never fills a
//! cell the owner left empty.
//!
//! The column widths follow the projection's width classes, and the table
//! scrolls sideways when the user's own column set is wider than the page.
//! The header and the body share that one sideways scroll, so a column
//! always carries its own label.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_forms::{style_chip, style_plain};
use crate::view::components::icon_button;
use crate::view::runtime::connection_search;
use crate::view::scrollbar;
use crate::view::theme;
use crate::view::{svg_icons, svg_icons::Icon};
use iced::advanced::widget::Id;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Padding, Theme, border};
use infiltrator_contract::connection_view_preference::ConnectionColumn;
use infiltrator_contract::design_resources::{TextRole, typeface};
use infiltrator_domain::connection_view::ConnectionSortDirection;
use infiltrator_presentation::connection_search::ConnectionSearchRow;
use infiltrator_presentation::connection_view_table_projection::{
    ConnectionColumnWidth, ConnectionTableCell, ConnectionTableHeader, ConnectionTableProjection,
    ConnectionTableStatus, ConnectionTableTone, UNAVAILABLE, ellipsize,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// The width a narrow column gets.
const NARROW_WIDTH_PX: f32 = 92.0;
/// The width a regular column gets.
const REGULAR_WIDTH_PX: f32 = 152.0;
/// The width a wide column gets.
const WIDE_WIDTH_PX: f32 = 224.0;
/// The action column at the end of a live row.
const ACTION_WIDTH_PX: f32 = 72.0;
/// The horizontal clearance inside one cell.
const CELL_PADDING_X: f32 = 8.0;
/// The vertical clearance inside one cell.
const CELL_PADDING_Y: f32 = 7.0;

/// The width one column class gets.
fn column_width(width: ConnectionColumnWidth) -> f32 {
    match width {
        ConnectionColumnWidth::Narrow => NARROW_WIDTH_PX,
        ConnectionColumnWidth::Regular => REGULAR_WIDTH_PX,
        ConnectionColumnWidth::Wide => WIDE_WIDTH_PX,
    }
}

/// The role one tone reads in.
fn tone_role(tone: ConnectionTableTone) -> TextRole {
    match tone {
        ConnectionTableTone::Plain | ConnectionTableTone::Accent | ConnectionTableTone::Muted => {
            TextRole::Label
        }
        ConnectionTableTone::Numeric => TextRole::Mono,
    }
}

/// How much text one cell may hold, measured in em.
fn text_budget_em(width_px: f32, role: TextRole) -> f32 {
    let font_px = typeface(role).size_px;
    ((width_px - CELL_PADDING_X * 2.0) / font_px).max(1.0)
}

/// The status dot of one connection row.
///
/// A live row reads as the success ink and an ended row as the muted ink:
/// a closed connection is a fact, not a failure.
pub(crate) fn connection_status_dot<'a>(status: ConnectionTableStatus) -> Element<'a, Message> {
    let color = move |t: &Theme| match status {
        ConnectionTableStatus::Active => theme::tokens(t).success,
        ConnectionTableStatus::Closed => theme::tokens(t).text_tertiary,
    };
    container(Space::new().width(10).height(10))
        .style(move |t: &Theme| container::Style {
            background: Some(color(t).into()),
            border: Border {
                radius: border::Radius::from(theme::R_PILL),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

/// The connections page in its table morphology.
pub(crate) fn connection_table<'a>(
    state: &'a AppState,
    projection: &ConnectionTableProjection,
    status: ConnectionTableStatus,
    search_row: impl Fn(&str) -> Option<&'a ConnectionSearchRow>,
) -> Element<'a, Message> {
    let headers = &projection.headers;
    let header_cells: Vec<Element<'a, Message>> = headers
        .iter()
        .map(|header| header_cell(header, Lang(&state.shell.lang)))
        .collect();
    let header_row = row(header_cells)
        .spacing(0)
        .align_y(Alignment::Center)
        .width(Length::Shrink);

    let mut row_widgets: Vec<Element<'a, Message>> = Vec::with_capacity(projection.rows.len());
    for table_row in &projection.rows {
        let mut cells: Vec<Element<'a, Message>> = Vec::with_capacity(headers.len() + 1);
        for (header, cell) in headers.iter().zip(table_row.cells.iter()) {
            cells.push(body_cell(state, header, cell, search_row(&table_row.id)));
        }
        if status == ConnectionTableStatus::Active {
            cells.push(action_cell(&table_row.id));
        }
        row_widgets.push(
            container(row(cells).spacing(0).align_y(Alignment::Center))
                .id(connection_search::row_id(&table_row.id))
                .width(Length::Shrink)
                .style(row_surface)
                .into(),
        );
    }

    let body = scrollbar::vertical_scrollable(column(row_widgets).spacing(0)).height(Length::Fill);
    scrollbar::horizontal_scrollable(column![header_row, body].spacing(0))
        .height(Length::Fill)
        .into()
}

/// The stable id of one column's header sort affordance.
fn header_sort_id(semantic_id: &str) -> Id {
    Id::from(format!("connection-table-sort-{semantic_id}"))
}

/// One header cell.
///
/// A column the owner ranks reads as a control the reader can press, the way
/// the reference product's header does; a column it cannot rank stays plain
/// text, so no header is ever inert.
fn header_cell<'a>(header: &ConnectionTableHeader, lang: Lang<'a>) -> Element<'a, Message> {
    let width = column_width(header.width);
    let label = lang.tr(header.semantic_id).to_string();
    let budget = text_budget_em(width, TextRole::Caption);
    let caption = |accent: bool| {
        theme::role_text(ellipsize(&label, budget), TextRole::Caption).style(move |t: &Theme| {
            text::Style {
                color: Some(if accent {
                    theme::tokens(t).text_primary
                } else {
                    theme::tokens(t).text_tertiary
                }),
            }
        })
    };
    let Some(sort_key) = header.sort else {
        return container(caption(false))
            .width(Length::Fixed(width))
            .padding(cell_padding())
            .align_x(Alignment::Start)
            .into();
    };
    // The active rank is the only mark a header carries: an unpressed column
    // shows no arrow at all, so the row never pays for a glyph that says
    // nothing. iced's `Row` has no `push_maybe`, so the mark is appended on
    // the one branch that has it.
    let mut label = row![caption(header.sorted)]
        .spacing(theme::SP_XS)
        .align_y(Alignment::Center);
    if header.sorted {
        let glyph = match sort_key.direction() {
            ConnectionSortDirection::Descending => Icon::ArrowDown,
            ConnectionSortDirection::Ascending => Icon::ArrowUp,
        };
        label = label.push(container(svg_icons::icon_themed(
            glyph,
            11.0,
            |t: &Theme| theme::tokens(t).text_primary,
        )));
    }
    container(
        button(label)
            .padding([2, 0])
            .style(style_plain)
            .on_press(Message::UpdateRuntimeConnectionSort(sort_key)),
    )
    .id(header_sort_id(header.semantic_id))
    .width(Length::Fixed(width))
    .padding(cell_padding())
    .align_x(Alignment::Start)
    .into()
}

/// One body cell.
fn body_cell<'a>(
    state: &'a AppState,
    header: &ConnectionTableHeader,
    cell: &ConnectionTableCell,
    search: Option<&'a ConnectionSearchRow>,
) -> Element<'a, Message> {
    let width = column_width(header.width);
    let column = header.column;
    let tone = header.tone;
    let content: Element<'a, Message> = match cell {
        ConnectionTableCell::Status(status) => connection_status_dot(*status),
        ConnectionTableCell::Chain { outbound, rest } => {
            // The reference product renders the outbound as a chip the
            // reader can press to narrow the list to that proxy; the hops
            // after it stay plain text.
            let active = state.runtime.connection_query.trim() == outbound.trim();
            let filter = if active {
                String::new()
            } else {
                outbound.clone()
            };
            let chip = button(theme::role_text(outbound.clone(), TextRole::Label))
                .padding([3, 8])
                .style(style_chip)
                .on_press(Message::UpdateRuntimeConnectionFilter(filter));
            if rest.is_empty() {
                chip.into()
            } else {
                row![
                    chip,
                    Space::new().width(theme::SP_XS),
                    theme::role_text(rest.clone(), TextRole::Label).style(|t: &Theme| {
                        text::Style {
                            color: Some(theme::tokens(t).text_tertiary),
                        }
                    }),
                ]
                .spacing(0)
                .align_y(Alignment::Center)
                .into()
            }
        }
        ConnectionTableCell::Text(value) => {
            // The search highlight is the page's own answer to "which row
            // matched"; the table keeps it on the endpoint column, so the
            // table morphology never loses the search feature.
            if column == ConnectionColumn::Host
                && let Some(search) = search
                && !search.endpoint.is_empty()
            {
                connection_search::connection_text(state, &search.endpoint, 13)
            } else {
                let role = tone_role(tone);
                let budget = text_budget_em(width, role);
                theme::role_text(ellipsize(value, budget), role)
                    .style(move |t: &Theme| text::Style {
                        color: Some(tone_color(t, tone)),
                    })
                    .into()
            }
        }
        ConnectionTableCell::Live(value) => {
            let budget = text_budget_em(width, TextRole::Mono);
            theme::role_text(ellipsize(value, budget), TextRole::Mono)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).success),
                })
                .into()
        }
        ConnectionTableCell::Unavailable => {
            theme::role_text(UNAVAILABLE.to_owned(), TextRole::Caption)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_tertiary),
                })
                .into()
        }
    };
    container(content)
        .width(Length::Fixed(width))
        .padding(cell_padding())
        .align_x(Alignment::Start)
        .clip(true)
        .into()
}

/// The trailing action cell of a live row.
fn action_cell<'a>(id: &str) -> Element<'a, Message> {
    container(
        row![
            icon_button(
                Icon::Activity,
                14.0,
                Message::InspectConnection(Some(id.to_owned()))
            ),
            Space::new().width(theme::SP_XS),
            icon_button(Icon::X, 14.0, Message::CloseConnection(id.to_owned())),
        ]
        .spacing(0)
        .align_y(Alignment::Center),
    )
    .width(Length::Fixed(ACTION_WIDTH_PX))
    .padding(cell_padding())
    .align_x(Alignment::Start)
    .into()
}

/// The clearance inside one cell.
fn cell_padding() -> Padding {
    Padding {
        top: CELL_PADDING_Y,
        right: CELL_PADDING_X,
        bottom: CELL_PADDING_Y,
        left: CELL_PADDING_X,
    }
}

/// The ink one tone reads in.
fn tone_color(theme: &Theme, tone: ConnectionTableTone) -> iced::Color {
    let tokens = theme::tokens(theme);
    match tone {
        ConnectionTableTone::Plain => tokens.text_primary,
        ConnectionTableTone::Accent => tokens.accent,
        ConnectionTableTone::Muted => tokens.text_secondary,
        ConnectionTableTone::Numeric => tokens.text_secondary,
    }
}

/// The surface one table row reads on.
fn row_surface(theme: &Theme) -> container::Style {
    let tokens = theme::tokens(theme);
    container::Style {
        background: Some(tokens.card_bg.into()),
        border: Border {
            radius: border::Radius::from(0.0),
            width: theme::HAIRLINE,
            color: tokens.divider,
        },
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "../../../tests/gui/connections_table_tests.rs"]
mod tests;

//! Form controls, button styles and frame surfaces for the Infiltrator shell.
//!
//! Split out of [`crate::view::components`] so the base widget primitives and
//! the token-driven form/button vocabulary stay independently reviewable.

use crate::view::components::{BadgeKind, icon_button, toggle_switch};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme;
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Border, Color, Element, Length, Theme, border};

/// Search input field with leading magnifying glass icon and trailing clear button.
pub fn search_input<'a, Message: 'a + Clone>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
    on_clear: Message,
) -> Element<'a, Message> {
    let icon = svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| {
        theme::tokens(t).text_tertiary
    });
    let input = text_input(placeholder, value)
        .on_input(on_input)
        .padding([7, 10])
        .size(13)
        .width(Length::Fill)
        .style(form_input_style);

    let mut items = vec![icon, input.into()];
    if !value.is_empty() {
        items.push(icon_button(Icon::X, 12.0, on_clear));
    }

    row(items)
        .spacing(theme::SP_SM)
        .align_y(iced::Alignment::Center)
        .into()
}

/// Reusable dynamic list editor: card rows for existing items with a delete button,
/// and a bottom text input row with an add button.
pub fn dynamic_list_editor<'a, Message: 'a + Clone>(
    items: &[String],
    draft: &str,
    placeholder: &str,
    on_input: impl Fn(String) -> Message + 'a,
    on_add: Message,
    on_remove: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let item_rows: Vec<Element<'a, Message>> = items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let label = text(item.clone())
                .size(12)
                .font(theme::MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_primary),
                });
            let delete_btn = icon_button(Icon::Trash2, 13.0, on_remove(idx));
            let item_content = row![label, Space::new().width(Length::Fill), delete_btn]
                .align_y(iced::Alignment::Center)
                .spacing(theme::SP_SM);

            container(item_content)
                .width(Length::Fill)
                .padding([6, 10])
                .style(row_card_surface)
                .into()
        })
        .collect();

    let input_widget = text_input(placeholder, draft)
        .on_input(on_input)
        .on_submit(on_add.clone())
        .padding([7, 10])
        .size(13)
        .width(Length::Fill)
        .style(form_input_style);
    let add_btn = icon_button(Icon::Plus, 13.0, on_add);
    let input_row = row![input_widget, add_btn]
        .spacing(theme::SP_SM)
        .align_y(iced::Alignment::Center);

    let mut col = column(item_rows).spacing(theme::SP_SM).width(Length::Fill);
    col = col.push(input_row);
    col.into()
}

// ---------------------------------------------------------------------------
// Standard Token-Driven Button Styles & Helpers
// ---------------------------------------------------------------------------

pub fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = theme::tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => (
            Color {
                a: 0.85,
                ..tk.accent
            },
            tk.on_accent,
        ),
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

pub fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = theme::tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered => Some(tk.hover.into()),
            button::Status::Pressed => Some(tk.pressed.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
}

pub fn style_danger(t: &Theme, status: button::Status) -> button::Style {
    let tk = theme::tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.text_tertiary),
        button::Status::Hovered | button::Status::Pressed => (
            Color {
                a: 0.24,
                ..tk.danger
            },
            tk.on_accent,
        ),
        _ => (
            Color {
                a: 0.14,
                ..tk.danger
            },
            tk.danger,
        ),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

/// Standard text push button: renders disabled style when `on_press == None`.
pub fn text_btn<'a, Message: 'a + Clone>(
    label: impl Into<String>,
    style: fn(&Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label.into()).size(12).font(theme::FONT_MEDIUM))
        .padding([7, 14])
        .style(style)
        .on_press_maybe(on_press)
        .into()
}

// ---------------------------------------------------------------------------
// Standard Form Controls & Frame Styles
// ---------------------------------------------------------------------------

pub fn form_input_style(
    t: &Theme,
    status: iced::widget::text_input::Status,
) -> iced::widget::text_input::Style {
    let tk = theme::tokens(t);
    let (border_color, border_width) = match status {
        iced::widget::text_input::Status::Focused { .. } => (tk.focus_ring, 1.5),
        _ => (tk.card_border, 1.0),
    };
    iced::widget::text_input::Style {
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: border_width,
            color: border_color,
        },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color {
            a: 0.25,
            ..tk.accent
        },
    }
}

pub fn form_pick_style(
    t: &Theme,
    _status: iced::widget::pick_list::Status,
) -> iced::widget::pick_list::Style {
    let tk = theme::tokens(t);
    iced::widget::pick_list::Style {
        text_color: tk.text_primary,
        placeholder_color: tk.text_tertiary,
        handle_color: tk.text_secondary,
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
    }
}

pub fn form_field_label(value: impl Into<String>) -> text::Text<'static> {
    text(value.into()).size(11).style(|t: &Theme| text::Style {
        color: Some(theme::tokens(t).text_secondary),
    })
}

pub fn form_toggle_row<'a, Message: 'a + Clone>(
    label: impl Into<String>,
    value: bool,
    on_change: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label.into()).size(13).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_primary),
        }),
        Space::new().width(Length::Fill),
        toggle_switch(value, on_change),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Inline notification alert banner for section headers and forms.
pub fn banner_alert<'a, Message: 'a + Clone>(
    kind: BadgeKind,
    title: impl Into<String>,
    detail: impl Into<String>,
    action: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title_str = title.into();
    let detail_str = detail.into();

    let icon_name = match kind {
        BadgeKind::Accent | BadgeKind::Warning | BadgeKind::Neutral => Icon::Activity,
        BadgeKind::Success => Icon::ListChecks,
        BadgeKind::Danger => Icon::Shield,
    };

    let status_icon = svg_icons::icon_themed(icon_name, 16.0, move |t: &Theme| {
        kind.color(theme::tokens(t))
    });

    let mut text_col =
        column![
            text(title_str)
                .size(13)
                .font(theme::FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_primary),
                })
        ];

    if !detail_str.is_empty() {
        text_col = text_col.push(text(detail_str).size(12).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }));
    }
    text_col = text_col.spacing(2);

    let mut banner_row = row![status_icon, text_col]
        .spacing(theme::SP_MD)
        .align_y(iced::Alignment::Center);

    if let Some(action_elem) = action {
        banner_row = banner_row
            .push(Space::new().width(Length::Fill))
            .push(action_elem);
    }

    container(banner_row)
        .width(Length::Fill)
        .padding([10, 14])
        .style(move |t: &Theme| {
            let color = kind.color(theme::tokens(t));
            container::Style {
                background: Some(Color { a: 0.10, ..color }.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
                    color: Color { a: 0.25, ..color },
                },
                ..Default::default()
            }
        })
        .into()
}

/// Bordered surface for embedded code / JSON / config text editors.
pub fn editor_frame_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.control_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

/// Bordered row-card surface for items inside section lists.
pub fn row_card_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CONTROL),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

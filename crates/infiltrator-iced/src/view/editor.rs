use crate::locales::{Lang, Localizer};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, R_CONTROL, tokens};
use crate::{AppState, Message};
use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Token-driven control styles (ui-wave2-r)
// ---------------------------------------------------------------------------

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
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
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
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

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let filename = state
        .editor
        .editor_path
        .as_ref()
        .and_then(|p: &PathBuf| p.file_name())
        .and_then(|n| n.to_str())
        .unwrap_or("Untitled");

    // Toolbar row: filename + save (accent) / cancel (ghost) actions.
    let toolbar = row![
        column![
            text(filename)
                .size(18)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_primary),
                }),
            text("YAML")
                .size(11)
                .font(theme::FONT_MEDIUM)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        ]
        .spacing(2),
        iced::widget::Space::new().width(Length::Fill),
        button(
            text(lang.tr("btn_save").to_string())
                .size(12)
                .font(theme::FONT_MEDIUM),
        )
        .padding([7, 14])
        .style(style_accent)
        .on_press(Message::SaveProfile),
        iced::widget::Space::new().width(theme::SP_MD),
        button(
            row![
                crate::view::svg_icons::icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t)
                    .text_secondary),
                text(lang.tr("btn_cancel").to_string())
                    .size(12)
                    .font(theme::FONT_MEDIUM),
            ]
            .spacing(theme::SP_SM),
        )
        .padding([7, 14])
        .style(style_ghost)
        .on_press(Message::Navigate(crate::types::Route::Profiles)),
    ]
    .align_y(Alignment::Center);

    // Editor area framed in a card surface, mono typeface for YAML.
    let editor = container(
        text_editor(&state.editor.editor_content)
            .on_action(Message::EditorAction)
            .font(MONO)
            .padding(12)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: 1.0,
                color: tk.card_border,
            },
            shadow: tk.card_shadow,
            ..Default::default()
        }
    });

    let content = column![
        toolbar,
        iced::widget::Space::new().height(theme::SP_MD),
        editor,
    ]
    .spacing(theme::SP_SM);

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

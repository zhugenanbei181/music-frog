//! Global hotkey manager card.

use super::integration::secondary_text;
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{card, style_accent, style_ghost};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length};
use infiltrator_shared::locales::{Lang, Localizer};

pub(super) fn hotkeys_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    use infiltrator_contract::shortcuts::ShortcutAction;
    let mut rows = column![].spacing(theme::SP_SM);
    let capturing = state.shell.hotkey_capture;
    for action in ShortcutAction::ALL {
        let Some(binding) = state.shell.shortcut_registry.get(action) else {
            continue;
        };
        let action_name = lang.tr(action.label_key());
        let accelerator = binding.chord.display_string(cfg!(target_os = "macos"));
        let is_capturing = capturing == Some(action);
        let capturing_label = if is_capturing {
            lang.tr("hotkey_capture_hint").to_string()
        } else {
            accelerator.clone()
        };

        // Capture button: starts listening for the next chord; the mutation
        // itself is validated + persisted by the shared shortcut facade.
        let capture_button = button(text(capturing_label).size(11).font(MONO))
            .padding([4, 10])
            .style(if is_capturing {
                style_accent
            } else {
                style_ghost
            });
        let capture_button = if is_capturing {
            capture_button.on_press(Message::CancelHotkeyCapture)
        } else {
            capture_button.on_press(Message::BeginHotkeyCapture(action))
        };

        let row_item = row![
            column![
                text(action_name.to_string()).size(13).font(FONT_SEMIBOLD),
                secondary_text(format!(
                    "{}: {} · {}",
                    lang.tr("shortcut_action"),
                    accelerator,
                    action.id()
                )),
            ]
            .width(Length::Fill),
            capture_button,
            Space::new().width(theme::SP_SM),
            button(text(if binding.enabled { "Active" } else { "Off" }).size(11))
                .padding([4, 8])
                .style(if binding.enabled {
                    style_accent
                } else {
                    style_ghost
                })
                .on_press(Message::ToggleHotkeyEnabled(action)),
            Space::new().width(theme::SP_XS),
            button(text(lang.tr("hotkey_reset").to_string()).size(11))
                .padding([4, 8])
                .style(style_ghost)
                .on_press(Message::ResetHotkey(action)),
        ]
        .align_y(Alignment::Center);

        rows = rows.push(row_item);
    }

    card(
        Some(lang.tr("hotkey_manager_title").to_string()),
        column![
            secondary_text(lang.tr("hotkey_manager_desc")),
            Space::new().height(theme::SP_XS),
            rows,
        ]
        .spacing(theme::SP_SM),
    )
}

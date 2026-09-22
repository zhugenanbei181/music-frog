//! Frameless-window chrome strip (DUAL-15-13).
//!
//! The strip is the replacement for the OS title bar: the title area is a
//! real drag surface (`Message::WindowChromeDragRequested` →
//! `iced::window::drag`) whose double click toggles maximize, and the three
//! controls dispatch minimize / maximize / close through the host window ops.
//! Height and double-click rule come from the shared chrome contract, so the
//! strip cannot drift from the settings that made the window frameless.

use crate::accessibility::labelled;
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::icon_button;
use crate::view::svg_icons::Icon;
use crate::view::theme;
use iced::widget::{Space, container, mouse_area, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::a11y::ShellA11yNode;

/// The mounted strip's height in logical pixels; the contract owns the number.
pub(crate) fn strip_height_px() -> f32 {
    infiltrator_contract::window_chrome::WindowChrome::FRAMELESS.chrome_height_px() as f32
}

/// The drag/controls strip mounted at the top of the frameless window.
///
/// Returns an empty zero-height element when the active chrome style keeps the
/// system decorations (the OS already owns dragging), so the view root can
/// mount it unconditionally.
pub fn chrome_strip(state: &AppState) -> Element<'_, Message> {
    if !infiltrator_contract::window_chrome::WindowChrome::FRAMELESS.needs_custom_controls() {
        return Space::new().height(Length::Shrink).into();
    }

    let title = text(state.title())
        .size(12)
        .color(theme::tokens(&state.shell.theme).text_secondary);

    let drag_surface = mouse_area(
        row![title, Space::new().width(Length::Fill)]
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .on_press(Message::WindowChromeDragRequested)
    .on_double_click(Message::WindowChromeToggleMaximize);

    let controls = row![
        // DUAL-15-10: text-free window controls carry the shared semantic
        // labels (the same rows Bevy mounts as AccessKit nodes).
        labelled(
            ShellA11yNode::ChromeMinimize,
            &state.shell.lang,
            icon_button(Icon::Minus, 13.0, Message::WindowChromeMinimize),
        ),
        labelled(
            ShellA11yNode::ChromeMaximize,
            &state.shell.lang,
            icon_button(Icon::Square, 11.0, Message::WindowChromeToggleMaximize),
        ),
        labelled(
            ShellA11yNode::ChromeClose,
            &state.shell.lang,
            icon_button(Icon::X, 13.0, Message::WindowChromeClose),
        ),
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    let bar = container(
        row![drag_surface, controls]
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(strip_height_px())
    .padding([0.0, theme::SP_SM])
    .style(|theme: &Theme| container::Style {
        background: Some(theme::tokens(theme).sidebar.into()),
        ..Default::default()
    });

    let hairline = container(Space::new().height(theme::HAIRLINE))
        .width(Length::Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme::tokens(theme).card_border.into()),
            ..Default::default()
        });

    iced::widget::column![bar, hairline].into()
}

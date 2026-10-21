//! test-intent: behavior
//! The switch is the toolkit's own toggle switch.
//!
//! The framework owns the interaction and the geometry: a press publishes the
//! toggled value, a refused action leaves the control inert, and the laid out
//! control is the toolkit's 2:1 pill. The product owns only the skin, so the
//! track and the knob colours come from the shared resolution tokens.

use super::*;
use iced::advanced::Shell;
use iced::advanced::clipboard::Null;
use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::Tree;
use iced::mouse::{self, Button, Cursor};
use iced::{Event, Point, Rectangle, Size};

#[derive(Debug, Clone, PartialEq)]
enum SwitchMsg {
    Toggled(bool),
}

/// The area the switch is laid out in.
fn stage() -> Size {
    Size::new(120.0, 48.0)
}

/// Presses the switch and returns the messages the toolkit published.
fn press(element: &mut Element<'_, SwitchMsg, Theme, ()>, at: Point) -> Vec<SwitchMsg> {
    let size = stage();
    let mut tree = Tree::new(element.as_widget());
    let node = element
        .as_widget_mut()
        .layout(&mut tree, &(), &Limits::new(size, size));
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    element.as_widget_mut().update(
        &mut tree,
        &Event::Mouse(mouse::Event::ButtonPressed(Button::Left)),
        Layout::new(&node),
        Cursor::Available(at),
        &(),
        &mut Null,
        &mut shell,
        &Rectangle::with_size(size),
    );
    messages
}

/// The toolkit's toggle switch publishes the toggled value on the press
/// itself, so the surface never has to keep a second press state.
#[test]
fn the_switch_publishes_the_toggled_value_on_the_press() {
    let mut element: Element<'_, SwitchMsg, Theme, ()> = toggle_switch(true, SwitchMsg::Toggled);
    let messages = press(&mut element, Point::new(22.0, 11.0));
    assert_eq!(
        messages,
        vec![SwitchMsg::Toggled(false)],
        "the toolkit's toggle switch owns the toggle"
    );
}

/// A refused action leaves the control inert: the toolkit's toggle switch
/// publishes nothing when it has no toggle handler.
#[test]
fn a_refused_switch_action_stays_inert() {
    let mut element: Element<'_, SwitchMsg, Theme, ()> =
        toggle_switch_with_actions(true, |_value| None);
    let messages = press(&mut element, Point::new(22.0, 11.0));
    assert!(
        messages.is_empty(),
        "a refused action must not publish a toggle: {messages:?}"
    );
}

/// The switch geometry is the toolkit's, not a locally drawn pill: the
/// control is laid out as the toolkit's 2:1 track.
#[test]
fn the_switch_geometry_comes_from_the_toolkit() {
    let mut element: Element<'_, SwitchMsg, Theme, ()> = toggle_switch(true, SwitchMsg::Toggled);
    let size = stage();
    let mut tree = Tree::new(element.as_widget());
    let node = element
        .as_widget_mut()
        .layout(&mut tree, &(), &Limits::new(size, size));
    assert_eq!(node.size().width, 2.0 * SWITCH_SIZE_PX);
    assert_eq!(node.size().height, SWITCH_SIZE_PX);
}

/// The skin is the product's: the track, the knob and the pill radius read
/// the shared resolution tokens.
#[test]
fn the_switch_skin_reads_the_shared_tokens() {
    let dark = Theme::Dark;
    let tk = theme::tokens(&dark);

    let on = switch_style(&dark, toggler::Status::Active { is_toggled: true });
    assert_eq!(on.background, tk.accent.into());
    assert_eq!(on.foreground, tk.switch_knob.into());
    assert_eq!(
        on.border_radius,
        Some(border::Radius::from(SWITCH_SIZE_PX / 2.0)),
        "the switch keeps the product's pill radius"
    );

    let off = switch_style(&dark, toggler::Status::Active { is_toggled: false });
    assert_eq!(off.background, tk.switch_track.into());
    assert_eq!(off.foreground, tk.switch_knob.into());
    assert_ne!(on.background, off.background);
}

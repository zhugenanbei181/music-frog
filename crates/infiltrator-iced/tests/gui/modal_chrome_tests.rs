//! test-intent: behavior
//! Native event dispatch with Iced's headless renderer; this does not claim pixel evidence.
use super::modal_backdrop;
use crate::types::message::Message;
use iced::advanced::Shell;
use iced::advanced::clipboard::Null;
use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::Tree;
use iced::mouse::{self, Button, Cursor, ScrollDelta};
use iced::widget::{Space, button, mouse_area, stack};
use iced::{Element, Event, Length, Point, Rectangle, Size, Theme};

fn layer() -> Element<'static, Message, Theme, ()> {
    let live: Element<'static, Message, Theme, ()> =
        mouse_area(Space::new().width(Length::Fill).height(Length::Fill))
            .on_press(Message::SelectProxy(
                "live-group".into(),
                "live-node".into(),
            ))
            .on_scroll(|_| Message::TestAllProxyDelays)
            .into();
    let cancel: Element<'static, Message, Theme, ()> = button(Space::new().width(100).height(40))
        .padding(0)
        .on_press(Message::CancelProxyGroupOrder)
        .into();
    stack([live, modal_backdrop(cancel)]).into()
}
fn dispatch(
    element: &mut Element<'_, Message, Theme, ()>,
    tree: &mut Tree,
    event: mouse::Event,
    point: Point,
) -> Vec<Message> {
    let size = Size::new(720.0, 480.0);
    let node = element
        .as_widget_mut()
        .layout(tree, &(), &Limits::new(size, size));
    let mut messages = vec![];
    let mut shell = Shell::new(&mut messages);
    element.as_widget_mut().update(
        tree,
        &Event::Mouse(event),
        Layout::new(&node),
        Cursor::Available(point),
        &(),
        &mut Null,
        &mut shell,
        &Rectangle::with_size(size),
    );
    assert!(
        shell.is_event_captured(),
        "the native top layer owns this event"
    );
    messages
}
#[test]
fn modal_scrim_consumes_background_clicks_and_scroll_without_selecting_or_probing_hidden_nodes() {
    for event in [
        mouse::Event::ButtonPressed(Button::Left),
        mouse::Event::ButtonPressed(Button::Right),
        mouse::Event::ButtonPressed(Button::Middle),
        mouse::Event::WheelScrolled {
            delta: ScrollDelta::Lines { x: 0.0, y: -1.0 },
        },
    ] {
        let mut element = layer();
        let mut tree = Tree::new(element.as_widget());
        let messages = dispatch(&mut element, &mut tree, event, Point::new(10.0, 10.0));
        assert_eq!(messages.len(), 1);
        assert!(
            matches!(messages[0], Message::Noop),
            "the scrim cannot leak a live proxy command"
        );
    }
}
#[test]
fn real_modal_buttons_still_receive_clicks_and_cancel_without_background_side_effects() {
    let mut element = layer();
    let mut tree = Tree::new(element.as_widget());
    let center = Point::new(360.0, 240.0);
    assert!(
        dispatch(
            &mut element,
            &mut tree,
            mouse::Event::ButtonPressed(Button::Left),
            center
        )
        .is_empty()
    );
    let messages = dispatch(
        &mut element,
        &mut tree,
        mouse::Event::ButtonReleased(Button::Left),
        center,
    );
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], Message::CancelProxyGroupOrder));
}

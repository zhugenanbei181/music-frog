//! test-intent: behavior
//! Drive production native widgets through ordinary pointer and keyboard events.
use crate::types::message::Message;
use iced::advanced::Shell;
use iced::advanced::clipboard::Null;
use iced::advanced::layout::{Layout, Limits};
use iced::advanced::widget::{Id, Operation, Tree};
use iced::keyboard::{self, Key, Location, Modifiers, key};
use iced::mouse::{self, Button, Cursor};
use iced::{Element, Point, Rectangle, Size, Theme};
struct Region {
    id: Id,
    bounds: Option<Rectangle>,
}
impl Operation<()> for Region {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<()>)) {
        visit(self);
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&self.id) {
            self.bounds = Some(bounds);
        }
    }
}
fn key_event(value: &str, modifiers: Modifiers, text: Option<&str>) -> iced::Event {
    iced::Event::Keyboard(keyboard::Event::KeyPressed {
        key: Key::Character(value.into()),
        modified_key: Key::Character(value.into()),
        physical_key: key::Physical::Code(key::Code::KeyA),
        location: Location::Standard,
        modifiers,
        text: text.map(Into::into),
        repeat: false,
    })
}
pub fn native(
    mut element: Element<'_, Message, Theme, ()>,
    id: Id,
    replace: Option<&str>,
) -> Vec<Message> {
    let mut tree = Tree::new(element.as_widget());
    let size = Size::new(1100.0, 1600.0);
    let node = element
        .as_widget_mut()
        .layout(&mut tree, &(), &Limits::new(Size::ZERO, size));
    let layout = Layout::new(&node);
    let mut target = Region { id, bounds: None };
    element
        .as_widget_mut()
        .operate(&mut tree, layout, &(), &mut target);
    let bounds = target.bounds.expect("native operation surface");
    assert!(bounds.width > 0.0 && bounds.height > 0.0);
    let cursor = Cursor::Available(Point::new(
        bounds.x + bounds.width / 2.0,
        bounds.y + bounds.height / 2.0,
    ));
    let mut events = vec![
        iced::Event::Mouse(mouse::Event::ButtonPressed(Button::Left)),
        iced::Event::Mouse(mouse::Event::ButtonReleased(Button::Left)),
    ];
    if let Some(value) = replace {
        events.push(iced::Event::Keyboard(keyboard::Event::ModifiersChanged(
            Modifiers::CTRL,
        )));
        events.push(key_event("a", Modifiers::CTRL, None));
        events.push(iced::Event::Keyboard(keyboard::Event::ModifiersChanged(
            Modifiers::empty(),
        )));
        for character in value.chars() {
            let value = character.to_string();
            events.push(key_event(&value, Modifiers::empty(), Some(&value)));
        }
    }
    let mut messages = vec![];
    for event in events {
        element.as_widget_mut().update(
            &mut tree,
            &event,
            layout,
            cursor,
            &(),
            &mut Null,
            &mut Shell::new(&mut messages),
            &Rectangle::with_size(size),
        );
    }
    messages
}

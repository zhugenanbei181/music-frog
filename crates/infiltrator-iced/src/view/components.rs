use crate::{Message, Route};
use iced::widget::{Space, button, canvas, container, scrollable, text};
use iced::{
    Border, Color, Element, Length, Point, Rectangle, Renderer, Theme, border, font, mouse,
};
use std::collections::VecDeque;

use crate::view::icons;

pub const WEB_ACCENT: Color = Color::from_rgb(0.22, 0.74, 0.97);
pub const WEB_SUCCESS: Color = Color::from_rgb(0.20, 0.83, 0.60);
pub const WEB_SURFACE: Color = Color::from_rgba(0.06, 0.09, 0.16, 0.82);
pub const SCROLLBAR_GUTTER: f32 = 16.0;

pub fn modern_scrollable<'a, T: 'a>(
    content: impl Into<Element<'a, T>>,
) -> iced::widget::Scrollable<'a, T> {
    let safe_content = container(content).padding(iced::Padding {
        top: 0.0,
        right: SCROLLBAR_GUTTER,
        bottom: 0.0,
        left: 0.0,
    });

    scrollable(safe_content).direction(scrollable::Direction::Vertical(
        scrollable::Scrollbar::new().width(6).margin(4),
    ))
}

pub fn nav_button<'a>(label: String, route: Route, current_route: &Route) -> Element<'a, Message> {
    let is_active = route == *current_route;
    let icon = match route {
        Route::Overview => icons::DASHBOARD,
        Route::Profiles => icons::PROFILE,
        Route::Proxies => icons::PROXY,
        Route::Runtime => icons::ACTIVITY,
        Route::Rules => icons::RULES,
        Route::Dns => icons::DNS,
        Route::Sync => icons::SYNC,
        Route::Settings => icons::SETTINGS,
        Route::Editor => icons::EDITOR,
    };

    let content = container(
        row![
            if is_active {
                Element::from(container(Space::new().width(4).height(18)).style(
                    |_theme: &Theme| container::Style {
                        background: Some(WEB_ACCENT.into()),
                        border: Border {
                            radius: border::Radius::from(2.0),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                ))
            } else {
                Element::from(Space::new().width(4).height(18))
            },
            Space::new().width(15),
            text(icon).size(16),
            Space::new().width(12),
            text(label).size(15).font(font::Font {
                weight: if is_active {
                    font::Weight::Bold
                } else {
                    font::Weight::Normal
                },
                ..Default::default()
            }),
        ]
        .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding([12, 0]);

    button(content)
        .width(Length::Fill)
        .style(move |_theme, status| {
            let is_dark = _theme == &Theme::Dark;
            let mut style = button::Style::default();

            if is_active {
                style.background = Some(Color::from_rgba(0.22, 0.74, 0.97, 0.1).into());
                style.text_color = WEB_ACCENT;
            } else {
                style.background = None;
                style.text_color = if status == button::Status::Hovered {
                    if is_dark { Color::WHITE } else { Color::BLACK }
                } else {
                    if is_dark {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.4)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.5)
                    }
                };
            }
            style
        })
        .on_press(Message::Navigate(route))
        .into()
}

use iced::widget::row;

pub fn status_dot<'a>(active: bool) -> Element<'a, Message> {
    let color = if active {
        WEB_SUCCESS
    } else {
        Color::from_rgb(0.97, 0.44, 0.44)
    };
    container(Space::new().width(10).height(10))
        .style(move |_theme| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: border::Radius::from(5.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

pub fn card<'a, Message: 'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .style(|_theme: &Theme| {
            let is_dark = _theme == &Theme::Dark;
            container::Style {
                background: Some(if is_dark { WEB_SURFACE } else { Color::WHITE }.into()),
                border: Border {
                    radius: border::Radius::from(16.0),
                    width: 1.0,
                    color: if is_dark {
                        Color::from_rgba(1.0, 1.0, 1.0, 0.08)
                    } else {
                        Color::from_rgba(0.0, 0.0, 0.0, 0.05)
                    },
                },
                ..Default::default()
            }
        })
        .padding(24)
        .into()
}

pub fn premium_card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .style(|_theme: &Theme| container::Style {
            background: Some(Color::from_rgba(0.22, 0.74, 0.97, 0.05).into()),
            border: Border {
                radius: border::Radius::from(20.0),
                width: 1.5,
                color: Color::from_rgba(0.22, 0.74, 0.97, 0.2),
            },
            ..Default::default()
        })
        .padding(30)
        .into()
}

pub struct TrafficChart {
    pub history: VecDeque<(u64, u64)>,
}

impl<Message> canvas::Program<Message> for TrafficChart {
    type State = ();
    fn draw(
        &self,
        _state: &(),
        _renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(_renderer, bounds.size());
        if self.history.len() < 2 {
            return vec![frame.into_geometry()];
        }
        let (width, height) = (bounds.width, bounds.height);
        let max_points = 60;
        let x_step = width / (max_points - 1) as f32;
        let mut max_speed = self
            .history
            .iter()
            .map(|(u, d)| std::cmp::max(*u, *d))
            .max()
            .unwrap_or(1024 * 100);
        if max_speed < 1024 * 100 {
            max_speed = 1024 * 100;
        }
        let scale = |speed: u64| height - (speed as f32 / max_speed as f32) * height;
        let down_path = canvas::Path::new(|p| {
            p.move_to(Point::new(0.0, height));
            for (i, (_, down)) in self.history.iter().enumerate() {
                p.line_to(Point::new(i as f32 * x_step, scale(*down)));
            }
            p.line_to(Point::new((self.history.len() - 1) as f32 * x_step, height));
            p.close();
        });
        frame.fill(&down_path, Color::from_rgba(0.22, 0.74, 0.97, 0.1));
        let down_line = canvas::Path::new(|p| {
            for (i, (_, down)) in self.history.iter().enumerate() {
                let pt = Point::new(i as f32 * x_step, scale(*down));
                if i == 0 {
                    p.move_to(pt);
                } else {
                    p.line_to(pt);
                }
            }
        });
        frame.stroke(
            &down_line,
            canvas::Stroke::default()
                .with_color(WEB_ACCENT)
                .with_width(2.5),
        );
        let up_line = canvas::Path::new(|p| {
            for (i, (up, _)) in self.history.iter().enumerate() {
                let pt = Point::new(i as f32 * x_step, scale(*up));
                if i == 0 {
                    p.move_to(pt);
                } else {
                    p.line_to(pt);
                }
            }
        });
        frame.stroke(
            &up_line,
            canvas::Stroke::default()
                .with_color(WEB_SUCCESS)
                .with_width(2.0),
        );
        vec![frame.into_geometry()]
    }
}

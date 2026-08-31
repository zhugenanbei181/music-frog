//! Shared, design-token-driven widgets for the Infiltrator shell.
//!
//! Every color comes from [`crate::view::theme::tokens`] so light and dark
//! are both first-class. Page views should compose these primitives instead
//! of hand-rolling containers with hardcoded colors.

use crate::types::app::Route;
use crate::types::message::Message;
use iced::widget::{Space, button, canvas, column, container, row, scrollable, text};
use iced::{
    Border, Color, Element, Length, Point, Rectangle, Renderer, Shadow, Size, Theme, Vector,
    border, mouse,
};
use std::collections::VecDeque;

use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, Tokens};

// ---------------------------------------------------------------------------
// Legacy palette constants (pre-token pages still import these).
// New code must take colors from `theme::tokens` instead.
// ---------------------------------------------------------------------------
pub const WEB_ACCENT: Color = theme::LIGHT.accent;
pub const WEB_SUCCESS: Color = theme::LIGHT.success;
pub const WEB_SURFACE: Color = Color::from_rgba(0.06, 0.09, 0.16, 0.82);
pub const SCROLLBAR_GUTTER: f32 = 16.0;

// ---------------------------------------------------------------------------
// Scrollable
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// Standard card surface: card background, 1px hairline border, radius 16
/// and the soft card shadow. Reusable by any container-based widget.
pub fn card_surface(t: &Theme) -> container::Style {
    let t = theme::tokens(t);
    container::Style {
        background: Some(t.card_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CARD),
            width: 1.0,
            color: t.card_border,
        },
        shadow: t.card_shadow,
        ..Default::default()
    }
}

/// The canonical card. `title` renders as a semibold header above the
/// content (pass `None` for borderless-of-text cards). Layout: 24px padding,
/// full width.
pub fn card<'a, Message: 'a>(
    title: Option<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let content = content.into();

    let body = match title {
        Some(title) => {
            let header = text(title)
                .size(14)
                .font(theme::FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_primary),
                });
            column![header, content]
                .spacing(theme::SP_MD)
                .width(Length::Fill)
                .into()
        }
        None => content,
    };

    container(body)
        .width(Length::Fill)
        .padding(theme::SP_XXL)
        .style(card_surface)
        .into()
}

/// Accent-tinted highlight card (used for hero/summary panels).
pub fn premium_card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .padding(30)
        .style(|t: &Theme| {
            let t = theme::tokens(t);
            container::Style {
                background: Some(t.accent_soft.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: 1.0,
                    color: Color {
                        a: 0.25,
                        ..t.accent
                    },
                },
                shadow: t.card_shadow,
                ..Default::default()
            }
        })
        .into()
}

/// KPI tile: icon chip + label + value, with an optional selected accent.
pub fn stat_card<'a, Message: 'a>(
    icon: Icon,
    label: &str,
    value: &str,
    accent: Color,
    selected: bool,
) -> Element<'a, Message> {
    let icon_chip = container(svg_icons::icon(icon, 20.0, Color { a: 0.9, ..accent }))
        .width(40)
        .height(40)
        .align_x(iced::Alignment::Center)
        .align_y(iced::Alignment::Center)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Color { a: 0.14, ..accent }.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        });

    let texts = column![
        text(label.to_string())
            .size(11)
            .font(theme::FONT_MEDIUM)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_secondary),
            }),
        text(value.to_string())
            .size(18)
            .font(theme::FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_primary),
            }),
    ]
    .spacing(2);

    let content = row![icon_chip, texts]
        .spacing(theme::SP_MD)
        .align_y(iced::Alignment::Center);

    container(content)
        .width(Length::Fill)
        .padding(theme::SP_LG)
        .style(move |t: &Theme| {
            let t = theme::tokens(t);
            container::Style {
                background: Some(t.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: if selected { 1.5 } else { 1.0 },
                    color: if selected { accent } else { t.card_border },
                },
                shadow: t.card_shadow,
                ..Default::default()
            }
        })
        .into()
}

/// Section title row with optional trailing content (buttons, badges, ...).
pub fn section_header<'a, Message: 'a>(
    title: &str,
    trailing: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title = text(title.to_string())
        .size(13)
        .font(theme::FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        });

    row![
        title,
        Space::new().width(Length::Fill),
        trailing.unwrap_or_else(|| Space::new().width(0).into()),
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Centered placeholder for empty lists/views.
pub fn empty_state<'a, Message: 'a>(icon: Icon, title: &str, hint: &str) -> Element<'a, Message> {
    column![
        svg_icons::icon_themed(icon, 36.0, |t: &Theme| theme::tokens(t).text_tertiary),
        text(title.to_string())
            .size(14)
            .font(theme::FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_secondary),
            }),
        text(hint.to_string())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_tertiary),
            }),
    ]
    .spacing(theme::SP_SM)
    .align_x(iced::Alignment::Center)
    .width(Length::Fill)
    .padding(theme::SP_XXL)
    .into()
}

// ---------------------------------------------------------------------------
// Small indicators
// ---------------------------------------------------------------------------

/// Semantic badge kinds mapped to token colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BadgeKind {
    Accent,
    Success,
    Warning,
    Danger,
    Neutral,
}

impl BadgeKind {
    fn color(self, t: &Tokens) -> Color {
        match self {
            BadgeKind::Accent => t.accent,
            BadgeKind::Success => t.success,
            BadgeKind::Warning => t.warning,
            BadgeKind::Danger => t.danger,
            BadgeKind::Neutral => t.text_secondary,
        }
    }
}

/// Small tinted pill for statuses ("ACTIVE", "ERROR", counts, ...).
pub fn badge<'a, Message: 'a>(label: impl Into<String>, kind: BadgeKind) -> Element<'a, Message> {
    container(text(label.into()).size(11).font(theme::FONT_SEMIBOLD))
        .padding([3, 8])
        .style(move |t: &Theme| {
            let color = kind.color(theme::tokens(t));
            container::Style {
                background: Some(Color { a: 0.14, ..color }.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    ..Default::default()
                },
                text_color: Some(color),
                ..Default::default()
            }
        })
        .into()
}

/// Neutral pill for protocol/type tags ("Shadowsocks", "VMess", ...).
pub fn chip<'a, Message: 'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(text(label.into()).size(11).font(theme::FONT_MEDIUM))
        .padding([3, 10])
        .style(|t: &Theme| {
            let t = theme::tokens(t);
            container::Style {
                background: Some(t.chip_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    ..Default::default()
                },
                text_color: Some(t.text_secondary),
                ..Default::default()
            }
        })
        .into()
}

/// Colored latency numeral: green <=200 ms, orange <=500 ms, red above,
/// gray/em-dash when untested. Rendered with the bundled JetBrains Mono so
/// live updates do not jitter.
pub fn latency_badge<'a, Message: 'a>(ms: Option<u32>) -> Element<'a, Message> {
    let label = match ms {
        Some(ms) => format!("{ms} ms"),
        None => "—".to_string(),
    };
    text(label)
        .size(12)
        .font(theme::MONO)
        .style(move |t: &Theme| text::Style {
            color: Some(theme::latency_color(theme::tokens(t), ms)),
        })
        .into()
}

/// Small round status indicator.
pub fn status_dot<'a>(active: bool) -> Element<'a, Message> {
    let color = move |t: &Theme| {
        let t = theme::tokens(t);
        if active { t.success } else { t.danger }
    };
    container(Space::new().width(10).height(10))
        .style(move |t: &Theme| container::Style {
            background: Some(color(t).into()),
            border: Border {
                radius: border::Radius::from(5.0),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// Controls
// ---------------------------------------------------------------------------

/// Ghost icon-only button (transparent until hover).
pub fn icon_button<'a, Message: 'a + Clone>(
    icon: Icon,
    size: f32,
    on_press: Message,
) -> Element<'a, Message> {
    let glyph = svg_icons::icon_themed(icon, size, |t: &Theme| theme::tokens(t).text_secondary);
    button(glyph)
        .padding(6)
        .style(|t: &Theme, status| {
            let tokens = theme::tokens(t);
            button::Style {
                background: match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(tokens.control_bg.into())
                    }
                    _ => None,
                },
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .on_press(on_press)
        .into()
}

/// Canvas-drawn iOS-style toggle: 44x26 pill, sliding 22px knob.
struct Switch {
    value: bool,
}

impl<Message> canvas::Program<Message> for Switch {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let t = theme::tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());

        let height = 26.0_f32.min(bounds.height);
        let radius = height / 2.0;
        let track_size = Size::new(bounds.width.min(44.0), height);

        let track = if self.value { t.accent } else { t.switch_track };
        frame.fill(
            &canvas::Path::rounded_rectangle(
                Point::ORIGIN,
                track_size,
                border::Radius::from(radius),
            ),
            track,
        );

        // Knob travel: 2px inset each side.
        let knob_radius = radius - 2.0;
        let inset = 2.0;
        let knob_x = if self.value {
            track_size.width - knob_radius - inset
        } else {
            knob_radius + inset
        };
        let knob_y = radius;

        // Soft knob shadow.
        frame.fill(
            &canvas::Path::circle(Point::new(knob_x, knob_y + 0.5), knob_radius),
            Color {
                a: 0.20,
                ..Color::BLACK
            },
        );
        frame.fill(
            &canvas::Path::circle(Point::new(knob_x, knob_y), knob_radius),
            t.switch_knob,
        );

        vec![frame.into_geometry()]
    }
}

/// iOS-style toggle switch (26pt tall pill, sliding knob). Emits
/// `on_change(!value)` when pressed.
pub fn toggle_switch<'a, Message: 'a + Clone>(
    value: bool,
    on_change: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    button(canvas::Canvas::new(Switch { value }).width(44).height(26))
        .padding(0)
        .style(|_t: &Theme, _status| button::Style {
            background: None,
            ..Default::default()
        })
        .on_press(on_change(!value))
        .into()
}

/// Segmented control: subtle `control_bg` track with the ACTIVE segment
/// rendered as a solid accent pill in `on_accent` text (Clash-Party style —
/// the selected option must read as clearly chosen in light AND dark).
/// Inactive segments sit directly on the track in `text_secondary` and gain
/// a faint hover tint. `selected` indexes into `options`; out-of-range
/// values render nothing active.
pub fn segmented_control<'a, Message: 'a + Clone>(
    options: &[String],
    selected: usize,
    on_change: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let on_change = &on_change;

    let segments: Vec<Element<'a, Message>> = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let is_active = index == selected;
            let label = text(option.clone())
                .size(12)
                .font(if is_active {
                    theme::FONT_SEMIBOLD
                } else {
                    theme::FONT_MEDIUM
                })
                .style(move |t: &Theme| text::Style {
                    color: Some(if is_active {
                        theme::tokens(t).on_accent
                    } else {
                        theme::tokens(t).text_secondary
                    }),
                });

            button(container(label).padding([5, 14]).style(move |t: &Theme| {
                let t = theme::tokens(t);
                container::Style {
                    // Active: solid accent pill. Inactive: transparent
                    // so the control_bg track shows through.
                    background: if is_active {
                        Some(t.accent.into())
                    } else {
                        None
                    },
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        ..Default::default()
                    },
                    shadow: if is_active {
                        Shadow {
                            color: Color {
                                a: 0.18,
                                ..t.accent
                            },
                            offset: Vector::new(0.0, 1.0),
                            blur_radius: 3.0,
                        }
                    } else {
                        Shadow::default()
                    },
                    text_color: None,
                    snap: false,
                }
            }))
            .padding(0)
            .style(move |t: &Theme, status| {
                // Cheap hover feedback for inactive segments only — the
                // active pill is already fully saturated.
                let mut style = button::Style {
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                if !is_active && let button::Status::Hovered | button::Status::Pressed = status {
                    style.background = Some(theme::tokens(t).chip_bg.into());
                }
                style
            })
            .on_press(on_change(index))
            .into()
        })
        .collect();

    container(row(segments).spacing(2))
        .padding(2)
        .style(|t: &Theme| {
            let t = theme::tokens(t);
            container::Style {
                background: Some(t.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }
        })
        .into()
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

pub fn nav_button<'a>(label: String, route: Route, current_route: &Route) -> Element<'a, Message> {
    let is_active = route == *current_route;
    let icon = match route {
        Route::Overview => Icon::LayoutGrid,
        Route::Profiles => Icon::FileText,
        Route::Proxies => Icon::Globe,
        Route::Runtime => Icon::Activity,
        Route::Rules => Icon::Shield,
        Route::Dns => Icon::Network,
        Route::Sync => Icon::RefreshCw,
        Route::Settings => Icon::Settings,
        Route::Editor => Icon::Code2,
    };

    let indicator =
        container(Space::new().width(4).height(18)).style(move |t: &Theme| container::Style {
            background: if is_active {
                Some(theme::tokens(t).accent.into())
            } else {
                None
            },
            border: Border {
                radius: border::Radius::from(2.0),
                ..Default::default()
            },
            ..Default::default()
        });

    // Icon tone follows the active route and theme.
    let glyph = svg_icons::icon_themed(icon, 18.0, move |t: &Theme| {
        let tokens = theme::tokens(t);
        if is_active {
            tokens.accent
        } else {
            tokens.sidebar_text_muted
        }
    });

    let label_text = text(label).size(14).font(if is_active {
        theme::FONT_SEMIBOLD
    } else {
        theme::FONT_MEDIUM
    });

    let content = container(
        row![indicator, glyph, label_text]
            .spacing(theme::SP_MD)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding([10, 12]);

    button(content)
        .width(Length::Fill)
        .style(move |t, status| {
            let tokens = theme::tokens(t);
            let mut style = button::Style {
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            };

            if is_active {
                style.background = Some(tokens.accent_soft.into());
                style.text_color = tokens.accent;
            } else {
                style.background = match status {
                    button::Status::Hovered | button::Status::Pressed => {
                        Some(tokens.control_bg.into())
                    }
                    _ => None,
                };
                style.text_color = match status {
                    button::Status::Hovered | button::Status::Pressed => tokens.text_primary,
                    _ => tokens.sidebar_text_muted,
                };
            }
            style
        })
        .on_press(Message::Navigate(route))
        .into()
}

// ---------------------------------------------------------------------------
// Traffic chart
// ---------------------------------------------------------------------------

/// Canvas-drawn up/down traffic history.
///
/// ui-fix: deliberately paints NO background and NO border of its own —
/// the wrapping card (or the page surface) provides it, so the chart blends
/// seamlessly in both appearances. Do not wrap this in an extra tinted
/// container (e.g. `control_bg`): that is what caused the visible gray
/// frame/seam inside the runtime traffic card.
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
        let tokens = theme::tokens(_theme);
        let accent = tokens.accent;
        let success = tokens.success;

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
        frame.fill(&down_path, Color { a: 0.10, ..accent });
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
            canvas::Stroke::default().with_color(accent).with_width(2.5),
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
                .with_color(success)
                .with_width(2.0),
        );
        vec![frame.into_geometry()]
    }
}

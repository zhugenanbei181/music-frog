//! Shared, design-token-driven widgets for the Infiltrator shell.
//!
//! Every color comes from [`crate::view::theme::tokens`] so light and dark
//! are both first-class. Page views should compose these primitives instead
//! of hand-rolling containers with hardcoded colors.

use crate::types::app::Route;
use crate::types::message::Message;
use iced::widget::{Space, button, canvas, column, container, row, scrollable, text, text_input};
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
        top: 0.0, right: SCROLLBAR_GUTTER, bottom: 0.0, left: 0.0,
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
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border { radius: border::Radius::from(theme::R_CARD), width: 1.0, color: tk.card_border },
        shadow: tk.card_shadow,
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
            let header = text(title).size(14).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_primary),
            });
            column![header, content].spacing(theme::SP_MD).width(Length::Fill).into()
        }
        None => content,
    };
    container(body).width(Length::Fill).padding(theme::SP_XXL).style(card_surface).into()
}

/// Accent-tinted highlight card (used for hero/summary panels).
pub fn premium_card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(content).width(Length::Fill).padding(30).style(|t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.accent_soft.into()),
            border: Border { radius: border::Radius::from(theme::R_CARD), width: 1.0, color: Color { a: 0.25, ..tk.accent } },
            shadow: tk.card_shadow,
            ..Default::default()
        }
    }).into()
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
        .width(40).height(40).align_x(iced::Alignment::Center).align_y(iced::Alignment::Center)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Color { a: 0.14, ..accent }.into()),
            border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
            ..Default::default()
        });

    let texts = column![
        text(label.to_string()).size(11).font(theme::FONT_MEDIUM).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }),
        text(value.to_string()).size(18).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_primary),
        }),
    ].spacing(2);

    let content = row![icon_chip, texts].spacing(theme::SP_MD).align_y(iced::Alignment::Center);

    container(content).width(Length::Fill).padding(theme::SP_LG).style(move |t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: if selected { 1.5 } else { 1.0 },
                color: if selected { accent } else { tk.card_border },
            },
            shadow: tk.card_shadow,
            ..Default::default()
        }
    }).into()
}

/// Section title row with optional trailing content (buttons, badges, ...).
pub fn section_header<'a, Message: 'a>(
    title: &str,
    trailing: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title = text(title.to_string()).size(13).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
        color: Some(theme::tokens(t).text_secondary),
    });

    row![title, Space::new().width(Length::Fill), trailing.unwrap_or_else(|| Space::new().width(0).into())]
        .align_y(iced::Alignment::Center).width(Length::Fill).into()
}

/// Centered placeholder for empty lists/views.
pub fn empty_state<'a, Message: 'a>(icon: Icon, title: &str, hint: &str) -> Element<'a, Message> {
    column![
        svg_icons::icon_themed(icon, 36.0, |t: &Theme| theme::tokens(t).text_tertiary),
        text(title.to_string()).size(14).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }),
        text(hint.to_string()).size(12).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_tertiary),
        }),
    ].spacing(theme::SP_SM).align_x(iced::Alignment::Center).width(Length::Fill).padding(theme::SP_XXL).into()
}

/// Placeholder container with rounded corners and control background for loading states.
pub fn skeleton_box<'a, Message: 'a>(
    width: impl Into<Length>,
    height: impl Into<Length>,
) -> Element<'a, Message> {
    container(Space::new().width(width).height(height)).style(|t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
            ..Default::default()
        }
    }).into()
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
            BadgeKind::Accent => t.badge_accent,
            BadgeKind::Success => t.success,
            BadgeKind::Warning => t.warning,
            BadgeKind::Danger => t.danger,
            BadgeKind::Neutral => t.text_secondary,
        }
    }
}

/// Small tinted pill for statuses ("ACTIVE", "ERROR", counts, ...).
pub fn badge<'a, Message: 'a>(label: impl Into<String>, kind: BadgeKind) -> Element<'a, Message> {
    container(text(label.into()).size(11).font(theme::FONT_SEMIBOLD)).padding([3, 8]).style(move |t: &Theme| {
        let color = kind.color(theme::tokens(t));
        container::Style {
            background: Some(Color { a: 0.14, ..color }.into()),
            border: Border { radius: border::Radius::from(theme::R_CHIP), ..Default::default() },
            text_color: Some(color),
            ..Default::default()
        }
    }).into()
}

/// Neutral pill for protocol/type tags ("Shadowsocks", "VMess", ...).
pub fn chip<'a, Message: 'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(text(label.into()).size(11).font(theme::FONT_MEDIUM)).padding([3, 10]).style(|t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.chip_bg.into()),
            border: Border { radius: border::Radius::from(theme::R_CHIP), ..Default::default() },
            text_color: Some(tk.text_secondary),
            ..Default::default()
        }
    }).into()
}

/// Keyboard keycap pill (e.g. "Ctrl", "Cmd", "K", "Esc") with subtle border,
/// 10px monospace font, and background tint.
pub fn kbd_badge<'a, Message: 'a>(key: impl Into<String>) -> Element<'a, Message> {
    container(text(key.into()).size(10).font(theme::MONO).style(|t: &Theme| text::Style {
        color: Some(theme::tokens(t).text_secondary),
    })).padding([2, 6]).style(|t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border { radius: border::Radius::from(4.0), width: 1.0, color: tk.card_border },
            ..Default::default()
        }
    }).into()
}

/// Colored latency numeral: green <=200 ms, orange <=500 ms, red above,
/// gray/em-dash when untested. Rendered with the bundled JetBrains Mono so
/// live updates do not jitter.
pub fn latency_badge<'a, Message: 'a>(ms: Option<u32>) -> Element<'a, Message> {
    let label = match ms {
        Some(ms) => format!("{ms} ms"),
        None => "—".to_string(),
    };
    text(label).size(12).font(theme::MONO).style(move |t: &Theme| text::Style {
        color: Some(theme::latency_color(theme::tokens(t), ms)),
    }).into()
}

/// Small round status indicator.
pub fn status_dot<'a>(active: bool) -> Element<'a, Message> {
    let color = move |t: &Theme| {
        let tk = theme::tokens(t);
        if active { tk.success } else { tk.danger }
    };
    container(Space::new().width(10).height(10)).style(move |t: &Theme| container::Style {
        background: Some(color(t).into()),
        border: Border { radius: border::Radius::from(5.0), ..Default::default() },
        ..Default::default()
    }).into()
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
    button(glyph).padding(6).style(|t: &Theme, status| {
        let tk = theme::tokens(t);
        button::Style {
            background: match status {
                button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
                _ => None,
            },
            border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
            ..Default::default()
        }
    }).on_press(on_press).into()
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
        let tk = theme::tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let height = 26.0_f32.min(bounds.height);
        let radius = height / 2.0;
        let track_size = Size::new(bounds.width.min(44.0), height);
        let track = if self.value { tk.accent } else { tk.switch_track };
        frame.fill(&canvas::Path::rounded_rectangle(Point::ORIGIN, track_size, border::Radius::from(radius)), track);

        let knob_radius = radius - 2.0;
        let inset = 2.0;
        let knob_x = if self.value { track_size.width - knob_radius - inset } else { knob_radius + inset };
        let knob_y = radius;

        frame.fill(&canvas::Path::circle(Point::new(knob_x, knob_y + 0.5), knob_radius), Color { a: 0.20, ..Color::BLACK });
        frame.fill(&canvas::Path::circle(Point::new(knob_x, knob_y), knob_radius), tk.switch_knob);

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
        .style(|_t: &Theme, _status| button::Style { background: None, ..Default::default() })
        .on_press(on_change(!value))
        .into()
}

/// Segmented control: subtle `control_bg` track with the ACTIVE segment
/// rendered as a solid accent pill in `on_accent` text (Clash-Party style).
pub fn segmented_control<'a, Message: 'a + Clone>(
    options: &[String],
    selected: usize,
    on_change: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    let on_change = &on_change;
    let segments: Vec<Element<'a, Message>> = options.iter().enumerate().map(|(index, option)| {
        let is_active = index == selected;
        let label = text(option.clone()).size(12).font(if is_active { theme::FONT_SEMIBOLD } else { theme::FONT_MEDIUM }).style(move |t: &Theme| text::Style {
            color: Some(if is_active { theme::tokens(t).on_accent } else { theme::tokens(t).text_secondary }),
        });

        button(container(label).padding([5, 14]).style(move |t: &Theme| {
            let tk = theme::tokens(t);
            container::Style {
                background: if is_active { Some(tk.accent.into()) } else { None },
                border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
                shadow: if is_active {
                    Shadow { color: Color { a: 0.18, ..tk.accent }, offset: Vector::new(0.0, 1.0), blur_radius: 3.0 }
                } else {
                    Shadow::default()
                },
                text_color: None,
                snap: false,
            }
        }))
        .padding(0)
        .style(move |t: &Theme, status| {
            let mut style = button::Style {
                border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
                ..Default::default()
            };
            if !is_active && matches!(status, button::Status::Hovered | button::Status::Pressed) {
                style.background = Some(theme::tokens(t).chip_bg.into());
            }
            style
        })
        .on_press(on_change(index))
        .into()
    }).collect();

    container(row(segments).spacing(2)).padding(2).style(|t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
            ..Default::default()
        }
    }).into()
}

/// Search input field with leading magnifying glass icon and trailing clear button.
pub fn search_input<'a, Message: 'a + Clone>(
    placeholder: &str,
    value: &str,
    on_input: impl Fn(String) -> Message + 'a,
    on_clear: Message,
) -> Element<'a, Message> {
    let icon = svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| theme::tokens(t).text_tertiary);
    let input = text_input(placeholder, value).on_input(on_input).padding([7, 10]).size(13).width(Length::Fill).style(form_input_style);

    let mut items = vec![icon, input.into()];
    if !value.is_empty() {
        items.push(icon_button(Icon::X, 12.0, on_clear));
    }

    row(items).spacing(theme::SP_SM).align_y(iced::Alignment::Center).into()
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
    let item_rows: Vec<Element<'a, Message>> = items.iter().enumerate().map(|(idx, item)| {
        let label = text(item.clone()).size(12).font(theme::MONO).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_primary),
        });
        let delete_btn = icon_button(Icon::Trash2, 13.0, on_remove(idx));
        let item_content = row![label, Space::new().width(Length::Fill), delete_btn]
            .align_y(iced::Alignment::Center).spacing(theme::SP_SM);

        container(item_content).width(Length::Fill).padding([6, 10]).style(row_card_surface).into()
    }).collect();

    let input_widget = text_input(placeholder, draft)
        .on_input(on_input).on_submit(on_add.clone()).padding([7, 10]).size(13).width(Length::Fill).style(form_input_style);
    let add_btn = icon_button(Icon::Plus, 13.0, on_add);
    let input_row = row![input_widget, add_btn].spacing(theme::SP_SM).align_y(iced::Alignment::Center);

    let mut col = column(item_rows).spacing(theme::SP_SM).width(Length::Fill);
    col = col.push(input_row);
    col.into()
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

    let indicator = container(Space::new().width(4).height(18)).style(move |t: &Theme| {
        container::Style {
            background: if is_active { Some(theme::tokens(t).accent.into()) } else { None },
            border: Border { radius: border::Radius::from(2.0), ..Default::default() },
            ..Default::default()
        }
    });

    let glyph = svg_icons::icon_themed(icon, 18.0, move |t: &Theme| {
        let tk = theme::tokens(t);
        if is_active { tk.accent } else { tk.sidebar_text_muted }
    });

    let label_text = text(label).size(14).font(if is_active { theme::FONT_SEMIBOLD } else { theme::FONT_MEDIUM });
    let content = container(row![indicator, glyph, label_text].spacing(theme::SP_MD).align_y(iced::Alignment::Center))
        .width(Length::Fill).padding([10, 12]);

    button(content).width(Length::Fill).style(move |t, status| {
        let tk = theme::tokens(t);
        let mut style = button::Style {
            border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
            ..Default::default()
        };
        if is_active {
            style.background = Some(tk.accent_soft.into());
            style.text_color = tk.accent;
        } else {
            style.background = match status {
                button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
                _ => None,
            };
            style.text_color = match status {
                button::Status::Hovered | button::Status::Pressed => tk.text_primary,
                _ => tk.sidebar_text_muted,
            };
        }
        style
    }).on_press(Message::Navigate(route)).into()
}

// ---------------------------------------------------------------------------
// Traffic chart & Waveforms
// ---------------------------------------------------------------------------

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
        let tk = theme::tokens(_theme);
        let accent = tk.accent;
        let success = tk.success;

        let mut frame = canvas::Frame::new(_renderer, bounds.size());
        if self.history.len() < 2 {
            return vec![frame.into_geometry()];
        }
        let (width, height) = (bounds.width, bounds.height);
        let max_points = 60;
        let x_step = width / (max_points - 1) as f32;
        let mut max_speed = self.history.iter().map(|(u, d)| std::cmp::max(*u, *d)).max().unwrap_or(1024 * 100);
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
                if i == 0 { p.move_to(pt); } else { p.line_to(pt); }
            }
        });
        frame.stroke(&down_line, canvas::Stroke::default().with_color(accent).with_width(2.5));
        let up_line = canvas::Path::new(|p| {
            for (i, (up, _)) in self.history.iter().enumerate() {
                let pt = Point::new(i as f32 * x_step, scale(*up));
                if i == 0 { p.move_to(pt); } else { p.line_to(pt); }
            }
        });
        frame.stroke(&up_line, canvas::Stroke::default().with_color(success).with_width(2.0));
        vec![frame.into_geometry()]
    }
}

/// Compact 60x24 Canvas sparkline for sidebar speed or KPI card traffic preview.
pub struct MiniWaveform {
    pub samples: Vec<u64>,
}

impl<Message> canvas::Program<Message> for MiniWaveform {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tk = theme::tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let (width, height) = (bounds.width, bounds.height);

        if self.samples.is_empty() {
            let mid_y = height / 2.0;
            let baseline = canvas::Path::line(Point::new(0.0, mid_y), Point::new(width, mid_y));
            frame.stroke(&baseline, canvas::Stroke::default().with_color(Color { a: 0.25, ..tk.text_tertiary }).with_width(1.0));
            return vec![frame.into_geometry()];
        }

        if self.samples.len() == 1 {
            let mid_y = height / 2.0;
            let baseline = canvas::Path::line(Point::new(0.0, mid_y), Point::new(width, mid_y));
            frame.stroke(&baseline, canvas::Stroke::default().with_color(tk.accent).with_width(1.5));
            return vec![frame.into_geometry()];
        }

        let max_val = *self.samples.iter().max().unwrap_or(&1).max(&1);
        let pad_y = 2.0_f32;
        let usable_h = (height - pad_y * 2.0).max(1.0);
        let step = width / (self.samples.len() - 1) as f32;
        let scale_y = |val: u64| -> f32 {
            let ratio = (val as f32 / max_val as f32).clamp(0.0, 1.0);
            height - pad_y - (ratio * usable_h)
        };

        let area_path = canvas::Path::new(|p| {
            p.move_to(Point::new(0.0, height));
            for (i, &val) in self.samples.iter().enumerate() {
                p.line_to(Point::new(i as f32 * step, scale_y(val)));
            }
            p.line_to(Point::new(width, height));
            p.close();
        });
        frame.fill(&area_path, Color { a: 0.12, ..tk.accent });

        let line_path = canvas::Path::new(|p| {
            for (i, &val) in self.samples.iter().enumerate() {
                let pt = Point::new(i as f32 * step, scale_y(val));
                if i == 0 { p.move_to(pt); } else { p.line_to(pt); }
            }
        });
        frame.stroke(&line_path, canvas::Stroke::default().with_color(tk.accent).with_width(1.5));

        vec![frame.into_geometry()]
    }
}

/// Compact 60x24 Canvas sparkline for sidebar speed or KPI card traffic preview.
pub fn mini_waveform<'a, Message: 'a>(samples: &[u64]) -> Element<'a, Message> {
    canvas::Canvas::new(MiniWaveform {
        samples: samples.to_vec(),
    })
    .width(60)
    .height(24)
    .into()
}

// ---------------------------------------------------------------------------
// Standard Token-Driven Button Styles & Helpers
// ---------------------------------------------------------------------------

pub fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = theme::tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => (Color { a: 0.85, ..tk.accent }, tk.on_accent),
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
        text_color: fg,
        ..Default::default()
    }
}

pub fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = theme::tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: tk.card_border },
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
        button::Status::Hovered | button::Status::Pressed => (Color { a: 0.24, ..tk.danger }, tk.on_accent),
        _ => (Color { a: 0.14, ..tk.danger }, tk.danger),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), ..Default::default() },
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
        iced::widget::text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    iced::widget::text_input::Style {
        background: tk.control_bg.into(),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: border_width, color: border_color },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color { a: 0.25, ..tk.accent },
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
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: tk.card_border },
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

    let mut text_col = column![text(title_str).size(13).font(theme::FONT_SEMIBOLD).style(|t: &Theme| text::Style {
        color: Some(theme::tokens(t).text_primary),
    })];

    if !detail_str.is_empty() {
        text_col = text_col.push(text(detail_str).size(12).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }));
    }
    text_col = text_col.spacing(2);

    let mut banner_row = row![status_icon, text_col].spacing(theme::SP_MD).align_y(iced::Alignment::Center);

    if let Some(action_elem) = action {
        banner_row = banner_row.push(Space::new().width(Length::Fill)).push(action_elem);
    }

    container(banner_row)
        .width(Length::Fill)
        .padding([10, 14])
        .style(move |t: &Theme| {
            let color = kind.color(theme::tokens(t));
            container::Style {
                background: Some(Color { a: 0.10, ..color }.into()),
                border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: Color { a: 0.25, ..color } },
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
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: tk.card_border },
        ..Default::default()
    }
}

/// Bordered row-card surface for items inside section lists.
pub fn row_card_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border { radius: border::Radius::from(theme::R_CONTROL), width: 1.0, color: tk.card_border },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone)]
    #[allow(dead_code)]
    enum TestMsg {
        Search(String),
        Clear,
        Action,
        Add,
        Remove(usize),
        Input(String),
    }

    #[test]
    fn test_search_input_widget() {
        let _elem_empty: Element<'_, TestMsg> =
            search_input("Search...", "", TestMsg::Search, TestMsg::Clear);
        let _elem_filled: Element<'_, TestMsg> =
            search_input("Search...", "query", TestMsg::Search, TestMsg::Clear);
    }

    #[test]
    fn test_banner_alert_widget() {
        let _alert_accent: Element<'_, TestMsg> =
            banner_alert(BadgeKind::Accent, "Notice", "Details here", None);
        let _alert_with_action: Element<'_, TestMsg> = banner_alert(
            BadgeKind::Danger,
            "Error",
            "Something failed",
            Some(text_btn("Retry", style_ghost, Some(TestMsg::Action))),
        );
    }

    #[test]
    fn test_kbd_badge_widget() {
        let _ctrl: Element<'_, TestMsg> = kbd_badge("Ctrl");
        let _k: Element<'_, TestMsg> = kbd_badge("K");
    }

    #[test]
    fn test_skeleton_box_widget() {
        let _fixed: Element<'_, TestMsg> = skeleton_box(100.0, 24.0);
        let _fill: Element<'_, TestMsg> = skeleton_box(Length::Fill, 16.0);
    }

    #[test]
    fn test_dynamic_list_editor_widget() {
        let items = vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()];
        let _elem: Element<'_, TestMsg> = dynamic_list_editor(
            &items,
            "1.0.0.1",
            "Enter IP...",
            TestMsg::Input,
            TestMsg::Add,
            TestMsg::Remove,
        );
        let empty_items: Vec<String> = vec![];
        let _elem_empty: Element<'_, TestMsg> = dynamic_list_editor(
            &empty_items,
            "",
            "Enter IP...",
            TestMsg::Input,
            TestMsg::Add,
            TestMsg::Remove,
        );
    }

    #[test]
    fn test_mini_waveform_widget() {
        let empty: &[u64] = &[];
        let _elem_empty: Element<'_, TestMsg> = mini_waveform(empty);
        let single = [1000u64];
        let _elem_single: Element<'_, TestMsg> = mini_waveform(&single);
        let samples = [100u64, 450, 800, 300, 950, 1200];
        let _elem_multi: Element<'_, TestMsg> = mini_waveform(&samples);
        let zeros = [0u64, 0, 0];
        let _elem_zeros: Element<'_, TestMsg> = mini_waveform(&zeros);
    }
}

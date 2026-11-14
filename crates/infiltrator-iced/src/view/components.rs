//! Shared, design-token-driven widgets for the Infiltrator shell.
//!
//! Every color comes from [`crate::view::theme::tokens`] so light and dark
//! are both first-class. Page views should compose these primitives instead
//! of hand-rolling containers with hardcoded colors.

use crate::sidebar_chrome::{
    NAV_CONTENT_OFFSET_PX, NAV_INDICATOR_HEIGHT_PX, NAV_INDICATOR_WIDTH_PX,
};
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::svg_icons::Icon;
use crate::view::theme::{Elevation, Tokens, skins};
use crate::view::{scrollbar, svg_icons, theme};
use canvas::gradient::Linear;
use iced::advanced::text::Renderer;
use iced::widget::{
    Container, Scrollable, Space, Stack, button, canvas, column, container, row, text, toggler,
    tooltip,
};
use iced::{
    Border, Color, Element, Length, Point, Rectangle, Shadow, Theme, Vector, border, mouse,
};
use infiltrator_contract::design_resources::TextRole;
use infiltrator_contract::scrollbar::ScrollbarMetrics;
use zhuge_ui_kit_iced::surface::{self, BadgeGeometry, SurfaceGeometry, rgba, surface_style};
use zhuge_ui_tokens::surface::SurfaceColors;

// ---------------------------------------------------------------------------
// Legacy palette constants (pre-token pages still import these).
// New code must take colors from `theme::tokens` instead.
// ---------------------------------------------------------------------------
pub const WEB_ACCENT: Color = skins::LIGHT.accent;
pub const WEB_SUCCESS: Color = skins::LIGHT.success;
pub const WEB_SURFACE: Color = Color::from_rgba(0.06, 0.09, 0.16, 0.82);
pub const SCROLLBAR_GUTTER: f32 = ScrollbarMetrics::DEFAULT.content_gutter_px;

// ---------------------------------------------------------------------------
// Scrollable
// ---------------------------------------------------------------------------

pub fn modern_scrollable<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Scrollable<'a, T> {
    scrollbar::modern_scrollable(content)
}

// ---------------------------------------------------------------------------
// Surfaces
// ---------------------------------------------------------------------------

/// Standard card surface: card background, 1px hairline border, radius 16
/// and the soft card shadow. Reusable by any container-based widget.
pub fn card_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    surface_style(
        SurfaceColors {
            background: rgba(tk.card_bg),
            border: rgba(tk.card_border),
            foreground: None,
        },
        SurfaceGeometry {
            radius: border::Radius::from(theme::R_CARD),
            border_width: theme::HAIRLINE,
            shadow: tk.card_shadow,
        },
    )
}

/// A page-body frame: the card IS the page body.
///
/// The density standard's framed shape is one card that reaches the page body's
/// bottom edge, with its list scrolling inside. Slint has had that owner for a
/// while (`CardHeight.page-body`); Iced painted its rows straight onto the page
/// ground, so whatever the rows did not cover stayed page-coloured tail air -
/// 40.8% of the body on `rules-search-highlight-standard` against a 4% limit,
/// while the same business on Slint measured 0.0% tail. A page mounts its body
/// through this frame and lets the content inside take the leftover, so the
/// frame's own ground reaches the body's last pixel.
pub fn page_body_card<'a, T: 'a>(content: impl Into<Element<'a, T>>) -> Container<'a, T> {
    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(theme::SP_MD)
        .style(card_surface)
}

/// Card surface with hover lift state: when hovered, slightly elevates background
/// brightness (+3%) and upgrades shadow from low to floating elevation.
pub fn hover_lift_card_surface(t: &Theme, hovered: bool) -> container::Style {
    let tk = theme::tokens(t);
    if hovered {
        let base_bg = tk.card_bg;
        let lifted_bg = Color {
            r: (base_bg.r + 0.03).min(1.0),
            g: (base_bg.g + 0.03).min(1.0),
            b: (base_bg.b + 0.03).min(1.0),
            a: base_bg.a,
        };
        container::Style {
            background: Some(lifted_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: theme::HAIRLINE,
                color: Color {
                    a: (tk.card_border.a + 0.15).min(1.0),
                    ..tk.card_border
                },
            },
            shadow: tk.floating_shadow,
            ..Default::default()
        }
    } else {
        card_surface(t)
    }
}

/// Outer ambient shadow surface for dual-layer physical elevation rendering.
pub fn card_ambient_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CARD),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: tk.elevation_shadow(Elevation::Card).ambient,
        ..Default::default()
    }
}

/// Inner key-light surface for dual-layer physical elevation rendering.
pub fn card_key_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CARD),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        shadow: tk.elevation_shadow(Elevation::Card).key,
        ..Default::default()
    }
}

/// Outer ambient shadow surface for floating elements.
pub fn floating_ambient_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CARD),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        shadow: tk.elevation_shadow(Elevation::Floating).ambient,
        ..Default::default()
    }
}

/// Inner key-light surface for floating elements.
pub fn floating_key_surface(t: &Theme) -> container::Style {
    let tk = theme::tokens(t);
    container::Style {
        background: Some(tk.card_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CARD),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        shadow: tk.elevation_shadow(Elevation::Floating).key,
        ..Default::default()
    }
}

/// Explicit scissor-clipped card container wrapper.
pub fn clipped_card<'a, Message: 'a>(
    title: Option<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    card(title, content)
}

/// Dual-shadow composite card combining directional Key Light contact shadow
/// and diffuse Ambient Light shadow with hardware scissor clipping.
pub fn dual_shadow_card<'a, Message: 'a>(
    title: Option<String>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let content = content.into();
    let body = match title {
        Some(title) => {
            let header = theme::role_body_text(title).style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_primary),
            });
            column![header, content]
                .spacing(theme::SP_MD)
                .width(Length::Fill)
                .into()
        }
        None => content,
    };
    let inner = container(body)
        .width(Length::Fill)
        .padding(theme::SP_XXL)
        .style(card_key_surface)
        .clip(true);

    container(inner)
        .width(Length::Fill)
        .style(card_ambient_surface)
        .into()
}

/// Floating dual-shadow composite card for high-elevation floating modals and popups.
pub fn floating_dual_shadow_card<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let inner = container(content.into())
        .width(Length::Fill)
        .padding(theme::SP_XXL)
        .style(floating_key_surface)
        .clip(true);

    container(inner)
        .width(Length::Fill)
        .style(floating_ambient_surface)
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
            let tk = theme::tokens(t);
            container::Style {
                background: Some(tk.accent_soft.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CARD),
                    width: theme::HAIRLINE,
                    color: Color {
                        a: 0.25,
                        ..tk.accent
                    },
                },
                shadow: tk.card_shadow,
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
        theme::role_body_text(label.to_string()).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }),
        theme::role_body_text(value.to_string()).style(|t: &Theme| text::Style {
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
        })
        .into()
}

/// Section title row with optional trailing content (buttons, badges, ...).
pub fn section_header<'a, Message: 'a>(
    title: &str,
    trailing: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let title = theme::role_body_text(title.to_string()).style(|t: &Theme| text::Style {
        color: Some(theme::tokens(t).text_secondary),
    });

    row![
        title,
        Space::new().width(Length::Fill),
        trailing.unwrap_or_else(|| Space::new().width(0).into())
    ]
    .align_y(iced::Alignment::Center)
    .width(Length::Fill)
    .into()
}

/// Centered placeholder for empty lists/views.
pub fn empty_state<'a, Message: 'a>(icon: Icon, title: &str, hint: &str) -> Element<'a, Message> {
    column![
        svg_icons::icon_themed(icon, 36.0, |t: &Theme| theme::tokens(t).text_tertiary),
        theme::role_body_text(title.to_string()).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }),
        theme::role_body_text(hint.to_string()).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_tertiary),
        }),
    ]
    .spacing(theme::SP_SM)
    .align_x(iced::Alignment::Center)
    .width(Length::Fill)
    .padding(theme::SP_XXL)
    .into()
}

/// Canvas-based shimmer wave skeleton widget providing a 45-degree diagonal
/// moving specular light gradient across rounded boxes during data loading.
#[derive(Debug, Clone, Copy)]
pub struct ShimmerSkeleton {
    pub radius: f32,
    pub phase: f32,
}

impl ShimmerSkeleton {
    pub fn new(radius: f32, phase: f32) -> Self {
        Self { radius, phase }
    }
}

impl<Message> canvas::Program<Message> for ShimmerSkeleton {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tk = theme::tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let rect = canvas::Path::rounded_rectangle(
            Point::ORIGIN,
            bounds.size(),
            border::Radius::from(self.radius),
        );

        // 1. Base control background
        frame.fill(&rect, tk.control_bg);

        // 2. 45-degree diagonal shimmer gradient wave
        let w = bounds.width;
        let h = bounds.height;
        let span = w + h;
        if span > 0.0 {
            let center = self.phase.fract() * span;
            let band_width = 80.0_f32.min(span * 0.5);

            let start = Point::new(center - band_width, 0.0);
            let end = Point::new(center + band_width, h.min(band_width));

            let highlight = Color {
                a: 0.16,
                ..Color::WHITE
            };
            let transparent = Color {
                a: 0.0,
                ..Color::WHITE
            };

            let gradient = Linear::new(start, end)
                .add_stop(0.0, transparent)
                .add_stop(0.5, highlight)
                .add_stop(1.0, transparent);

            frame.fill(&rect, gradient);
        }

        vec![frame.into_geometry()]
    }
}

/// Shimmer skeleton box with custom radius and animated phase.
pub fn shimmer_box<'a, Message: 'a>(
    width: impl Into<Length>,
    height: impl Into<Length>,
    phase: f32,
) -> Element<'a, Message> {
    canvas(ShimmerSkeleton::new(theme::R_CONTROL, phase))
        .width(width)
        .height(height)
        .into()
}

/// Dynamic shimmer wave skeleton screen box providing fluid gradient loading aesthetics.
pub fn skeleton_box<'a, Message: 'a>(
    width: impl Into<Length>,
    height: impl Into<Length>,
) -> Element<'a, Message> {
    shimmer_box(width, height, 0.5)
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
    pub(crate) fn color(self, t: &Tokens) -> Color {
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
    surface::badge(
        label,
        BadgeGeometry {
            size: 11.0,
            font: Some(theme::font_for(TextRole::LabelStrong)),
            padding: [3.0, 8.0].into(),
            height: Length::Shrink,
            centered: false,
        },
        move |t: &Theme| {
            let color = kind.color(theme::tokens(t));
            surface_style(
                SurfaceColors {
                    background: rgba(Color { a: 0.14, ..color }),
                    border: rgba(Color::TRANSPARENT),
                    foreground: Some(rgba(color)),
                },
                SurfaceGeometry {
                    radius: border::Radius::from(theme::R_CHIP),
                    ..Default::default()
                },
            )
        },
    )
}

/// Neutral pill for protocol/type tags ("Shadowsocks", "VMess", ...).
pub fn chip<'a, Message: 'a>(label: impl Into<String>) -> Element<'a, Message> {
    container(theme::role_body_text(label.into()))
        .padding([3, 10])
        .style(|t: &Theme| {
            let tk = theme::tokens(t);
            container::Style {
                background: Some(tk.chip_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    ..Default::default()
                },
                text_color: Some(tk.text_secondary),
                ..Default::default()
            }
        })
        .into()
}

/// Keyboard keycap pill (e.g. "Ctrl", "Cmd", "K", "Esc") with subtle border,
/// 10px monospace font, and background tint.
pub fn kbd_badge<'a, Message: 'a>(key: impl Into<String>) -> Element<'a, Message> {
    container(
        theme::role_body_text(key.into()).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_secondary),
        }),
    )
    .padding([2, 6])
    .style(|t: &Theme| {
        let tk = theme::tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_XS),
                width: theme::HAIRLINE,
                color: tk.card_border,
            },
            ..Default::default()
        }
    })
    .into()
}

/// Small round status indicator.
pub fn status_dot<'a>(active: bool) -> Element<'a, Message> {
    let color = move |t: &Theme| {
        let tk = theme::tokens(t);
        if active { tk.success } else { tk.danger }
    };
    container(Space::new().width(10).height(10))
        .style(move |t: &Theme| container::Style {
            background: Some(color(t).into()),
            border: Border {
                radius: border::Radius::from(theme::R_PILL),
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
    icon_button_maybe(icon, size, Some(on_press))
}

/// The same ghost icon button, but it may carry no action at all.
///
/// A control the owner has not authorized is drawn disabled instead of
/// swallowing the click: a button that always accepts the press and then
/// does nothing tells the user nothing about why the action is unavailable.
pub fn icon_button_maybe<'a, Message: 'a + Clone>(
    icon: Icon,
    size: f32,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let glyph = svg_icons::icon_themed(icon, size, |t: &Theme| theme::tokens(t).text_secondary);
    let control = button(glyph).padding(6).style(|t: &Theme, status| {
        let tk = theme::tokens(t);
        button::Style {
            background: match status {
                button::Status::Hovered => Some(tk.hover.into()),
                button::Status::Pressed => Some(tk.pressed.into()),
                _ => None,
            },
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                ..Default::default()
            },
            ..Default::default()
        }
    });
    match on_press {
        Some(message) => control.on_press(message).into(),
        None => control.on_press_maybe(None::<Message>).into(),
    }
}

/// The switch geometry.
///
/// The toolkit's toggle switch is a 2:1 pill, so the product's 44 px wide
/// switch keeps its width and takes the toolkit's height from it.
const SWITCH_SIZE_PX: f32 = 22.0;

/// The knob inset as a ratio of the switch height. The toolkit rounds it to
/// whole pixels, so this keeps the knob two pixels inside the track.
const SWITCH_PADDING_RATIO: f32 = 0.09;

/// The product's skin for the toolkit's toggle switch.
///
/// The toolkit owns the switch geometry and the interaction; the product only
/// owns the colours, so the track, the knob and the pill radius live here.
fn switch_style(t: &Theme, status: toggler::Status) -> toggler::Style {
    let tk = theme::tokens(t);
    let is_toggled = match status {
        toggler::Status::Active { is_toggled }
        | toggler::Status::Hovered { is_toggled }
        | toggler::Status::Disabled { is_toggled } => is_toggled,
    };
    toggler::Style {
        background: if is_toggled {
            tk.accent.into()
        } else {
            tk.switch_track.into()
        },
        foreground: tk.switch_knob.into(),
        foreground_border_width: 0.0,
        foreground_border_color: Color::TRANSPARENT,
        background_border_width: 0.0,
        background_border_color: Color::TRANSPARENT,
        text_color: None,
        border_radius: Some(border::Radius::from(SWITCH_SIZE_PX / 2.0)),
        padding_ratio: SWITCH_PADDING_RATIO,
    }
}

/// The switch control: the toolkit's own toggle switch.
///
/// The behaviour belongs to [`iced::widget::toggler`]: a press publishes the
/// toggled value, and an action the surface refuses leaves the control inert.
/// The product keeps only the skin.
pub fn toggle_switch<'a, Message, R>(
    value: bool,
    on_change: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message, Theme, R>
where
    Message: 'a + Clone,
    R: Renderer + 'a,
{
    toggle_switch_with_actions(value, move |value| Some(on_change(value)))
}

pub fn toggle_switch_with_actions<'a, Message, R>(
    value: bool,
    on_change: impl Fn(bool) -> Option<Message> + 'a,
) -> Element<'a, Message, Theme, R>
where
    Message: 'a + Clone,
    R: Renderer + 'a,
{
    let message = on_change(!value);
    toggler(value)
        .size(SWITCH_SIZE_PX)
        .style(switch_style)
        .on_toggle_maybe(message.map(|message| move |_| message.clone()))
        .into()
}

/// Segmented control: subtle `control_bg` track with the ACTIVE segment
/// rendered as a solid accent pill in `on_accent` text (Clash-Party style).
pub fn segmented_control<'a, Message: 'a + Clone>(
    options: &[String],
    selected: usize,
    on_change: impl Fn(usize) -> Message + 'a,
) -> Element<'a, Message> {
    segmented_control_with_actions(options, selected, move |index| Some(on_change(index)))
}

pub fn segmented_control_with_actions<'a, Message: 'a + Clone>(
    options: &[String],
    selected: usize,
    on_change: impl Fn(usize) -> Option<Message> + 'a,
) -> Element<'a, Message> {
    let on_change = &on_change;
    let segments: Vec<Element<'a, Message>> = options
        .iter()
        .enumerate()
        .map(|(index, option)| {
            let is_active = index == selected;
            let label = theme::role_body_text(option.clone())
                .font(if is_active {
                    theme::font_for(TextRole::LabelStrong)
                } else {
                    theme::font_for(TextRole::Label)
                })
                .style(move |t: &Theme| text::Style {
                    color: Some(if is_active {
                        theme::tokens(t).on_accent
                    } else {
                        theme::tokens(t).text_secondary
                    }),
                });

            button(
                container(label)
                    .padding([5.0, theme::SP_XS])
                    .style(move |t: &Theme| {
                        let tk = theme::tokens(t);
                        container::Style {
                            background: if is_active {
                                Some(tk.accent.into())
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
                                        ..tk.accent
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
                    }),
            )
            .padding(0)
            .style(move |t: &Theme, status| {
                let mut style = button::Style {
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        ..Default::default()
                    },
                    ..Default::default()
                };
                if !is_active && matches!(status, button::Status::Hovered | button::Status::Pressed)
                {
                    style.background = Some(theme::tokens(t).hover.into());
                }
                style
            })
            .on_press_maybe(on_change(index))
            .into()
        })
        .collect();

    container(row(segments).spacing(2))
        .padding(2)
        .style(|t: &Theme| {
            let tk = theme::tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
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

/// One labelled navigation row. The glyph is a parameter, never a table: the
/// destination's semantic icon arrives from the shell navigation registry, and
/// a route-to-glyph match here would be a second source for it.
pub fn nav_button<'a>(
    label: String,
    route: Route,
    current_route: &Route,
    icon: Icon,
) -> Element<'a, Message> {
    let is_active = route == *current_route;

    let indicator = container(
        Space::new()
            .width(NAV_INDICATOR_WIDTH_PX)
            .height(NAV_INDICATOR_HEIGHT_PX),
    )
    .style(move |t: &Theme| container::Style {
        background: if is_active {
            Some(theme::tokens(t).accent.into())
        } else {
            None
        },
        border: Border {
            radius: border::Radius::from(theme::R_PILL),
            ..Default::default()
        },
        ..Default::default()
    });

    let glyph = svg_icons::icon_themed(icon, theme::NAV_ICON_SIZE, move |t: &Theme| {
        let tk = theme::tokens(t);
        if is_active {
            tk.accent
        } else {
            tk.sidebar_text_muted
        }
    });

    let label_text = theme::role_body_text(label).font(if is_active {
        theme::font_for(TextRole::LabelStrong)
    } else {
        theme::font_for(TextRole::Label)
    });

    // The reference row, in one geometry: the face is the whole column (the
    // wash spans it, the empty right side is breathing room), the content is
    // inset by exactly the leading the receipt publishes
    // (`sidebar_chrome::NAV_CONTENT_OFFSET_PX`, the shared ladder's default
    // gap) and the trailing inset is the reference's own, so the first glyph
    // box starts where the frame says it starts (`SPACEUTIL-011`). The
    // indicator is painted AT the column's left edge on its own layer: a
    // content-width capsule was the wrong geometry for a navigation rail, and
    // an in-flow indicator would have made the painted leading ambiguous.
    let content = container(
        row![glyph, label_text]
            .spacing(theme::SP_MD)
            .align_y(iced::Alignment::Center),
    )
    .width(Length::Fill)
    .padding(iced::Padding {
        top: theme::NAV_ROW_PADDING,
        right: theme::SP_LG,
        bottom: theme::NAV_ROW_PADDING,
        left: NAV_CONTENT_OFFSET_PX as f32,
    });
    let indicator_layer = container(indicator)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(iced::Alignment::Start)
        .align_y(iced::Alignment::Center);
    let face = Stack::new()
        .push(content)
        .push(indicator_layer)
        .width(Length::Fill);

    button(face)
        .width(Length::Fill)
        .padding(0)
        .style(move |t, status| {
            let tk = theme::tokens(t);
            let mut style = button::Style {
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            };
            if is_active {
                style.background = Some(tk.accent_soft.into());
                style.text_color = tk.accent;
            } else {
                style.background = match status {
                    button::Status::Hovered => Some(tk.hover.into()),
                    button::Status::Pressed => Some(tk.pressed.into()),
                    _ => None,
                };
                style.text_color = match status {
                    button::Status::Hovered | button::Status::Pressed => tk.text_primary,
                    _ => tk.sidebar_text_muted,
                };
            }
            style
        })
        .on_press(Message::Navigate(route))
        .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_components_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/gui/toggle_switch_tests.rs"]
mod toggle_switch_tests;

/// Compact icon button for the responsive rail mode sidebar. The glyph comes
/// from the same registry row as the labelled form, so the two morphologies of
/// one destination can never disagree, and the registry's label travels in the
/// hover tooltip - a rail is navigation by glyph alone, so the word has to live
/// somewhere a pointer can reach it.
pub fn nav_rail_icon<'a>(
    label: String,
    route: Route,
    current_route: &Route,
    icon: Icon,
) -> Element<'a, Message> {
    let is_active = route == *current_route;

    let glyph = svg_icons::icon_themed(icon, theme::NAV_ICON_SIZE, move |t: &Theme| {
        let tk = theme::tokens(t);
        if is_active {
            tk.accent
        } else {
            tk.sidebar_text_muted
        }
    });

    let face = button(glyph)
        .padding([theme::NAV_ROW_PADDING, theme::SP_SM])
        .style(move |t: &Theme, status| {
            let tk = theme::tokens(t);
            // #120: the active row wears the control wash and the highlight
            // hairline; idle rows stay bare.
            let bg = if is_active {
                Some(tk.control_wash.into())
            } else if matches!(status, button::Status::Hovered) {
                Some(tk.control_bg.into())
            } else {
                None
            };
            button::Style {
                background: bg,
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: if is_active { theme::HAIRLINE } else { 0.0 },
                    color: if is_active {
                        tk.highlight_border
                    } else {
                        Color::TRANSPARENT
                    },
                },
                ..Default::default()
            }
        })
        .on_press(Message::Navigate(route));

    tooltip(
        face,
        container(theme::role_body_text(label))
            .padding(6)
            .style(container::rounded_box),
        tooltip::Position::Right,
    )
    .gap(4.0)
    .into()
}

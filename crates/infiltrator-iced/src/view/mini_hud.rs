//! Compact floating Mini Speed HUD widget for desktop immersion.
//!
//! Renders a sleek 260x90 floating dashboard showing live duplex bandwidth,
//! mini sparkline waveforms, active node pill, and one-click quick controls.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{badge, icon_button, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use crate::view::waveform::mini_waveform;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn mini_hud_view<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let (up_speed, down_speed) = state
        .diag
        .traffic
        .as_ref()
        .map(|t| (t.up, t.down))
        .unwrap_or((0, 0));

    let down_samples: Vec<u64> = state.diag.traffic_history.iter().map(|(_, d)| *d).collect();
    let up_samples: Vec<u64> = state.diag.traffic_history.iter().map(|(u, _)| *u).collect();

    // Downstream speed & waveform
    let down_card = row![
        svg_icons::icon_themed(Icon::ArrowDown, 14.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_XS),
        column![
            text(format!("{}/s", format_bytes(down_speed)))
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).accent),
                }),
        ],
        Space::new().width(theme::SP_SM),
        mini_waveform(&down_samples),
    ]
    .align_y(Alignment::Center);

    // Upstream speed & waveform
    let up_card = row![
        svg_icons::icon_themed(Icon::ArrowUp, 14.0, |t: &Theme| tokens(t).success),
        Space::new().width(theme::SP_XS),
        column![
            text(format!("{}/s", format_bytes(up_speed)))
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success),
                }),
        ],
        Space::new().width(theme::SP_SM),
        mini_waveform(&up_samples),
    ]
    .align_y(Alignment::Center);

    let active_node = if !state.runtime.runtime_selected_proxy.is_empty() {
        state.runtime.runtime_selected_proxy.clone()
    } else if let Some(active) = state.profile.profiles.iter().find(|p| p.active) {
        active.name.clone()
    } else {
        "Default".to_string()
    };

    let mode_label = state
        .runtime
        .proxy_mode
        .as_deref()
        .unwrap_or("rule")
        .to_ascii_uppercase();

    // Top title row: logo + active mode + expand button
    let header_row = row![
        svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_XS),
        text(lang.tr("mini_hud_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(theme::SP_SM),
        badge(mode_label, BadgeKind::Neutral),
        Space::new().width(Length::Fill),
        icon_button(
            Icon::Pin,
            12.0,
            Message::SetAlwaysOnTop(!state.shell.always_on_top),
        ),
        Space::new().width(theme::SP_XS),
        icon_button(Icon::ChevronUp, 14.0, Message::ToggleMiniHudMode),
    ]
    .align_y(Alignment::Center);

    // Node pill & quick controls
    let footer_row = row![
        badge(active_node, BadgeKind::Accent),
        Space::new().width(Length::Fill),
        button(
            svg_icons::icon_themed(Icon::Wifi, 12.0, move |t: &Theme| {
                if state.runtime.system_proxy_enabled {
                    tokens(t).accent
                } else {
                    tokens(t).text_tertiary
                }
            })
        )
        .padding([4, 6])
        .style(|t: &Theme, _| {
            let tk = tokens(t);
            button::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    width: 1.0,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .on_press(Message::SetSystemProxy(!state.runtime.system_proxy_enabled)),
        Space::new().width(theme::SP_XS),
        button(
            svg_icons::icon_themed(Icon::Zap, 12.0, move |t: &Theme| {
                if state.runtime.tun_enabled.unwrap_or(false) {
                    tokens(t).success
                } else {
                    tokens(t).text_tertiary
                }
            })
        )
        .padding([4, 6])
        .style(|t: &Theme, _| {
            let tk = tokens(t);
            button::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CHIP),
                    width: 1.0,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        })
        .on_press(Message::SetTunEnabled(!state.runtime.tun_enabled.unwrap_or(false))),
    ]
    .align_y(Alignment::Center);

    let hud_card = container(
        column![
            header_row,
            Space::new().height(theme::SP_XS),
            down_card,
            up_card,
            Space::new().height(theme::SP_XS),
            footer_row,
        ]
        .spacing(theme::SP_XS),
    )
    .padding([10, 14])
    .width(280)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.overlay.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: 1.0,
                color: tk.card_border,
            },
            shadow: tk.floating_shadow,
            text_color: Some(tk.overlay_text),
            ..Default::default()
        }
    });

    container(hud_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

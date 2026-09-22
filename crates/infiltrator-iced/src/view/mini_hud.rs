//! Compact floating Mini Speed HUD widget for desktop immersion.
//!
//! Renders the shared read model (`infiltrator_contract::mini_hud`): live
//! duplex bandwidth, mini sparkline waveforms, the real exit-node pill, the
//! mode chip and the two quick switches whose states come from the shared
//! system-toggle snapshot. Dragging the card moves the HUD window and persists
//! the placement through the shared application facade.

use crate::state::AppState;
use crate::types::message::Message;
use crate::utils::format_bytes;
use crate::view::components::{BadgeKind, badge, icon_button};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use crate::view::waveform::{StripInk, hud_waveform};
use iced::widget::{Space, button, column, container, mouse_area, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleState};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn mini_hud_view<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let model = state.mini_hud_read_model();

    // Downstream speed & the shared waveform strip (same bars the Bevy overlay
    // rasterizes from the one read model).
    let down_card = row![
        svg_icons::icon_themed(Icon::ArrowDown, 14.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_XS),
        column![
            text(format!("{}/s", format_bytes(model.down_bytes_per_sec)))
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).accent),
                }),
        ],
        Space::new().width(theme::SP_SM),
        hud_waveform(&model.waveform.down, StripInk::Accent),
    ]
    .align_y(Alignment::Center);

    // Upstream speed & waveform
    let up_card = row![
        svg_icons::icon_themed(Icon::ArrowUp, 14.0, |t: &Theme| tokens(t).success),
        Space::new().width(theme::SP_XS),
        column![
            text(format!("{}/s", format_bytes(model.up_bytes_per_sec)))
                .size(12)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success),
                }),
        ],
        Space::new().width(theme::SP_SM),
        hud_waveform(&model.waveform.up, StripInk::Success),
    ]
    .align_y(Alignment::Center);

    // Top title row: logo + real mode chip + pin/expand buttons
    let header_title = crate::accessibility::labelled(
        infiltrator_contract::a11y::ShellA11yNode::MiniHudCard,
        &state.shell.lang,
        text(lang.tr("mini_hud_title").to_string())
            .size(11)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            })
            .into(),
    );
    let mut header_row = row![
        svg_icons::icon_themed(Icon::Activity, 14.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_XS),
        header_title,
    ]
    .align_y(Alignment::Center);
    if !model.mode_zh.is_empty() {
        header_row = header_row
            .push(Space::new().width(theme::SP_SM))
            .push(badge(model.mode_zh.clone(), BadgeKind::Neutral));
    }
    header_row = header_row
        .push(Space::new().width(Length::Fill))
        .push(icon_button(
            Icon::Pin,
            12.0,
            Message::SetAlwaysOnTop(!model.placement.pinned),
        ))
        .push(Space::new().width(theme::SP_XS))
        .push(icon_button(
            Icon::ChevronUp,
            14.0,
            Message::ToggleMiniHudMode,
        ));

    // Node pill & quick controls, all state from the shared read model.
    let proxy_state = model.system_proxy.clone();
    let proxy_desired = model.next_value(SystemToggle::SystemProxy);
    let tun_state = model.tun.clone();
    let tun_desired = model.next_value(SystemToggle::Tun);
    let proxy_tint = {
        let state = proxy_state.clone();
        move |t: &Theme| proxy_color(&state, t)
    };
    let tun_tint = {
        let state = tun_state.clone();
        move |t: &Theme| proxy_color(&state, t)
    };

    let mut footer_row = row![].align_y(Alignment::Center);
    if !model.exit_node.is_empty() {
        footer_row = footer_row.push(badge(model.exit_node.clone(), BadgeKind::Accent));
    }
    footer_row = footer_row.push(Space::new().width(Length::Fill));

    let proxy_button = button(svg_icons::icon_themed(Icon::Wifi, 12.0, proxy_tint))
        .padding([4, 6])
        .style(control_style);
    let proxy_button = match proxy_desired {
        Some(next) => proxy_button.on_press(Message::SetSystemProxy(next)),
        None => proxy_button,
    };

    let tun_button = button(svg_icons::icon_themed(Icon::Zap, 12.0, tun_tint))
        .padding([4, 6])
        .style(control_style);
    let tun_button = match tun_desired {
        Some(next) => tun_button.on_press(Message::SetTunEnabled(next)),
        None => tun_button,
    };

    footer_row = footer_row
        .push(crate::accessibility::labelled(
            infiltrator_contract::a11y::ShellA11yNode::MiniHudSystemProxySwitch,
            &state.shell.lang,
            proxy_button.into(),
        ))
        .push(Space::new().width(theme::SP_XS));

    // The compact state letters are the shared contract vocabulary; the
    // surrounding labels are localized, so both ends read the same states.
    let state_text = format!(
        "{} {} · {} {}",
        lang.tr("mini_hud_system_proxy_short"),
        proxy_state.compact_label(),
        lang.tr("mini_hud_tun_short"),
        tun_state.compact_label()
    );
    footer_row = footer_row
        .push(crate::accessibility::labelled(
            infiltrator_contract::a11y::ShellA11yNode::MiniHudTunSwitch,
            &state.shell.lang,
            tun_button.into(),
        ))
        .push(Space::new().width(theme::SP_SM))
        .push(text(state_text).size(10).style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_tertiary),
        }));

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
                width: theme::HAIRLINE,
                color: tk.card_border,
            },
            shadow: tk.floating_shadow,
            text_color: Some(tk.overlay_text),
            ..Default::default()
        }
    });

    // Whole-card drag: the pointer position is forwarded into the shared
    // placement math in `update/ui.rs`; releasing persists and snaps.
    let draggable = mouse_area(hud_card)
        .on_move(|point| Message::MiniHudMoved {
            x: point.x,
            y: point.y,
        })
        .on_release(Message::MiniHudDragReleased);

    container(draggable)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into()
}

fn proxy_color(state: &SystemToggleState, t: &Theme) -> iced::Color {
    if state.is_enabled() {
        tokens(t).accent
    } else {
        tokens(t).text_tertiary
    }
}

fn control_style(t: &Theme, _status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: Some(tk.control_bg.into()),
        border: Border {
            radius: border::Radius::from(theme::R_CHIP),
            width: theme::HAIRLINE,
            color: tk.card_border,
        },
        ..Default::default()
    }
}

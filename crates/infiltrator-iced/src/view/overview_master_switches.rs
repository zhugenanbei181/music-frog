//! Iced adapter for Overview's large system proxy/TUN master controls.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::card_surface;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::system_toggle::{SystemToggle, SystemToggleState};
use infiltrator_shared::locales::{Lang, Localizer};

/// The same two primary controls as the sidebar, rendered as Overview cards
/// and driven by the shared SystemToggleSnapshot.
pub fn overview_master_switches<'a>(state: &'a AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    row![
        switch_card(
            SystemToggle::SystemProxy,
            lang.tr("system_proxy").to_string(),
            "Take over the local HTTP/SOCKS endpoint",
            Icon::Wifi,
            &state.runtime.system_toggles,
            lang,
        ),
        switch_card(
            SystemToggle::Tun,
            lang.tr("tun_mode").to_string(),
            "Route traffic through the virtual interface",
            Icon::Zap,
            &state.runtime.system_toggles,
            lang,
        ),
    ]
    .spacing(theme::SP_MD)
    .width(Length::Fill)
    .into()
}

fn switch_card<'a>(
    toggle: SystemToggle,
    title: String,
    description: &'static str,
    icon: Icon,
    snapshot: &'a infiltrator_contract::system_toggle::SystemToggleSnapshot,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    let state = snapshot.state(toggle);
    let enabled = state.is_enabled();
    let status = status_label(state, lang);
    let action = action_label(state, lang);
    let action_message = if state.can_toggle() {
        Some(match toggle {
            SystemToggle::SystemProxy => Message::SetSystemProxy(!enabled),
            SystemToggle::Tun => Message::SetTunEnabled(!enabled),
        })
    } else {
        None
    };
    let status_color: fn(&Theme) -> iced::Color = match state {
        SystemToggleState::Enabled => |t: &Theme| tokens(t).success,
        SystemToggleState::Pending { .. } => |t: &Theme| tokens(t).warning,
        SystemToggleState::Disabled => |t: &Theme| tokens(t).text_secondary,
        SystemToggleState::Unknown
        | SystemToggleState::Unsupported { .. }
        | SystemToggleState::Failed { .. } => |t: &Theme| tokens(t).danger,
    };

    let control = button(
        row![
            text(action).size(11).font(FONT_MEDIUM),
            Space::new().width(theme::SP_XS),
            icon_themed(Icon::ChevronRight, 12.0, status_color),
        ]
        .align_y(Alignment::Center),
    )
    .padding([5, 9])
    .on_press_maybe(action_message);

    container(
        column![
            row![
                row![
                    icon_themed(icon, 16.0, |t: &Theme| tokens(t).accent),
                    Space::new().width(theme::SP_SM),
                    text(title)
                        .size(13)
                        .font(FONT_SEMIBOLD)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_primary),
                        }),
                ]
                .align_y(Alignment::Center),
                Space::new().width(Length::Fill),
                text(status)
                    .size(11)
                    .font(FONT_MEDIUM)
                    .style(move |t: &Theme| text::Style {
                        color: Some(status_color(t)),
                    }),
            ]
            .align_y(Alignment::Center),
            row![
                text(description)
                    .size(10)
                    .font(FONT_MEDIUM)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary),
                    }),
                Space::new().width(Length::Fill),
                control,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_LG)
    .style(card_surface)
    .into()
}

fn status_label(state: &SystemToggleState, lang: &Lang<'_>) -> String {
    match state {
        SystemToggleState::Enabled => lang.tr("overview_toggle_enabled").to_string(),
        SystemToggleState::Disabled => lang.tr("overview_toggle_disabled").to_string(),
        SystemToggleState::Pending { .. } => lang.tr("overview_toggle_pending").to_string(),
        SystemToggleState::Unknown => lang.tr("overview_toggle_unknown").to_string(),
        SystemToggleState::Unsupported { .. } | SystemToggleState::Failed { .. } => {
            lang.tr("overview_toggle_unavailable").to_string()
        }
    }
}

fn action_label(state: &SystemToggleState, lang: &Lang<'_>) -> String {
    match state {
        SystemToggleState::Enabled => lang.tr("overview_toggle_disable").to_string(),
        SystemToggleState::Disabled => lang.tr("overview_toggle_enable").to_string(),
        SystemToggleState::Pending { .. } => lang.tr("overview_toggle_pending").to_string(),
        SystemToggleState::Unknown
        | SystemToggleState::Unsupported { .. }
        | SystemToggleState::Failed { .. } => lang.tr("overview_toggle_unavailable").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_master_control_is_not_offered_as_an_action() {
        let snapshot = SystemToggleState::Unsupported {
            failure: infiltrator_contract::error::Failure::unsupported("host missing"),
        };
        let lang = Lang("en-US");
        assert_eq!(status_label(&snapshot, &lang), "Unavailable");
        assert_eq!(action_label(&snapshot, &lang), "Unavailable");
        assert!(!snapshot.can_toggle());
    }
}

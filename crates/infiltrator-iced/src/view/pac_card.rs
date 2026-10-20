//! PAC Auto-Proxy Service & Bypass Subnet Manager component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::component_card::card;
use crate::view::component_forms::{form_input_style, style_accent};
use crate::view::components::{BadgeKind, badge, toggle_switch};
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, MONO, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::pac::PacServiceState;
use infiltrator_presentation::host_network_projection::pac;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn pac_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let snapshot = &state.runtime.pac_snapshot;
    let status = pac(snapshot, &Lang(lang.0));
    let running = matches!(snapshot.state, PacServiceState::Running { .. });

    let toggle = toggle_switch(
        state.runtime.settings_model.pac().enabled,
        Message::TogglePacMode,
    );

    let compile_btn = button(
        row![
            svg_icons::icon_themed(Icon::Code2, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("pac_btn_compile").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 12])
    .style(style_accent)
    .on_press(Message::CompileAndValidatePac);

    let bypass_input = text_input(
        "localhost, 127.*, 192.168.*, 10.*, *.lan",
        if state.runtime.settings_text.pac_bypass_domains.is_empty() {
            "localhost, 127.*, 192.168.*, 10.*"
        } else {
            &state.runtime.settings_text.pac_bypass_domains
        },
    )
    .on_input(Message::UpdatePacBypassSubnets)
    .padding([6, 10])
    .size(12)
    .font(MONO)
    .width(Length::Fill)
    .style(form_input_style);

    let status_feedback: Element<'_, Message> = if let Some(st) = &state.runtime.pac_compile_status
    {
        container(
            row![
                svg_icons::icon_themed(Icon::ListChecks, 14.0, |t: &Theme| tokens(t).success),
                Space::new().width(theme::SP_XS),
                text(st.clone()).size(11).style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success)
                }),
            ]
            .align_y(Alignment::Center),
        )
        .into()
    } else {
        Element::from(Space::new().height(0))
    };

    card(
        Some(lang.tr("pac_title").to_string()),
        column![
            row![
                text(lang.tr("pac_desc").to_string())
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    })
                    .width(Length::Fill),
                toggle,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            row![
                text(format!("{}:", lang.tr("pac_url_label")))
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                Space::new().width(theme::SP_SM),
                text(status.clone())
                    .size(12)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_primary)
                    }),
                Space::new().width(Length::Fill),
                badge(
                    status,
                    if running {
                        BadgeKind::Success
                    } else {
                        BadgeKind::Neutral
                    }
                ),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            text(lang.tr("pac_bypass_cidrs").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            bypass_input,
            Space::new().height(theme::SP_XS),
            row![
                status_feedback,
                Space::new().width(Length::Fill),
                compile_btn,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    )
}

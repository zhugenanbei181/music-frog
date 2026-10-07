use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::card_surface;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, R_CONTROL, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_application::proxy_mode_projection::{mode_copy, mode_status_copy};
use infiltrator_contract::command::ProxyMode;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn overview_mode_segment<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let current_mode = state.runtime.proxy_mode_state.current;

    let mut pills_row = row![]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    for mode in ProxyMode::ALL {
        let is_current = Some(mode) == current_mode;
        let is_selectable = state.runtime.pending_runtime_patch.is_none()
            && state.runtime.proxy_mode_state.is_mode_selectable(mode);

        let label = mode_copy(mode, lang.0);

        let btn_content = text(label).size(13).font(if is_current {
            FONT_SEMIBOLD
        } else {
            FONT_MEDIUM
        });

        let btn = button(btn_content)
            .padding([8, 16])
            .style(move |t: &Theme, _status| {
                let tk = tokens(t);
                let bg = if is_current { tk.accent } else { tk.control_bg };
                let txt_color = if is_current {
                    tk.on_accent
                } else if is_selectable {
                    tk.text_primary
                } else {
                    tk.text_secondary
                };
                button::Style {
                    background: Some(bg.into()),
                    text_color: txt_color,
                    border: Border {
                        radius: border::Radius::from(R_CONTROL),
                        width: theme::HAIRLINE,
                        color: if is_current {
                            tk.accent
                        } else {
                            Color::TRANSPARENT
                        },
                    },
                    ..Default::default()
                }
            });

        let interactive_btn: Element<'a, Message> = if is_selectable && !is_current {
            btn.on_press(Message::SetProxyMode(mode.to_wire().to_owned()))
                .into()
        } else {
            btn.into()
        };

        pills_row = pills_row.push(interactive_btn);
    }

    let header = row![
        icon_themed(Icon::Settings, 16.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text(lang.tr("overview_proxy_mode_title").into_owned())
            .size(14)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        Space::new().width(Length::Fill),
        text(mode_status_copy(&state.runtime.proxy_mode_state, lang.0))
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    container(column![header, Space::new().height(theme::SP_XS), pills_row].spacing(theme::SP_SM))
        .width(Length::Fill)
        .padding(theme::SP_LG)
        .style(card_surface)
        .into()
}

#[cfg(test)]
#[path = "../../tests/gui/overview_mode_segment_tests.rs"]
mod overview_mode_segment_tests;

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::card_surface;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme;
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, R_CONTROL, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_presentation::routing_intent_projection::{
    routing_intent_copy, routing_intent_status_copy,
};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::routing_intent::RoutingIntent;

pub fn overview_routing_intent_segment<'a>(
    state: &AppState,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    let mut pills_row = row![]
        .spacing(theme::SP_SM)
        .align_y(Alignment::Center)
        .width(Length::Fill);

    for intent in [
        RoutingIntent::FollowRules,
        RoutingIntent::RouteAllThroughProxy,
        RoutingIntent::BypassProxy,
    ] {
        let is_current = state.runtime.routing_intent_state.current.as_ref() == Some(&intent);
        let is_selectable = state.runtime.pending_runtime_patch.is_none()
            && state
                .runtime
                .routing_intent_state
                .is_intent_selectable(&intent);

        let label = routing_intent_copy(&intent, lang.0);

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
            btn.on_press(Message::SetRoutingIntent(intent.clone()))
                .into()
        } else {
            btn.into()
        };

        pills_row = pills_row.push(interactive_btn);
    }

    let header = row![
        icon_themed(Icon::Settings, 16.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text(lang.tr("overview_routing_intent_title").into_owned())
            .size(14)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary)
            }),
        Space::new().width(Length::Fill),
        text(routing_intent_status_copy(
            &state.runtime.routing_intent_state,
            lang.0
        ))
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

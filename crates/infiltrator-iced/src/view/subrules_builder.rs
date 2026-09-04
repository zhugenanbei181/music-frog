//! Sub-Rules & Logical Rule Visual Builder card.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, icon_button, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn subrules_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let draft = &state.editor.subrule_draft;

    let op_selector = row![
        text(lang.tr("subrules_operator").to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
        Space::new().width(theme::SP_SM),
        button(text("AND").size(11))
            .padding([4, 8])
            .style(if draft.operator == "AND" { style_accent } else { style_ghost })
            .on_press(Message::UpdateSubRuleOperator("AND".to_string())),
        Space::new().width(theme::SP_XS),
        button(text("OR").size(11))
            .padding([4, 8])
            .style(if draft.operator == "OR" { style_accent } else { style_ghost })
            .on_press(Message::UpdateSubRuleOperator("OR".to_string())),
        Space::new().width(theme::SP_XS),
        button(text("NOT").size(11))
            .padding([4, 8])
            .style(if draft.operator == "NOT" { style_accent } else { style_ghost })
            .on_press(Message::UpdateSubRuleOperator("NOT".to_string())),
    ]
    .align_y(Alignment::Center);

    let mut cond_rows = column![].spacing(theme::SP_XS);
    for (idx, cond) in draft.conditions.iter().enumerate() {
        let cond_str = cond.clone();
        cond_rows = cond_rows.push(
            row![
                badge(format!("#{}", idx + 1), BadgeKind::Neutral),
                Space::new().width(theme::SP_SM),
                text(cond_str).size(12).font(MONO).width(Length::Fill),
                icon_button(Icon::Trash2, 12.0, Message::RemoveSubRuleCondition(idx)),
            ]
            .align_y(Alignment::Center),
        );
    }

    let add_cond_row = row![
        button(
            row![
                svg_icons::icon_themed(Icon::Plus, 12.0, |t: &Theme| tokens(t).accent),
                Space::new().width(theme::SP_XS),
                text("+ DOMAIN-SUFFIX").size(11),
            ]
            .align_y(Alignment::Center)
        )
        .padding([4, 8])
        .style(style_ghost)
        .on_press(Message::AddSubRuleCondition("DOMAIN-SUFFIX,google.com".to_string())),
        Space::new().width(theme::SP_XS),
        button(
            row![
                svg_icons::icon_themed(Icon::Plus, 12.0, |t: &Theme| tokens(t).accent),
                Space::new().width(theme::SP_XS),
                text("+ NETWORK-UDP").size(11),
            ]
            .align_y(Alignment::Center)
        )
        .padding([4, 8])
        .style(style_ghost)
        .on_press(Message::AddSubRuleCondition("NETWORK,UDP".to_string())),
    ]
    .align_y(Alignment::Center);

    let conds_str = draft.conditions.join(", ");
    let preview_text = format!("{}(({conds_str})),{}", draft.operator, draft.target);

    let preview_card = container(
        row![
            text(format!("{}:", lang.tr("subrules_result_preview")))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary)
                }),
            Space::new().width(theme::SP_SM),
            text(preview_text).size(12).font(MONO).width(Length::Fill).style(|t: &Theme| text::Style {
                color: Some(tokens(t).accent)
            }),
            button(
                row![
                    svg_icons::icon_themed(Icon::Plus, 12.0, |t: &Theme| tokens(t).on_accent),
                    Space::new().width(theme::SP_XS),
                    text(lang.tr("subrules_btn_insert").to_string()).size(11),
                ]
                .align_y(Alignment::Center)
            )
            .padding([4, 10])
            .style(style_accent)
            .on_press(Message::InsertSubRuleIntoRules),
        ]
        .align_y(Alignment::Center),
    )
    .padding([8, 12])
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.control_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CONTROL),
                width: 1.0,
                color: tk.card_border,
            },
            ..Default::default()
        }
    });

    card(
        Some(lang.tr("subrules_title").to_string()),
        column![
            op_selector,
            Space::new().height(theme::SP_XS),
            cond_rows,
            add_cond_row,
            Space::new().height(theme::SP_XS),
            preview_card,
        ]
        .spacing(theme::SP_SM),
    )
}

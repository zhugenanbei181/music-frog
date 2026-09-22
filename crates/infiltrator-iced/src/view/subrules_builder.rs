//! Sub-Rules & Logical Rule Visual Builder card (DUAL-11-02).
//!
//! The panel is a pure view over the shared
//! `infiltrator_domain::rules::logical` reduction: the operator vocabulary, the
//! condition presets, the canonical `OP((cond),(cond),TARGET)` preview and the
//! validation status all come from there, so the Bevy card renders identically.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card, form_field_label, form_input_style, icon_button, style_accent,
    style_ghost,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_domain::rules::logical;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn subrules_panel<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let draft = &state.editor.subrule_draft;
    let issue = logical::draft_issue(draft);

    let mut op_selector = row![
        text(lang.tr("subrules_operator").to_string())
            .size(12)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            })
    ]
    .align_y(Alignment::Center);
    for (index, choice) in logical::LOGICAL_OPERATOR_CHOICES.iter().enumerate() {
        op_selector = op_selector.push(Space::new().width(if index == 0 {
            theme::SP_SM
        } else {
            theme::SP_XS
        }));
        op_selector = op_selector.push(
            button(text(*choice).size(11))
                .padding([4, 8])
                .style(if draft.operator == *choice {
                    style_accent
                } else {
                    style_ghost
                })
                .on_press(Message::UpdateSubRuleOperator((*choice).to_string())),
        );
    }

    let mut cond_rows = column![].spacing(theme::SP_XS);
    if draft.conditions.is_empty() {
        cond_rows = cond_rows.push(
            text(lang.tr("subrules_no_conditions").to_string())
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        );
    }
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

    let mut add_cond_row = row![
        text(lang.tr("subrules_btn_add_leaf").to_string())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary)
            }),
    ]
    .align_y(Alignment::Center);
    for preset in logical::SUB_RULE_CONDITION_PRESETS.iter() {
        add_cond_row = add_cond_row.push(Space::new().width(theme::SP_XS));
        add_cond_row = add_cond_row.push(
            button(
                row![
                    svg_icons::icon_themed(Icon::Plus, 12.0, |t: &Theme| tokens(t).accent),
                    Space::new().width(theme::SP_XS),
                    text((*preset).to_string()).size(11),
                ]
                .align_y(Alignment::Center),
            )
            .padding([4, 8])
            .style(style_ghost)
            .on_press(Message::AddSubRuleCondition((*preset).to_string())),
        );
    }

    let target_row = row![
        form_field_label(lang.tr("subrules_target").to_string()),
        Space::new().width(theme::SP_SM),
        text_input("PROXY", &draft.target)
            .on_input(Message::UpdateSubRuleTarget)
            .padding([6, 10])
            .size(12)
            .font(MONO)
            .style(form_input_style)
            .width(Length::Fill),
    ]
    .align_y(Alignment::Center);

    let preview_text = logical::draft_expression(draft);
    let status_row: Element<'a, Message> = match &issue {
        None => row![badge(
            lang.tr("subrules_validate_ok").to_string(),
            BadgeKind::Success
        )]
        .align_y(Alignment::Center)
        .into(),
        Some(issue) => row![
            badge(
                lang.tr("subrules_validate_failed").to_string(),
                BadgeKind::Danger
            ),
            Space::new().width(theme::SP_SM),
            text(issue.clone()).size(11).style(|t: &Theme| text::Style {
                color: Some(tokens(t).danger)
            }),
        ]
        .align_y(Alignment::Center)
        .into(),
    };

    let preview_card = container(
        column![
            row![
                text(format!("{}:", lang.tr("subrules_result_preview")))
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    }),
                Space::new().width(theme::SP_SM),
                text(preview_text)
                    .size(12)
                    .font(MONO)
                    .width(Length::Fill)
                    .style(|t: &Theme| text::Style {
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
                .style(if issue.is_none() {
                    style_accent
                } else {
                    style_ghost
                })
                .on_press(Message::InsertSubRuleIntoRules),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            status_row,
        ]
        .spacing(theme::SP_XS),
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
            target_row,
            Space::new().height(theme::SP_XS),
            preview_card,
        ]
        .spacing(theme::SP_SM),
    )
}

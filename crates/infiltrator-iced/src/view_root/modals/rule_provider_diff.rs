//! Rule-provider diff modal: added/removed rules and the unpack action.

use super::card::{modal_backdrop, modal_card};
use crate::state::AppState;
use crate::types::message::Message;
use iced::widget::{Space, button, column, container, row, scrollable, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn rule_provider_diff_modal<'a>(
    state: &'a AppState,
    diff: &'a infiltrator_domain::rules::RuleProviderDiff,
) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let _is_en = state.shell.lang.starts_with("en");
    let mut diff_items = column![].spacing(6);

    let chips_row = row![
        crate::view::components::chip(format!(
            "{}: {} {}",
            lang.tr("modal_local"),
            diff.local_count,
            lang.tr("modal_items_count")
        )),
        Space::new().width(crate::view::theme::SP_XS),
        crate::view::components::chip(format!(
            "{}: {} {}",
            lang.tr("modal_remote"),
            diff.remote_count,
            lang.tr("modal_items_count")
        )),
        Space::new().width(crate::view::theme::SP_XS),
        crate::view::components::chip(format!(
            "{}: {}",
            lang.tr("modal_unchanged"),
            diff.unchanged_count
        )),
    ]
    .spacing(4)
    .align_y(Alignment::Center);

    if diff.added_rules.is_empty() && diff.removed_rules.is_empty() {
        diff_items =
            diff_items.push(
                container(text(lang.tr("modal_no_diff").to_string()).size(12).style(
                    |t: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(t).text_secondary),
                    },
                ))
                .padding([12, 16]),
            );
    } else {
        for added in &diff.added_rules {
            let row_item = row![
                crate::view::components::badge(
                    "+ Add",
                    crate::view::components::BadgeKind::Success
                ),
                Space::new().width(crate::view::theme::SP_SM),
                text(added.clone())
                    .size(11)
                    .font(crate::view::theme::MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(t).text_primary),
                    }),
            ]
            .align_y(Alignment::Center);

            diff_items = diff_items.push(
                container(row_item)
                    .padding([6, 10])
                    .width(Length::Fill)
                    .style(|t: &Theme| {
                        let tk = crate::view::theme::tokens(t);
                        container::Style {
                            background: Some(tk.control_bg.into()),
                            border: Border {
                                radius: 6.0.into(),
                                width: crate::view::theme::HAIRLINE,
                                color: Color {
                                    a: 0.15,
                                    ..tk.success
                                },
                            },
                            ..Default::default()
                        }
                    }),
            );
        }
        for removed in &diff.removed_rules {
            let row_item = row![
                crate::view::components::badge("- Del", crate::view::components::BadgeKind::Danger),
                Space::new().width(crate::view::theme::SP_SM),
                text(removed.clone())
                    .size(11)
                    .font(crate::view::theme::MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(t).text_primary),
                    }),
            ]
            .align_y(Alignment::Center);

            diff_items = diff_items.push(
                container(row_item)
                    .padding([6, 10])
                    .width(Length::Fill)
                    .style(|t: &Theme| {
                        let tk = crate::view::theme::tokens(t);
                        container::Style {
                            background: Some(tk.control_bg.into()),
                            border: Border {
                                radius: 6.0.into(),
                                width: crate::view::theme::HAIRLINE,
                                color: Color {
                                    a: 0.15,
                                    ..tk.danger
                                },
                            },
                            ..Default::default()
                        }
                    }),
            );
        }
    }

    let diff_scrollable = scrollable(diff_items)
        .height(Length::Fixed(240.0))
        .direction(scrollable::Direction::Vertical(
            scrollable::Scrollbar::new().width(4).margin(2),
        ));

    let provider_name = diff.provider_name.clone();
    let header = row![
        text(format!(
            "{} · {}",
            lang.tr("modal_diff_title"),
            diff.provider_name
        ))
        .size(16)
        .font(crate::view::theme::FONT_SEMIBOLD)
        .style(|t: &Theme| text::Style {
            color: Some(crate::view::theme::tokens(t).text_primary),
        }),
        Space::new().width(Length::Fill),
        button(crate::view::svg_icons::icon_themed(
            crate::view::svg_icons::Icon::X,
            14.0,
            |t: &Theme| crate::view::theme::tokens(t).text_secondary
        ))
        .padding(4)
        .style(crate::view::components::style_ghost)
        .on_press(Message::InspectRuleProviderDiff(None)),
    ]
    .align_y(Alignment::Center);

    let actions = row![
        button(
            text(lang.tr("modal_close").to_string())
                .size(12)
                .font(crate::view::theme::FONT_MEDIUM)
        )
        .padding([7, 14])
        .style(crate::view::components::style_ghost)
        .on_press(Message::InspectRuleProviderDiff(None)),
        Space::new().width(Length::Fill),
        button(
            text(lang.tr("modal_unpack_rules").to_string())
                .size(12)
                .font(crate::view::theme::FONT_MEDIUM)
        )
        .padding([7, 16])
        .style(crate::view::components::style_accent)
        .on_press(Message::UnpackRuleProvider(provider_name)),
    ]
    .align_y(Alignment::Center);

    let form = column![
        header,
        Space::new().height(crate::view::theme::SP_SM),
        chips_row,
        Space::new().height(crate::view::theme::SP_SM),
        container(diff_scrollable)
            .padding(8)
            .width(Length::Fill)
            .style(|t: &Theme| {
                let tk = crate::view::theme::tokens(t);
                container::Style {
                    background: Some(tk.control_bg.into()),
                    border: Border {
                        radius: 10.0.into(),
                        width: crate::view::theme::HAIRLINE,
                        color: tk.card_border,
                    },
                    ..Default::default()
                }
            }),
        Space::new().height(crate::view::theme::SP_MD),
        actions,
    ]
    .spacing(6);

    modal_backdrop(modal_card(form.into(), 520.0))
}

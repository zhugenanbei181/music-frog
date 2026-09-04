//! Snapshot Visual Diff & Rollback Modal Dialog.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, icon_button, modern_scrollable, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn snapshot_diff_modal<'a>(state: &'a AppState, snapshot_id: &str) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let title_row = row![
        svg_icons::icon_themed(Icon::FileText, 18.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text(lang.tr("snapshot_diff_title").to_string())
            .size(15)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_SM),
        badge(snapshot_id.to_string(), BadgeKind::Neutral),
        Space::new().width(Length::Fill),
        icon_button(Icon::X, 14.0, Message::CloseSnapshotDiff),
    ]
    .align_y(Alignment::Center);

    let subtitle = text(format!("{}: {}", lang.tr("snapshot_diff_compare_with"), snapshot_id))
        .size(12)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        });

    let mock_diff_rows = column![
        row![
            badge("+ Added".to_string(), BadgeKind::Success),
            Space::new().width(theme::SP_SM),
            text("proxies: [SS-Tokyo, Vless-Reality-US, HK-01]")
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).success)
                }),
        ]
        .align_y(Alignment::Center),
        row![
            badge("- Removed".to_string(), BadgeKind::Danger),
            Space::new().width(theme::SP_SM),
            text("rules: [DOMAIN-SUFFIX,google.com,DIRECT]")
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).danger)
                }),
        ]
        .align_y(Alignment::Center),
        row![
            badge("~ Modified".to_string(), BadgeKind::Warning),
            Space::new().width(theme::SP_SM),
            text("tun: { enable: false → true, stack: gvisor }")
                .size(11)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).warning)
                }),
        ]
        .align_y(Alignment::Center),
    ]
    .spacing(theme::SP_SM);

    let diff_container = container(modern_scrollable(mock_diff_rows).height(Length::Fixed(160.0)))
        .padding([12, 16])
        .width(Length::Fill)
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

    let target_id = snapshot_id.to_string();
    let actions = row![
        button(text(lang.tr("btn_cancel").to_string()).size(12))
            .padding([6, 14])
            .style(style_ghost)
            .on_press(Message::CloseSnapshotDiff),
        Space::new().width(theme::SP_SM),
        button(
            row![
                svg_icons::icon_themed(Icon::RefreshCw, 12.0, |t: &Theme| tokens(t).on_accent),
                Space::new().width(theme::SP_XS),
                text(lang.tr("snapshot_diff_rollback_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .align_y(Alignment::Center)
        )
        .padding([6, 16])
        .style(style_accent)
        .on_press(Message::RollbackToSnapshot(target_id)),
    ]
    .align_y(Alignment::Center);

    let modal_card = container(
        column![
            title_row,
            subtitle,
            Space::new().height(theme::SP_SM),
            diff_container,
            Space::new().height(theme::SP_MD),
            row![Space::new().width(Length::Fill), actions],
        ]
        .spacing(theme::SP_SM),
    )
    .padding([20, 24])
    .width(540)
    .style(|t: &Theme| {
        let tk = tokens(t);
        container::Style {
            background: Some(tk.card_bg.into()),
            border: Border {
                radius: border::Radius::from(theme::R_CARD),
                width: 1.0,
                color: tk.card_border,
            },
            shadow: tk.floating_shadow,
            ..Default::default()
        }
    });

    container(modal_card)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .style(|_t: &Theme| container::Style {
            background: Some(Color { a: 0.50, r: 0.0, g: 0.0, b: 0.0 }.into()),
            ..Default::default()
        })
        .into()
}

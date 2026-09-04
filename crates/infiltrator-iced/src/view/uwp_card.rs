//! Windows UWP Loopback Exemption Utility component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{badge, card, modern_scrollable, style_accent, style_ghost, BadgeKind};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn uwp_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let uwp = &state.shell.uwp_loopback;

    let scan_btn = button(
        row![
            svg_icons::icon_themed(Icon::Search, 12.0, |t: &Theme| tokens(t).text_secondary),
            Space::new().width(theme::SP_XS),
            text(lang.tr("uwp_btn_scan").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press_maybe((!uwp.is_scanning).then_some(Message::ScanUwpApps));

    let exempt_all_btn = button(
        row![
            svg_icons::icon_themed(Icon::ListChecks, 12.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_XS),
            text(lang.tr("uwp_btn_exempt_all").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_accent)
    .on_press(Message::ExemptAllUwpApps);

    let clear_all_btn = button(
        row![
            svg_icons::icon_themed(Icon::Trash2, 12.0, |t: &Theme| tokens(t).danger),
            Space::new().width(theme::SP_XS),
            text(lang.tr("uwp_btn_clear_all").to_string())
                .size(11)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([4, 10])
    .style(style_ghost)
    .on_press(Message::ClearAllUwpExemptions);

    let mut apps_column = column![].spacing(theme::SP_XS);
    for app in &uwp.apps {
        let sid = app.sid.clone();
        let is_exempt = app.is_exempt;
        let glyph = if is_exempt { "☑" } else { "☐" };

        let item = button(
            row![
                text(glyph).size(14).font(MONO).style(move |t: &Theme| text::Style {
                    color: Some(if is_exempt { tokens(t).accent } else { tokens(t).text_tertiary }),
                }),
                Space::new().width(theme::SP_SM),
                text(app.display_name.clone()).size(12).font(FONT_MEDIUM),
                Space::new().width(Length::Fill),
                badge(if is_exempt { "Exempted" } else { "Isolated" }, if is_exempt { BadgeKind::Success } else { BadgeKind::Neutral }),
            ]
            .align_y(Alignment::Center),
        )
        .padding([4, 8])
        .style(|_t: &Theme, _| button::Style::default())
        .on_press(Message::ToggleUwpAppExemption(sid));

        apps_column = apps_column.push(item);
    }

    let apps_container = container(modern_scrollable(apps_column).height(Length::Fixed(110.0)))
        .padding(6)
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
        Some(lang.tr("uwp_title").to_string()),
        column![
            row![
                text(lang.tr("uwp_desc").to_string()).size(12).style(|t: &Theme| text::Style { color: Some(tokens(t).text_secondary) }).width(Length::Fill),
                scan_btn,
                Space::new().width(theme::SP_XS),
                exempt_all_btn,
                Space::new().width(theme::SP_XS),
                clear_all_btn,
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            apps_container,
        ]
        .spacing(theme::SP_SM),
    )
}

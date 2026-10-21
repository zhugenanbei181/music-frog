//! Windows UWP Loopback Exemption Utility component.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::checkbox::check_box;
use crate::view::component_card::card;
use crate::view::component_forms::{style_accent, style_ghost};
use crate::view::components::{BadgeKind, badge, modern_scrollable};
use crate::view::svg_icons::Icon;
use crate::view::theme::{FONT_MEDIUM, tokens};
use crate::view::{svg_icons, theme};
use iced::widget::{Column, Space, button, column, container, row, text};
use iced::{Alignment, Border, Element, Length, Theme, border};
use infiltrator_presentation::routing_projection::uwp_summary;
use infiltrator_shared::locales::{Lang, Localizer};

/// The exemption list of the card.
///
/// The wide tier keeps the list in its own scrollable, so a long package
/// list never pushes the card out of the page. The compact tier scrolls the
/// page itself, so the list is plain content there.
fn apps_list<'a, Message: 'a>(
    state: &AppState,
    content: Column<'a, Message>,
) -> Element<'a, Message> {
    if state.shell.viewport.tier.is_narrow() {
        content.into()
    } else {
        modern_scrollable(content)
            .height(Length::Fixed(110.0))
            .into()
    }
}

pub fn uwp_card<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let uwp = &state.shell.uwp_loopback;
    let availability = uwp
        .scan_phase()
        .failure()
        .map(|failure| failure.message.clone())
        .unwrap_or_else(|| {
            uwp_summary(
                uwp.uwp_availability(),
                uwp.uwp_packages()
                    .iter()
                    .map(|package| package.loopback_exempt),
                &Lang(lang.0),
            )
        });

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
    .on_press_maybe((!uwp.scan_phase().is_running()).then_some(Message::ScanUwpApps));

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
    for app in uwp.uwp_packages() {
        let sid = app.sid.clone();
        let is_exempt = app.loopback_exempt;

        // The exemption control is the toolkit's own check box: the box and
        // its label are the click target, so the state badge beside them is no
        // longer a button of its own.
        let exemption = check_box(is_exempt, app.display_name.clone(), move |_| {
            Message::ToggleUwpAppExemption(sid.clone())
        });

        let item = container(
            row![
                exemption,
                Space::new().width(theme::SP_SM),
                Space::new().width(Length::Fill),
                badge(
                    if is_exempt { "Exempted" } else { "Isolated" },
                    if is_exempt {
                        BadgeKind::Success
                    } else {
                        BadgeKind::Neutral
                    }
                ),
            ]
            .align_y(Alignment::Center),
        )
        .padding([4, 8])
        .style(|_t: &Theme| container::Style::default());

        apps_column = apps_column.push(item);
    }

    let apps_container = container(apps_list(state, apps_column))
        .padding(6)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                ..Default::default()
            }
        });
    card(
        Some(lang.tr("uwp_title").to_string()),
        column![
            row![
                text(lang.tr("uwp_desc").to_string())
                    .size(12)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary)
                    })
                    .width(Length::Fill),
                scan_btn,
                Space::new().width(theme::SP_XS),
                exempt_all_btn,
                Space::new().width(theme::SP_XS),
                clear_all_btn,
            ]
            .align_y(Alignment::Center),
            text(availability).size(11).style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
            Space::new().height(theme::SP_XS),
            apps_container,
        ]
        .spacing(theme::SP_SM),
    )
}

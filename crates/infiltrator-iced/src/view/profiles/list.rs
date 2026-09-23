//! Profile card list with quota/expiry detail.

use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::component_forms::{style_ghost, text_btn};
use crate::view::components::{
    BadgeKind, badge, card, chip, empty_state, icon_button, kbd_badge, section_header,
};
use crate::view::svg_icons::Icon;
use crate::view::theme::{self, FONT_SEMIBOLD, MONO, R_CARD, SP_MD, tokens};
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

use super::helpers::{format_datetime, traffic_row};

pub(super) fn profiles_section<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let mut profiles_list = column![].spacing(SP_MD);
    let profile_filter = state.profile.profiles_filter.trim().to_lowercase();
    let filtered_profiles: Vec<_> = state
        .profile
        .profiles
        .iter()
        .filter(|p| {
            profile_filter.is_empty()
                || p.name.to_lowercase().contains(&profile_filter)
                || p.path.to_lowercase().contains(&profile_filter)
        })
        .collect();

    if state.profile.is_loading_profiles {
        profiles_list = profiles_list.push(empty_state(
            Icon::FileText,
            lang.tr("loading_profiles").as_ref(),
            "",
        ));
    } else if state.profile.profiles.is_empty() {
        profiles_list = profiles_list.push(empty_state(
            Icon::FileText,
            lang.tr("no_profiles").as_ref(),
            "",
        ));
    } else if filtered_profiles.is_empty() {
        profiles_list = profiles_list.push(empty_state(
            Icon::Search,
            lang.tr("profiles_no_match").as_ref(),
            "",
        ));
    } else {
        for profile in filtered_profiles {
            let is_active = profile.active;
            let is_subscription = profile.subscription_url.is_some();

            let source_badge: Element<'_, Message> = if is_subscription {
                badge(lang.tr("subscription").as_ref(), BadgeKind::Accent)
            } else {
                chip(lang.tr("profiles_badge_local").to_string())
            };

            let mut actions = row![].spacing(theme::SP_SM).align_y(Alignment::Center);
            if !is_active {
                actions = actions.push(text_btn(
                    lang.tr("use").to_string(),
                    style_ghost,
                    Some(Message::SetActiveProfile(profile.name.clone())),
                ));
            }
            actions = actions.push(icon_button(
                Icon::Pencil,
                14.0,
                Message::EditProfile(profile.path.clone().into()),
            ));
            actions = actions.push(icon_button(
                Icon::Code2,
                14.0,
                Message::EditProfileAs(profile.path.clone().into(), EditorPane::Mixin),
            ));
            if !is_active {
                actions = actions.push(icon_button(
                    Icon::Trash2,
                    14.0,
                    Message::RequestConfirmation(ConfirmAction::DeleteProfile(
                        profile.name.clone(),
                    )),
                ));
            }

            profiles_list = profiles_list.push(
                container(
                    column![
                        row![
                            column![
                                row![
                                    text(&profile.name).size(15).font(FONT_SEMIBOLD).style(
                                        |t: &Theme| text::Style {
                                            color: Some(tokens(t).text_primary)
                                        }
                                    ),
                                    Space::new().width(theme::SP_SM),
                                    source_badge,
                                    if is_active {
                                        Space::new().width(theme::SP_SM)
                                    } else {
                                        Space::new().width(0)
                                    },
                                    if is_active {
                                        Element::from(badge(
                                            lang.tr("active_tag").trim().to_string(),
                                            BadgeKind::Success,
                                        ))
                                    } else {
                                        Element::from(Space::new().width(0))
                                    },
                                ]
                                .align_y(Alignment::Center),
                                row![
                                    kbd_badge(if is_subscription { "SUB" } else { "YAML" }),
                                    Space::new().width(theme::SP_XS),
                                    text(profile.path.clone()).size(11).font(MONO).style(
                                        |t: &Theme| text::Style {
                                            color: Some(tokens(t).text_tertiary)
                                        }
                                    ),
                                ]
                                .align_y(Alignment::Center),
                            ]
                            .spacing(theme::SP_XS)
                            .width(Length::Fill),
                            actions,
                        ]
                        .align_y(Alignment::Center),
                        if is_subscription {
                            let last_up = format_datetime(
                                profile.last_updated,
                                lang.tr("profiles_never").as_ref(),
                            );
                            let mut sub_details = column![
                                row![
                                    text(format!(
                                        "{} {}",
                                        lang.tr("profiles_last_updated"),
                                        last_up
                                    ))
                                    .size(11)
                                    .font(MONO)
                                    .style(|t: &Theme| {
                                        text::Style {
                                            color: Some(tokens(t).text_secondary),
                                        }
                                    })
                                ]
                                .align_y(Alignment::Center)
                            ]
                            .spacing(theme::SP_XS);
                            if let Some(traffic_elem) = traffic_row(profile, &lang) {
                                sub_details = sub_details.push(traffic_elem);
                            }
                            Element::from(sub_details)
                        } else {
                            Element::from(Space::new().width(0).height(0))
                        },
                    ]
                    .spacing(theme::SP_XS),
                )
                .padding(SP_MD)
                .width(Length::Fill)
                .style(move |t: &Theme| {
                    let tk = tokens(t);
                    container::Style {
                        background: Some(
                            if is_active {
                                Color {
                                    a: 0.03,
                                    ..tk.accent
                                }
                            } else {
                                tk.card_bg
                            }
                            .into(),
                        ),
                        border: Border {
                            radius: border::Radius::from(R_CARD),
                            width: if is_active { 2.0 } else { 1.0 },
                            color: if is_active { tk.accent } else { tk.card_border },
                        },
                        shadow: tk.card_shadow,
                        ..Default::default()
                    }
                }),
            );
        }
    }

    card(
        None,
        column![
            section_header(
                "PROFILES",
                Some(
                    text(format!("{}", state.profile.profiles.len()))
                        .size(12)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_tertiary)
                        })
                        .into()
                )
            ),
            Space::new().height(theme::SP_MD),
            profiles_list,
        ],
    )
}

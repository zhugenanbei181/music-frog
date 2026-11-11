//! Profile card list with quota/expiry detail.

use super::helpers::{format_datetime, traffic_row};
use crate::state::AppState;
use crate::types::app::ConfirmAction;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::component_card::card;
use crate::view::component_forms::{style_ghost, text_btn};
use crate::view::components::{
    BadgeKind, badge, chip, empty_state, icon_button, kbd_badge, section_header,
};
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{R_CARD, SP_MD, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::responsive_viewport::{ProfileGridForm, profile_columns};
use infiltrator_domain::profiles::ProfileInfo;
use infiltrator_presentation::profiles_page_model::{ProfileFilterTarget, ProfilesOperation};
use infiltrator_shared::locales::{Lang, Localizer};

/// The profile facts the shared list filter reads.
///
/// The shared filter matches the profile name and its kind; the Iced list
/// shows the profile path as its kind, so the filter answers the same way it
/// always did.
struct ProfileRow<'a>(&'a ProfileInfo);

impl ProfileFilterTarget for ProfileRow<'_> {
    fn profile_name(&self) -> &str {
        &self.0.name
    }

    fn profile_kind(&self) -> &str {
        &self.0.path
    }
}

/// The profiles grid geometry for the current window: the seam the row pays
/// and the column count the shared width rule answers for the page body.
pub(crate) fn profile_grid_geometry(state: &AppState) -> (f32, usize) {
    let tier = state.shell.viewport.tier;
    let grid_gap = if tier.has_labelled_sidebar() {
        SP_MD
    } else {
        theme::SP_SM
    };
    let columns = profile_columns(
        if tier.has_labelled_sidebar() {
            ProfileGridForm::Desktop
        } else {
            ProfileGridForm::Stacked
        },
        tier.page_body_width_px(state.shell.viewport.width_px.round() as u16),
        grid_gap as u16,
    ) as usize;
    (grid_gap, columns)
}

pub(super) fn profiles_section<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);
    let mut profiles_list = column![].spacing(SP_MD);
    let (grid_gap, grid_columns) = profile_grid_geometry(state);
    // The list filter is the shared page model's; the surface keeps no
    // second copy of the match rule.
    let profile_rows: Vec<ProfileRow<'_>> = state.profile.profiles.iter().map(ProfileRow).collect();
    let filtered_profiles = state.profile.profiles_model.visible_profiles(&profile_rows);

    if state.profile.operation_running(ProfilesOperation::LoadList) {
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
        let profile_cards: Vec<Element<'_, Message>> = filtered_profiles
            .iter()
            .map(|row_profile| -> Element<'_, Message> {
                let profile = row_profile.0;
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

                container(
                    column![
                        row![
                            column![
                                row![
                                    theme::role_body_text(&profile.name).style(|t: &Theme| {
                                        text::Style {
                                            color: Some(tokens(t).text_primary),
                                        }
                                    }),
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
                                    theme::role_body_text(profile.path.clone()).style(
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
                                    theme::role_body_text(format!(
                                        "{} {}",
                                        lang.tr("profiles_last_updated"),
                                        last_up
                                    ))
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
                .height(Length::Shrink)
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
                })
                .into()
            })
            .collect();
        if grid_columns > 1 {
            let mut grid = column![].spacing(grid_gap);
            let mut grid_row = row![].spacing(grid_gap).align_y(Alignment::Start);
            let mut row_used = 0usize;
            for profile_card in profile_cards {
                if row_used == grid_columns {
                    grid = grid.push(grid_row);
                    grid_row = row![].spacing(grid_gap).align_y(Alignment::Start);
                    row_used = 0;
                }
                grid_row = grid_row.push(
                    container(profile_card)
                        .width(Length::FillPortion(1))
                        .height(Length::Shrink),
                );
                row_used += 1;
            }
            for _ in row_used..grid_columns {
                grid_row = grid_row.push(Space::new().width(Length::FillPortion(1)));
            }
            grid = grid.push(grid_row);
            profiles_list = profiles_list.push(grid);
        } else {
            let mut stacked = column![].spacing(SP_MD);
            for profile_card in profile_cards {
                stacked = stacked.push(profile_card);
            }
            profiles_list = profiles_list.push(stacked);
        }
    }

    container(card(
        None,
        column![
            section_header(
                "PROFILES",
                Some(
                    theme::role_body_text(format!("{}", state.profile.profiles.len()))
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_tertiary)
                        })
                        .into()
                )
            ),
            Space::new().height(theme::SP_MD),
            profiles_list,
        ],
    ))
    .id(InteractionRegion::ProfilesGrid.id())
    .into()
}

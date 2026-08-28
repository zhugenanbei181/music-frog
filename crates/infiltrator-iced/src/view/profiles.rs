use crate::locales::{Lang, Localizer};
use crate::view::components::{card, modern_scrollable};
use crate::view::icons;
use crate::{AppState, Message};
use chrono::{DateTime, Local, Utc};
use iced::widget::{Space, button, checkbox, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Color, Element, Font, Length};

fn format_datetime(value: Option<DateTime<Utc>>, fallback: &str) -> String {
    value
        .map(|ts| {
            ts.with_timezone(&Local)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string()
        })
        .unwrap_or_else(|| fallback.to_string())
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.lang);
    let bold_font = Font {
        weight: iced::font::Weight::Bold,
        ..Default::default()
    };

    let clear_profiles_btn: Element<'_, Message> = if state.is_loading_profiles {
        button(text(lang.tr("profiles_clearing")).size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else {
        button(text(lang.tr("profiles_clear_all")).size(12))
            .on_press(Message::ClearProfiles)
            .padding([6, 12])
            .style(button::danger)
            .into()
    };

    let header = row![
        text(lang.tr("profiles_title")).size(24).font(bold_font),
        Space::new().width(20),
        text_input(
            lang.tr("profiles_search_placeholder").as_ref(),
            &state.profiles_filter
        )
        .on_input(Message::UpdateProfilesFilter)
        .padding(10)
        .size(13)
        .width(Length::Fixed(240.0)),
        Space::new().width(10),
        clear_profiles_btn,
        Space::new().width(Length::Fill),
        button(
            row![
                text(icons::EDITOR).size(12),
                text(lang.tr("profiles_open_folder")).size(12)
            ]
            .spacing(8)
        )
        .on_press(Message::OpenConfigDir)
        .padding([6, 12])
        .style(button::secondary)
    ]
    .align_y(Alignment::Center);

    let import_actions: Element<'_, Message> = if state.is_importing {
        button(text(lang.tr("profiles_importing")).size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else {
        button(
            row![
                text(icons::ADD).size(12),
                text(lang.tr("profiles_import_btn")).size(12)
            ]
            .spacing(8),
        )
        .on_press(Message::ImportProfile)
        .padding([6, 12])
        .style(button::primary)
        .into()
    };

    let import_section = card(column![
        text(lang.tr("profiles_import_sub")).font(bold_font),
        Space::new().height(10),
        row![
            text_input(
                lang.tr("profiles_import_name_placeholder").as_ref(),
                &state.import_name
            )
            .on_input(Message::UpdateImportName)
            .padding(10)
            .size(14)
            .width(Length::FillPortion(1)),
            Space::new().width(10),
            text_input(lang.tr("profiles_sub_url").as_ref(), &state.import_url)
                .on_input(Message::UpdateImportUrl)
                .padding(10)
                .size(14)
                .width(Length::FillPortion(2)),
            Space::new().width(10),
            import_actions
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        checkbox(state.import_activate)
            .label(lang.tr("profiles_import_activate").into_owned())
            .on_toggle(Message::UpdateImportActivate)
            .size(16),
    ]);

    let local_import_action: Element<'_, Message> = if state.is_importing_local {
        button(text(lang.tr("profiles_importing")).size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else {
        button(
            row![
                text(icons::ADD).size(12),
                text(lang.tr("profiles_import_local_btn")).size(12)
            ]
            .spacing(8),
        )
        .on_press(Message::ImportLocalProfile)
        .padding([6, 12])
        .style(button::primary)
        .into()
    };

    let local_import_section = card(column![
        text(lang.tr("profiles_local_import_title")).font(bold_font),
        Space::new().height(10),
        row![
            text_input(
                lang.tr("profiles_local_path_placeholder").as_ref(),
                &state.local_import_path
            )
            .on_input(Message::UpdateLocalImportPath)
            .padding(10)
            .size(14)
            .width(Length::FillPortion(2)),
            Space::new().width(10),
            button(
                row![
                    text(icons::EDITOR).size(12),
                    text(lang.tr("profiles_browse_btn")).size(12)
                ]
                .spacing(8)
            )
            .on_press(Message::BrowseLocalImportFile)
            .padding([8, 12])
            .style(button::secondary),
            Space::new().width(10),
            text_input(
                lang.tr("profiles_local_name_placeholder").as_ref(),
                &state.local_import_name
            )
            .on_input(Message::UpdateLocalImportName)
            .padding(10)
            .size(14)
            .width(Length::FillPortion(1)),
            Space::new().width(10),
            local_import_action
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        checkbox(state.local_import_activate)
            .label(lang.tr("profiles_import_activate").into_owned())
            .on_toggle(Message::UpdateLocalImportActivate)
            .size(16),
    ]);

    let profile_options: Vec<String> = state.profiles.iter().map(|p| p.name.clone()).collect();
    let selected_profile = if state.subscription_profile_name.is_empty() {
        None
    } else {
        Some(&state.subscription_profile_name)
    };
    let selected_profile_meta = state
        .profiles
        .iter()
        .find(|profile| profile.name == state.subscription_profile_name);
    let interval_options: Vec<String> = ["12", "24", "48", "168"]
        .iter()
        .map(|item| (*item).to_string())
        .collect();
    let selected_interval = if state.subscription_update_interval_hours.trim().is_empty() {
        Some("24".to_string())
    } else {
        Some(state.subscription_update_interval_hours.clone())
    };

    let subscription_save_action: Element<'_, Message> = if state.is_saving_subscription {
        button(text(lang.tr("profiles_saving_subscription")).size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else {
        button(text(lang.tr("profiles_save_subscription")).size(12))
            .on_press(Message::SaveSubscriptionSettings)
            .padding([6, 12])
            .style(button::primary)
            .into()
    };

    let subscription_update_now_action: Element<'_, Message> = if state.is_updating_subscription_now
    {
        button(text(lang.tr("profiles_updating_subscription")).size(12))
            .padding([6, 12])
            .style(button::secondary)
            .into()
    } else {
        button(text(lang.tr("profiles_update_now")).size(12))
            .on_press(Message::UpdateSubscriptionNow)
            .padding([6, 12])
            .style(button::secondary)
            .into()
    };

    let subscription_section = card(column![
        text(lang.tr("profiles_subscription_settings_title")).font(bold_font),
        Space::new().height(10),
        pick_list(
            profile_options,
            selected_profile,
            Message::SelectSubscriptionProfile
        )
        .placeholder(lang.tr("profiles_select_profile").as_ref())
        .width(Length::Fill),
        Space::new().height(10),
        text_input(
            lang.tr("profiles_subscription_url").as_ref(),
            &state.subscription_url
        )
        .on_input(Message::UpdateSubscriptionUrl)
        .padding(10)
        .size(14)
        .width(Length::Fill),
        Space::new().height(10),
        row![
            container(
                checkbox(state.subscription_auto_update_enabled)
                    .label(lang.tr("profiles_auto_update").into_owned())
                    .on_toggle(Message::UpdateSubscriptionAutoUpdate)
                    .size(16)
            )
            .width(Length::FillPortion(2)),
            pick_list(
                interval_options,
                selected_interval,
                Message::UpdateSubscriptionInterval
            )
            .placeholder(lang.tr("profiles_update_interval").as_ref())
            .text_size(13)
            .width(Length::FillPortion(1)),
        ]
        .align_y(Alignment::Center),
        Space::new().height(8),
        if let Some(profile) = selected_profile_meta {
            Element::from(
                row![
                    text(format!(
                        "{} {}",
                        lang.tr("profiles_last_updated"),
                        format_datetime(
                            profile.last_updated.clone(),
                            lang.tr("profiles_never").as_ref()
                        )
                    ))
                    .size(12)
                    .style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6))
                    }),
                    Space::new().width(12),
                    text(format!(
                        "{} {}",
                        lang.tr("profiles_next_update"),
                        format_datetime(
                            profile.next_update.clone(),
                            lang.tr("profiles_not_scheduled").as_ref()
                        )
                    ))
                    .size(12)
                    .style(|_theme| text::Style {
                        color: Some(Color::from_rgb(0.6, 0.6, 0.6))
                    }),
                ]
                .align_y(Alignment::Center),
            )
        } else {
            Element::from(Space::new().height(0))
        },
        Space::new().height(6),
        row![
            text(lang.tr("profiles_interval_presets")).size(12),
            Space::new().width(8),
            button(text("12h").size(11))
                .on_press(Message::UpdateSubscriptionInterval("12".to_string()))
                .padding([4, 8])
                .style(button::secondary),
            button(text("24h").size(11))
                .on_press(Message::UpdateSubscriptionInterval("24".to_string()))
                .padding([4, 8])
                .style(button::secondary),
            button(text("48h").size(11))
                .on_press(Message::UpdateSubscriptionInterval("48".to_string()))
                .padding([4, 8])
                .style(button::secondary),
            button(text("168h").size(11))
                .on_press(Message::UpdateSubscriptionInterval("168".to_string()))
                .padding([4, 8])
                .style(button::secondary),
        ]
        .align_y(Alignment::Center),
        Space::new().height(10),
        row![
            subscription_save_action,
            Space::new().width(10),
            subscription_update_now_action
        ]
        .align_y(Alignment::Center)
    ]);

    let mut profiles_list = column![].spacing(12);
    let profile_filter = state.profiles_filter.trim().to_lowercase();
    let filtered_profiles: Vec<_> = state
        .profiles
        .iter()
        .filter(|profile| {
            if profile_filter.is_empty() {
                return true;
            }
            profile.name.to_lowercase().contains(&profile_filter)
                || profile
                    .path
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&profile_filter)
        })
        .collect();
    if state.is_loading_profiles {
        profiles_list = profiles_list.push(text(lang.tr("loading_profiles")));
    } else if state.profiles.is_empty() {
        profiles_list = profiles_list.push(text(lang.tr("no_profiles")));
    } else if filtered_profiles.is_empty() {
        profiles_list = profiles_list.push(text(lang.tr("profiles_no_match")));
    } else {
        for profile in filtered_profiles {
            let is_active = profile.active;

            let mut actions = row![].spacing(8);
            if !is_active {
                actions = actions.push(
                    button(text(lang.tr("use")).size(12))
                        .on_press(Message::SetActiveProfile(profile.name.clone()))
                        .padding([6, 12])
                        .style(button::secondary),
                );
            } else {
                let tag = lang.tr("active_tag").trim().to_string();
                actions = actions.push(
                    container(text(tag).size(10).font(bold_font))
                        .padding([4, 8])
                        .style(|_theme| container::Style {
                            background: Some(Color::from_rgb(0.2, 0.6, 0.2).into()),
                            border: iced::Border {
                                radius: 4.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }),
                );
            }

            actions = actions.push(
                button(
                    row![text(icons::EDITOR).size(12), text(lang.tr("edit")).size(12)].spacing(8),
                )
                .on_press(Message::EditProfile(profile.path.clone()))
                .padding([6, 12])
                .style(button::secondary),
            );

            if !is_active {
                actions = actions.push(
                    button(
                        row![
                            text(icons::DELETE).size(12),
                            text(lang.tr("profiles_delete_btn")).size(12)
                        ]
                        .spacing(8),
                    )
                    .on_press(Message::DeleteProfile(profile.name.clone()))
                    .padding([6, 12])
                    .style(button::danger),
                );
            }

            profiles_list = profiles_list.push(
                container(
                    row![
                        column![
                            text(&profile.name).size(16).font(bold_font),
                            text(profile.path.to_string_lossy().to_string())
                                .size(12)
                                .style(|_theme| text::Style {
                                    color: Some(Color::from_rgb(0.5, 0.5, 0.5))
                                }),
                        ]
                        .width(Length::Fill),
                        actions
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(15)
                .style(move |_theme| container::Style {
                    background: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.05).into()),
                    border: iced::Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                }),
            );
        }
    }

    let content = column![
        header,
        Space::new().height(20),
        import_section,
        Space::new().height(16),
        local_import_section,
        Space::new().height(16),
        subscription_section,
        Space::new().height(20),
        profiles_list,
        Space::new().height(40),
    ]
    .spacing(10);

    modern_scrollable(content).height(Length::Fill).into()
}

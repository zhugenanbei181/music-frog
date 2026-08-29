use crate::locales::{Lang, Localizer};
use crate::view::components::{
    BadgeKind, card, chip, empty_state, icon_button, modern_scrollable, section_header,
    segmented_control,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CARD, R_CONTROL, SP_MD, tokens};
use crate::{AppState, Message};
use chrono::{DateTime, Local, Utc};
use iced::widget::{Space, button, column, container, pick_list, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

// ---------------------------------------------------------------------------
// Token-driven control styles (ui-wave2-r)
// ---------------------------------------------------------------------------

fn style_accent(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.accent),
        button::Status::Hovered | button::Status::Pressed => {
            (Color { a: 0.85, ..tk.accent }, tk.on_accent)
        }
        _ => (tk.accent, tk.on_accent),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

fn style_ghost(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    button::Style {
        background: match status {
            button::Status::Hovered | button::Status::Pressed => Some(tk.control_bg.into()),
            _ => None,
        },
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
        text_color: match status {
            button::Status::Disabled => tk.text_tertiary,
            button::Status::Hovered | button::Status::Pressed => tk.text_primary,
            _ => tk.text_secondary,
        },
        ..Default::default()
    }
}

fn style_danger(t: &Theme, status: button::Status) -> button::Style {
    let tk = tokens(t);
    let (bg, fg) = match status {
        button::Status::Disabled => (tk.accent_soft, tk.text_tertiary),
        button::Status::Hovered | button::Status::Pressed => {
            (Color { a: 0.24, ..tk.danger }, tk.on_accent)
        }
        _ => (Color { a: 0.14, ..tk.danger }, tk.danger),
    };
    button::Style {
        background: Some(bg.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        text_color: fg,
        ..Default::default()
    }
}

/// Text push button: `on_press == None` renders the disabled state.
fn text_btn<'a>(
    label: String,
    style: fn(&Theme, button::Status) -> button::Style,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    button(text(label).size(12).font(FONT_MEDIUM))
        .padding([7, 14])
        .style(style)
        .on_press_maybe(on_press)
        .into()
}

fn input_style(t: &Theme, status: text_input::Status) -> text_input::Style {
    let tk = tokens(t);
    let (border_color, border_width) = match status {
        text_input::Status::Focused { .. } => (tk.accent, 1.5),
        _ => (tk.card_border, 1.0),
    };
    text_input::Style {
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: border_width,
            color: border_color,
        },
        icon: tk.text_tertiary,
        placeholder: tk.text_tertiary,
        value: tk.text_primary,
        selection: Color { a: 0.25, ..tk.accent },
    }
}

fn pick_style(t: &Theme, _status: pick_list::Status) -> pick_list::Style {
    let tk = tokens(t);
    pick_list::Style {
        text_color: tk.text_primary,
        placeholder_color: tk.text_tertiary,
        handle_color: tk.text_secondary,
        background: tk.control_bg.into(),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            width: 1.0,
            color: tk.card_border,
        },
    }
}

/// iOS-style toggle row: label on the left, switch on the right.
fn toggle_row<'a>(
    label: String,
    value: bool,
    on_change: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    row![
        text(label)
            .size(13)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        crate::view::components::toggle_switch(value, on_change),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

fn field_label(value: String) -> text::Text<'static> {
    text(value)
        .size(11)
        .style(|t: &Theme| text::Style {
            color: Some(tokens(t).text_secondary),
        })
}

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
    let is_zh = !matches!(state.lang.as_str(), "en-US" | "en");

    let clear_profiles_btn: Element<'_, Message> = if state.is_loading_profiles {
        text_btn(lang.tr("profiles_clearing").to_string(), style_danger, None)
    } else {
        text_btn(
            lang.tr("profiles_clear_all").to_string(),
            style_danger,
            Some(Message::ClearProfiles),
        )
    };

    let header = row![
        text(lang.tr("profiles_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(theme::SP_LG),
        svg_icons::icon_themed(Icon::Search, 14.0, |t: &Theme| tokens(t).text_tertiary),
        Space::new().width(theme::SP_SM),
        text_input(
            lang.tr("profiles_search_placeholder").as_ref(),
            &state.profiles_filter
        )
        .on_input(Message::UpdateProfilesFilter)
        .padding([8, 12])
        .size(13)
        .width(Length::Fixed(240.0))
        .style(input_style),
        Space::new().width(theme::SP_SM),
        clear_profiles_btn,
        Space::new().width(Length::Fill),
        text_btn(
            lang.tr("profiles_open_folder").to_string(),
            style_ghost,
            Some(Message::OpenConfigDir),
        ),
    ]
    .align_y(Alignment::Center);

    let import_actions: Element<'_, Message> = if state.is_importing {
        text_btn(lang.tr("profiles_importing").to_string(), style_accent, None)
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent),
                text(lang.tr("profiles_import_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .spacing(theme::SP_SM),
        )
        .padding([7, 14])
        .style(style_accent)
        .on_press(Message::ImportProfile)
        .into()
    };

    let import_section = card(
        Some(lang.tr("profiles_import_sub").to_string()),
        column![
            row![
                column![
                    field_label(lang.tr("profiles_import_name_placeholder").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(
                        lang.tr("profiles_import_name_placeholder").as_ref(),
                        &state.import_name
                    )
                    .on_input(Message::UpdateImportName)
                    .padding([8, 12])
                    .size(13)
                    .style(input_style),
                ]
                .width(Length::FillPortion(1))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![
                    field_label(lang.tr("profiles_sub_url").to_string()),
                    Space::new().height(theme::SP_XS),
                    text_input(lang.tr("profiles_sub_url").as_ref(), &state.import_url)
                        .on_input(Message::UpdateImportUrl)
                        .padding([8, 12])
                        .size(13)
                        .style(input_style),
                ]
                .width(Length::FillPortion(2))
                .spacing(theme::SP_XS),
                Space::new().width(theme::SP_MD),
                column![
                    Space::new().height(18.0),
                    import_actions,
                ]
                .spacing(theme::SP_XS),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_MD),
            toggle_row(
                lang.tr("profiles_import_activate").to_string(),
                state.import_activate,
                Message::UpdateImportActivate,
            ),
        ],
    );

    let local_import_action: Element<'_, Message> = if state.is_importing_local {
        text_btn(lang.tr("profiles_importing").to_string(), style_accent, None)
    } else {
        button(
            row![
                svg_icons::icon_themed(Icon::Plus, 14.0, |t: &Theme| tokens(t).on_accent),
                text(lang.tr("profiles_import_local_btn").to_string())
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .spacing(theme::SP_SM),
        )
        .padding([7, 14])
        .style(style_accent)
        .on_press(Message::ImportLocalProfile)
        .into()
    };

    let local_import_section = card(
        Some(lang.tr("profiles_local_import_title").to_string()),
        row![
            column![
                field_label(lang.tr("profiles_local_path_placeholder").to_string()),
                Space::new().height(theme::SP_XS),
                text_input(
                    lang.tr("profiles_local_path_placeholder").as_ref(),
                    &state.local_import_path
                )
                .on_input(Message::UpdateLocalImportPath)
                .padding([8, 12])
                .size(13)
                .style(input_style),
            ]
            .width(Length::FillPortion(2))
            .spacing(theme::SP_XS),
            Space::new().width(theme::SP_MD),
            column![
                Space::new().height(18.0),
                text_btn(
                    lang.tr("profiles_browse_btn").to_string(),
                    style_ghost,
                    Some(Message::BrowseLocalImportFile),
                ),
            ]
            .spacing(theme::SP_XS),
            Space::new().width(theme::SP_MD),
            column![
                field_label(lang.tr("profiles_local_name_placeholder").to_string()),
                Space::new().height(theme::SP_XS),
                text_input(
                    lang.tr("profiles_local_name_placeholder").as_ref(),
                    &state.local_import_name
                )
                .on_input(Message::UpdateLocalImportName)
                .padding([8, 12])
                .size(13)
                .style(input_style),
            ]
            .width(Length::FillPortion(1))
            .spacing(theme::SP_XS),
            Space::new().width(theme::SP_MD),
            column![
                Space::new().height(18.0),
                local_import_action,
            ]
            .spacing(theme::SP_XS),
        ]
        .align_y(Alignment::Center),
    );

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
        text_btn(lang.tr("profiles_saving_subscription").to_string(), style_accent, None)
    } else {
        text_btn(
            lang.tr("profiles_save_subscription").to_string(),
            style_accent,
            Some(Message::SaveSubscriptionSettings),
        )
    };

    let subscription_update_now_action: Element<'_, Message> = if state.is_updating_subscription_now
    {
        text_btn(lang.tr("profiles_updating_subscription").to_string(), style_ghost, None)
    } else {
        text_btn(
            lang.tr("profiles_update_now").to_string(),
            style_ghost,
            Some(Message::UpdateSubscriptionNow),
        )
    };

    // Preset chips as a segmented control over the 4 supported intervals.
    let interval_labels: Vec<String> = ["12h", "24h", "48h", "168h"]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
    let interval_selected = ["12", "24", "48", "168"]
        .iter()
        .position(|h| Some(h.to_string()) == selected_interval)
        .unwrap_or(usize::MAX);
    let interval_control = segmented_control(&interval_labels, interval_selected, |index| {
        let hours = match index {
            1 => "24",
            2 => "48",
            3 => "168",
            _ => "12",
        };
        Message::UpdateSubscriptionInterval(hours.to_string())
    });

    let subscription_section = card(
        Some(lang.tr("profiles_subscription_settings_title").to_string()),
        column![
            pick_list(
                profile_options,
                selected_profile,
                Message::SelectSubscriptionProfile
            )
            .placeholder(lang.tr("profiles_select_profile").as_ref())
            .width(Length::Fill)
            .style(pick_style),
            Space::new().height(theme::SP_MD),
            text_input(
                lang.tr("profiles_subscription_url").as_ref(),
                &state.subscription_url
            )
            .on_input(Message::UpdateSubscriptionUrl)
            .padding([8, 12])
            .size(13)
            .width(Length::Fill)
            .style(input_style),
            Space::new().height(theme::SP_MD),
            toggle_row(
                lang.tr("profiles_auto_update").to_string(),
                state.subscription_auto_update_enabled,
                Message::UpdateSubscriptionAutoUpdate,
            ),
            Space::new().height(theme::SP_SM),
            row![
                pick_list(
                    interval_options,
                    selected_interval.clone(),
                    Message::UpdateSubscriptionInterval
                )
                .placeholder(lang.tr("profiles_update_interval").as_ref())
                .text_size(13)
                .width(Length::Fixed(180.0))
                .style(pick_style),
                Space::new().width(theme::SP_MD),
                interval_control,
                Space::new().width(Length::Fill),
            ]
            .align_y(Alignment::Center),
            if selected_profile_meta.is_some() {
                Space::new().height(theme::SP_SM)
            } else {
                Space::new().width(0)
            },
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
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                        Space::new().width(theme::SP_MD),
                        text(format!(
                            "{} {}",
                            lang.tr("profiles_next_update"),
                            format_datetime(
                                profile.next_update.clone(),
                                lang.tr("profiles_not_scheduled").as_ref()
                            )
                        ))
                        .size(12)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_secondary),
                        }),
                    ]
                    .align_y(Alignment::Center),
                )
            } else {
                Element::from(Space::new().width(0))
            },
            Space::new().height(theme::SP_MD),
            row![
                subscription_save_action,
                Space::new().width(theme::SP_MD),
                subscription_update_now_action,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(theme::SP_SM),
    );

    let mut profiles_list = column![].spacing(SP_MD);
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
        profiles_list = profiles_list.push(
            empty_state(Icon::FileText, lang.tr("loading_profiles").as_ref(), ""),
        );
    } else if state.profiles.is_empty() {
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
                crate::view::components::badge(lang.tr("subscription").as_ref(), BadgeKind::Accent)
            } else {
                chip(if is_zh { "本地" } else { "Local" })
            };

            let mut actions = row![].spacing(theme::SP_SM);
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
                Message::EditProfile(profile.path.clone()),
            ));
            if !is_active {
                actions = actions.push(icon_button(
                    Icon::Trash2,
                    14.0,
                    Message::DeleteProfile(profile.name.clone()),
                ));
            }

            profiles_list = profiles_list.push(
                container(
                    column![
                        row![
                            column![
                                row![
                                    text(&profile.name)
                                        .size(15)
                                        .font(FONT_SEMIBOLD)
                                        .style(|t: &Theme| text::Style {
                                            color: Some(tokens(t).text_primary),
                                        }),
                                    Space::new().width(theme::SP_SM),
                                    source_badge,
                                    if is_active {
                                        Space::new().width(theme::SP_SM)
                                    } else {
                                        Space::new().width(0)
                                    },
                                    if is_active {
                                        Element::from(crate::view::components::badge(
                                            lang.tr("active_tag").trim().to_string(),
                                            BadgeKind::Success,
                                        ))
                                    } else {
                                        Element::from(Space::new().width(0))
                                    },
                                ]
                                .align_y(Alignment::Center),
                                text(profile.path.to_string_lossy().to_string())
                                    .size(11)
                                    .style(|t: &Theme| text::Style {
                                        color: Some(tokens(t).text_tertiary),
                                    }),
                            ]
                            .spacing(2)
                            .width(Length::Fill),
                            actions,
                        ]
                        .align_y(Alignment::Center),
                        if is_subscription {
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
                                    .size(11)
                                    .font(MONO)
                                    .style(|t: &Theme| text::Style {
                                        color: Some(tokens(t).text_secondary),
                                    }),
                                ]
                                .align_y(Alignment::Center),
                            )
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
                        background: Some(tk.card_bg.into()),
                        border: Border {
                            radius: border::Radius::from(R_CARD),
                            width: if is_active { 1.5 } else { 1.0 },
                            color: if is_active { tk.accent } else { tk.card_border },
                        },
                        shadow: tk.card_shadow,
                        ..Default::default()
                    }
                }),
            );
        }
    }

    let profiles_section = card(
        None,
        column![
            section_header(
                "PROFILES",
                Some(
                    text(format!("{}", state.profiles.len()))
                        .size(12)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(tokens(t).text_tertiary),
                        })
                        .into(),
                ),
            ),
            Space::new().height(theme::SP_MD),
            profiles_list,
        ],
    );

    let content = column![
        header,
        Space::new().height(theme::SP_LG),
        import_section,
        Space::new().height(SP_MD),
        local_import_section,
        Space::new().height(SP_MD),
        subscription_section,
        Space::new().height(theme::SP_LG),
        profiles_section,
        Space::new().height(theme::SP_XL),
    ]
    .spacing(10);

    modern_scrollable(content).height(Length::Fill).into()
}

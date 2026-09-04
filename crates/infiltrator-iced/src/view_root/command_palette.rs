//! Global Command Palette (Ctrl+K) modal overlay with fuzzy matching.

use crate::state::AppState;
use crate::types::app::{CommandAction, CommandCategory, CommandItem, Route};
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, kbd_badge, modern_scrollable};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::fuzzy_search::pinyin_fuzzy_match;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn build_all_commands(state: &AppState) -> Vec<(CommandItem, Icon)> {
    let mut items = vec![
        // Navigation
        (
            CommandItem {
                id: "nav_overview".into(),
                title_key: "cmd_nav_overview",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("1"),
                action: CommandAction::Navigate(Route::Overview),
            },
            Icon::Activity,
        ),
        (
            CommandItem {
                id: "nav_proxies".into(),
                title_key: "cmd_nav_proxies",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("2"),
                action: CommandAction::Navigate(Route::Proxies),
            },
            Icon::Server,
        ),
        (
            CommandItem {
                id: "nav_rules".into(),
                title_key: "cmd_nav_rules",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("3"),
                action: CommandAction::Navigate(Route::Rules),
            },
            Icon::Target,
        ),
        (
            CommandItem {
                id: "nav_runtime".into(),
                title_key: "cmd_nav_connections",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("4"),
                action: CommandAction::Navigate(Route::Runtime),
            },
            Icon::Network,
        ),
        (
            CommandItem {
                id: "nav_dns".into(),
                title_key: "cmd_nav_dns",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("5"),
                action: CommandAction::Navigate(Route::Dns),
            },
            Icon::Globe,
        ),
        (
            CommandItem {
                id: "nav_sync".into(),
                title_key: "cmd_nav_sync",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("6"),
                action: CommandAction::Navigate(Route::Sync),
            },
            Icon::RefreshCw,
        ),
        (
            CommandItem {
                id: "nav_editor".into(),
                title_key: "cmd_nav_editor",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("7"),
                action: CommandAction::Navigate(Route::Editor),
            },
            Icon::FileText,
        ),
        (
            CommandItem {
                id: "nav_settings".into(),
                title_key: "cmd_nav_settings",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("8"),
                action: CommandAction::Navigate(Route::Settings),
            },
            Icon::Settings,
        ),
        (
            CommandItem {
                id: "nav_app_routing".into(),
                title_key: "nav_app_routing",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("9"),
                action: CommandAction::Navigate(Route::AppRouting),
            },
            Icon::LayoutGrid,
        ),
        (
            CommandItem {
                id: "nav_doctor".into(),
                title_key: "nav_doctor",
                category: CommandCategory::Navigation,
                shortcut_hint: Some("0"),
                action: CommandAction::Navigate(Route::Doctor),
            },
            Icon::ListChecks,
        ),
        // Modes
        (
            CommandItem {
                id: "mode_rule".into(),
                title_key: "cmd_mode_rule",
                category: CommandCategory::Modes,
                shortcut_hint: None,
                action: CommandAction::SetMode("rule".into()),
            },
            Icon::Target,
        ),
        (
            CommandItem {
                id: "mode_global".into(),
                title_key: "cmd_mode_global",
                category: CommandCategory::Modes,
                shortcut_hint: None,
                action: CommandAction::SetMode("global".into()),
            },
            Icon::Globe,
        ),
        (
            CommandItem {
                id: "mode_direct".into(),
                title_key: "cmd_mode_direct",
                category: CommandCategory::Modes,
                shortcut_hint: None,
                action: CommandAction::SetMode("direct".into()),
            },
            Icon::Zap,
        ),
        // Actions
        (
            CommandItem {
                id: "action_toggle_sysproxy".into(),
                title_key: "cmd_action_toggle_sysproxy",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::ToggleSystemProxy,
            },
            Icon::Plug,
        ),
        (
            CommandItem {
                id: "action_toggle_tun".into(),
                title_key: "cmd_action_toggle_tun",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::ToggleTun,
            },
            Icon::Shield,
        ),
        (
            CommandItem {
                id: "toggle_mini_hud".into(),
                title_key: "command_mini_hud",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::ToggleMiniHud,
            },
            Icon::Activity,
        ),
        (
            CommandItem {
                id: "action_flush_fakeip".into(),
                title_key: "cmd_action_flush_fakeip",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::FlushFakeIp,
            },
            Icon::Trash2,
        ),
        (
            CommandItem {
                id: "action_speed_test_all".into(),
                title_key: "cmd_action_speed_test_all",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::SpeedTestAll,
            },
            Icon::Zap,
        ),
        (
            CommandItem {
                id: "action_close_all_conns".into(),
                title_key: "cmd_action_close_all_conns",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::CloseAllConnections,
            },
            Icon::X,
        ),
        (
            CommandItem {
                id: "action_restart_kernel".into(),
                title_key: "cmd_action_restart_kernel",
                category: CommandCategory::Actions,
                shortcut_hint: None,
                action: CommandAction::RestartKernel,
            },
            Icon::RefreshCw,
        ),
    ];

    // Profile switching items
    for p in &state.profile.profiles {
        let name = p.name.clone();
        items.push((
            CommandItem {
                id: format!("profile_{name}"),
                title_key: "cmd_cat_profiles",
                category: CommandCategory::Profiles,
                shortcut_hint: None,
                action: CommandAction::SwitchProfile(name.clone()),
            },
            Icon::Pin,
        ));
    }

    items
}

pub fn command_palette_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let all_commands = build_all_commands(state);
    let query = state.shell.command_query.trim();

    let mut filtered_items = Vec::new();
    for (item, icon) in all_commands {
        let title_translated = if item.category == CommandCategory::Profiles {
            if let CommandAction::SwitchProfile(ref p_name) = item.action {
                format!("{}: {}", lang.tr("cmd_cat_profiles"), p_name)
            } else {
                lang.tr(item.title_key).to_string()
            }
        } else {
            lang.tr(item.title_key).to_string()
        };

        if query.is_empty()
            || pinyin_fuzzy_match(&title_translated, query)
            || pinyin_fuzzy_match(&item.id, query)
        {
            filtered_items.push((item, icon, title_translated));
        }
    }

    // Modal Search Input header
    let search_row = row![
        icon_themed(Icon::Search, 18.0, |t: &Theme| tokens(t).accent),
        Space::new().width(theme::SP_SM),
        text_input(
            &lang.tr("cmd_palette_placeholder"),
            &state.shell.command_query,
        )
        .on_input(Message::SetCommandQuery)
        .on_submit(
            if let Some((first_item, _, _)) = filtered_items.first() {
                Message::ExecuteCommand(first_item.action.clone())
            } else {
                Message::Noop
            }
        )
        .padding([8, 12])
        .size(14)
        .width(Length::Fill)
        .style(|t: &Theme, _status| {
            let tk = tokens(t);
            text_input::Style {
                background: Color::TRANSPARENT.into(),
                border: Border::default(),
                icon: tk.text_tertiary,
                placeholder: tk.text_tertiary,
                value: tk.text_primary,
                selection: Color { a: 0.25, ..tk.accent },
            }
        }),
        button(icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).text_tertiary))
            .style(crate::view::components::style_ghost)
            .padding(4)
            .on_press(Message::CloseCommandPalette),
    ]
    .align_y(Alignment::Center);

    let search_container = container(search_row)
        .padding([12, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius { top_left: 12.0, top_right: 12.0, bottom_right: 0.0, bottom_left: 0.0 },
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    // Items list
    let list_element: Element<'_, Message> = if filtered_items.is_empty() {
        container(
            column![
                icon_themed(Icon::Search, 24.0, |t: &Theme| tokens(t).text_tertiary),
                Space::new().height(theme::SP_SM),
                text(lang.tr("cmd_no_results"))
                    .size(13)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_tertiary),
                    }),
            ]
            .align_x(Alignment::Center),
        )
        .padding(32)
        .width(Length::Fill)
        .align_x(Alignment::Center)
        .into()
    } else {
        let mut list_col = column![].spacing(4);
        let selected_idx = state
            .shell
            .command_selected_index
            .min(filtered_items.len().saturating_sub(1));

        for (idx, (item, icon, title_str)) in filtered_items.into_iter().enumerate() {
            let is_selected = idx == selected_idx;
            let cat_label = match item.category {
                CommandCategory::Navigation => lang.tr("cmd_cat_nav"),
                CommandCategory::Modes => lang.tr("cmd_cat_modes"),
                CommandCategory::Actions => lang.tr("cmd_cat_actions"),
                CommandCategory::Profiles => lang.tr("cmd_cat_profiles"),
            };
            let cat_badge_kind = match item.category {
                CommandCategory::Navigation => BadgeKind::Accent,
                CommandCategory::Modes => BadgeKind::Success,
                CommandCategory::Actions => BadgeKind::Warning,
                CommandCategory::Profiles => BadgeKind::Neutral,
            };

            let mut item_row = row![
                icon_themed(icon, 16.0, move |t: &Theme| {
                    if is_selected {
                        tokens(t).accent
                    } else {
                        tokens(t).text_secondary
                    }
                }),
                Space::new().width(theme::SP_SM),
                text(title_str)
                    .size(13)
                    .font(if is_selected {
                        FONT_SEMIBOLD
                    } else {
                        FONT_MEDIUM
                    })
                    .style(move |t: &Theme| text::Style {
                        color: Some(if is_selected {
                            tokens(t).text_primary
                        } else {
                            tokens(t).text_secondary
                        }),
                    })
                    .width(Length::Fill),
                badge(&*cat_label, cat_badge_kind),
            ]
            .align_y(Alignment::Center);

            if let Some(hint) = item.shortcut_hint {
                item_row = item_row
                    .push(Space::new().width(theme::SP_XS))
                    .push(kbd_badge(hint));
            }

            let item_btn = button(item_row)
                .padding([8, 12])
                .width(Length::Fill)
                .style(move |t: &Theme, status| {
                    let tk = tokens(t);
                    let is_hovered =
                        matches!(status, button::Status::Hovered | button::Status::Pressed);
                    let bg = if is_selected || is_hovered {
                        Color {
                            a: 0.12,
                            ..tk.accent
                        }
                    } else {
                        Color::TRANSPARENT
                    };
                    button::Style {
                        background: Some(bg.into()),
                        border: Border {
                            radius: 8.0.into(),
                            width: if is_selected { 1.0 } else { 0.0 },
                            color: if is_selected {
                                Color {
                                    a: 0.35,
                                    ..tk.accent
                                }
                            } else {
                                Color::TRANSPARENT
                            },
                        },
                        text_color: tk.text_primary,
                        ..Default::default()
                    }
                })
                .on_press(Message::ExecuteCommand(item.action));

            list_col = list_col.push(item_btn);
        }

        modern_scrollable(list_col)
            .height(Length::Fixed(280.0))
            .into()
    };

    // Footer with keyboard navigation badges
    let footer = row![
        row![
            kbd_badge("↑↓"),
            Space::new().width(theme::SP_XS),
            text(lang.tr("cmd_palette_hint_nav"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(theme::SP_MD),
        row![
            kbd_badge("↵"),
            Space::new().width(theme::SP_XS),
            text(lang.tr("cmd_palette_hint_select"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        ]
        .align_y(Alignment::Center),
        Space::new().width(theme::SP_MD),
        row![
            kbd_badge("ESC"),
            Space::new().width(theme::SP_XS),
            text(lang.tr("cmd_palette_hint_close"))
                .size(11)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                }),
        ]
        .align_y(Alignment::Center),
    ]
    .align_y(Alignment::Center);

    let footer_container = container(footer)
        .padding([10, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius { top_left: 0.0, top_right: 0.0, bottom_right: 12.0, bottom_left: 12.0 },
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    let dialog_content = column![
        search_container,
        container(list_element).padding(12).width(Length::Fill),
        footer_container,
    ];

    let card = container(dialog_content)
        .width(Length::Fixed(560.0))
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: 12.0.into(),
                    width: 1.0,
                    color: tk.card_border,
                },
                shadow: tk.floating_shadow,
                text_color: Some(tk.text_primary),
                ..Default::default()
            }
        });

    container(container(card).center_x(Length::Fill).center_y(Length::Fill))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_t: &Theme| container::Style {
            background: Some(Color { a: 0.45, ..Color::BLACK }.into()),
            ..Default::default()
        })
        .into()
}

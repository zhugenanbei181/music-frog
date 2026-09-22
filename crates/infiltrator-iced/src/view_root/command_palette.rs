//! Global Command Palette (Ctrl+K) modal overlay with fuzzy matching.
//!
//! The catalogue, the category vocabulary and the typed targets are the shared
//! contract (`infiltrator_contract::command_catalogue`); this module is the
//! Iced projection only: icons, localized copy, pinyin matching on top of the
//! shared substring rule, and dispatch into the same handlers the global
//! chords use.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, kbd_badge, modern_scrollable};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, tokens};
use iced::widget::{Space, button, column, container, row, text, text_input};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::command_catalogue::{CommandCategory, CommandTarget, ShellPage};
use infiltrator_shared::locales::{Lang, Localizer};

/// The Iced icon for a shared command target (presentation only).
pub fn command_icon(target: &CommandTarget) -> Icon {
    match target {
        CommandTarget::Navigate(page) => match page {
            ShellPage::Overview => Icon::Activity,
            ShellPage::Proxies => Icon::Server,
            ShellPage::Profiles => Icon::FileText,
            ShellPage::Rules => Icon::Target,
            ShellPage::Connections => Icon::Network,
            ShellPage::Logs => Icon::ListChecks,
            ShellPage::Dns => Icon::Globe,
            ShellPage::Doctor => Icon::ListChecks,
            ShellPage::AppRouting => Icon::LayoutGrid,
            ShellPage::Sync => Icon::RefreshCw,
            ShellPage::Settings => Icon::Settings,
        },
        CommandTarget::SetProxyMode(mode) => match mode {
            ProxyMode::Rule => Icon::Target,
            ProxyMode::Global => Icon::Globe,
            ProxyMode::Direct => Icon::Zap,
            ProxyMode::Script => Icon::Code2,
        },
        CommandTarget::SwitchProfile { .. } => Icon::Pin,
        CommandTarget::ToggleSystemProxy => Icon::Plug,
        CommandTarget::ToggleTun => Icon::Shield,
        CommandTarget::ToggleMiniHud => Icon::Activity,
        CommandTarget::CycleTheme => Icon::Settings,
        CommandTarget::FlushDnsCache => Icon::Trash2,
        CommandTarget::TestAllProxyGroups => Icon::Zap,
        CommandTarget::RunDoctor => Icon::ListChecks,
        CommandTarget::CloseAllConnections => Icon::X,
        CommandTarget::RestartKernel => Icon::RefreshCw,
    }
}

/// The badge tint for a shared category (presentation only).
pub fn command_category_badge(category: CommandCategory) -> BadgeKind {
    match category {
        CommandCategory::Navigation => BadgeKind::Accent,
        CommandCategory::Modes => BadgeKind::Success,
        CommandCategory::Maintenance => BadgeKind::Warning,
        CommandCategory::Appearance => BadgeKind::Neutral,
        CommandCategory::Profiles => BadgeKind::Neutral,
    }
}

/// The localized title of one catalogue row. Profile rows render the category
/// label plus the live profile name, exactly like the shared `title_zh`.
fn localized_title(
    lang: &Lang<'_>,
    entry: &infiltrator_contract::command_catalogue::CommandEntry,
) -> String {
    match entry.profile_name() {
        Some(name) => format!("{}: {}", lang.tr(entry.title_key), name),
        None => lang.tr(entry.title_key).into_owned(),
    }
}

pub fn command_palette_modal(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let filtered = state.filtered_command_indices();

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
            match filtered
                .first()
                .and_then(|index| state.shell.command_catalogue.entry(*index))
            {
                Some(entry) => Message::ExecuteCommand(entry.target.clone()),
                None => Message::Noop,
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
                selection: Color {
                    a: 0.25,
                    ..tk.accent
                },
            }
        }),
        button(icon_themed(Icon::X, 14.0, |t: &Theme| tokens(t).text_tertiary))
            .style(crate::view::components::style_ghost)
            .padding(4)
            .on_press(Message::CloseCommandPalette),
    ]
    .align_y(Alignment::Center);

    // DUAL-15-10/11: the query line is a shared semantics row; Iced cannot
    // publish roles, so it carries the localized label as a real tooltip on
    // the search row (the same key Bevy mounts on the query node).
    let search_row = crate::accessibility::labelled(
        ShellA11yNode::CommandPaletteQuery,
        &state.shell.lang,
        search_row.into(),
    );

    let search_container = container(search_row)
        .padding([12, 16])
        .width(Length::Fill)
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.control_bg.into()),
                border: Border {
                    radius: border::Radius {
                        top_left: 12.0,
                        top_right: 12.0,
                        bottom_right: 0.0,
                        bottom_left: 0.0,
                    },
                    width: 0.0,
                    color: Color::TRANSPARENT,
                },
                ..Default::default()
            }
        });

    // Items list
    let list_element: Element<'_, Message> = if filtered.is_empty() {
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
            .min(filtered.len().saturating_sub(1));

        for (position, index) in filtered.into_iter().enumerate() {
            let Some(entry) = state.shell.command_catalogue.entry(index) else {
                continue;
            };
            let is_selected = position == selected_idx;
            let title_str = localized_title(&lang, entry);
            let cat_label = lang.tr(entry.category.label_key());
            let cat_badge_kind = command_category_badge(entry.category);
            let icon = command_icon(&entry.target);
            let target = entry.target.clone();
            // A global-chord action renders the live registry accelerator
            // instead of a decorative hint.
            let accelerator = entry
                .target
                .shortcut_action()
                .and_then(|action| state.shell.shortcut_registry.get(action))
                .map(|binding| binding.chord.display_string(false));

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

            if let Some(hint) = accelerator {
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
                .on_press(Message::ExecuteCommand(target));

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
                    radius: border::Radius {
                        top_left: 0.0,
                        top_right: 0.0,
                        bottom_right: 12.0,
                        bottom_left: 12.0,
                    },
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
        .width(Length::Fixed(
            state.shell.viewport.clamped_modal_width(560.0),
        ))
        .style(|t: &Theme| {
            let tk = tokens(t);
            container::Style {
                background: Some(tk.card_bg.into()),
                border: Border {
                    radius: 12.0.into(),
                    width: theme::HAIRLINE,
                    color: tk.card_border,
                },
                shadow: tk.floating_shadow,
                text_color: Some(tk.text_primary),
                ..Default::default()
            }
        });

    container(
        container(card)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_t: &Theme| container::Style {
        background: Some(
            Color {
                a: 0.45,
                ..Color::BLACK
            }
            .into(),
        ),
        ..Default::default()
    })
    .into()
}

//! iOS-style control sidebar (Clash Party reference).
//!
//! Top-to-bottom: app header (logo tile + version + refresh/settings actions),
//! proxy-mode segmented control, system-proxy / TUN toggle cards, active profile
//! card with traffic usage progress, shortcut matrix grid (Proxies, Rules,
//! Runtime, DNS) with count badges, live speed footer with mini sparkline
//! waveform, and the main nav entries (Overview, Sync, Settings) separated by
//! a hairline divider. Every color comes from [`crate::view::theme::tokens`]
//! so light, dark, forest, and AMOLED themes are equally first-class.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, card_surface, icon_button, nav_button, segmented_control,
    toggle_switch,
};
use crate::view::waveform::mini_waveform;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CARD, R_CONTROL};
use iced::widget::{Space, button, column, container, progress_bar, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::{Lang, Localizer};

/// Sidebar width (~260–280 band) so every card wraps gracefully.
const SIDEBAR_WIDTH: f32 = 272.0;

/// Canonical mihomo proxy-mode identifiers, in segmented-control order.
/// The Script segment only appears when the running core reports a
/// top-level `script:` block (see `RuntimeState::script_block_present`).
fn mode_ids(state: &AppState) -> Vec<&'static str> {
    if state.runtime.script_block_present {
        vec!["rule", "global", "direct", "script"]
    } else {
        vec!["rule", "global", "direct"]
    }
}

pub fn sidebar(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    let content = column![
        header(state),
        mode_control(state, &lang),
        toggles(state, &lang),
        profile_card(state, &lang),
        shortcut_matrix(state, &lang),
        speed_footer(state, &lang),
        divider(),
        nav_button(
            lang.tr("nav_overview").into_owned(),
            Route::Overview,
            &state.shell.current_route,
        ),
        nav_button(
            lang.tr("nav_app_routing").into_owned(),
            Route::AppRouting,
            &state.shell.current_route,
        ),
        nav_button(
            lang.tr("nav_doctor").into_owned(),
            Route::Doctor,
            &state.shell.current_route,
        ),
        nav_button(
            lang.tr("nav_sync").into_owned(),
            Route::Sync,
            &state.shell.current_route,
        ),
        nav_button(
            lang.tr("nav_settings").into_owned(),
            Route::Settings,
            &state.shell.current_route,
        ),
        Space::new().height(Length::Fill),
    ]
    .spacing(theme::SP_SM)
    .padding([theme::SP_LG, theme::SP_LG]);

    container(content)
        .width(SIDEBAR_WIDTH)
        .height(Length::Fill)
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).sidebar.into()),
            ..Default::default()
        })
        .into()
}

/// Compact 64px rail sidebar for responsive tablet or narrow desktop views.
pub const RAIL_WIDTH: f32 = 64.0;

pub fn sidebar_rail(state: &AppState) -> Element<'_, Message> {
    let routes = [
        Route::Overview,
        Route::Proxies,
        Route::Profiles,
        Route::Rules,
        Route::Runtime,
        Route::Dns,
        Route::Doctor,
        Route::AppRouting,
        Route::Sync,
        Route::Settings,
    ];

    let mut items = column![
        container(icon_themed(Icon::Server, 20.0, |t| theme::tokens(t).on_accent))
            .width(36)
            .height(36)
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .style(|t: &Theme| container::Style {
                background: Some(theme::tokens(t).accent.into()),
                border: Border {
                    radius: border::Radius::from(R_CONTROL),
                    ..Default::default()
                },
                ..Default::default()
            }),
        divider(),
    ]
    .spacing(theme::SP_SM)
    .align_x(Alignment::Center);

    for route in routes {
        items = items.push(crate::view::components::nav_rail_icon(route, &state.shell.current_route));
    }

    container(items)
        .width(RAIL_WIDTH)
        .height(Length::Fill)
        .padding([theme::SP_MD, theme::SP_XS])
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).sidebar.into()),
            ..Default::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

/// Logo mark in an accent tile + app name with version `v0.20.0`,
/// accompanied by restart/refresh and settings gear action buttons on right.
fn header(state: &AppState) -> Element<'_, Message> {
    let logo_tile = container(icon_themed(Icon::Server, 20.0, |t| {
        theme::tokens(t).on_accent
    }))
    .width(36)
    .height(36)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(|t: &Theme| container::Style {
        background: Some(theme::tokens(t).accent.into()),
        border: Border {
            radius: border::Radius::from(R_CONTROL),
            ..Default::default()
        },
        ..Default::default()
    });

    let version_str = format!("v{}", env!("CARGO_PKG_VERSION"));

    let title_col = column![
        text("MusicFrog")
            .size(15)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_primary)
            }),
        text(version_str).size(10).font(MONO).style(|t: &Theme| text::Style {
            color: Some(theme::tokens(t).text_tertiary),
        }),
    ]
    .spacing(1);

    let can_back = state.shell.history.can_go_back();
    let can_fwd = state.shell.history.can_go_forward();

    let nav_history = row![
        icon_button(
            Icon::ChevronLeft,
            13.0,
            if can_back { Message::NavigateBack } else { Message::Noop },
        ),
        icon_button(
            Icon::ChevronRight,
            13.0,
            if can_fwd { Message::NavigateForward } else { Message::Noop },
        ),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    let status_dot = container(Space::new().width(8).height(8)).style(move |t: &Theme| {
        let is_running = matches!(state.runtime.status, crate::types::runtime::RuntimeStatus::Running);
        let col = if is_running {
            theme::tokens(t).success
        } else {
            theme::tokens(t).danger
        };
        container::Style {
            background: Some(col.into()),
            border: Border {
                radius: border::Radius::from(4.0),
                ..Default::default()
            },
            ..Default::default()
        }
    });

    let actions = row![
        nav_history,
        status_dot,
        icon_button(Icon::Search, 14.0, Message::ToggleCommandPalette),
        icon_button(Icon::RefreshCw, 14.0, Message::RefreshRuntimeNow),
        icon_button(Icon::Settings, 14.0, Message::Navigate(Route::Settings)),
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center);

    row![
        logo_tile,
        Space::new().width(theme::SP_MD),
        title_col,
        Space::new().width(Length::Fill),
        actions,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill)
    .into()
}

// ---------------------------------------------------------------------------
// Proxy mode segmented control
// ---------------------------------------------------------------------------

fn mode_control<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let ids = mode_ids(state);
    let keys = [
        "proxy_mode_rule",
        "proxy_mode_global",
        "proxy_mode_direct",
        "proxy_mode_script",
    ];
    let labels: Vec<String> = ids
        .iter()
        .map(|id| {
            let key = match *id {
                "rule" => keys[0],
                "global" => keys[1],
                "direct" => keys[2],
                _ => keys[3],
            };
            lang.tr(key).into_owned()
        })
        .collect();

    let selected = state
        .runtime
        .proxy_mode
        .as_deref()
        .and_then(|mode| ids.iter().position(|id| *id == mode))
        .unwrap_or(usize::MAX);

    let ids_for_callback = ids.clone();
    let control = segmented_control(&labels, selected, move |index| {
        let mode = ids_for_callback
            .get(index)
            .copied()
            .unwrap_or(ids_for_callback[0]);
        Message::SetProxyMode(mode.to_string())
    });

    container(control).width(Length::Fill).into()
}

// ---------------------------------------------------------------------------
// Quick toggle cards
// ---------------------------------------------------------------------------

/// 系统代理 / TUN cards side by side: icon top-left, switch top-right,
/// label underneath.
fn toggles<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let system_proxy = toggle_card(
        Icon::Wifi,
        short_label(&lang.tr("system_proxy")),
        state.runtime.system_proxy_enabled,
        Message::SetSystemProxy,
    );
    let tun = toggle_card(
        Icon::Zap,
        short_label(&lang.tr("tun_mode")),
        state.runtime.tun_enabled.unwrap_or(false),
        Message::SetTunEnabled,
    );

    row![system_proxy, tun]
        .spacing(theme::SP_SM)
        .width(Length::Fill)
        .into()
}

fn toggle_card<'a>(
    glyph: Icon,
    label: String,
    on: bool,
    on_toggle: fn(bool) -> Message,
) -> Element<'a, Message> {
    let icon_element = icon_themed(glyph, 16.0, move |t: &Theme| {
        let tokens = theme::tokens(t);
        if on {
            tokens.accent
        } else {
            tokens.text_tertiary
        }
    });

    container(
        column![
            row![
                icon_element,
                Space::new().width(Length::Fill),
                toggle_switch(on, on_toggle),
            ]
            .align_y(Alignment::Center)
            .width(Length::Fill),
            Space::new().height(theme::SP_SM),
            text(label).size(11).font(FONT_MEDIUM).style(|t: &Theme| {
                text::Style {
                    color: Some(theme::tokens(t).text_secondary),
                }
            }),
        ]
        .spacing(theme::SP_XS),
    )
    .width(Length::FillPortion(1))
    .padding(theme::SP_MD)
    .style(card_surface)
    .into()
}

/// Trim an explanatory parenthetical from a locale string, e.g.
/// "系统代理 (System Proxy)" -> "系统代理" (compact-card friendly).
fn short_label(value: &str) -> String {
    value.split(" (").next().unwrap_or(value).to_string()
}

// ---------------------------------------------------------------------------
// Profile card
// ---------------------------------------------------------------------------

/// Active profile name + optional 订阅 badge with traffic usage progress bar
/// and informative subtitle.
fn profile_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let active = state.profile.profiles.iter().find(|p| p.active);
    let _is_zh = state.shell.lang.starts_with("zh");

    let (name, is_subscription, subtitle) = match active {
        Some(profile) => {
            let is_sub = profile.subscription_url.is_some();
            let sub = if is_sub {
                lang.tr("sidebar_sub_profile").to_string()
            } else {
                lang.tr("sidebar_local_profile").to_string()
            };
            (profile.name.clone(), is_sub, sub)
        }
        None => (
            lang.tr("no_profiles").into_owned(),
            false,
            lang.tr("sidebar_import_hint").to_string(),
        ),
    };

    let traffic = active.and_then(|profile| {
        let total = profile.traffic_total?;
        let used = profile.traffic_upload.unwrap_or(0) + profile.traffic_download.unwrap_or(0);
        Some((total, used))
    });

    let header_row = row![
        icon_themed(Icon::FileText, 16.0, |t| theme::tokens(t).accent),
        Space::new().width(theme::SP_SM),
        column![
            text(name).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| {
                text::Style {
                    color: Some(theme::tokens(t).text_primary),
                }
            }),
            text(subtitle).size(10).style(|t: &Theme| {
                text::Style {
                    color: Some(theme::tokens(t).text_tertiary),
                }
            }),
        ]
        .spacing(1),
        Space::new().width(Length::Fill),
        if is_subscription {
            badge(lang.tr("subscription").into_owned(), BadgeKind::Accent)
        } else {
            Space::new().width(0).into()
        },
        Space::new().width(theme::SP_XS),
        icon_themed(Icon::ChevronRight, 14.0, |t| theme::tokens(t).text_tertiary),
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    let mut body = column![header_row].spacing(theme::SP_SM);

    // Miniature usage indicator when the provider advertises traffic info.
    if let Some((total, used)) = traffic {
        let fraction = if total > 0 {
            (used as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
        body = body.push(
            column![
                progress_bar(0.0..=1.0, fraction).length(Length::Fill),
                row![
                    text(format!("{} / {}", format_gb(used), format_gb(total)))
                        .size(10)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(theme::tokens(t).text_tertiary),
                        }),
                    Space::new().width(Length::Fill),
                    text(format!("{:.0}%", fraction * 100.0))
                        .size(10)
                        .font(MONO)
                        .style(|t: &Theme| text::Style {
                            color: Some(theme::tokens(t).text_tertiary),
                        }),
                ]
                .align_y(Alignment::Center)
                .width(Length::Fill),
            ]
            .spacing(3),
        );
    }

    clickable_card(body, Message::Navigate(Route::Profiles))
}

/// Compact GB display for the sidebar usage line (falls back to MB below 1 GB).
fn format_gb(value: u64) -> String {
    let mib = value as f64 / (1024.0 * 1024.0);
    if mib >= 1024.0 {
        format!("{:.2} GB", mib / 1024.0)
    } else {
        format!("{:.0} MB", mib)
    }
}

// ---------------------------------------------------------------------------
// 2x2 Shortcut Matrix & Stat Badges
// ---------------------------------------------------------------------------

/// Proxies, Rules, Runtime (Connections), and DNS shortcut tiles organized
/// in a neat 2x2 grid with count badges.
fn shortcut_matrix<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let groups = state
        .runtime
        .proxies
        .values()
        .filter(|p| p.is_group())
        .count();

    let connections = state
        .diag
        .connections
        .as_ref()
        .map(|s| s.connections.len().to_string())
        .unwrap_or_else(|| "0".to_string());

    let rules_count = state.editor.rules.len().to_string();
    let dns_count = state.editor.dns_nameservers.len().to_string();

    let proxies_tile = shortcut_tile(
        Icon::Globe,
        &lang.tr("nav_proxies"),
        &groups.to_string(),
        Route::Proxies,
        state.shell.current_route == Route::Proxies,
    );
    let rules_tile = shortcut_tile(
        Icon::Shield,
        &lang.tr("nav_rules"),
        &rules_count,
        Route::Rules,
        state.shell.current_route == Route::Rules,
    );
    let runtime_tile = shortcut_tile(
        Icon::Activity,
        &lang.tr("nav_runtime"),
        &connections,
        Route::Runtime,
        state.shell.current_route == Route::Runtime,
    );
    let dns_tile = shortcut_tile(
        Icon::Network,
        &lang.tr("nav_dns"),
        &dns_count,
        Route::Dns,
        state.shell.current_route == Route::Dns,
    );

    column![
        row![proxies_tile, rules_tile]
            .spacing(theme::SP_SM)
            .width(Length::Fill),
        row![runtime_tile, dns_tile]
            .spacing(theme::SP_SM)
            .width(Length::Fill),
    ]
    .spacing(theme::SP_SM)
    .width(Length::Fill)
    .into()
}

fn shortcut_tile<'a>(
    icon: Icon,
    label: &str,
    count: &str,
    route: Route,
    is_active: bool,
) -> Element<'a, Message> {
    let badge_kind = if is_active {
        BadgeKind::Accent
    } else {
        BadgeKind::Neutral
    };

    let content = column![
        row![
            icon_themed(icon, 16.0, move |t: &Theme| {
                let tk = theme::tokens(t);
                if is_active {
                    tk.accent
                } else {
                    tk.text_secondary
                }
            }),
            Space::new().width(Length::Fill),
            badge(count.to_string(), badge_kind),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
        Space::new().height(theme::SP_XS),
        text(label.to_string())
            .size(11)
            .font(if is_active { FONT_SEMIBOLD } else { FONT_MEDIUM })
            .style(move |t: &Theme| text::Style {
                color: Some(if is_active {
                    theme::tokens(t).accent
                } else {
                    theme::tokens(t).text_secondary
                }),
            }),
    ]
    .spacing(2);

    button(content)
        .width(Length::FillPortion(1))
        .padding(theme::SP_MD)
        .style(move |t: &Theme, status| {
            let tokens = theme::tokens(t);
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(tokens.card_bg.into()),
                border: Border {
                    radius: border::Radius::from(R_CARD),
                    width: if is_active { 1.5 } else { 1.0 },
                    color: if is_active || hovered {
                        tokens.accent
                    } else {
                        tokens.card_border
                    },
                },
                shadow: tokens.card_shadow,
                ..Default::default()
            }
        })
        .on_press(Message::Navigate(route))
        .into()
}

// ---------------------------------------------------------------------------
// Speed footer with Mini Sparkline Waveform
// ---------------------------------------------------------------------------

/// Live up/down rates from the traffic state in mono numerals, accompanied by
/// the mini sparkline waveform for visual traffic dynamics.
fn speed_footer<'a>(state: &AppState, _lang: &Lang<'a>) -> Element<'a, Message> {
    let samples: Vec<u64> = state
        .diag
        .traffic_history
        .iter()
        .map(|(up, down)| (*up).max(*down))
        .collect();

    let (up_speed, down_speed) = match &state.diag.traffic {
        Some(t) => (t.up, t.down),
        None => (0, 0),
    };

    let speeds_col = column![
        speed_leg(Icon::ArrowUp, up_speed, |t| theme::tokens(t).success),
        speed_leg(Icon::ArrowDown, down_speed, |t| theme::tokens(t).accent),
    ]
    .spacing(2);

    let waveform = mini_waveform(&samples);

    let content = row![
        speeds_col,
        Space::new().width(Length::Fill),
        waveform,
    ]
    .align_y(Alignment::Center)
    .width(Length::Fill);

    container(content)
        .width(Length::Fill)
        .padding(theme::SP_MD)
        .style(card_surface)
        .into()
}

fn speed_leg<'a>(
    glyph: Icon,
    bytes_per_second: u64,
    color: impl Fn(&Theme) -> Color + Copy + 'a,
) -> Element<'a, Message> {
    row![
        icon_themed(glyph, 12.0, color),
        Space::new().width(theme::SP_XS),
        text(format!(
            "{}/s",
            crate::utils::format_bytes(bytes_per_second)
        ))
        .size(11)
        .font(MONO)
        .style(move |t: &Theme| text::Style {
            color: Some(color(t))
        }),
    ]
    .align_y(Alignment::Center)
    .into()
}

// ---------------------------------------------------------------------------
// Remaining navigation + hairline divider
// ---------------------------------------------------------------------------

fn divider() -> Element<'static, Message> {
    container(Space::new().height(1))
        .width(Length::Fill)
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).divider.into()),
            ..Default::default()
        })
        .into()
}

// ---------------------------------------------------------------------------
// Shared surface
// ---------------------------------------------------------------------------

/// Card-styled button used for the whole-card clickable surfaces.
fn clickable_card<'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
) -> Element<'a, Message> {
    button(content)
        .width(Length::Fill)
        .padding(theme::SP_MD)
        .style(|t: &Theme, status| {
            let tokens = theme::tokens(t);
            let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(tokens.card_bg.into()),
                text_color: tokens.text_primary,
                border: Border {
                    radius: border::Radius::from(R_CARD),
                    width: 1.0,
                    color: if hovered {
                        tokens.accent
                    } else {
                        tokens.card_border
                    },
                },
                shadow: tokens.card_shadow,
                ..Default::default()
            }
        })
        .on_press(on_press)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode_ids() {
        let (mut state, _) = AppState::new();
        state.runtime.script_block_present = false;
        assert_eq!(mode_ids(&state), vec!["rule", "global", "direct"]);

        state.runtime.script_block_present = true;
        assert_eq!(
            mode_ids(&state),
            vec!["rule", "global", "direct", "script"]
        );
    }

    #[test]
    fn test_short_label() {
        assert_eq!(short_label("系统代理 (System Proxy)"), "系统代理");
        assert_eq!(short_label("TUN 模式 (TUN Mode)"), "TUN 模式");
        assert_eq!(short_label("Simple"), "Simple");
    }

    #[test]
    fn test_format_gb() {
        assert_eq!(format_gb(500 * 1024 * 1024), "500 MB");
        assert_eq!(format_gb(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_gb(2560 * 1024 * 1024), "2.50 GB");
    }

    #[test]
    fn test_sidebar_render_smoke() {
        let (state, _) = AppState::new();
        let _elem = sidebar(&state);

        let (mut state2, _) = AppState::new();
        state2.runtime.script_block_present = true;
        state2.runtime.system_proxy_enabled = true;
        state2.runtime.tun_enabled = Some(true);
        state2.shell.current_route = Route::Proxies;
        let _elem_active = sidebar(&state2);
    }

    #[test]
    fn test_sidebar_render_with_traffic_and_samples() {
        let (mut state, _) = AppState::new();
        state.diag.traffic = Some(infiltrator_domain::runtime::TrafficData {
            up: 1024 * 50,
            down: 1024 * 1024 * 2,
        });
        state.diag.traffic_history.push_back((100, 200));
        state.diag.traffic_history.push_back((300, 800));
        state.diag.traffic_history.push_back((500, 1200));

        let _elem = sidebar(&state);
    }
}

    #[test]
    fn test_sidebar_rail_render_smoke() {
        let (state, _) = AppState::new();
        let _rail = sidebar_rail(&state);
        assert_eq!(RAIL_WIDTH, 64.0);
    }

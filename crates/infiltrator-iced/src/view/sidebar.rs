//! iOS-style control sidebar (Clash Party reference).
//!
//! Top-to-bottom: app header (logo tile + settings gear), proxy-mode
//! segmented control, system-proxy / TUN toggle cards, active profile card,
//! stat shortcut cards (proxy groups / rules), compact destination cards
//! (runtime connections / DNS), a live speed footer and the remaining nav
//! entries under a hairline divider. Every color comes from
//! [`crate::view::theme::tokens`] so light and dark are equally first-class.

use infiltrator_shared::locales::{Lang, Localizer};
use crate::types::app::Route;
use crate::view::components::{
    BadgeKind, badge, card_surface, icon_button, nav_button, segmented_control, stat_card,
    toggle_switch,
};
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, R_CARD, R_CONTROL};
use crate::state::AppState;
use crate::types::message::Message;
use iced::widget::{progress_bar, Space, button, column, container, row, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};

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
        stats_grid(state, &lang),
        compact_grid(state, &lang),
        speed_footer(state, &lang),
        divider(),
        nav_button(
            lang.tr("nav_overview").into_owned(),
            Route::Overview,
            &state.shell.current_route
        ),
        nav_button(
            lang.tr("nav_sync").into_owned(),
            Route::Sync,
            &state.shell.current_route
        ),
        nav_button(
            lang.tr("nav_settings").into_owned(),
            Route::Settings,
            &state.shell.current_route
        ),
        Space::new().height(Length::Fill),
        version_footer(),
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

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

/// Logo mark in an accent tile + app name, with the settings gear at right.
fn header(_state: &AppState) -> Element<'_, Message> {
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

    row![
        logo_tile,
        Space::new().width(theme::SP_MD),
        column![
            text("MusicFrog")
                .size(15)
                .font(FONT_SEMIBOLD)
                .style(|t: &Theme| text::Style {
                    color: Some(theme::tokens(t).text_primary)
                }),
            text("Infiltrator").size(10).style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_tertiary),
            }),
        ]
        .spacing(1),
        Space::new().width(Length::Fill),
        icon_button(Icon::Settings, 18.0, Message::Navigate(Route::Settings)),
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
    // The sidebar segment is narrow (~60px per segment), so the control uses
    // the short `proxy_mode_*` labels (规则/全局/直连/脚本); the full
    // `mode_*` names live on the overview meta and the runtime pick list.
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
        // Unknown / unset mode: no segment rendered as active.
        .unwrap_or(usize::MAX);

    let ids_for_callback = ids.clone();
    let control = segmented_control(&labels, selected, move |index| {
        let mode = ids_for_callback.get(index).copied().unwrap_or(ids_for_callback[0]);
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

/// Active profile name + optional 订阅 badge (only when the profile metadata
/// carries a subscription URL — local profiles show no badge).
fn profile_card<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let active = state.profile.profiles.iter().find(|p| p.active);
    let (name, is_subscription) = match active {
        Some(profile) => (profile.name.clone(), profile.subscription_url.is_some()),
        None => (lang.tr("no_profiles").into_owned(), false),
    };
    let traffic = active.and_then(|profile| {
        let total = profile.traffic_total?;
        let used = profile.traffic_upload.unwrap_or(0) + profile.traffic_download.unwrap_or(0);
        Some((total, used))
    });

    let mut body = column![
        row![
            icon_themed(Icon::FileText, 16.0, |t| theme::tokens(t).accent),
            Space::new().width(theme::SP_SM),
            text(name).size(13).font(FONT_SEMIBOLD).style(|t: &Theme| {
                text::Style {
                    color: Some(theme::tokens(t).text_primary),
                }
            }),
            Space::new().width(Length::Fill),
            icon_themed(Icon::ChevronRight, 14.0, |t| theme::tokens(t).text_tertiary),
        ]
        .align_y(Alignment::Center)
        .width(Length::Fill),
    ]
    .spacing(theme::SP_XS);

    if is_subscription {
        body = body.push(badge(
            lang.tr("subscription").into_owned(),
            BadgeKind::Accent,
        ));
    }

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
                text(format!("{} / {}", format_gb(used), format_gb(total)))
                    .size(10)
                    .style(|t: &Theme| text::Style {
                        color: Some(theme::tokens(t).text_tertiary),
                    }),
            ]
            .spacing(2),
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
// Stat + compact destination cards
// ---------------------------------------------------------------------------

/// 代理组 / 规则 shortcut cards (accent-outlined when their page is active).
fn stats_grid<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let tokens = theme::tokens(&state.shell.theme);
    let groups = state
        .runtime
        .proxies
        .values()
        .filter(|p| p.is_group())
        .count();

    let proxies = wrap_clickable(
        stat_card(
            Icon::Globe,
            &lang.tr("proxy_groups"),
            &groups.to_string(),
            tokens.accent,
            state.shell.current_route == Route::Proxies,
        ),
        Message::Navigate(Route::Proxies),
    );
    let rules = wrap_clickable(
        stat_card(
            Icon::Shield,
            &lang.tr("nav_rules"),
            &state.editor.rules.len().to_string(),
            tokens.accent,
            state.shell.current_route == Route::Rules,
        ),
        Message::Navigate(Route::Rules),
    );

    row![proxies, rules]
        .spacing(theme::SP_SM)
        .width(Length::Fill)
        .into()
}

/// 连接 (runtime) and DNS compact cards — the two destinations not already
/// offered by the cards above.
fn compact_grid<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let tokens = theme::tokens(&state.shell.theme);

    let connections = state
        .diag
        .connections
        .as_ref()
        .map(|snapshot| snapshot.connections.len().to_string())
        .unwrap_or_else(|| "—".to_string());
    let nameservers = if state.editor.dns_nameservers.is_empty() {
        "—".to_string()
    } else {
        state.editor.dns_nameservers.len().to_string()
    };

    let runtime = wrap_clickable(
        stat_card(
            Icon::Activity,
            &lang.tr("nav_runtime"),
            &connections,
            tokens.accent,
            state.shell.current_route == Route::Runtime,
        ),
        Message::Navigate(Route::Runtime),
    );
    let dns = wrap_clickable(
        stat_card(
            Icon::Network,
            &lang.tr("nav_dns"),
            &nameservers,
            tokens.accent,
            state.shell.current_route == Route::Dns,
        ),
        Message::Navigate(Route::Dns),
    );

    row![runtime, dns]
        .spacing(theme::SP_SM)
        .width(Length::Fill)
        .into()
}

/// Transparent button wrapper that makes an already-styled card clickable.
fn wrap_clickable<'a>(
    content: impl Into<Element<'a, Message>>,
    on_press: Message,
) -> Element<'a, Message> {
    button(content)
        .width(Length::FillPortion(1))
        .padding(0)
        .style(|_t: &Theme, _status| button::Style {
            border: Border {
                radius: border::Radius::from(R_CARD),
                ..Default::default()
            },
            ..Default::default()
        })
        .on_press(on_press)
        .into()
}

// ---------------------------------------------------------------------------
// Speed footer
// ---------------------------------------------------------------------------

/// Live up/down rates from the existing traffic state, mono numerals.
fn speed_footer<'a>(state: &AppState, lang: &Lang<'a>) -> Element<'a, Message> {
    let body: Element<'a, Message> = if let Some(traffic) = &state.diag.traffic {
        row![
            speed_leg(Icon::ArrowUp, traffic.up, |t| theme::tokens(t).success),
            speed_leg(Icon::ArrowDown, traffic.down, |t| {
                theme::tokens(t).text_secondary
            }),
        ]
        .spacing(theme::SP_SM)
        .width(Length::Fill)
        .into()
    } else {
        text(lang.tr("waiting_traffic").into_owned())
            .size(11)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_tertiary),
            })
            .into()
    };

    container(body)
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
        .size(12)
        .font(MONO)
        .style(move |t: &Theme| text::Style {
            color: Some(color(t))
        }),
    ]
    .align_y(Alignment::Center)
    .width(Length::FillPortion(1))
    .into()
}

// ---------------------------------------------------------------------------
// Remaining navigation + footer
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

fn version_footer() -> Element<'static, Message> {
    container(
        text(format!("v{}", env!("CARGO_PKG_VERSION")))
            .size(10)
            .style(|t: &Theme| text::Style {
                color: Some(theme::tokens(t).text_tertiary),
            }),
    )
    .width(Length::Fill)
    .align_x(Alignment::Center)
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

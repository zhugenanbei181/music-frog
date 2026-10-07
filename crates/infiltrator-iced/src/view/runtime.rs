//! Runtime page view: status guard, header toolbar and proxy selector live
//! here; the traffic / connections / delay / logs sections are built by the
//! sibling section modules and composed into one page-level scrollable.

use crate::view::pcap_panel::pcap_card;
use infiltrator_application::proxy_mode_projection::mode_status_copy;
use infiltrator_contract::command::ProxyMode;
use infiltrator_contract::proxy_mode::ProxyModeStatus;
use std::fmt;
use std::fmt::{Display, Formatter};
pub(crate) mod connection_search;
mod connections;
pub(crate) mod connections_controls;
mod delay;
pub(crate) mod logs;
pub(crate) mod logs_controls;
mod traffic;
pub(crate) mod traffic_legend;

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::component_card::card;
use crate::view::component_forms::{
    form_pick_style, style_accent, style_danger, style_ghost, text_btn,
};
use crate::view::components::{empty_state, icon_button, modern_scrollable, toggle_switch};
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{FONT_SEMIBOLD, SP_LG, SP_MD, tokens};
use iced::widget::{Space, column, container, pick_list, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};

/// Localized proxy-mode pick-list option: renders `label` (via `Display`)
/// while `Message::SetProxyMode` keeps carrying the raw mihomo `value`.
#[derive(Clone, PartialEq)]
struct ModeOption {
    value: &'static str,
    label: String,
}

impl Display for ModeOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);

    if !matches!(
        state.runtime.status,
        RuntimeStatus::Running | RuntimeStatus::Starting
    ) {
        let stopped = card(
            None,
            column![
                empty_state(Icon::Plug, lang.tr("proxy_not_running").as_ref(), ""),
                Space::new().height(SP_MD),
                text_btn(
                    lang.tr("start_proxy").to_string(),
                    style_accent,
                    Some(Message::StartProxy)
                ),
            ]
            .align_x(Alignment::Center),
        );
        return modern_scrollable(
            column![
                stopped,
                card(None, logs::logs_section(state, Lang(&state.shell.lang)))
            ]
            .spacing(SP_LG),
        )
        .id("runtime-scroll")
        .height(Length::Fill)
        .into();
    }

    let mode_options = vec![
        ModeOption {
            value: "rule",
            label: lang.tr("proxy_mode_rule").to_string(),
        },
        ModeOption {
            value: "global",
            label: lang.tr("proxy_mode_global").to_string(),
        },
        ModeOption {
            value: "direct",
            label: lang.tr("proxy_mode_direct").to_string(),
        },
        ModeOption {
            value: "script",
            label: lang.tr("proxy_mode_script").to_string(),
        },
    ];
    let mode_options: Vec<_> = mode_options
        .into_iter()
        .filter(|option| {
            ProxyMode::from_wire(option.value)
                .is_some_and(|mode| state.runtime.proxy_mode_state.is_mode_selectable(mode))
        })
        .collect();
    let selected_mode = mode_options
        .iter()
        .find(|option| Some(option.value) == state.runtime.proxy_mode.as_deref())
        .cloned();
    let mut runtime_group_options: Vec<String> = state
        .runtime
        .proxies
        .iter()
        .filter_map(|(name, proxy)| {
            if proxy.is_group() {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();
    runtime_group_options.sort();
    if let Some(index) = runtime_group_options
        .iter()
        .position(|group| group == "GLOBAL")
    {
        let global = runtime_group_options.remove(index);
        runtime_group_options.insert(0, global);
    }
    let selected_runtime_group = if state.runtime.runtime_selected_group.trim().is_empty() {
        None
    } else {
        Some(&state.runtime.runtime_selected_group)
    };
    let runtime_proxy_options: Vec<String> = state
        .runtime
        .proxies
        .get(&state.runtime.runtime_selected_group)
        .and_then(|proxy| proxy.all())
        .map(|all| all.to_vec())
        .unwrap_or_default();
    let selected_runtime_proxy = if state.runtime.runtime_selected_proxy.trim().is_empty() {
        None
    } else {
        Some(&state.runtime.runtime_selected_proxy)
    };

    let runtime_action_btn: Element<'_, Message> =
        if matches!(state.runtime.status, RuntimeStatus::Starting) {
            text_btn(lang.tr("status_starting").to_string(), style_ghost, None)
        } else if matches!(state.runtime.status, RuntimeStatus::Running) {
            text_btn(
                lang.tr("stop_proxy").to_string(),
                style_danger,
                Some(Message::StopProxy),
            )
        } else {
            text_btn(
                lang.tr("start_proxy").to_string(),
                style_accent,
                Some(Message::StartProxy),
            )
        };

    let header = row![
        text(lang.tr("runtime_title").to_string())
            .size(24)
            .font(FONT_SEMIBOLD)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_primary),
            }),
        Space::new().width(Length::Fill),
        text(lang.tr("proxy_mode").to_string())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(theme::SP_SM),
        if state.runtime.proxy_mode_state.status == ProxyModeStatus::Ready
            && state.runtime.pending_runtime_patch.is_none()
        {
            Element::from(
                pick_list(mode_options, selected_mode, |mode: ModeOption| {
                    Message::SetProxyMode(mode.value.to_string())
                })
                .text_size(12)
                .width(Length::Shrink)
                .style(form_pick_style),
            )
        } else {
            Element::from(
                text(mode_status_copy(
                    &state.runtime.proxy_mode_state,
                    &state.shell.lang,
                ))
                .size(12),
            )
        },
        Space::new().width(theme::SP_LG),
        text(lang.tr("runtime_auto_refresh").to_string())
            .size(12)
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            }),
        Space::new().width(theme::SP_SM),
        toggle_switch(
            state.runtime.runtime_auto_refresh,
            Message::UpdateRuntimeAutoRefresh
        ),
        Space::new().width(theme::SP_SM),
        icon_button(Icon::RefreshCw, 16.0, Message::RefreshRuntimeNow),
        Space::new().width(theme::SP_SM),
        runtime_action_btn,
    ]
    .align_y(Alignment::Center);

    let apply_proxy_enabled = !state.runtime.runtime_selected_group.trim().is_empty()
        && !state.runtime.runtime_selected_proxy.trim().is_empty();
    let apply_proxy_btn = text_btn(
        lang.tr("runtime_apply_proxy").to_string(),
        if apply_proxy_enabled {
            style_accent
        } else {
            style_ghost
        },
        apply_proxy_enabled.then_some(Message::ApplyRuntimeSelectedProxy),
    );

    let selector_body: Element<'_, Message> = if state.shell.viewport.tier.is_compact() {
        column![
            column![
                text(lang.tr("runtime_proxy_group").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                Space::new().height(theme::SP_XS),
                pick_list(
                    runtime_group_options,
                    selected_runtime_group,
                    Message::UpdateRuntimeSelectedGroup,
                )
                .width(Length::Fill)
                .text_size(12)
                .style(form_pick_style),
            ],
            column![
                text(lang.tr("runtime_proxy_node").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                Space::new().height(theme::SP_XS),
                pick_list(
                    runtime_proxy_options,
                    selected_runtime_proxy,
                    Message::UpdateRuntimeSelectedProxy,
                )
                .width(Length::Fill)
                .text_size(12)
                .style(form_pick_style),
            ],
            container(apply_proxy_btn).width(Length::Fill),
        ]
        .spacing(theme::SP_SM)
        .into()
    } else {
        row![
            column![
                text(lang.tr("runtime_proxy_group").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                Space::new().height(theme::SP_XS),
                pick_list(
                    runtime_group_options,
                    selected_runtime_group,
                    Message::UpdateRuntimeSelectedGroup,
                )
                .width(Length::FillPortion(1))
                .text_size(12)
                .style(form_pick_style),
            ]
            .width(Length::FillPortion(1)),
            Space::new().width(theme::SP_MD),
            column![
                text(lang.tr("runtime_proxy_node").to_string())
                    .size(11)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }),
                Space::new().height(theme::SP_XS),
                pick_list(
                    runtime_proxy_options,
                    selected_runtime_proxy,
                    Message::UpdateRuntimeSelectedProxy,
                )
                .width(Length::FillPortion(1))
                .text_size(12)
                .style(form_pick_style),
            ]
            .width(Length::FillPortion(1)),
            Space::new().width(theme::SP_MD),
            container(apply_proxy_btn).align_y(Alignment::Center),
        ]
        .align_y(Alignment::Center)
        .into()
    };
    let runtime_proxy_selector = card(None, selector_body);

    let content = column![
        header,
        runtime_proxy_selector,
        traffic::traffic_section(state, Lang(&state.shell.lang)),
        pcap_card(state, &lang),
        card(
            None,
            connections::connections_section(state, Lang(&state.shell.lang))
        ),
        card(None, delay::delay_section(state, Lang(&state.shell.lang))),
        card(None, logs::logs_section(state, Lang(&state.shell.lang))),
    ]
    .spacing(SP_LG);

    // Page-level scrolling (same idiom as overview/dns): sections keep their
    // natural height, so the Fill-height scrollables inside Shrink-height
    // cards can no longer collapse to blank slivers.
    modern_scrollable(content)
        .id("runtime-scroll")
        .height(Length::Fill)
        .into()
}

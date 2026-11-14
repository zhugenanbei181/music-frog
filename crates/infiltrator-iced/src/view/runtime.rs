//! Runtime page view: status guard, header toolbar and proxy selector live
//! here; the traffic / connections / delay / logs sections are built by the
//! sibling section modules and composed into one page-level scrollable.

use crate::view::pcap_panel::pcap_card;
use infiltrator_presentation::routing_intent_projection::{
    routing_intent_copy, routing_intent_status_copy,
};
use lurker_core_contract::routing_intent::{RoutingIntent, RoutingIntentStatus};
use std::fmt::{self, Display, Formatter};
pub(crate) mod connection_search;
pub(crate) mod connection_view_controls;
pub(crate) mod connections;
pub(crate) mod connections_controls;
pub(crate) mod connections_table;
mod delay;
pub(crate) mod logs;
pub(crate) mod logs_controls;
pub(crate) mod traffic;
pub(crate) mod traffic_legend;

use crate::state::{AppState, RuntimeOutboundSelectionAvailability};
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::component_card::card;
use crate::view::component_forms::{
    form_pick_style, style_accent, style_danger, style_ghost, text_btn,
};
use crate::view::components::{empty_state, icon_button, modern_scrollable, toggle_switch};
use crate::view::svg_icons::Icon;
use crate::view::theme;
use crate::view::theme::{SP_LG, SP_MD, tokens};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::{Space, column, container, pick_list, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_shared::locales::{Lang, Localizer};
use lurker_core_contract::outbound::inventory::SelectionAvailability;

/// Localized common routing choice for the pick-list.
#[derive(Clone, PartialEq)]
struct RoutingIntentOption {
    intent: RoutingIntent,
    label: String,
}

/// One runtime outbound the picker may offer.
///
/// The option carries the typed request it would dispatch, so the pick list
/// never invents a message at press time and never swallows a refusal the
/// owner published.
#[derive(Clone)]
struct RuntimeOutboundOption {
    name: String,
    message: Message,
}

impl PartialEq for RuntimeOutboundOption {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Display for RuntimeOutboundOption {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}

impl Display for RoutingIntentOption {
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
        let stopped = card(None, not_running_content(&lang));
        return modern_scrollable(
            column![
                stopped,
                container(card(
                    None,
                    logs::logs_section(state, Lang(&state.shell.lang))
                ))
                .id(InteractionRegion::LogsSection.id())
            ]
            .spacing(SP_LG),
        )
        .id("runtime-scroll")
        .height(Length::Fill)
        .into();
    }

    let routing_intent_options: Vec<_> = [
        RoutingIntent::FollowRules,
        RoutingIntent::RouteAllThroughProxy,
        RoutingIntent::BypassProxy,
    ]
    .into_iter()
    .map(|intent| RoutingIntentOption {
        label: routing_intent_copy(&intent, &Lang(&state.shell.lang)),
        intent,
    })
    .filter(|option| {
        state
            .runtime
            .routing_intent_state
            .is_intent_selectable(&option.intent)
    })
    .collect();
    let selected_routing_intent = routing_intent_options
        .iter()
        .find(|option| Some(&option.intent) == state.runtime.routing_intent.as_ref())
        .cloned();
    let mut runtime_group_options: Vec<String> = state
        .runtime
        .proxy_groups
        .iter()
        .filter(|group| group.availability == SelectionAvailability::Selectable)
        .map(|group| group.name.clone())
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
    // The owner publishes whether the selection can be built at all. The
    // picker only offers members whose typed request is ready; when the scope
    // is unobserved or the collection refuses manual selection the control is
    // drawn unavailable instead of swallowing the press.
    let runtime_outbound_availability = state.runtime_outbound_selection_availability();
    let runtime_proxy_options: Vec<RuntimeOutboundOption> = match &runtime_outbound_availability {
        RuntimeOutboundSelectionAvailability::Available(selections) => selections
            .iter()
            .map(|selection| RuntimeOutboundOption {
                name: selection.name.clone(),
                message: Message::PrepareRuntimeOutboundSelection {
                    scope: selection.scope.clone(),
                    request: selection.request.clone(),
                },
            })
            .collect(),
        RuntimeOutboundSelectionAvailability::ScopeUnobserved
        | RuntimeOutboundSelectionAvailability::SelectionNotSelectable => Vec::new(),
    };
    let runtime_outbound_selectable = matches!(
        runtime_outbound_availability,
        RuntimeOutboundSelectionAvailability::Available(_)
    );
    let selected_runtime_proxy = if state.runtime.runtime_selected_proxy.trim().is_empty() {
        None
    } else {
        runtime_proxy_options
            .iter()
            .find(|option| option.name == state.runtime.runtime_selected_proxy)
            .cloned()
    };

    let runtime_action_btn: Element<'_, Message> =
        if matches!(state.runtime.status, RuntimeStatus::Starting) {
            text_btn(lang.tr("status_starting").to_string(), style_ghost, None)
        } else if matches!(state.runtime.status, RuntimeStatus::Running) {
            text_btn(
                lang.tr("stop_core").to_string(),
                style_danger,
                Some(Message::StopCore),
            )
        } else {
            text_btn(
                lang.tr("start_core").to_string(),
                style_accent,
                Some(Message::StartCore),
            )
        };

    let header = row![
        theme::role_body_text(lang.tr("runtime_title").to_string()).style(|t: &Theme| {
            text::Style {
                color: Some(tokens(t).text_primary),
            }
        }),
        Space::new().width(Length::Fill),
        theme::role_body_text(lang.tr("routing_intent_title").to_string()).style(|t: &Theme| {
            text::Style {
                color: Some(tokens(t).text_secondary),
            }
        }),
        Space::new().width(theme::SP_SM),
        if state.runtime.routing_intent_state.status == RoutingIntentStatus::Ready
            && !state.runtime.runtime_patch_stage.is_applying()
        {
            Element::from(
                pick_list(
                    routing_intent_options,
                    selected_routing_intent,
                    |option: RoutingIntentOption| Message::SetRoutingIntent(option.intent),
                )
                .text_size(12)
                .width(Length::Shrink)
                .style(form_pick_style),
            )
        } else {
            Element::from(theme::role_body_text(routing_intent_status_copy(
                &state.runtime.routing_intent_state,
                &Lang(&state.shell.lang),
            )))
        },
        Space::new().width(theme::SP_LG),
        theme::role_body_text(lang.tr("runtime_auto_refresh").to_string()).style(|t: &Theme| {
            text::Style {
                color: Some(tokens(t).text_secondary),
            }
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

    let runtime_outbound_picker = |width: Length| -> Element<'_, Message> {
        if runtime_outbound_selectable {
            pick_list(
                runtime_proxy_options.clone(),
                selected_runtime_proxy.clone(),
                |option: RuntimeOutboundOption| option.message.clone(),
            )
            .width(width)
            .text_size(12)
            .style(form_pick_style)
            .into()
        } else {
            theme::role_body_text(
                lang.tr("runtime_outbound_selection_unavailable")
                    .to_string(),
            )
            .style(|t: &Theme| text::Style {
                color: Some(tokens(t).text_secondary),
            })
            .into()
        }
    };
    let selector_body: Element<'_, Message> = if state.shell.viewport.tier.is_compact() {
        column![
            column![
                theme::role_body_text(lang.tr("runtime_proxy_group").to_string()).style(
                    |t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                ),
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
                theme::role_body_text(lang.tr("runtime_proxy_node").to_string()).style(
                    |t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                ),
                Space::new().height(theme::SP_XS),
                runtime_outbound_picker(Length::Fill),
            ],
            container(apply_proxy_btn).width(Length::Fill),
        ]
        .spacing(theme::SP_SM)
        .into()
    } else {
        row![
            column![
                theme::role_body_text(lang.tr("runtime_proxy_group").to_string()).style(
                    |t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                ),
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
                theme::role_body_text(lang.tr("runtime_proxy_node").to_string()).style(
                    |t: &Theme| text::Style {
                        color: Some(tokens(t).text_secondary),
                    }
                ),
                Space::new().height(theme::SP_XS),
                runtime_outbound_picker(Length::FillPortion(1)),
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
        container(card(
            None,
            connections::connections_section(state, Lang(&state.shell.lang))
        ),)
        .id(InteractionRegion::ConnectionsSection.id()),
        card(None, delay::delay_section(state, Lang(&state.shell.lang))),
        container(card(
            None,
            logs::logs_section(state, Lang(&state.shell.lang))
        ))
        .id(InteractionRegion::LogsSection.id()),
    ]
    // The section stack's gutter is `SP_MD`, not `SP_LG`: the same business on
    // Slint measures a 3.5% interior band at 720x480 while this stack measured
    // 4.1%, and the difference was exactly this 16px seam. A compact window's
    // page body is a short ruler and a card stack pays for every loose gutter
    // on it (`docs/SPACE_UTILIZATION_STANDARD.md` §5, 提升密度).
    .spacing(SP_MD);

    // Page-level scrolling (same idiom as overview/dns): sections keep their
    // natural height, so the Fill-height scrollables inside Shrink-height
    // cards can no longer collapse to blank slivers.
    modern_scrollable(content)
        .id("runtime-scroll")
        .height(Length::Fill)
        .into()
}

pub(crate) fn not_running_content<'a>(lang: &Lang<'a>) -> Element<'a, Message> {
    column![
        empty_state(Icon::Plug, lang.tr("core_not_running").as_ref(), ""),
        Space::new().height(SP_MD),
        text_btn(
            lang.tr("start_core").to_string(),
            style_accent,
            Some(Message::StartCore)
        ),
    ]
    .align_x(Alignment::Center)
    .into()
}

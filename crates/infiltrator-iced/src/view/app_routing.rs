//! Per-App Split Tunneling & Process Routing page (应用级分流控制台).

use crate::state::AppState;
use crate::types::app_routing::{AppRouteRule, AppRoutingMode};
use crate::types::message::Message;
use crate::view::components::{
    BadgeKind, badge, empty_state, modern_scrollable, row_card_surface,
    search_input, section_header, segmented_control, style_accent,
};
use crate::view::svg_icons::{self, Icon};
use crate::view::theme::{self, FONT_MEDIUM, FONT_SEMIBOLD, MONO, tokens};
use iced::widget::{Space, button, column, container, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_desktop::process_enumerator::ProcessCategory;
use infiltrator_shared::locales::{Lang, Localizer};

fn category_badge_kind(cat: ProcessCategory) -> BadgeKind {
    match cat {
        ProcessCategory::Browser => BadgeKind::Accent,
        ProcessCategory::Developer => BadgeKind::Success,
        ProcessCategory::Communication => BadgeKind::Accent,
        ProcessCategory::Media => BadgeKind::Warning,
        ProcessCategory::Gaming => BadgeKind::Danger,
        ProcessCategory::Office => BadgeKind::Neutral,
        ProcessCategory::NetworkVpn => BadgeKind::Warning,
        ProcessCategory::SystemDaemon => BadgeKind::Neutral,
        ProcessCategory::Other => BadgeKind::Neutral,
    }
}

fn category_icon(cat: ProcessCategory) -> Icon {
    match cat {
        ProcessCategory::Browser => Icon::Globe,
        ProcessCategory::Developer => Icon::Code2,
        ProcessCategory::Communication => Icon::Activity,
        ProcessCategory::Media => Icon::Zap,
        ProcessCategory::Gaming => Icon::Target,
        ProcessCategory::Office => Icon::FileText,
        ProcessCategory::NetworkVpn => Icon::Network,
        ProcessCategory::SystemDaemon => Icon::Server,
        ProcessCategory::Other => Icon::LayoutGrid,
    }
}

pub fn view<'a>(state: &'a AppState) -> Element<'a, Message> {
    let lang = Lang(&state.shell.lang);

    let title_line = section_header(
        &lang.tr("app_routing_title"),
        None,
    );

    let mode_labels = vec![
        lang.tr("app_routing_mode_global").to_string(),
        lang.tr("app_routing_mode_whitelist").to_string(),
        lang.tr("app_routing_mode_blacklist").to_string(),
    ];
    let mode_idx = match state.app_routing.mode {
        AppRoutingMode::Global => 0,
        AppRoutingMode::Whitelist => 1,
        AppRoutingMode::Blacklist => 2,
    };
    let mode_switcher = segmented_control(&mode_labels, mode_idx, |idx| {
        Message::SetAppRoutingMode(match idx {
            1 => AppRoutingMode::Whitelist,
            2 => AppRoutingMode::Blacklist,
            _ => AppRoutingMode::Global,
        })
    });

    let refresh_btn = button(
        row![
            svg_icons::icon_themed(Icon::RefreshCw, 14.0, |t: &Theme| tokens(t).on_accent),
            Space::new().width(theme::SP_SM),
            text(lang.tr("app_routing_btn_refresh").to_string())
                .size(12)
                .font(FONT_MEDIUM),
        ]
        .align_y(Alignment::Center),
    )
    .padding([8, 16])
    .style(style_accent)
    .on_press_maybe((!state.app_routing.is_refreshing).then_some(Message::RefreshAppRoutingProcesses));

    let search_bar = search_input(
        lang.tr("app_routing_search").as_ref(),
        &state.app_routing.filter_query,
        Message::SetAppRoutingFilter,
        Message::SetAppRoutingFilter(String::new()),
    );

    let top_controls = row![
        container(mode_switcher).width(Length::FillPortion(3)),
        Space::new().width(theme::SP_MD),
        refresh_btn,
    ]
    .align_y(Alignment::Center);

    let query_lower = state.app_routing.filter_query.trim().to_lowercase();
    let filtered_processes: Vec<_> = state
        .app_routing
        .processes
        .iter()
        .filter(|p| {
            if let Some(cat) = state.app_routing.selected_category
                && p.category != cat
            {
                return false;
            }
            if query_lower.is_empty() {
                return true;
            }
            p.name.to_lowercase().contains(&query_lower)
                || p.display_name.to_lowercase().contains(&query_lower)
                || p.binary_path
                    .as_deref()
                    .unwrap_or("")
                    .to_lowercase()
                    .contains(&query_lower)
        })
        .collect();

    let list_content: Element<'_, Message> = if filtered_processes.is_empty() {
        empty_state(
            Icon::LayoutGrid,
            &lang.tr("app_routing_empty"),
            &lang.tr("app_routing_search"),
        )
    } else {
        let mut proc_column = column![].spacing(theme::SP_SM);
        for proc in filtered_processes {
            let cat_icon = category_icon(proc.category);
            let bkind = category_badge_kind(proc.category);

            let cur_rule = state
                .app_routing
                .custom_rules
                .get(&proc.name)
                .copied()
                .unwrap_or(AppRouteRule::Proxy);

            let direct_label = lang.tr("app_routing_direct").to_string();
            let proxy_label = lang.tr("app_routing_proxy").to_string();
            let block_label = lang.tr("app_routing_block").to_string();

            let proc_name = proc.name.clone();
            let proc_name_clone = proc_name.clone();

            let rule_switcher = segmented_control(
                &[proxy_label, direct_label, block_label],
                match cur_rule {
                    AppRouteRule::Proxy => 0,
                    AppRouteRule::Direct => 1,
                    AppRouteRule::Block => 2,
                },
                move |idx| {
                    let rule = match idx {
                        1 => AppRouteRule::Direct,
                        2 => AppRouteRule::Block,
                        _ => AppRouteRule::Proxy,
                    };
                    Message::SetAppRouteRule {
                        process: proc_name.clone(),
                        rule,
                    }
                },
            );

            let proc_card = container(
                row![
                    svg_icons::icon_themed(cat_icon, 18.0, move |t: &Theme| tokens(t).accent),
                    Space::new().width(theme::SP_MD),
                    column![
                        row![
                            text(proc.display_name.clone())
                                .size(13)
                                .font(FONT_SEMIBOLD)
                                .style(|t: &Theme| text::Style {
                                    color: Some(tokens(t).text_primary),
                                }),
                            Space::new().width(theme::SP_SM),
                            badge(proc.category.to_string(), bkind),
                        ]
                        .align_y(Alignment::Center),
                        Space::new().height(theme::SP_XS),
                        text(proc.binary_path.clone().unwrap_or_else(|| proc_name_clone.clone()))
                            .size(11)
                            .font(MONO)
                            .style(|t: &Theme| text::Style {
                                color: Some(tokens(t).text_secondary),
                            }),
                    ]
                    .width(Length::Fill),
                    container(rule_switcher).width(220),
                ]
                .align_y(Alignment::Center),
            )
            .padding([12, 16])
            .style(row_card_surface);

            proc_column = proc_column.push(proc_card);
        }
        modern_scrollable(proc_column).height(Length::Fill).into()
    };

    column![
        title_line,
        Space::new().height(theme::SP_SM),
        top_controls,
        Space::new().height(theme::SP_SM),
        crate::view::uwp_card::uwp_card(state, &lang),
        Space::new().height(theme::SP_SM),
        search_bar,
        Space::new().height(theme::SP_MD),
        list_content,
    ]
    .spacing(theme::SP_SM)
    .into()
}

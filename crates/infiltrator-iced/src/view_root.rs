//! View root for the iced desktop client: sidebar + main view routing,
//! notification toasts, operation errors, rebuild status HUD and modal dialogs.

use crate::view::component_forms::style_ghost;
use crate::view::svg_icons::{Icon, icon_themed};
use crate::view::theme::{FONT_MEDIUM, FONT_SEMIBOLD, SP_MD, SP_SM, tokens};
use infiltrator_contract::responsive_viewport::ViewportTier;
use infiltrator_contract::window_chrome::WindowChrome;
use infiltrator_shared::locales::Lang;
use modals::add_node;
use modals::confirmation::confirmation_modal;
use modals::dns_cache::cache_modal;
use modals::dns_hosts::hosts_editor_modal;
use modals::dns_query::query_modal;
use modals::log_export;
use modals::proxy_group_order::group_order_modal;
use modals::proxy_inspect::inspect_proxy_modal;
use modals::proxy_probe_settings::probe_settings_modal;
use modals::rule_provider_diff::rule_provider_diff_modal;
use modals::script_export;
use modals::snapshot_restore;
use view::chrome::chrome_strip;
use view::mini_hud::mini_hud_view;
use view::mode_issue::mode_issue;
use view::sidebar::sidebar_for_tier;
use view::{
    app_routing, dns, doctor, editor, overview, profiles, proxies, rules, runtime, settings, sync,
};
mod aggregator_modal;
mod command_palette;
mod connection_drawer;
mod custom_node_modal;
pub(crate) mod interaction_regions;
pub(crate) mod modals;
mod snapshot_diff_modal;
pub(crate) mod speedtest_detail_modal;

use crate::state::AppState;
use crate::types::app::{Route, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use crate::view;
use crate::view::theme::{HAIRLINE, R_CHIP, R_CONTROL};
use iced::advanced::widget::Id;
use iced::widget::{Space, button, column, container, row, stack, text};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_shared::locales::Localizer;
use std::time::Instant;

impl AppState {
    pub fn view(&self) -> Element<'_, Message> {
        if self.shell.mini_hud_mode {
            return mini_hud_view(self);
        }
        let tier = self.shell.viewport.tier;
        let sidebar = sidebar_for_tier(self);

        // 声明式动画进度计算
        let progress = if let Some(start) = self.shell.transition.start_time {
            let elapsed = Instant::now().duration_since(start).as_millis() as f32;
            let duration = self.shell.transition.duration.as_millis() as f32;
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // 核心性能优化：不再同时渲染两个页面。转场时只渲染新页面并做淡入。
        let page = match self.shell.current_route {
            Route::Overview => overview::view(self),
            Route::Profiles => profiles::view(self),
            Route::Proxies => proxies::view(self),
            Route::Runtime => runtime::view(self),
            Route::Rules => rules::view(self),
            Route::Dns => dns::view(self),
            Route::Sync => sync::view(self),
            Route::Editor => editor::view(self),
            Route::Settings => settings::view(self),
            Route::AppRouting => app_routing::view(self),
            Route::Doctor => doctor::view(self),
        };
        let body = if self.runtime.mode_actions.failure.is_some() {
            Element::from(column![mode_issue(self), page].spacing(8))
        } else {
            page
        };
        let main_content = container(body)
            .id(Id::new("page-content"))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(tier.content_padding_px())
            .style(move |theme: &Theme| container::Style {
                background: Some(tokens(theme).canvas.into()),
                text_color: Some(Color {
                    a: progress,
                    ..theme.palette().text
                }),
                ..Default::default()
            });

        // Compact tier stacks the bottom navigation bar under the content;
        // every wider tier keeps the vertical sidebar beside it.
        let main_view: Element<Message> = if tier == ViewportTier::Compact {
            column![main_content, sidebar].into()
        } else {
            row![sidebar, main_content].into()
        };

        // DUAL-15-13: the frameless host has no OS title bar, so the drag
        // strip and the window controls are mounted above the shell.
        let main_view: Element<Message> = column![chrome_strip(self), main_view].into();

        let mut layers: Vec<Element<Message>> = vec![main_view];
        if self.editor.script_sandbox.export_visible {
            layers.push(script_export::modal(self));
        }
        if self.diag.log_export.open {
            layers.push(log_export::modal(self));
        }
        if self.diag.dns_query.open {
            layers.push(query_modal(self));
        }
        if self.diag.dns_cache_actions.open {
            layers.push(cache_modal(self));
        }
        if self.editor.dns_hosts_editor.open {
            layers.push(hosts_editor_modal(self));
        }
        if self.runtime.group_order_open {
            layers.push(group_order_modal(self));
        }
        if self.runtime.probe_options_open {
            layers.push(probe_settings_modal(self));
        }

        if !self.shell.toasts.is_empty() {
            let mut toast_column = column![].spacing(10);
            for (content, status) in &self.shell.toasts {
                let (icon, color): (Icon, fn(&Theme) -> Color) = match status {
                    ToastStatus::Info => (Icon::Activity, |theme: &Theme| tokens(theme).accent),
                    ToastStatus::Success => {
                        (Icon::ListChecks, |theme: &Theme| tokens(theme).success)
                    }
                    ToastStatus::Warning => (Icon::Activity, |theme: &Theme| tokens(theme).warning),
                    ToastStatus::Error => (Icon::Shield, |theme: &Theme| tokens(theme).danger),
                };

                let toast_row = row![
                    icon_themed(icon, 14.0, color),
                    text(content.clone())
                        .size(13)
                        .style(|theme: &Theme| text::Style {
                            color: Some(tokens(theme).overlay_text),
                        }),
                ]
                .spacing(10)
                .align_y(Alignment::Center);

                toast_column = toast_column.push(container(toast_row).padding([10, 18]).style(
                    move |theme: &Theme| {
                        let tokens = tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: border::Radius::from(R_CONTROL),
                                width: HAIRLINE,
                                color: color(theme),
                            },
                            shadow: tokens.floating_shadow,
                            text_color: Some(tokens.overlay_text),
                            ..Default::default()
                        }
                    },
                ));
            }

            layers.push(
                container(toast_column)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(30)
                    .align_x(Alignment::End)
                    .align_y(Alignment::End)
                    .into(),
            );
        }

        if let Some(error) = &self.shell.error_msg {
            let lang = Lang(&self.shell.lang);
            let title = lang.tr("modal_op_failed");
            let dismiss = lang.tr("modal_close");
            layers.push(
                container(
                    container(
                        row![
                            icon_themed(Icon::Shield, 16.0, |theme: &Theme| tokens(theme).danger,),
                            Space::new().width(SP_SM),
                            column![
                                text(title).size(12).font(FONT_SEMIBOLD),
                                text(error.clone())
                                    .size(11)
                                    .style(|theme: &Theme| text::Style {
                                        color: Some(tokens(theme).overlay_text_muted,),
                                    })
                            ]
                            .spacing(2),
                            Space::new().width(SP_MD),
                            button(text(dismiss).size(11).font(FONT_MEDIUM))
                                .padding([4, 10])
                                .style(style_ghost)
                                .on_press(Message::ClearError),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([8, 16])
                    .style(|theme: &Theme| {
                        let tokens = tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: R_CHIP.into(),
                                width: HAIRLINE,
                                color: tokens.danger,
                            },
                            shadow: tokens.floating_shadow,
                            text_color: Some(tokens.overlay_text),
                            ..Default::default()
                        }
                    }),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([16, 24])
                .align_x(Alignment::Center)
                .align_y(Alignment::Start)
                .into(),
            );
        }

        if !matches!(self.runtime.rebuild_flow, RebuildFlowState::Idle) {
            let (icon, title, detail, color): (Icon, &str, &str, fn(&Theme) -> Color) =
                match &self.runtime.rebuild_flow {
                    RebuildFlowState::Saving { label } => (
                        Icon::RefreshCw,
                        "Saving configuration",
                        label.as_str(),
                        |theme: &Theme| tokens(theme).accent,
                    ),
                    RebuildFlowState::Rebuilding { label } => (
                        Icon::Activity,
                        "Rebuilding runtime",
                        label.as_str(),
                        |theme: &Theme| tokens(theme).warning,
                    ),
                    RebuildFlowState::Done { label } => (
                        Icon::ListChecks,
                        "Completed",
                        label.as_str(),
                        |theme: &Theme| tokens(theme).success,
                    ),
                    RebuildFlowState::Failed { label, .. } => {
                        (Icon::Shield, "Failed", label.as_str(), |theme: &Theme| {
                            tokens(theme).danger
                        })
                    }
                    RebuildFlowState::Idle => (Icon::Activity, "", "", |theme: &Theme| {
                        tokens(theme).overlay_text
                    }),
                };

            let mut info_col = column![text(title).size(12).font(FONT_SEMIBOLD)];
            if !detail.is_empty() {
                info_col =
                    info_col.push(text(detail).size(11).style(|theme: &Theme| text::Style {
                        color: Some(tokens(theme).overlay_text_muted),
                    }));
            }
            info_col = info_col.spacing(2);

            let content = row![
                icon_themed(icon, 16.0, color),
                Space::new().width(SP_SM),
                info_col,
            ]
            .align_y(Alignment::Center);

            layers.push(
                container(
                    container(content)
                        .padding([8, 18])
                        .style(move |theme: &Theme| {
                            let tokens = tokens(theme);
                            container::Style {
                                background: Some(tokens.overlay.into()),
                                border: Border {
                                    radius: border::Radius::from(R_CHIP),
                                    width: HAIRLINE,
                                    color: color(theme),
                                },
                                shadow: tokens.floating_shadow,
                                text_color: Some(tokens.overlay_text),
                                ..Default::default()
                            }
                        }),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding([16, 0])
                .align_x(Alignment::Center)
                .align_y(Alignment::Start)
                .into(),
            );
        }

        if let Some(proxy_name) = &self.runtime.inspecting_proxy {
            layers.push(inspect_proxy_modal(self, proxy_name));
        }

        if self.runtime.is_adding_custom_node {
            layers.push(add_node::custom_node_modal(self));
        }

        if let Some(diff) = &self.editor.inspecting_rule_provider_diff {
            layers.push(rule_provider_diff_modal(self, diff));
        }

        if let Some(action) = &self.shell.confirmation {
            layers.push(confirmation_modal(self, action));
        }

        if let Some(conn_id) = &self.diag.inspecting_connection_id {
            layers.push(connection_drawer::connection_drawer_modal(self, conn_id));
        }

        if self.shell.command_palette_open {
            layers.push(command_palette::command_palette_modal(self));
        }

        if self.runtime.custom_node_modal_open {
            layers.push(custom_node_modal::custom_node_modal(self));
        }

        if self.profile.aggregator_modal_open {
            layers.push(aggregator_modal::aggregator_modal(self));
        }

        if let Some(snap_id) = &self.editor.snapshot_diff_selected_id
            && self.editor.snapshot_diff_modal_open
        {
            layers.push(snapshot_diff_modal::snapshot_diff_modal(self, snap_id));
        }

        if self.diag.speedtest_detail_open {
            layers.push(speedtest_detail_modal::speedtest_detail_modal(self));
        }

        if self.diag.perf_panel_visible {
            layers.push(
                container(
                    container(
                        column![
                            text("Performance Snapshot").size(13).font(FONT_SEMIBOLD),
                            text(format!(
                                "Navigate->FirstPaint: {:?}",
                                self.diag.perf_snapshot.navigate_to_first_paint_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules cache build: {} ms",
                                self.diag.perf_snapshot.rules_cache_build_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules editor apply: {} ms",
                                self.diag.perf_snapshot.rules_with_text_apply_ms
                            ))
                            .size(11),
                            text(format!(
                                "DNS editor apply: {} ms",
                                self.diag.perf_snapshot.dns_with_text_apply_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules visible rows: {}",
                                self.diag.perf_snapshot.rules_visible_rows
                            ))
                            .size(11),
                        ]
                        .spacing(6),
                    )
                    .padding([10, 12])
                    .style(|theme: &Theme| {
                        let tokens = tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: border::Radius::from(R_CONTROL),
                                width: HAIRLINE,
                                color: tokens.overlay_border,
                            },
                            shadow: tokens.floating_shadow,
                            text_color: Some(tokens.overlay_text),
                            ..Default::default()
                        }
                    }),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .padding(iced::Padding {
                    top: 46.0,
                    right: 12.0,
                    bottom: 0.0,
                    left: 0.0,
                })
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .into(),
            );
        }

        if self.editor.snapshot_restore.visible {
            layers.push(snapshot_restore::modal(self));
        }
        let root: Element<Message> = stack(layers).into();
        if WindowChrome::FRAMELESS.needs_custom_controls() {
            container(root)
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|theme: &Theme| {
                    let tokens = tokens(theme);
                    container::Style {
                        background: Some(tokens.canvas.into()),
                        border: Border {
                            radius: border::Radius::from(R_CONTROL),
                            width: HAIRLINE,
                            color: tokens.overlay_border,
                        },
                        ..Default::default()
                    }
                })
                .into()
        } else {
            root
        }
    }
}

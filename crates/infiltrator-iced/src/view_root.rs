//! View root for the iced desktop client: sidebar + main view routing,
//! notification toasts, operation errors, rebuild status HUD and modal dialogs.

mod modals;

use crate::state::AppState;
use crate::types::app::{Route, ToastStatus};
use crate::types::message::Message;
use crate::types::runtime::RebuildFlowState;
use crate::view;
use iced::widget::{Space, button, column, container, row, stack, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};
use std::time::Instant;

impl AppState {
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = view::sidebar::sidebar(self);

        // 声明式动画进度计算
        let progress = if let Some(start) = self.shell.transition.start_time {
            let elapsed = Instant::now().duration_since(start).as_millis() as f32;
            let duration = self.shell.transition.duration.as_millis() as f32;
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // 核心性能优化：不再同时渲染两个页面。转场时只渲染新页面并做淡入。
        let main_content = container(match self.shell.current_route {
            Route::Overview => view::overview::view(self),
            Route::Profiles => view::profiles::view(self),
            Route::Proxies => view::proxies::view(self),
            Route::Runtime => view::runtime::view(self),
            Route::Rules => view::rules::view(self),
            Route::Dns => view::dns::view(self),
            Route::Sync => view::sync::view(self),
            Route::Editor => view::editor::view(self),
            Route::Settings => view::settings::view(self),
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(48)
        .style(move |theme: &Theme| container::Style {
            background: Some(crate::view::theme::tokens(theme).canvas.into()),
            text_color: Some(Color {
                a: progress,
                ..theme.palette().text
            }),
            ..Default::default()
        });

        let main_view = row![sidebar, main_content];

        let mut layers: Vec<Element<Message>> = vec![main_view.into()];

        if !self.shell.toasts.is_empty() {
            let mut toast_column = column![].spacing(10);
            for (content, status) in &self.shell.toasts {
                let (icon, color): (
                    crate::view::svg_icons::Icon,
                    fn(&Theme) -> Color,
                ) = match status {
                    ToastStatus::Info => (
                        crate::view::svg_icons::Icon::Activity,
                        |theme: &Theme| crate::view::theme::tokens(theme).accent,
                    ),
                    ToastStatus::Success => (
                        crate::view::svg_icons::Icon::ListChecks,
                        |theme: &Theme| crate::view::theme::tokens(theme).success,
                    ),
                    ToastStatus::Warning => (
                        crate::view::svg_icons::Icon::Activity,
                        |theme: &Theme| crate::view::theme::tokens(theme).warning,
                    ),
                    ToastStatus::Error => (
                        crate::view::svg_icons::Icon::Shield,
                        |theme: &Theme| crate::view::theme::tokens(theme).danger,
                    ),
                };

                let toast_row = row![
                    crate::view::svg_icons::icon_themed(icon, 14.0, color),
                    text(content.clone()).size(13).style(|theme: &Theme| text::Style {
                        color: Some(crate::view::theme::tokens(theme).overlay_text),
                    }),
                ]
                .spacing(10)
                .align_y(Alignment::Center);

                toast_column = toast_column.push(
                    container(toast_row)
                        .padding([10, 18])
                        .style(move |theme: &Theme| {
                            let tokens = crate::view::theme::tokens(theme);
                            container::Style {
                                background: Some(tokens.overlay.into()),
                                border: Border {
                                    radius: 12.0.into(),
                                    width: 1.0,
                                    color: color(theme),
                                },
                                shadow: tokens.floating_shadow,
                                text_color: Some(tokens.overlay_text),
                                ..Default::default()
                            }
                        }),
                );
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
            let is_en = self.shell.lang.starts_with("en");
            let title = if is_en {
                "Operation failed"
            } else {
                "操作失败"
            };
            let dismiss = if is_en { "Dismiss" } else { "关闭" };
            layers.push(
                container(
                    container(
                        row![
                            crate::view::svg_icons::icon_themed(
                                crate::view::svg_icons::Icon::Shield,
                                16.0,
                                |theme: &Theme| crate::view::theme::tokens(theme).danger,
                            ),
                            Space::new().width(crate::view::theme::SP_SM),
                            column![
                                text(title)
                                    .size(12)
                                    .font(crate::view::theme::FONT_SEMIBOLD),
                                text(error.clone())
                                    .size(11)
                                    .style(|theme: &Theme| text::Style {
                                        color: Some(
                                            crate::view::theme::tokens(theme).overlay_text_muted,
                                        ),
                                    })
                            ]
                            .spacing(2),
                            Space::new().width(crate::view::theme::SP_MD),
                            button(text(dismiss).size(11).font(crate::view::theme::FONT_MEDIUM))
                                .padding([4, 10])
                                .style(crate::view::components::style_ghost)
                                .on_press(Message::ClearError),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding([8, 16])
                    .style(|theme: &Theme| {
                        let tokens = crate::view::theme::tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: crate::view::theme::R_CHIP.into(),
                                width: 1.0,
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
            let (icon, title, detail, color): (
                crate::view::svg_icons::Icon,
                &str,
                &str,
                fn(&Theme) -> Color,
            ) = match &self.runtime.rebuild_flow {
                RebuildFlowState::Saving { label } => (
                    crate::view::svg_icons::Icon::RefreshCw,
                    "Saving configuration",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).accent,
                ),
                RebuildFlowState::Rebuilding { label } => (
                    crate::view::svg_icons::Icon::Activity,
                    "Rebuilding runtime",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).warning,
                ),
                RebuildFlowState::Done { label } => (
                    crate::view::svg_icons::Icon::ListChecks,
                    "Completed",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).success,
                ),
                RebuildFlowState::Failed { label, .. } => (
                    crate::view::svg_icons::Icon::Shield,
                    "Failed",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).danger,
                ),
                RebuildFlowState::Idle => (
                    crate::view::svg_icons::Icon::Activity,
                    "",
                    "",
                    |theme: &Theme| crate::view::theme::tokens(theme).overlay_text,
                ),
            };

            let mut info_col = column![text(title)
                .size(12)
                .font(crate::view::theme::FONT_SEMIBOLD)];
            if !detail.is_empty() {
                info_col = info_col.push(text(detail).size(11).style(|theme: &Theme| {
                    text::Style {
                        color: Some(crate::view::theme::tokens(theme).overlay_text_muted),
                    }
                }));
            }
            info_col = info_col.spacing(2);

            let content = row![
                crate::view::svg_icons::icon_themed(icon, 16.0, color),
                Space::new().width(crate::view::theme::SP_SM),
                info_col,
            ]
            .align_y(Alignment::Center);

            layers.push(
                container(
                    container(content)
                        .padding([8, 18])
                        .style(move |theme: &Theme| {
                            let tokens = crate::view::theme::tokens(theme);
                            container::Style {
                                background: Some(tokens.overlay.into()),
                                border: Border {
                                    radius: crate::view::theme::R_CHIP.into(),
                                    width: 1.0,
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
            layers.push(modals::inspect_proxy_modal(self, proxy_name));
        }

        if self.runtime.is_adding_custom_node {
            layers.push(modals::custom_node_modal(self));
        }

        if let Some(diff) = &self.editor.inspecting_rule_provider_diff {
            layers.push(modals::rule_provider_diff_modal(self, diff));
        }

        if let Some(action) = &self.shell.confirmation {
            layers.push(modals::confirmation_modal(self, action));
        }

        if self.diag.perf_panel_visible {
            layers.push(
                container(
                    container(
                        column![
                            text("Performance Snapshot")
                                .size(13)
                                .font(crate::view::theme::FONT_SEMIBOLD),
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
                        let tokens = crate::view::theme::tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: 10.0.into(),
                                width: 1.0,
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

        if self.shell.demo {
            self.write_capture_marker();
        }

        stack(layers).into()
    }
}

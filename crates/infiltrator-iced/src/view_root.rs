use crate::state::AppState;
use crate::types::{Message, RebuildFlowState, Route, ToastStatus};
use crate::view;
use iced::widget::{button, column, container, row, stack, text};
use iced::{Alignment, Border, Color, Element, Length, Theme};
use std::time::Instant;

impl AppState {
    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = view::sidebar::sidebar(self);

        // 声明式动画进度计算
        let progress = if let Some(start) = self.transition.start_time {
            let elapsed = Instant::now().duration_since(start).as_millis() as f32;
            let duration = self.transition.duration.as_millis() as f32;
            (elapsed / duration).clamp(0.0, 1.0)
        } else {
            1.0
        };

        // 核心性能优化：不再同时渲染两个页面。转场时只渲染新页面并做淡入。
        let main_content = container(match self.current_route {
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
        .style(move |theme: &Theme| {
            container::Style {
                background: Some(crate::view::theme::tokens(theme).canvas.into()),
                // 仅对文字颜色进行透明度插值，这在 CPU 渲染下极快
                text_color: Some(Color {
                    a: progress,
                    ..theme.palette().text
                }),
                ..Default::default()
            }
        });

        let main_view = row![sidebar, main_content];

        let mut layers: Vec<Element<Message>> = vec![main_view.into()];

        if !self.toasts.is_empty() {
            let mut toast_column = column![].spacing(10);
            for (content, status) in &self.toasts {
                let color = move |theme: &Theme| {
                    let tokens = crate::view::theme::tokens(theme);
                    match status {
                        ToastStatus::Info => tokens.accent,
                        ToastStatus::Success => tokens.success,
                        ToastStatus::Warning => tokens.warning,
                        ToastStatus::Error => tokens.danger,
                    }
                };

                toast_column = toast_column.push(
                    container(text(content.clone()).size(13))
                        .padding([12, 24])
                        .style(move |theme: &Theme| {
                            let tokens = crate::view::theme::tokens(theme);
                            container::Style {
                                background: Some(tokens.overlay.into()),
                                border: Border {
                                    radius: 12.0.into(),
                                    width: 1.0,
                                    color: color(theme),
                                },
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

        if !matches!(self.rebuild_flow, RebuildFlowState::Idle) {
            let (title, detail, color): (&str, &str, fn(&Theme) -> Color) = match &self.rebuild_flow
            {
                RebuildFlowState::Saving { label } => (
                    "Saving configuration",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).accent,
                ),
                RebuildFlowState::Rebuilding { label } => (
                    "Rebuilding runtime",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).warning,
                ),
                RebuildFlowState::Done { label } => (
                    "Completed",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).success,
                ),
                RebuildFlowState::Failed { label, .. } => (
                    "Failed",
                    label.as_str(),
                    |theme: &Theme| crate::view::theme::tokens(theme).danger,
                ),
                RebuildFlowState::Idle => (
                    "",
                    "",
                    |theme: &Theme| crate::view::theme::tokens(theme).overlay_text,
                ),
            };

            layers.push(
                container(
                    container(
                        column![
                            text(title).size(14),
                            text(detail).size(12).style(|theme: &Theme| text::Style {
                                color: Some(
                                    crate::view::theme::tokens(theme).overlay_text_muted,
                                )
                            })
                        ]
                        .spacing(4),
                    )
                    .padding([12, 18])
                    .style(move |theme: &Theme| {
                        let tokens = crate::view::theme::tokens(theme);
                        container::Style {
                            background: Some(tokens.overlay.into()),
                            border: Border {
                                radius: 10.0.into(),
                                width: 1.0,
                                color: color(theme),
                            },
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

        // FPS 诊断 HUD (调试用)。ui-fix: 默认隐藏 —— perf_panel_visible 现在
        // 同时控制 "0 FPS / Perf" 徽标与性能快照面板（此前徽标无条件渲染，
        // 出现在每张截图右上角）。Message::TogglePerfPanel 仍负责切换。
        if self.perf_panel_visible {
            let fps_counter = container(
                row![
                    text(format!("{} FPS", self.fps))
                        .size(10)
                        .style(|theme: &Theme| text::Style {
                            color: Some(crate::view::theme::tokens(theme).text_tertiary),
                        }),
                    button(text("Perf").size(10))
                        .padding([2, 8])
                        .style(button::secondary)
                        .on_press(Message::TogglePerfPanel)
                ]
                .spacing(8),
            )
            .padding(10);

            layers.push(
                container(fps_counter)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(Alignment::End)
                    .align_y(Alignment::Start)
                    .into(),
            );
        }

        if self.perf_panel_visible {
            layers.push(
                container(
                    container(
                        column![
                            text("Performance Snapshot").size(13),
                            text(format!(
                                "Navigate->FirstPaint: {:?}",
                                self.perf_snapshot.navigate_to_first_paint_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules cache build: {} ms",
                                self.perf_snapshot.rules_cache_build_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules editor apply: {} ms",
                                self.perf_snapshot.rules_with_text_apply_ms
                            ))
                            .size(11),
                            text(format!(
                                "DNS editor apply: {} ms",
                                self.perf_snapshot.dns_with_text_apply_ms
                            ))
                            .size(11),
                            text(format!(
                                "Rules visible rows: {}",
                                self.perf_snapshot.rules_visible_rows
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

        // demo-mode: emit the capture marker only after the first real view
        // pass for the requested page (write-once; see demo.rs).
        if self.demo {
            self.write_capture_marker();
        }

        stack(layers).into()
    }
}

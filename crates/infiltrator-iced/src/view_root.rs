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
            let is_dark = theme == &Theme::Dark;
            container::Style {
                background: Some(
                    if is_dark {
                        Color::from_rgb(0.02, 0.04, 0.10)
                    } else {
                        Color::from_rgb(0.98, 0.98, 1.0)
                    }
                    .into(),
                ),
                // 仅对文字颜色进行透明度插值，这在 CPU 渲染下极快
                text_color: Some(Color {
                    a: progress,
                    ..theme.palette().text
                }),
                ..Default::default()
            }
        });

        let main_view = row![sidebar, main_content];

        // FPS 诊断显示 (调试用)
        let fps_counter = container(
            row![
                text(format!("{} FPS", self.fps))
                    .size(10)
                    .style(|_| text::Style {
                        color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.2)),
                    }),
                button(text("Perf").size(10))
                    .padding([2, 8])
                    .style(button::secondary)
                    .on_press(Message::TogglePerfPanel)
            ]
            .spacing(8),
        )
        .padding(10);

        let mut layers: Vec<Element<Message>> = vec![main_view.into()];

        if !self.toasts.is_empty() {
            let mut toast_column = column![].spacing(10);
            for (content, status) in &self.toasts {
                let color = match status {
                    ToastStatus::Info => Color::from_rgb(0.2, 0.5, 0.8),
                    ToastStatus::Success => Color::from_rgb(0.2, 0.7, 0.2),
                    ToastStatus::Warning => Color::from_rgb(0.8, 0.5, 0.2),
                    ToastStatus::Error => Color::from_rgb(0.8, 0.2, 0.2),
                };

                toast_column = toast_column.push(
                    container(text(content.clone()).size(13))
                        .padding([12, 24])
                        .style(move |_| container::Style {
                            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.9).into()),
                            border: Border {
                                radius: 12.0.into(),
                                width: 1.0,
                                color,
                            },
                            text_color: Some(Color::WHITE),
                            ..Default::default()
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
            let (title, detail, color) = match &self.rebuild_flow {
                RebuildFlowState::Saving { label } => (
                    "Saving configuration",
                    label.as_str(),
                    Color::from_rgb(0.2, 0.5, 0.8),
                ),
                RebuildFlowState::Rebuilding { label } => (
                    "Rebuilding runtime",
                    label.as_str(),
                    Color::from_rgb(0.8, 0.6, 0.2),
                ),
                RebuildFlowState::Done { label } => {
                    ("Completed", label.as_str(), Color::from_rgb(0.2, 0.7, 0.2))
                }
                RebuildFlowState::Failed { label, .. } => {
                    ("Failed", label.as_str(), Color::from_rgb(0.8, 0.2, 0.2))
                }
                RebuildFlowState::Idle => ("", "", Color::WHITE),
            };

            layers.push(
                container(
                    container(
                        column![
                            text(title).size(14),
                            text(detail).size(12).style(|_| text::Style {
                                color: Some(Color::from_rgba(1.0, 1.0, 1.0, 0.8))
                            })
                        ]
                        .spacing(4),
                    )
                    .padding([12, 18])
                    .style(move |_| container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.88).into()),
                        border: Border {
                            radius: 10.0.into(),
                            width: 1.0,
                            color,
                        },
                        text_color: Some(Color::WHITE),
                        ..Default::default()
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

        // 叠加 FPS 计数器
        layers.push(
            container(fps_counter)
                .width(Length::Fill)
                .height(Length::Fill)
                .align_x(Alignment::End)
                .align_y(Alignment::Start)
                .into(),
        );

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
                    .style(|_| container::Style {
                        background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.86).into()),
                        border: Border {
                            radius: 10.0.into(),
                            width: 1.0,
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.2),
                        },
                        text_color: Some(Color::WHITE),
                        ..Default::default()
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

        stack(layers).into()
    }
}

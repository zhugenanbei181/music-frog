//! Minimal Iced desktop responsive proxy client template (PUB-005.2).

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Task, Theme};
use infiltrator_contract::command::CommandIntent;
use infiltrator_ports::command_sink::{CommandSink, RecordingCommandSink};
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::Arc;

pub fn main() -> iced::Result {
    iced::application(
        || (AppState::default(), Task::none()),
        AppState::update,
        AppState::view,
    )
    .title(AppState::title)
    .theme(AppState::theme)
    .run()
}

#[derive(Clone)]
struct AppState {
    core_status: String,
    routing_intent: RoutingIntent,
    dark_mode: bool,
    sink: Arc<dyn CommandSink>,
}

#[derive(Debug, Clone)]
enum Message {
    StartCore,
    StopCore,
    SetRoutingIntent(RoutingIntent),
    CoreStarted,
    CoreStopped,
    ToggleTheme,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            core_status: "Stopped".to_owned(),
            routing_intent: RoutingIntent::FollowRules,
            dark_mode: true,
            sink: Arc::new(RecordingCommandSink::new()),
        }
    }
}

impl AppState {
    fn title(&self) -> String {
        "Lurker Iced Desktop Template".to_owned()
    }

    fn theme(&self) -> Theme {
        if self.dark_mode {
            Theme::Dark
        } else {
            Theme::Light
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::StartCore => {
                let sink = Arc::clone(&self.sink);
                Task::perform(
                    async move {
                        let _ = sink.submit(CommandIntent::StartCore).await;
                    },
                    |_| Message::CoreStarted,
                )
            }
            Message::StopCore => {
                let sink = Arc::clone(&self.sink);
                Task::perform(
                    async move {
                        let _ = sink.submit(CommandIntent::StopCore).await;
                    },
                    |_| Message::CoreStopped,
                )
            }
            Message::SetRoutingIntent(intent) => {
                self.routing_intent = intent.clone();
                let sink = Arc::clone(&self.sink);
                Task::perform(
                    async move {
                        let _ = sink
                            .submit(CommandIntent::SetRoutingIntent { intent })
                            .await;
                    },
                    |_| Message::ToggleTheme, // No-op task completion
                )
            }
            Message::CoreStarted => {
                self.core_status = "Running".to_owned();
                Task::none()
            }
            Message::CoreStopped => {
                self.core_status = "Stopped".to_owned();
                Task::none()
            }
            Message::ToggleTheme => {
                self.dark_mode = !self.dark_mode;
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let title = text("Lurker Iced Client Template").size(24);
        let status = text(format!("Core Status: {}", self.core_status)).size(16);
        let mode = text(format!("Routing Intent: {:?}", self.routing_intent)).size(16);

        let controls = row![
            button(text("Start Core")).on_press(Message::StartCore),
            button(text("Stop Core")).on_press(Message::StopCore),
            button(text("Toggle Theme")).on_press(Message::ToggleTheme),
        ]
        .spacing(12);

        let intents = row![
            button(text("Rules")).on_press(Message::SetRoutingIntent(RoutingIntent::FollowRules)),
            button(text("Global Proxy")).on_press(Message::SetRoutingIntent(
                RoutingIntent::RouteAllThroughProxy
            )),
            button(text("Direct Bypass"))
                .on_press(Message::SetRoutingIntent(RoutingIntent::BypassProxy)),
        ]
        .spacing(12);

        let content = column![title, status, mode, controls, intents]
            .spacing(20)
            .align_x(Alignment::Center);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

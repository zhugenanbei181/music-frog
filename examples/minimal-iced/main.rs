//! Minimal Lurker Iced example (PUB-006.9).
//!
//! The example shows the shape of every Lurker surface:
//!
//! * the neutral contract owns the lifecycle vocabulary;
//! * the presentation kernel owns the projection to a renderable verdict;
//! * the surface owns nothing but the rendering and the typed user intent.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example minimal-iced
//! ```

use iced::widget::{button, column, container, row, text};
use iced::{Alignment, Element, Length, Task, Theme};
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_ui_presentation::core_status_projection::{
    OverviewBannerVerdict, project_banner_verdict,
};

fn main() -> iced::Result {
    iced::application(
        || (Surface::default(), Task::none()),
        Surface::update,
        Surface::view,
    )
    .title(Surface::title)
    .theme(Theme::Dark)
    .run()
}

/// The whole application state of the example: one typed lifecycle value.
struct Surface {
    lifecycle: CoreLifecycle,
}

impl Default for Surface {
    fn default() -> Self {
        Self {
            lifecycle: CoreLifecycle::Stopped,
        }
    }
}

/// The typed user intent of the example. Every variant is one legal step of
/// the lifecycle; no boolean encodes the phase.
#[derive(Clone, Copy, Debug)]
enum Message {
    Start,
    Stop,
}

impl Surface {
    fn title(&self) -> String {
        "Lurker minimal Iced example".to_owned()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Start => self.lifecycle = CoreLifecycle::Running,
            Message::Stop => self.lifecycle = CoreLifecycle::Stopped,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let verdict = project_banner_verdict(SurfaceOrigin::Live, &self.lifecycle, true, false);
        let header = text("Lurker minimal Iced example").size(24);
        let status = text(format!("Core lifecycle: {:?}", self.lifecycle)).size(16);
        let banner = text(format!("Banner verdict: {}", verdict_text(&verdict))).size(16);

        let controls = row![
            button(text("Start core")).on_press(Message::Start),
            button(text("Stop core")).on_press(Message::Stop),
        ]
        .spacing(12);

        let content = column![header, status, banner, controls]
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

/// Renders the shared verdict vocabulary of the presentation kernel.
fn verdict_text(verdict: &OverviewBannerVerdict) -> &'static str {
    match verdict {
        OverviewBannerVerdict::Demo => "demo fixture",
        OverviewBannerVerdict::MissingHost => "missing host",
        OverviewBannerVerdict::CoreStopped => "core stopped",
        OverviewBannerVerdict::VpnUnauthorized => "vpn unauthorized",
        OverviewBannerVerdict::RealRunning => "real running",
        OverviewBannerVerdict::Transitioning => "transitioning",
    }
}

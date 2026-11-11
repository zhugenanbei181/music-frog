//! Standalone observation page bodies built from the runtime sections.
//!
//! The runtime page and the dedicated observation pages consume the same typed
//! sections, so neither surface owns a second copy of the business view.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::runtime::RuntimeStatus;
use crate::view::components::{modern_scrollable, page_body_card};
use crate::view::runtime;
use crate::view::runtime::{connections, logs, traffic};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::widget::container;
use iced::{Element, Length};
use infiltrator_shared::locales::Lang;

pub(crate) const TRAFFIC_PAGE_SCROLL_ID: &str = "traffic-page-scroll";

pub fn connections_body<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let content: Element<'a, Message> = match &state.runtime.status {
        RuntimeStatus::Running | RuntimeStatus::Starting => {
            connections::connections_page_section(state, lang)
        }
        RuntimeStatus::Stopped | RuntimeStatus::Error(_) => runtime::not_running_content(&lang),
    };
    framed_page(InteractionRegion::ConnectionsSection, content)
}

pub fn logs_body<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    framed_page(
        InteractionRegion::LogsSection,
        logs::logs_page_section(state, lang),
    )
}

pub fn traffic_body<'a>(state: &'a AppState, lang: Lang<'a>) -> Element<'a, Message> {
    let content: Element<'a, Message> = match &state.runtime.status {
        RuntimeStatus::Running | RuntimeStatus::Starting => traffic::traffic_section(state, lang),
        RuntimeStatus::Stopped | RuntimeStatus::Error(_) => runtime::not_running_content(&lang),
    };
    container(page_body_card(
        modern_scrollable(content)
            .id(TRAFFIC_PAGE_SCROLL_ID)
            .height(Length::Fill),
    ))
    .id(InteractionRegion::TrafficPage.id())
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
pub(crate) fn framed_page<'a>(
    region: InteractionRegion,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    container(page_body_card(content))
        .id(region.id())
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

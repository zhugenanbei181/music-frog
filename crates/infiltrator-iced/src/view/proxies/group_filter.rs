//! The filter every proxy group owns.
//!
//! The reference client (Mihomo Party / Clash Party) gives every group its
//! own search box in the group header, and the box collapses to a glyph
//! until the user opens it. The Iced group header follows the same
//! reference: a collapsed glyph keeps the header as tight as the
//! reference's, and the box takes the room only while it is in use.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::message::proxy_page_intent::ProxyPageIntent;
use crate::view::components::icon_button;
use crate::view::proxies_controls::proxies_input_style;
use crate::view::svg_icons::Icon;
use crate::view::theme;
use iced::widget::{row, text_input};
use iced::{Alignment, Element, Length};
use infiltrator_shared::locales::{Lang, Localizer};

/// The filter control of one proxy group.
///
/// A group without a filter shows one glyph; a group with a filter shows
/// the box that carries the group's own query and the glyph that closes
/// it again.
pub fn group_filter<'a>(
    state: &'a AppState,
    group: &'a str,
    lang: &Lang<'a>,
) -> Element<'a, Message> {
    if !state.runtime.proxy_group_filters.is_open(group) {
        return icon_button(
            Icon::Search,
            13.0,
            Message::ProxyPageIntent(ProxyPageIntent::ToggleGroupFilter(group.to_owned())),
        );
    }
    let text = state.runtime.proxy_group_filters.text(group);
    row![
        text_input(lang.tr("proxies_search_placeholder").as_ref(), text)
            .on_input(
                move |value| Message::ProxyPageIntent(ProxyPageIntent::FilterGroup {
                    group: group.to_owned(),
                    text: value,
                })
            )
            .padding([5, 9])
            .width(Length::Fixed(140.0))
            .style(proxies_input_style),
        icon_button(
            Icon::X,
            12.0,
            Message::ProxyPageIntent(ProxyPageIntent::ToggleGroupFilter(group.to_owned())),
        ),
    ]
    .spacing(theme::SP_XS)
    .align_y(Alignment::Center)
    .into()
}

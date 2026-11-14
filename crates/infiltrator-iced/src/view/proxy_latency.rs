//! Native badge replays the shared controller-history caption and band.
use crate::view::theme;
use crate::view::theme::tokens;
use iced::widget::{button, text};
use iced::{Border, Element, Theme, border};
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_presentation::latency_projection::project_proxy_latency;
use infiltrator_shared::locales::Lang;

/// The colour the band paints its caption in.
fn band_color(band: LatencyBand, theme: &Theme) -> iced::Color {
    let palette = tokens(theme);
    match band {
        LatencyBand::Fast => palette.success,
        LatencyBand::Medium => palette.warning,
        LatencyBand::Slow => palette.danger,
        LatencyBand::NotObserved | LatencyBand::UnconfirmedZero => palette.text_tertiary,
    }
}

/// The latency badge: the shared caption, painted in its band colour.
pub fn latency_badge<'a, Message: 'a>(
    delay: Option<u32>,
    language: &Lang<'_>,
) -> Element<'a, Message> {
    let caption = project_proxy_latency(delay);
    let label = caption.render(language);
    let band = caption.band;
    theme::role_body_text(label)
        .style(move |theme: &Theme| text::Style {
            color: Some(band_color(band, theme)),
        })
        .into()
}

/// The latency badge, pressable when the owner can test that one node.
///
/// FlClash and Clash Party both let a reader test a node where its delay is
/// shown; a badge that cannot be pressed is a number, not an affordance. The
/// owner publishes the two facts this needs - the member's probe availability
/// and the typed request itself - so the surface asks for the request and
/// paints the plain badge when there is none. An engine that cannot probe,
/// a node without an engine-scoped identity, and a tree whose core has not
/// been observed all end in the same plain badge: the affordance is offered
/// exactly when it can be answered.
///
/// The pressed badge paints the same ink as the plain one. The button carries
/// no padding, no ground and no border, so the caption stays where it was and
/// the affordance is the press itself.
pub fn probeable_latency_badge<'a, Message: 'a + Clone>(
    delay: Option<u32>,
    language: &Lang<'_>,
    message: Option<Message>,
) -> Element<'a, Message> {
    let caption = project_proxy_latency(delay);
    let label = caption.render(language);
    let band = caption.band;
    let caption_text = || {
        theme::role_body_text(label.clone()).style(move |theme: &Theme| text::Style {
            color: Some(band_color(band, theme)),
        })
    };
    match message {
        Some(message) => button(caption_text())
            .padding(0)
            .style(move |theme: &Theme, _status| button::Style {
                background: None,
                border: Border {
                    radius: border::Radius::from(0.0),
                    ..Default::default()
                },
                text_color: band_color(band, theme),
                ..Default::default()
            })
            .on_press(message)
            .into(),
        None => caption_text().into(),
    }
}

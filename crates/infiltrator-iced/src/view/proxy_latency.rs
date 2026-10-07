//! Native badge replays the shared controller-history caption and band.
use crate::view::theme::{MONO, tokens};
use iced::widget::text;
use iced::{Element, Theme};
use infiltrator_application::latency_projection::project_proxy_latency;
use infiltrator_contract::latency_display::LatencyBand;
use infiltrator_shared::locales::{Lang, Localizer};

pub fn latency_badge<'a, Message: 'a>(
    delay: Option<u32>,
    language: &Lang<'_>,
) -> Element<'a, Message> {
    let caption = project_proxy_latency(delay);
    let label = caption.render(&|key| language.tr(key).into_owned());
    let band = caption.band;
    text(label)
        .size(12)
        .font(MONO)
        .style(move |theme: &Theme| {
            let palette = tokens(theme);
            text::Style {
                color: Some(match band {
                    LatencyBand::Fast => palette.success,
                    LatencyBand::Medium => palette.warning,
                    LatencyBand::Slow => palette.danger,
                    LatencyBand::NotObserved | LatencyBand::UnconfirmedZero => {
                        palette.text_tertiary
                    }
                }),
            }
        })
        .into()
}

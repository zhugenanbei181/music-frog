//! Native plot of actual controller samples; unresolved zero entries break the line.
use crate::types::message::Message;
use crate::view::theme::tokens;
use iced::mouse::Cursor;
use iced::widget::canvas::{Frame, Geometry, Path, Program, Stroke};
use iced::{Point, Rectangle, Renderer, Theme};
use infiltrator_application::proxy_inspection_projection::ProxyHistoryPlot;

pub struct ProxyHistoryChart(pub ProxyHistoryPlot);
impl Program<Message> for ProxyHistoryChart {
    type State = ();
    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let palette = tokens(theme);
        let max = self.0.ceiling_ms;
        let height = (bounds.height - 8.0).max(1.0);
        let width = (bounds.width - 8.0).max(1.0);
        let count = self.0.samples.len().saturating_sub(1).max(1) as f32;
        let mut previous = None;
        for (index, delay) in self.0.samples.iter().copied().enumerate() {
            if !delay.is_finite() {
                previous = None;
                continue;
            }
            let point = Point::new(
                4.0 + index as f32 / count * width,
                4.0 + height * (1.0 - delay / max),
            );
            if let Some(previous) = previous {
                frame.stroke(
                    &Path::line(previous, point),
                    Stroke::default().with_width(2.0).with_color(palette.accent),
                );
            }
            frame.fill(&Path::circle(point, 2.5), palette.accent);
            previous = Some(point);
        }
        vec![frame.into_geometry()]
    }
}

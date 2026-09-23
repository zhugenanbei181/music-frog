//! Iced adapter for the shared live traffic-routing topology.
//!
//! The application/domain layer owns the five-stage model. This module only
//! paints the adapter-local animated flow strip; it never reads a controller,
//! connection list, or private runtime field.

use iced::widget::canvas;
use iced::{Color, Element, Point, Rectangle, Renderer, Theme, mouse};
use infiltrator_contract::traffic_topology::{
    TRAFFIC_TOPOLOGY_STAGE_COUNT, TrafficTopologySnapshot,
};

/// Animated flow strip placed above the topology stage cards.
pub struct TopologyFlowCanvas {
    pub snapshot: TrafficTopologySnapshot,
    /// Normalized animation phase supplied by the Iced update loop.
    pub phase: f32,
}

impl<Message> canvas::Program<Message> for TopologyFlowCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tokens = crate::view::theme::tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let width = bounds.width.max(1.0);
        let height = bounds.height.max(1.0);
        let center_y = height * 0.5;
        let last = (TRAFFIC_TOPOLOGY_STAGE_COUNT - 1) as f32;
        let x_at = |index: usize| width * index as f32 / last;

        for (index, link) in self.snapshot.links.iter().take(last as usize).enumerate() {
            let start = Point::new(x_at(index) + 7.0, center_y);
            let end = Point::new(x_at(index + 1) - 7.0, center_y);
            let path = canvas::Path::line(start, end);
            let color = if link.active {
                Color {
                    a: 0.28,
                    ..tokens.accent
                }
            } else {
                Color {
                    a: 0.65,
                    ..tokens.card_border
                }
            };
            frame.stroke(
                &path,
                canvas::Stroke::default().with_color(color).with_width(2.0),
            );

            if link.active && self.snapshot.is_flowing() {
                let phase = (self.phase + index as f32 * 0.19).fract();
                for offset in [0.0_f32, 0.52] {
                    let progress = (phase + offset).fract();
                    let x = start.x + (end.x - start.x) * progress;
                    let point = Point::new(x, center_y);
                    frame.fill(
                        &canvas::Path::circle(point, 5.5),
                        Color {
                            a: 0.16,
                            ..tokens.accent
                        },
                    );
                    frame.fill(&canvas::Path::circle(point, 2.4), tokens.accent);
                }
            }
        }

        for index in 0..TRAFFIC_TOPOLOGY_STAGE_COUNT {
            let point = Point::new(x_at(index), center_y);
            let active = self
                .snapshot
                .nodes
                .get(index)
                .is_some_and(|node| node.active);
            let color = if active {
                tokens.success
            } else {
                tokens.text_tertiary
            };
            frame.fill(
                &canvas::Path::circle(point, if active { 5.0 } else { 4.0 }),
                Color { a: 0.18, ..color },
            );
            frame.fill(&canvas::Path::circle(point, 2.0), color);
        }

        vec![frame.into_geometry()]
    }
}

pub fn topology_flow_canvas<'a, Message: 'a>(
    snapshot: &TrafficTopologySnapshot,
    phase: f32,
) -> Element<'a, Message> {
    canvas::Canvas::new(TopologyFlowCanvas {
        snapshot: snapshot.clone(),
        phase,
    })
    .width(iced::Length::Fill)
    .height(iced::Length::Fixed(42.0))
    .into()
}

#[cfg(test)]
#[path = "../../tests/gui/topology_tests.rs"]
mod topology_tests;

//! Cubic bezier spline and flow-pulse helpers (EFFECTS-032 #126-#127).
//!
//! `catmull_rom_segments` turns a sample polyline into cubic segments that
//! pass through every sample — the drawing-side smoothing behind the traffic
//! curves (#126). `pulse_positions` is the shared pure form of the topology
//! flow pulses (#127): evenly offset positions along a link, advanced by a
//! rate-driven phase.

use iced::Point;

/// One cubic bezier segment: the two control points and the end point; the
/// start is wherever the previous segment ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicSegment {
    pub control_a: Point,
    pub control_b: Point,
    pub to: Point,
}

/// Convert a polyline into Catmull-Rom cubic segments (#126): the spline
/// passes through every input point, with `tension` in `0.0..=1.0` scaling
/// the tangent handles (uniform Catmull-Rom at `1.0`, near-polyline at
/// `0.0`). Two points yield one straight segment.
pub fn catmull_rom_segments(points: &[Point], tension: f32) -> Vec<CubicSegment> {
    let mut segments = Vec::new();
    if points.len() < 2 {
        return segments;
    }
    let tension = tension.clamp(0.0, 1.0) / 6.0;
    for index in 0..points.len() - 1 {
        let p0 = points[index.saturating_sub(1)];
        let p1 = points[index];
        let p2 = points[index + 1];
        let p3 = points.get(index + 2).copied().unwrap_or(p2);
        segments.push(CubicSegment {
            control_a: Point::new(
                p1.x + (p2.x - p0.x) * tension,
                p1.y + (p2.y - p0.y) * tension,
            ),
            control_b: Point::new(
                p2.x - (p3.x - p1.x) * tension,
                p2.y - (p3.y - p1.y) * tension,
            ),
            to: p2,
        });
    }
    segments
}

/// The visible pulse positions along one link for a frame (#127): `count`
/// evenly offset pulses, all inside `0.0..1.0`, advanced by `phase` which
/// the update loop drives at the link's flow rate.
pub fn pulse_positions(phase: f32, count: usize) -> Vec<f32> {
    if count == 0 {
        return Vec::new();
    }
    let count_f = count as f32;
    (0..count)
        .map(|index| (phase + index as f32 / count_f).rem_euclid(1.0))
        .collect()
}

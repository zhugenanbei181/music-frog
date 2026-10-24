//! Cubic spline conversion and flow-pulse math (EFFECTS-032 #126-#127).
//! test-intent: behavior
use iced::Point;
use infiltrator_iced::view::chart::spline::{catmull_rom_segments, pulse_positions};

fn polyline(points: &[(f32, f32)]) -> Vec<Point> {
    points
        .iter()
        .map(|(x, y)| Point::new(*x, *y))
        .collect()
}

#[test]
fn the_spline_passes_through_every_sample() {
    let samples = polyline(&[(0.0, 0.0), (1.0, 2.0), (2.0, 1.0), (3.0, 3.0)]);
    let segments = catmull_rom_segments(&samples, 1.0);
    assert_eq!(segments.len(), samples.len() - 1);
    for (index, segment) in segments.iter().enumerate() {
        assert_eq!(segment.to, samples[index + 1], "segment ends on its sample");
    }
}

#[test]
fn degenerate_inputs_yield_no_or_one_segment() {
    assert!(catmull_rom_segments(&[], 1.0).is_empty());
    assert!(catmull_rom_segments(&polyline(&[(0.0, 0.0)]), 1.0).is_empty());
    let straight = catmull_rom_segments(&polyline(&[(0.0, 0.0), (4.0, 0.0)]), 1.0);
    assert_eq!(straight.len(), 1);
    // No neighbors: both controls sit on the samples themselves.
    assert_eq!(straight[0].control_a, Point::new(0.0, 0.0));
    assert_eq!(straight[0].control_b, Point::new(4.0, 0.0));
}

#[test]
fn zero_tension_collapses_the_controls_onto_the_samples() {
    let samples = polyline(&[(0.0, 0.0), (1.0, 5.0), (2.0, -3.0)]);
    // With tension 0 every handle sits at its anchor sample.
    let straightened = catmull_rom_segments(&samples, 0.0);
    assert_eq!(straightened[0].control_a, Point::new(0.0, 0.0));
    assert_eq!(straightened[0].control_b, Point::new(1.0, 5.0));
}

#[test]
fn monotone_x_stays_monotone_in_handles() {
    let samples = polyline(&[(0.0, 1.0), (1.0, 3.0), (2.0, 2.0), (3.0, 5.0)]);
    let segments = catmull_rom_segments(&samples, 1.0);
    for (index, segment) in segments.iter().enumerate() {
        let left = samples[index].x;
        let right = samples[index + 1].x;
        assert!(
            segment.control_a.x >= left - 1e-4 && segment.control_a.x <= right + 1e-4,
            "handle a stays inside its span"
        );
        assert!(
            segment.control_b.x >= left - 1e-4 && segment.control_b.x <= right + 1e-4,
            "handle b stays inside its span"
        );
    }
}

#[test]
fn pulses_are_evenly_offset_inside_the_link() {
    assert!(pulse_positions(0.0, 0).is_empty());
    let pulses = pulse_positions(0.25, 2);
    assert_eq!(pulses.len(), 2);
    assert!((pulses[0] - 0.25).abs() < 1e-6);
    assert!((pulses[1] - 0.75).abs() < 1e-6);
    for progress in pulse_positions(1.4, 3) {
        assert!((0.0..1.0).contains(&progress), "phase wraps into the link");
    }
}

//! Interactive crosshairs, hover telemetry tooltips, time-range zoom & pan,
//! and O(N) LOD downsampling (LTTB & MinMax).

use super::bezier::PlotPoint;
use crate::palette::UiPalette;

/// State of an active chart crosshair inspection.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CrosshairState {
    pub active: bool,
    pub cursor_x: f32,
    pub cursor_y: f32,
    pub snapped_index: Option<usize>,
    pub sample_x: Option<f32>,
    pub up_value: Option<f32>,
    pub down_value: Option<f32>,
}

impl CrosshairState {
    pub fn new(cursor_x: f32, cursor_y: f32) -> Self {
        Self {
            active: true,
            cursor_x,
            cursor_y,
            snapped_index: None,
            sample_x: None,
            up_value: None,
            down_value: None,
        }
    }
}

/// Configuration for crosshair guidelines and indicators.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CrosshairConfig {
    pub snap_to_sample: bool,
    pub show_vertical_line: bool,
    pub show_horizontal_line: bool,
    pub show_point_markers: bool,
}

impl Default for CrosshairConfig {
    fn default() -> Self {
        Self {
            snap_to_sample: true,
            show_vertical_line: true,
            show_horizontal_line: true,
            show_point_markers: true,
        }
    }
}

/// Time-range zoom and horizontal pan window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeRangeZoom {
    /// Zoom multiplier: `1.0` is 100% (full history), `2.0` is 2x zoom into a 50% time window.
    pub zoom_factor: f32,
    /// Pan offset from newest edge: `0.0` is pinned to newest, `1.0` is oldest.
    pub pan_offset: f32,
}

impl Default for TimeRangeZoom {
    fn default() -> Self {
        Self {
            zoom_factor: 1.0,
            pan_offset: 0.0,
        }
    }
}

impl TimeRangeZoom {
    pub fn new(zoom_factor: f32, pan_offset: f32) -> Self {
        Self {
            zoom_factor: zoom_factor.max(1.0),
            pan_offset: pan_offset.clamp(0.0, 1.0),
        }
    }
}

/// Extract zoomed and panned sub-slice from a time-series.
pub fn apply_zoom_pan(samples: &[f32], zoom: &TimeRangeZoom) -> Vec<f32> {
    if samples.is_empty() || zoom.zoom_factor <= 1.0 {
        return samples.to_vec();
    }

    let total = samples.len();
    let window_len = ((total as f32 / zoom.zoom_factor).round() as usize).clamp(2, total);
    let max_start = total.saturating_sub(window_len);
    let start_idx = ((max_start as f32) * zoom.pan_offset).round() as usize;
    let end_idx = (start_idx + window_len).min(total);

    samples[start_idx..end_idx].to_vec()
}

/// Find the nearest sample index for a given X pixel coordinate.
pub fn find_nearest_sample_index(samples_count: usize, width: f32, cursor_x: f32) -> Option<usize> {
    if samples_count == 0 || width <= 0.0 {
        return None;
    }
    if samples_count == 1 {
        return Some(0);
    }

    let clamped_x = cursor_x.clamp(0.0, width);
    let normalized = clamped_x / width;
    let raw_idx = (normalized * (samples_count - 1) as f32).round() as usize;
    Some(raw_idx.min(samples_count - 1))
}

/// Largest-Triangle-Three-Buckets (LTTB) downsampling algorithm.
///
/// Reduces high-frequency telemetry (e.g. 10,000 samples) down to `threshold` points
/// while preserving visual peaks, troughs, and waveform geometry with mathematical precision.
#[allow(clippy::needless_range_loop)]
pub fn decimate_lttb(samples: &[f32], threshold: usize) -> Vec<f32> {
    let count = samples.len();
    if threshold >= count || threshold <= 2 {
        return samples.to_vec();
    }

    let mut result = Vec::with_capacity(threshold);

    // 1. Always keep the first point
    result.push(samples[0]);

    let bucket_size = (count - 2) as f64 / (threshold - 2) as f64;
    let mut a_idx = 0;

    for i in 0..(threshold - 2) {
        // Compute average point for bucket (i + 1)
        let next_start = ((i + 1) as f64 * bucket_size).floor() as usize + 1;
        let next_end = (((i + 2) as f64 * bucket_size).floor() as usize + 1).min(count);

        let mut avg_x = 0.0f64;
        let mut avg_y = 0.0f64;
        let next_len = (next_end - next_start).max(1);

        for j in next_start..next_end {
            avg_x += j as f64;
            avg_y += samples[j] as f64;
        }
        avg_x /= next_len as f64;
        avg_y /= next_len as f64;

        // Current bucket
        let curr_start = (i as f64 * bucket_size).floor() as usize + 1;
        let curr_end = (((i + 1) as f64 * bucket_size).floor() as usize + 1).min(count);

        let ax = a_idx as f64;
        let ay = samples[a_idx] as f64;

        let mut max_area = -1.0f64;
        let mut max_idx = curr_start;

        for j in curr_start..curr_end {
            let bx = j as f64;
            let by = samples[j] as f64;

            // Area of triangle (A, B, C)
            let area = ((ax - avg_x) * (by - ay) - (ax - bx) * (avg_y - ay)).abs() * 0.5;
            if area > max_area {
                max_area = area;
                max_idx = j;
            }
        }

        result.push(samples[max_idx]);
        a_idx = max_idx;
    }

    // Always keep the last point
    result.push(samples[count - 1]);

    result
}

/// Min-Max downsampling algorithm: for each bucket, preserves both local minimum and maximum.
pub fn decimate_min_max(samples: &[f32], target_count: usize) -> Vec<f32> {
    let count = samples.len();
    if target_count >= count || target_count < 4 {
        return samples.to_vec();
    }

    let num_buckets = target_count / 2;
    let bucket_size = count as f32 / num_buckets as f32;
    let mut result = Vec::with_capacity(target_count);

    for b in 0..num_buckets {
        let start = (b as f32 * bucket_size).floor() as usize;
        let end = (((b + 1) as f32 * bucket_size).floor() as usize).min(count);
        if start >= end {
            continue;
        }

        let slice = &samples[start..end];
        let mut min_val = f32::INFINITY;
        let mut max_val = f32::NEG_INFINITY;
        let mut min_idx = 0;
        let mut max_idx = 0;

        for (i, &val) in slice.iter().enumerate() {
            if val < min_val {
                min_val = val;
                min_idx = i;
            }
            if val > max_val {
                max_val = val;
                max_idx = i;
            }
        }

        // Preserve chronological order of min and max inside bucket
        if min_idx <= max_idx {
            result.push(min_val);
            result.push(max_val);
        } else {
            result.push(max_val);
            result.push(min_val);
        }
    }

    result
}

/// Blend pixel helper for crosshair overlay.
fn blend_pixel(pixels: &mut [u8], width: u32, x: i32, y: i32, ink: [u8; 4], alpha: f32) {
    if x < 0 || y < 0 || x >= width as i32 {
        return;
    }
    let offset = (y as usize * width as usize + x as usize) * 4;
    let Some(pixel) = pixels.get_mut(offset..offset + 4) else {
        return;
    };
    let source_alpha = alpha.clamp(0.0, 1.0);
    let destination_alpha = pixel[3] as f32 / 255.0;
    let out_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    for c in 0..3 {
        let src = ink[c] as f32 / 255.0;
        let dst = pixel[c] as f32 / 255.0;
        let out = if out_alpha <= 0.0 {
            0.0
        } else {
            (src * source_alpha + dst * destination_alpha * (1.0 - source_alpha)) / out_alpha
        };
        pixel[c] = (out * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    pixel[3] = (out_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Render the interactive crosshair guidelines and snap dots directly onto the pixel buffer.
pub fn draw_crosshair_overlay(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    state: &CrosshairState,
    up_pt: Option<PlotPoint>,
    down_pt: Option<PlotPoint>,
    palette: &UiPalette,
) {
    if !state.active || width == 0 || height == 0 {
        return;
    }

    let cross_ink = crate::chart::to_rgba8(palette.ink_dim);
    let accent_ink = crate::chart::to_rgba8(palette.accent);
    let success_ink = crate::chart::to_rgba8(palette.success);

    let x_target = state.sample_x.unwrap_or(state.cursor_x).round() as i32;
    let y_target = state.cursor_y.round() as i32;

    // Vertical time guideline
    if x_target >= 0 && x_target < width as i32 {
        for y in 0..height as i32 {
            blend_pixel(pixels, width, x_target, y, cross_ink, 0.4);
        }
    }

    // Horizontal value guideline
    if y_target >= 0 && y_target < height as i32 {
        for x in 0..width as i32 {
            blend_pixel(pixels, width, x, y_target, cross_ink, 0.25);
        }
    }

    // Uplink curve snapped dot
    if let Some(pt) = up_pt {
        let px = pt.x.round() as i32;
        let py = pt.y.round() as i32;
        for dy in -2..=2 {
            for dx in -2..=2 {
                if dx * dx + dy * dy <= 4 {
                    blend_pixel(pixels, width, px + dx, py + dy, accent_ink, 1.0);
                }
            }
        }
    }

    // Downlink curve snapped dot
    if let Some(pt) = down_pt {
        let px = pt.x.round() as i32;
        let py = pt.y.round() as i32;
        for dy in -2..=2 {
            for dx in -2..=2 {
                if dx * dx + dy * dy <= 4 {
                    blend_pixel(pixels, width, px + dx, py + dy, success_ink, 1.0);
                }
            }
        }
    }
}

//! Smooth Bezier and spline waveform mathematics, dual-series shared scaling,
//! and honest gap handling.

/// One projected point in pixel space, y growing downward (y == 0 is top).
/// `None` indicates a gap in the observation stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotPoint {
    pub x: f32,
    pub y: f32,
}

impl PlotPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Linear interpolation between two points.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            x: self.x + (other.x - self.x) * t,
            y: self.y + (other.y - self.y) * t,
        }
    }
}

/// Dynamic scaling strategy for chart axes.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ScaleMode {
    /// Upper and lower series share a unified dynamic maximum range:
    /// `[0.0, max(up_max, down_max) * (1.0 + headroom)]`.
    #[default]
    Shared,
    /// Each series normalizes independently to its own min/max range.
    Independent,
    /// Fixed absolute scale ceiling.
    Fixed(f32),
}

/// A cubic Bezier curve segment bounded by two endpoints and two control points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CubicBezierSegment {
    pub p0: PlotPoint,
    pub c1: PlotPoint,
    pub c2: PlotPoint,
    pub p1: PlotPoint,
}

impl CubicBezierSegment {
    pub const fn new(p0: PlotPoint, c1: PlotPoint, c2: PlotPoint, p1: PlotPoint) -> Self {
        Self { p0, c1, c2, p1 }
    }

    /// Evaluate point on the cubic curve at parameter `t ∈ [0.0, 1.0]`.
    pub fn eval(&self, t: f32) -> PlotPoint {
        let t = t.clamp(0.0, 1.0);
        let u = 1.0 - t;
        let tt = t * t;
        let uu = u * u;
        let uuu = uu * u;
        let ttt = tt * t;

        let x =
            uuu * self.p0.x + 3.0 * uu * t * self.c1.x + 3.0 * u * tt * self.c2.x + ttt * self.p1.x;
        let y =
            uuu * self.p0.y + 3.0 * uu * t * self.c1.y + 3.0 * u * tt * self.c2.y + ttt * self.p1.y;

        PlotPoint { x, y }
    }

    /// Generate `subdivisions` intermediate evaluated points along the segment.
    pub fn sample_points(&self, subdivisions: usize) -> Vec<PlotPoint> {
        let steps = subdivisions.max(1);
        (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                self.eval(t)
            })
            .collect()
    }
}

/// Compute evenly spaced X coordinates across `width` for `count` samples.
pub fn spaced_x(count: usize, width: f32) -> Vec<f32> {
    match count {
        0 => Vec::new(),
        1 => vec![width],
        _ => (0..count)
            .map(|i| width * i as f32 / (count - 1) as f32)
            .collect(),
    }
}

/// Compute the shared maximum scale ceiling across both series.
pub fn compute_shared_scale(up: &[f32], down: &[f32], headroom: f32) -> f32 {
    let mut peak = 0.0f32;
    for &val in up.iter().chain(down.iter()) {
        if val.is_finite() && val > peak {
            peak = val;
        }
    }
    if peak <= 0.0 {
        1.0
    } else {
        peak * (1.0 + headroom.max(0.0))
    }
}

/// Linear projection of samples into pixel coordinates.
/// If `scale_max` is provided and series has variation, normalizes against `[0.0, scale_max]`.
/// If series is constant (e.g. flat zero), projects to mid line.
pub fn linear_polyline(
    samples: &[f32],
    width: f32,
    height: f32,
    scale_max: Option<f32>,
) -> Vec<Option<PlotPoint>> {
    if samples.is_empty() {
        return Vec::new();
    }

    let xs = spaced_x(samples.len(), width);
    let mid = height / 2.0;
    // let max_y = (height - 2.0).max(0.0);

    let mut min_val = f32::INFINITY;
    let mut max_val = f32::NEG_INFINITY;
    for &sample in samples {
        if sample.is_finite() {
            min_val = min_val.min(sample);
            max_val = max_val.max(sample);
        }
    }

    if !min_val.is_finite() || !max_val.is_finite() {
        return vec![None; samples.len()];
    }

    let (min, max) = match scale_max {
        Some(ceiling) if min_val < max_val || min_val > 0.0 => (0.0, ceiling.max(0.0001)),
        _ => (min_val, max_val),
    };

    let range = max - min;

    samples
        .iter()
        .zip(xs)
        .map(|(&sample, x)| {
            if !sample.is_finite() {
                return None;
            }
            let y = if range <= 0.0 {
                mid
            } else {
                let normalized = ((sample - min) / range).clamp(0.0, 1.0);
                height * (1.0 - normalized)
            };
            Some(PlotPoint { x, y })
        })
        .collect()
}

/// Generate a smooth Catmull-Rom cubic Bezier spline with monotonic clamping.
///
/// Disjoint valid runs (separated by `NaN` or `±∞`) are processed individually
/// without bridging across gaps.
pub fn bezier_smooth_polyline(
    samples: &[f32],
    width: f32,
    height: f32,
    scale_max: Option<f32>,
    subdivisions_per_segment: usize,
) -> Vec<Option<PlotPoint>> {
    if samples.is_empty() {
        return Vec::new();
    }

    let discrete = linear_polyline(samples, width, height, scale_max);
    if discrete.len() <= 2 {
        return discrete;
    }

    let mut result = Vec::with_capacity(samples.len() * subdivisions_per_segment.max(1));
    let mut run: Vec<(usize, PlotPoint)> = Vec::new();

    for (idx, opt_point) in discrete.iter().enumerate() {
        match opt_point {
            Some(pt) => run.push((idx, *pt)),
            None => {
                if !run.is_empty() {
                    append_smooth_run(&run, height, subdivisions_per_segment, &mut result);
                    run.clear();
                }
                result.push(None);
            }
        }
    }

    if !run.is_empty() {
        append_smooth_run(&run, height, subdivisions_per_segment, &mut result);
    }

    result
}

/// Helper to interpolate a contiguous run of finite points into Bezier curve samples.
fn append_smooth_run(
    run: &[(usize, PlotPoint)],
    height: f32,
    subdivisions: usize,
    out: &mut Vec<Option<PlotPoint>>,
) {
    let count = run.len();
    if count == 0 {
        return;
    }
    if count == 1 {
        out.push(Some(run[0].1));
        return;
    }
    if count == 2 {
        let p0 = run[0].1;
        let p1 = run[1].1;
        let steps = subdivisions.max(1);
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            out.push(Some(p0.lerp(p1, t)));
        }
        out.push(Some(p1));
        return;
    }

    for i in 0..count - 1 {
        let p0 = run[i].1;
        let p1 = run[i + 1].1;

        let p_prev = if i > 0 { run[i - 1].1 } else { p0 };
        let p_next = if i + 2 < count { run[i + 2].1 } else { p1 };

        // Catmull-Rom tangents: T_i = (P_{i+1} - P_{i-1}) * 0.5
        let t0_x = (p1.x - p_prev.x) * 0.5;
        let t0_y = (p1.y - p_prev.y) * 0.5;
        let t1_x = (p_next.x - p0.x) * 0.5;
        let t1_y = (p_next.y - p0.y) * 0.5;

        // Bezier control points: C_1 = P0 + T0 / 3, C_2 = P1 - T1 / 3
        let mut c1 = PlotPoint {
            x: p0.x + t0_x / 3.0,
            y: p0.y + t0_y / 3.0,
        };
        let mut c2 = PlotPoint {
            x: p1.x - t1_x / 3.0,
            y: p1.y - t1_y / 3.0,
        };

        // Monotonicity clamping: prevent overshoot beyond the segment bounds
        let min_y = p0.y.min(p1.y);
        let max_y = p0.y.max(p1.y);

        if (p1.y - p0.y).abs() < 1e-4 {
            c1.y = p0.y;
            c2.y = p1.y;
        } else {
            c1.y = c1.y.clamp(min_y, max_y).clamp(0.0, height);
            c2.y = c2.y.clamp(min_y, max_y).clamp(0.0, height);
        }

        c1.x = c1.x.clamp(p0.x, p1.x);
        c2.x = c2.x.clamp(p0.x, p1.x);

        let segment = CubicBezierSegment::new(p0, c1, c2, p1);
        let steps = subdivisions.max(1);
        for s in 0..steps {
            let t = s as f32 / steps as f32;
            out.push(Some(segment.eval(t)));
        }
    }

    // Push the final endpoint of the run
    out.push(Some(run[count - 1].1));
}

/// Construct dual-series waveform curves according to `ScaleMode` and smoothness setting.
pub fn build_dual_series_curves(
    up: &[f32],
    down: &[f32],
    width: f32,
    height: f32,
    scale_mode: ScaleMode,
    smooth: bool,
) -> (Vec<Option<PlotPoint>>, Vec<Option<PlotPoint>>, f32) {
    let (scale_up, scale_down, peak_scale) = match scale_mode {
        ScaleMode::Shared => {
            let shared_max = compute_shared_scale(up, down, 0.05);
            (Some(shared_max), Some(shared_max), shared_max)
        }
        ScaleMode::Independent => (None, None, 1.0),
        ScaleMode::Fixed(ceiling) => (Some(ceiling), Some(ceiling), ceiling),
    };

    let up_points = if smooth {
        bezier_smooth_polyline(up, width, height, scale_up, 8)
    } else {
        linear_polyline(up, width, height, scale_up)
    };

    let down_points = if smooth {
        bezier_smooth_polyline(down, width, height, scale_down, 8)
    } else {
        linear_polyline(down, width, height, scale_down)
    };

    (up_points, down_points, peak_scale)
}

/// Classification of a local waveform feature extremum point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtremumKind {
    Peak,
    Valley,
}

/// A detected local peak or valley feature on a telemetry waveform.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveformExtremum {
    pub index: usize,
    pub value: f32,
    pub kind: ExtremumKind,
    pub prominence: f32,
}

/// Detect local peak and valley extrema on a series of waveform samples.
pub fn find_waveform_extrema(samples: &[f32], min_prominence: f32) -> Vec<WaveformExtremum> {
    let n = samples.len();
    if n < 3 {
        return Vec::new();
    }

    let mut extrema = Vec::new();
    let prom = min_prominence.max(0.0);

    for i in 1..n - 1 {
        let prev = samples[i - 1];
        let curr = samples[i];
        let next = samples[i + 1];

        if curr > prev && curr > next {
            let prominence = curr - prev.max(next);
            if prominence >= prom {
                extrema.push(WaveformExtremum {
                    index: i,
                    value: curr,
                    kind: ExtremumKind::Peak,
                    prominence,
                });
            }
        } else if curr < prev && curr < next {
            let prominence = prev.min(next) - curr;
            if prominence >= prom {
                extrema.push(WaveformExtremum {
                    index: i,
                    value: curr,
                    kind: ExtremumKind::Valley,
                    prominence,
                });
            }
        }
    }

    extrema
}

/// Calculate the Crest Factor (ratio of peak amplitude to RMS value).
/// Higher values indicate bursty traffic with sharp peaks.
pub fn compute_crest_factor(samples: &[f32]) -> f32 {
    let valid: Vec<f32> = samples
        .iter()
        .copied()
        .filter(|v| v.is_finite() && *v >= 0.0)
        .collect();
    if valid.is_empty() {
        return 1.0;
    }

    let peak = valid.iter().copied().fold(0.0f32, f32::max);
    let mean_sq: f32 = valid.iter().map(|&v| v * v).sum::<f32>() / (valid.len() as f32);
    let rms = mean_sq.sqrt();

    if rms <= 1e-4 {
        1.0
    } else {
        (peak / rms).max(1.0)
    }
}

/// Analytical Catmull-Rom cubic spline interpolation engine.
pub struct CatmullRomSpline;

impl CatmullRomSpline {
    /// Evaluate 1D Catmull-Rom spline at parameter `t` in [0.0..1.0] across control points p0, p1, p2, p3.
    pub fn evaluate_1d(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
        let t2 = t * t;
        let t3 = t2 * t;
        0.5 * ((2.0 * p1)
            + (-p0 + p2) * t
            + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
            + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
    }

    /// Interpolate a sequence of samples into a denser curve with `steps` points per segment.
    pub fn interpolate_sequence(samples: &[f32], steps: usize) -> Vec<f32> {
        let n = samples.len();
        if n < 2 || steps == 0 {
            return samples.to_vec();
        }

        let mut output = Vec::with_capacity((n - 1) * steps + 1);

        for i in 0..n - 1 {
            let p0 = if i == 0 { samples[0] } else { samples[i - 1] };
            let p1 = samples[i];
            let p2 = samples[i + 1];
            let p3 = if i + 2 < n {
                samples[i + 2]
            } else {
                samples[n - 1]
            };

            for s in 0..steps {
                let t = (s as f32) / (steps as f32);
                output.push(Self::evaluate_1d(p0, p1, p2, p3, t));
            }
        }
        output.push(*samples.last().unwrap());

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_waveform_extrema() {
        let samples = [10.0, 25.0, 15.0, 5.0, 40.0, 10.0];
        let extrema = find_waveform_extrema(&samples, 5.0);

        assert_eq!(extrema.len(), 3);
        // index 1: peak at 25.0
        assert_eq!(extrema[0].index, 1);
        assert_eq!(extrema[0].kind, ExtremumKind::Peak);

        // index 3: valley at 5.0
        assert_eq!(extrema[1].index, 3);
        assert_eq!(extrema[1].kind, ExtremumKind::Valley);

        // index 4: peak at 40.0
        assert_eq!(extrema[2].index, 4);
        assert_eq!(extrema[2].kind, ExtremumKind::Peak);
    }

    #[test]
    fn test_compute_crest_factor() {
        // Flat constant samples: peak == rms -> crest factor = 1.0
        let flat = [20.0, 20.0, 20.0, 20.0];
        assert!((compute_crest_factor(&flat) - 1.0).abs() < 1e-4);

        // Bursty spike: high peak, low average
        let bursty = [1.0, 1.0, 1.0, 100.0, 1.0];
        assert!(compute_crest_factor(&bursty) > 1.5);
    }
    #[test]
    fn test_catmull_rom_spline_endpoints_and_continuity() {
        let p0 = 10.0;
        let p1 = 20.0;
        let p2 = 40.0;
        let p3 = 50.0;

        // t=0 must exactly equal p1
        assert!((CatmullRomSpline::evaluate_1d(p0, p1, p2, p3, 0.0) - 20.0).abs() < 1e-4);
        // t=1 must exactly equal p2
        assert!((CatmullRomSpline::evaluate_1d(p0, p1, p2, p3, 1.0) - 40.0).abs() < 1e-4);

        let sequence = [10.0, 30.0, 20.0];
        let smoothed = CatmullRomSpline::interpolate_sequence(&sequence, 4);
        assert_eq!(smoothed.len(), 9);
        assert_eq!(smoothed[0], 10.0);
        assert_eq!(*smoothed.last().unwrap(), 20.0);
    }
}

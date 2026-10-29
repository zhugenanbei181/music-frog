//! The live chart projection: three data layers, a smoothed polyline and a
//! viewport downsampling (EFFECTS-008 #29~#31).
//!
//! A live chart draws three different things and they must not be confused:
//!
//! * the **scale** is static: the axis the series is drawn against. It must
//!   not jump with every sample, so it only ever grows;
//! * the **polyline** is dynamic: the smoothed shape of the series;
//! * the **samples** are the live truth: the raw points of the window.
//!
//! The projector keeps the three apart, smooths the polyline with a
//! Catmull-Rom spline and downsamples it with the largest-triangle
//! three-buckets rule — which keeps every peak of the series.

use infiltrator_contract::traffic_waveform::TrafficWaveformSnapshot;

/// One point of a chart series.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartPoint {
    /// The horizontal position, in milliseconds since the window opened.
    pub x: f64,
    /// The vertical position, in the unit of the series.
    pub y: f64,
}

impl ChartPoint {
    /// One point.
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// One live sample of a series.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartSample {
    /// The position of the sample on the horizontal axis, in milliseconds.
    pub at_ms: f64,
    /// The value of the sample.
    pub value: f64,
}

/// The readout of a crosshair hover/touch probe over the live traffic chart (FLPARITY-022).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChartProbeSample {
    /// Normalized horizontal coordinate `[0.0, 1.0]` across chart width.
    pub relative_x: f64,
    /// Absolute sample index within the snapshot window.
    pub sample_index: usize,
    /// Upload speed at this sample point in bytes per second.
    pub upload_bps: f64,
    /// Download speed at this sample point in bytes per second.
    pub download_bps: f64,
    /// Offset in seconds relative to current time (0.0 = now, >0.0 = seconds ago).
    pub seconds_ago: f64,
}

impl ChartProbeSample {
    /// Format upload speed in human-readable rate (e.g. "1.2 MB/s").
    pub fn format_upload(&self) -> String {
        format_rate_standard(self.upload_bps)
    }

    /// Format download speed in human-readable rate (e.g. "4.5 MB/s").
    pub fn format_download(&self) -> String {
        format_rate_standard(self.download_bps)
    }

    /// Format relative time label (e.g. "当前" or "15 秒前").
    pub fn format_time_label(&self) -> String {
        if self.seconds_ago < 0.5 {
            "当前".to_owned()
        } else {
            format!("{:.0} 秒前", self.seconds_ago)
        }
    }

    /// Combined tooltip string for quick display (e.g. "↑ 1.2 MB/s  ↓ 4.5 MB/s (15 秒前)").
    pub fn format_tooltip(&self) -> String {
        format!(
            "↑ {}  ↓ {} ({})",
            self.format_upload(),
            self.format_download(),
            self.format_time_label()
        )
    }
}

/// Standardized human-readable transfer rate formatting (B/s, KB/s, MB/s, GB/s).
pub fn format_rate_standard(bps: f64) -> String {
    if !bps.is_finite() || bps <= 0.0 {
        return "0 B/s".to_owned();
    }
    if bps < 1_000.0 {
        format!("{:.0} B/s", bps)
    } else if bps < 1_000_000.0 {
        format!("{:.1} KB/s", bps / 1_000.0)
    } else if bps < 1_000_000_000.0 {
        format!("{:.1} MB/s", bps / 1_000_000.0)
    } else {
        format!("{:.1} GB/s", bps / 1_000_000_000.0)
    }
}

/// Probe a relative horizontal position `[0.0, 1.0]` against a waveform snapshot.
pub fn probe_traffic_waveform(
    snapshot: &TrafficWaveformSnapshot,
    relative_x: f64,
) -> Option<ChartProbeSample> {
    if snapshot.samples.is_empty() {
        return None;
    }
    let clamped_x = relative_x.clamp(0.0, 1.0);
    let total = snapshot.samples.len();
    let index = ((total.saturating_sub(1)) as f64 * clamped_x).round() as usize;
    let sample = snapshot.samples.get(index)?;
    let seconds_ago = (total.saturating_sub(1) - index) as f64;
    Some(ChartProbeSample {
        relative_x: clamped_x,
        sample_index: index,
        upload_bps: sample.upload_bps,
        download_bps: sample.download_bps,
        seconds_ago,
    })
}

/// The static scale of a chart.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChartScale {
    /// The lowest value of the axis.
    pub min: f64,
    /// The highest value of the axis.
    pub max: f64,
    /// The labelled ticks of the axis.
    pub ticks: Vec<f64>,
}

/// The three layers of the traffic chart (EFFECTS-008 #29).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrafficChartLayers {
    /// The static axis the series is drawn against.
    pub scale: ChartScale,
    /// The dynamic smoothed shape of the series.
    pub polyline: Vec<ChartPoint>,
    /// The live samples of the current window.
    pub samples: Vec<ChartSample>,
}

/// Which series of the traffic chart is projected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrafficSeries {
    /// The upload series.
    Upload,
    /// The download series.
    Download,
}

/// The projector of the live traffic chart.
#[derive(Clone, Debug)]
pub struct TrafficProjector {
    series: TrafficSeries,
    samples: Vec<ChartSample>,
    polyline: Vec<ChartPoint>,
    scale: ChartScale,
    smoothing: usize,
}

impl Default for TrafficProjector {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficProjector {
    /// A projector of the download series with a smooth polyline.
    pub fn new() -> Self {
        Self {
            series: TrafficSeries::Download,
            samples: Vec::new(),
            polyline: Vec::new(),
            scale: ChartScale::default(),
            smoothing: 8,
        }
    }

    /// The series this projector draws.
    pub const fn series(&self) -> TrafficSeries {
        self.series
    }

    /// Fold the owner's waveform snapshot into the three layers.
    pub fn observe(&mut self, snapshot: &TrafficWaveformSnapshot, series: TrafficSeries) {
        self.series = series;
        self.samples = snapshot
            .samples
            .iter()
            .enumerate()
            .map(|(index, sample)| ChartSample {
                at_ms: index as f64,
                value: match series {
                    TrafficSeries::Upload => sample.upload_bps,
                    TrafficSeries::Download => sample.download_bps,
                },
            })
            .collect();
        self.rebuild_polyline();
        self.rebuild_scale();
    }

    /// The live samples of the current window.
    pub fn samples(&self) -> &[ChartSample] {
        &self.samples
    }

    /// The dynamic polyline of the series.
    pub fn polyline(&self) -> &[ChartPoint] {
        &self.polyline
    }

    /// The static scale of the chart.
    pub fn scale(&self) -> &ChartScale {
        &self.scale
    }

    /// The three layers of the chart.
    pub fn layers(&self) -> TrafficChartLayers {
        TrafficChartLayers {
            scale: self.scale.clone(),
            polyline: self.polyline.clone(),
            samples: self.samples.clone(),
        }
    }

    /// The polyline reduced to `threshold` points, peaks included.
    pub fn downsampled(&self, threshold: usize) -> Vec<ChartPoint> {
        lttb_downsample(&self.polyline, threshold)
    }

    /// Forget the series and the scale.
    pub fn reset(&mut self) {
        self.samples.clear();
        self.polyline.clear();
        self.scale = ChartScale::default();
    }

    /// Probe at a normalized relative horizontal coordinate `[0.0, 1.0]`.
    pub fn probe(&self, relative_x: f64) -> Option<ChartProbeSample> {
        if self.samples.is_empty() {
            return None;
        }
        let clamped_x = relative_x.clamp(0.0, 1.0);
        let total = self.samples.len();
        let index = ((total.saturating_sub(1)) as f64 * clamped_x).round() as usize;
        let sample = self.samples.get(index)?;
        let seconds_ago = (total.saturating_sub(1) - index) as f64;
        let (upload_bps, download_bps) = match self.series {
            TrafficSeries::Upload => (sample.value, 0.0),
            TrafficSeries::Download => (0.0, sample.value),
        };
        Some(ChartProbeSample {
            relative_x: clamped_x,
            sample_index: index,
            upload_bps,
            download_bps,
            seconds_ago,
        })
    }

    /// Probe at a pixel coordinate along a given total pixel width.
    pub fn probe_at_pixel(&self, pixel_x: f64, total_width: f64) -> Option<ChartProbeSample> {
        if total_width <= 0.0 || !pixel_x.is_finite() {
            return None;
        }
        let relative_x = (pixel_x / total_width).clamp(0.0, 1.0);
        self.probe(relative_x)
    }

    fn rebuild_polyline(&mut self) {
        let points: Vec<ChartPoint> = self
            .samples
            .iter()
            .map(|sample| ChartPoint::new(sample.at_ms, sample.value))
            .collect();
        let smoothing = self.smoothing;
        self.polyline = catmull_rom(&points, smoothing);
    }

    fn rebuild_scale(&mut self) {
        let peak = self
            .samples
            .iter()
            .map(|sample| sample.value)
            .filter(|value| value.is_finite())
            .fold(0.0_f64, f64::max);
        let ceiling = nice_ceiling(peak);
        // The scale only ever grows: a shrinking axis would make the whole
        // chart jump whenever the traffic dips.
        if ceiling > self.scale.max {
            self.scale.max = ceiling;
        }
        self.scale.min = 0.0;
        let max = self.scale.max;
        self.scale.ticks = (0..=4).map(|step| max * step as f64 / 4.0).collect();
    }
}

/// Round a value up to the next readable step (1, 2 or 5 times a power of ten).
pub fn nice_ceiling(value: f64) -> f64 {
    if !value.is_finite() || value <= 0.0 {
        return 0.0;
    }
    let exponent = value.log10().floor();
    let magnitude = 10_f64.powf(exponent);
    let normalized = value / magnitude;
    let step = if normalized <= 1.0 {
        1.0
    } else if normalized <= 2.0 {
        2.0
    } else if normalized <= 5.0 {
        5.0
    } else {
        10.0
    };
    step * magnitude
}

/// One point of a Catmull-Rom spline.
fn catmull_rom_point(
    p0: ChartPoint,
    p1: ChartPoint,
    p2: ChartPoint,
    p3: ChartPoint,
    t: f64,
) -> ChartPoint {
    let t2 = t * t;
    let t3 = t2 * t;
    let blend = |a: f64, b: f64, c: f64, d: f64| {
        0.5 * (2.0 * b
            + (-a + c) * t
            + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
            + (-a + 3.0 * b - 3.0 * c + d) * t3)
    };
    ChartPoint::new(blend(p0.x, p1.x, p2.x, p3.x), blend(p0.y, p1.y, p2.y, p3.y))
}

/// The Catmull-Rom spline through a series of points.
///
/// The spline starts and ends exactly on the series, so the drawn curve never
/// drifts away from the samples it stands for.
pub fn catmull_rom(points: &[ChartPoint], samples_per_segment: usize) -> Vec<ChartPoint> {
    if points.len() < 2 {
        return points.to_vec();
    }
    if samples_per_segment == 0 {
        return points.to_vec();
    }
    let mut curve = Vec::with_capacity(points.len() * samples_per_segment);
    let last = points.len() - 1;
    for index in 0..last {
        let p0 = if index == 0 {
            points[0]
        } else {
            points[index - 1]
        };
        let p1 = points[index];
        let p2 = points[index + 1];
        let p3 = if index + 2 <= last {
            points[index + 2]
        } else {
            points[last]
        };
        for step in 0..samples_per_segment {
            let t = step as f64 / samples_per_segment as f64;
            curve.push(catmull_rom_point(p0, p1, p2, p3, t));
        }
    }
    curve.push(points[last]);
    curve
}

/// The indices the largest-triangle three-buckets rule keeps.
fn lttb_indices(points: &[ChartPoint], threshold: usize) -> Vec<usize> {
    let length = points.len();
    if threshold >= length || threshold == 0 {
        return (0..length).collect();
    }
    if threshold == 1 {
        return vec![0];
    }
    if threshold == 2 {
        return vec![0, length - 1];
    }
    let every = (length as f64 - 2.0) / (threshold as f64 - 2.0);
    let mut kept = Vec::with_capacity(threshold);
    kept.push(0);
    let mut anchor = 0usize;
    for bucket in 0..(threshold - 2) {
        let average_start = ((bucket + 1) as f64 * every).floor() as usize + 1;
        let average_end = (((bucket + 2) as f64 * every).floor() as usize + 1).min(length);
        let mut average_x = 0.0;
        let mut average_y = 0.0;
        let count = average_end.saturating_sub(average_start);
        for point in &points[average_start..average_end] {
            average_x += point.x;
            average_y += point.y;
        }
        if count > 0 {
            average_x /= count as f64;
            average_y /= count as f64;
        }
        let range_start = (bucket as f64 * every).floor() as usize + 1;
        let range_end = (((bucket + 1) as f64 * every).floor() as usize + 1).min(length);
        let anchor_point = points[anchor];
        let mut best_area = -1.0;
        let mut best = range_start.min(length.saturating_sub(1));
        for (index, point) in points.iter().enumerate().take(range_end).skip(range_start) {
            let area = ((anchor_point.x - average_x) * (point.y - anchor_point.y)
                - (anchor_point.x - point.x) * (average_y - anchor_point.y))
                .abs()
                * 0.5;
            if area > best_area {
                best_area = area;
                best = index;
            }
        }
        kept.push(best);
        anchor = best;
    }
    kept.push(length - 1);
    kept
}

/// Reduce a series to `threshold` points with the largest-triangle
/// three-buckets rule.
///
/// The rule keeps the shape of the curve, and the extremes of the series are
/// kept explicitly on top of it: a downsampled chart that lost its peak
/// would show a traffic spike that never happened.
pub fn lttb_downsample(points: &[ChartPoint], threshold: usize) -> Vec<ChartPoint> {
    if points.len() <= threshold || threshold == 0 {
        return points.to_vec();
    }
    let mut indices = lttb_indices(points, threshold);
    let mut peak = 0usize;
    let mut trough = 0usize;
    for (index, point) in points.iter().enumerate() {
        if point.y > points[peak].y {
            peak = index;
        }
        if point.y < points[trough].y {
            trough = index;
        }
    }
    for extreme in [trough, peak] {
        if !indices.contains(&extreme) {
            indices.push(extreme);
            indices.sort_unstable();
        }
    }
    indices.dedup();
    indices.into_iter().map(|index| points[index]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::traffic_waveform::TrafficSample;

    fn sample(index: usize, value: f64) -> TrafficSample {
        TrafficSample {
            sampled_at_epoch_ms: Some(index as i64),
            upload_bps: value / 2.0,
            download_bps: value,
        }
    }

    fn snapshot(values: &[f64]) -> TrafficWaveformSnapshot {
        TrafficWaveformSnapshot {
            generation: 1,
            revision: 1,
            samples: values
                .iter()
                .enumerate()
                .map(|(i, v)| sample(i, *v))
                .collect(),
        }
    }

    #[test]
    fn the_three_layers_stay_apart() {
        let mut projector = TrafficProjector::new();
        projector.observe(&snapshot(&[10.0, 20.0, 30.0]), TrafficSeries::Download);
        let layers = projector.layers();
        assert_eq!(layers.samples.len(), 3);
        // The polyline is smoothed, so it carries more points than the
        // samples it stands for.
        assert!(layers.polyline.len() > layers.samples.len());
        // The scale is its own layer and knows the axis.
        assert_eq!(layers.scale.min, 0.0);
        assert!(layers.scale.max >= 30.0);
        assert_eq!(layers.scale.ticks.len(), 5);
    }

    #[test]
    fn the_scale_only_grows_so_the_axis_never_jumps() {
        let mut projector = TrafficProjector::new();
        projector.observe(&snapshot(&[100.0, 100.0]), TrafficSeries::Download);
        let grown = projector.scale().max;
        projector.observe(&snapshot(&[10.0, 10.0]), TrafficSeries::Download);
        assert_eq!(
            projector.scale().max,
            grown,
            "a traffic dip must not shrink the axis"
        );
        projector.reset();
        assert_eq!(projector.scale().max, 0.0);
    }

    #[test]
    fn the_series_choice_picks_the_right_lane() {
        let mut projector = TrafficProjector::new();
        projector.observe(&snapshot(&[40.0]), TrafficSeries::Download);
        assert_eq!(projector.samples()[0].value, 40.0);
        projector.observe(&snapshot(&[40.0]), TrafficSeries::Upload);
        assert_eq!(projector.samples()[0].value, 20.0);
        assert_eq!(projector.series(), TrafficSeries::Upload);
    }

    #[test]
    fn the_spline_starts_and_ends_on_the_series() {
        let points = [
            ChartPoint::new(0.0, 0.0),
            ChartPoint::new(1.0, 10.0),
            ChartPoint::new(2.0, 5.0),
            ChartPoint::new(3.0, 20.0),
        ];
        let curve = catmull_rom(&points, 6);
        assert!(curve.len() > points.len());
        assert_eq!(curve[0], points[0]);
        assert_eq!(curve[curve.len() - 1], points[3]);
        for point in &curve {
            assert!(point.x.is_finite() && point.y.is_finite());
        }
        // A series of one point is returned untouched.
        assert_eq!(catmull_rom(&points[..1], 6), points[..1]);
    }

    #[test]
    fn the_downsampling_keeps_every_peak() {
        let mut points = Vec::new();
        for index in 0..200 {
            let value = if index == 137 { 900.0 } else { 10.0 };
            points.push(ChartPoint::new(index as f64, value));
        }
        let reduced = lttb_downsample(&points, 20);
        assert!(reduced.len() <= 22, "the reduction stays bounded");
        assert!(reduced.iter().any(|point| point.y == 900.0));
        assert!(reduced.iter().any(|point| point.y == 10.0));
        // The first and the last point of the series stay.
        assert_eq!(reduced[0], points[0]);
        assert_eq!(reduced[reduced.len() - 1], points[points.len() - 1]);
    }

    #[test]
    fn a_short_series_is_not_reduced() {
        let points = [ChartPoint::new(0.0, 1.0), ChartPoint::new(1.0, 2.0)];
        assert_eq!(lttb_downsample(&points, 10), points.to_vec());
    }

    #[test]
    fn the_axis_ceiling_is_readable() {
        assert_eq!(nice_ceiling(0.0), 0.0);
        assert_eq!(nice_ceiling(1.0), 1.0);
        assert_eq!(nice_ceiling(1.5), 2.0);
        assert_eq!(nice_ceiling(3.0), 5.0);
        assert_eq!(nice_ceiling(7.0), 10.0);
        assert_eq!(nice_ceiling(120.0), 200.0);
        assert!(nice_ceiling(f64::NAN) == 0.0);
    }

    #[test]
    fn probe_traffic_waveform_and_formatting() {
        use infiltrator_contract::traffic_waveform::TrafficSample;

        let empty_snapshot = TrafficWaveformSnapshot::default();
        assert!(probe_traffic_waveform(&empty_snapshot, 0.5).is_none());

        let snapshot = TrafficWaveformSnapshot {
            generation: 1,
            revision: 1,
            samples: vec![
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 1_000.0,
                    download_bps: 2_000.0,
                },
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 500_000.0,
                    download_bps: 1_500_000.0,
                },
                TrafficSample {
                    sampled_at_epoch_ms: None,
                    upload_bps: 12_000_000_000.0,
                    download_bps: 30_000_000_000.0,
                },
            ],
        };

        // Probe beginning (oldest: index 0, 2 seconds ago)
        let probe_start = probe_traffic_waveform(&snapshot, 0.0).unwrap();
        assert_eq!(probe_start.sample_index, 0);
        assert_eq!(probe_start.upload_bps, 1_000.0);
        assert_eq!(probe_start.format_upload(), "1.0 KB/s");
        assert_eq!(probe_start.format_download(), "2.0 KB/s");
        assert_eq!(probe_start.seconds_ago, 2.0);
        assert_eq!(probe_start.format_time_label(), "2 秒前");

        // Probe end (newest: index 2, 0 seconds ago)
        let probe_end = probe_traffic_waveform(&snapshot, 1.0).unwrap();
        assert_eq!(probe_end.sample_index, 2);
        assert_eq!(probe_end.upload_bps, 12_000_000_000.0);
        assert_eq!(probe_end.format_upload(), "12.0 GB/s");
        assert_eq!(probe_end.format_download(), "30.0 GB/s");
        assert_eq!(probe_end.seconds_ago, 0.0);
        assert_eq!(probe_end.format_time_label(), "当前");
        assert!(probe_end.format_tooltip().contains("当前"));

        // Rate format edge cases
        assert_eq!(format_rate_standard(-5.0), "0 B/s");
        assert_eq!(format_rate_standard(f64::NAN), "0 B/s");
        assert_eq!(format_rate_standard(999.0), "999 B/s");
    }

    #[test]
    fn traffic_projector_probe_at_pixel() {
        let mut projector = TrafficProjector::new();
        assert!(projector.probe_at_pixel(50.0, 100.0).is_none());

        projector.observe(&snapshot(&[10.0, 50.0, 100.0]), TrafficSeries::Download);
        let probe_mid = projector.probe_at_pixel(50.0, 100.0).unwrap();
        assert_eq!(probe_mid.sample_index, 1);
        assert_eq!(probe_mid.download_bps, 50.0);

        // Invalid pixel or width
        assert!(projector.probe_at_pixel(50.0, 0.0).is_none());
        assert!(projector.probe_at_pixel(f64::NAN, 100.0).is_none());
    }
}

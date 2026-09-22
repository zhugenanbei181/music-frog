//! Instantaneous per-connection rates (DUAL-13-10 / DUAL-13-12).
//!
//! The Mihomo controller reports only cumulative byte counters per connection
//! (`uploadTotal` / `downloadTotal`), so the instantaneous rate of a
//! connection is the honest difference of two successive observations of the
//! same id over a measured interval. Both surfaces therefore need the same
//! derivation and the same threshold:
//!
//! * the shared application reader publishes the derived bytes-per-second into
//!   the connections read model for the Bevy surface, and
//! * the Iced runtime stream feeds the same [`ConnectionRateDiffer`] with its
//!   own successive snapshots.
//!
//! Nothing here invents a rate: a first observation, a non-positive interval,
//! or a counter regression all report `0.0` rather than a fabricated value.

use crate::connection_view::ConnectionView;
use std::collections::BTreeMap;
use std::time::Instant;

/// Instantaneous bandwidth at or above which a connection row renders the
/// high-throughput pulse (5 MB/s, the product threshold of DUAL-13-10).
pub const HIGH_THROUGHPUT_THRESHOLD_BPS: f64 = 5.0 * 1024.0 * 1024.0;

/// Breathing frequency of the pulse (one full breath per 1.25 s). Both the
/// Iced frame tick and the Bevy animation system advance their phase with
/// this one rate, so the two breaths cannot drift apart.
pub const PULSE_BREATH_HZ: f32 = 0.8;

/// Lowest glow intensity of the breathing pulse.
pub const PULSE_MIN_INTENSITY: f32 = 0.25;

/// Highest glow intensity of the breathing pulse.
pub const PULSE_MAX_INTENSITY: f32 = 0.8;

/// One connection's instantaneous transfer rates in bytes per second.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConnectionRate {
    pub upload_bps: f64,
    pub download_bps: f64,
}

impl ConnectionRate {
    /// The larger of the two directions, the direction that decides whether a
    /// row pulses.
    pub fn peak_bps(self) -> f64 {
        self.upload_bps.max(self.download_bps)
    }

    /// Whether either direction crosses the shared high-throughput threshold.
    pub fn is_high_throughput(self) -> bool {
        is_high_throughput(self.upload_bps, self.download_bps)
    }
}

/// Whether either direction crosses [`HIGH_THROUGHPUT_THRESHOLD_BPS`].
pub fn is_high_throughput(upload_bps: f64, download_bps: f64) -> bool {
    upload_bps.max(download_bps) >= HIGH_THROUGHPUT_THRESHOLD_BPS
}

/// The breathing glow intensity for one row at `phase` (`0.0..1.0`, one full
/// breath per unit). Below the threshold the row does not pulse at all, so
/// the two surfaces cannot glow for a slow connection.
pub fn pulse_intensity(upload_bps: f64, download_bps: f64, phase: f32) -> f32 {
    if !is_high_throughput(upload_bps, download_bps) {
        return 0.0;
    }
    let breath = 0.5 - 0.5 * (phase.clamp(0.0, 1.0) * std::f32::consts::TAU).cos();
    PULSE_MIN_INTENSITY + (PULSE_MAX_INTENSITY - PULSE_MIN_INTENSITY) * breath
}

/// Byte-counter delta over a known interval. A first observation, a
/// non-positive or non-finite interval, and a counter reset have no honest
/// rate and report `0.0`.
pub fn instantaneous_rate(previous_total: u64, current_total: u64, elapsed_secs: f64) -> f64 {
    if !elapsed_secs.is_finite() || elapsed_secs <= 0.0 || current_total < previous_total {
        return 0.0;
    }
    (current_total - previous_total) as f64 / elapsed_secs
}

/// The rate book published for one connection snapshot, keyed by connection
/// id. A connection without a derived rate reads as zero.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConnectionRates {
    rates: BTreeMap<String, ConnectionRate>,
}

impl ConnectionRates {
    /// The rate of `id`, or the honest zero when it was not observed in both
    /// windows of the derivation.
    pub fn get(&self, id: &str) -> ConnectionRate {
        self.rates.get(id).copied().unwrap_or_default()
    }

    /// Number of connections carrying a rate entry.
    pub fn len(&self) -> usize {
        self.rates.len()
    }

    /// Whether no connection carries a rate entry.
    pub fn is_empty(&self) -> bool {
        self.rates.is_empty()
    }

    /// The connection ids of the book, in stable id order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.rates.keys().map(String::as_str)
    }

    /// Build a book from explicit pairs; used by surfaces and tests that
    /// already own a rate map.
    pub fn from_pairs<I: IntoIterator<Item = (String, ConnectionRate)>>(pairs: I) -> Self {
        Self {
            rates: pairs.into_iter().collect(),
        }
    }

    /// Whether any connection in the book crosses the shared threshold.
    pub fn has_high_throughput(&self) -> bool {
        self.rates.values().any(|rate| rate.is_high_throughput())
    }

    /// The strongest glow intensity across the book for `phase`, or `0.0`
    /// when nothing is above the threshold.
    pub fn peak_pulse_intensity(&self, phase: f32) -> f32 {
        self.rates
            .values()
            .map(|rate| pulse_intensity(rate.upload_bps, rate.download_bps, phase))
            .fold(0.0_f32, f32::max)
    }
}

/// Per-connection cumulative-counter differ. Each observation stores the
/// totals together with the observation instant, so the next observation
/// divides the delta by the real elapsed time instead of an assumed tick.
#[derive(Debug, Default)]
pub struct ConnectionRateDiffer {
    observed_at: Option<Instant>,
    totals: BTreeMap<String, (u64, u64)>,
}

impl ConnectionRateDiffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Observe one connection snapshot at `now` and return the rates valid at
    /// that instant. Connections seen for the first time (or after a gap)
    /// report zero, so a new connection never shows an invented burst.
    pub fn observe<C: ConnectionView>(&mut self, now: Instant, conns: &[C]) -> ConnectionRates {
        let window = self
            .observed_at
            .map(|previous| now.saturating_duration_since(previous).as_secs_f64())
            .filter(|seconds| seconds.is_finite() && *seconds > 0.0);

        let mut rates = BTreeMap::new();
        let mut next_totals = BTreeMap::new();
        for conn in conns {
            let id = conn.view_id();
            let upload = conn.view_upload_total();
            let download = conn.view_download_total();
            let rate = match (window, self.totals.get(id)) {
                (Some(elapsed), Some((previous_upload, previous_download))) => ConnectionRate {
                    upload_bps: instantaneous_rate(*previous_upload, upload, elapsed),
                    download_bps: instantaneous_rate(*previous_download, download, elapsed),
                },
                _ => ConnectionRate::default(),
            };
            rates.insert(id.to_string(), rate);
            next_totals.insert(id.to_string(), (upload, download));
        }

        self.observed_at = Some(now);
        self.totals = next_totals;
        ConnectionRates { rates }
    }

    /// Instant of the last observation, when one happened.
    pub fn observed_at(&self) -> Option<Instant> {
        self.observed_at
    }

    /// Number of connections currently tracked for diffing.
    pub fn tracked(&self) -> usize {
        self.totals.len()
    }

    /// Forget every counter window; the next observation reports zero rates.
    pub fn reset(&mut self) {
        self.observed_at = None;
        self.totals.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::Connection;
    use std::time::Duration;

    fn connection(id: &str, up: u64, down: u64) -> Connection {
        Connection {
            id: id.to_string(),
            upload: up,
            download: down,
            ..Connection::default()
        }
    }

    #[test]
    fn first_observation_never_fabricates_a_rate() {
        let base = Instant::now();
        let mut differ = ConnectionRateDiffer::new();
        let rates = differ.observe(base, &[connection("c1", 10_000, 20_000)]);
        assert_eq!(rates.get("c1"), ConnectionRate::default());
        assert_eq!(differ.tracked(), 1);
        assert_eq!(differ.observed_at(), Some(base));
    }

    #[test]
    fn second_observation_divides_the_delta_by_the_measured_interval() {
        let base = Instant::now();
        let mut differ = ConnectionRateDiffer::new();
        differ.observe(base, &[connection("c1", 1_000, 2_000)]);
        let rates = differ.observe(
            base + Duration::from_secs(2),
            &[connection("c1", 3_000, 10_000)],
        );
        assert_eq!(
            rates.get("c1"),
            ConnectionRate {
                upload_bps: 1_000.0,
                download_bps: 4_000.0,
            }
        );
    }

    #[test]
    fn new_connection_and_counter_reset_report_zero() {
        let base = Instant::now();
        let mut differ = ConnectionRateDiffer::new();
        differ.observe(base, &[connection("c1", 5_000, 5_000)]);
        let rates = differ.observe(
            base + Duration::from_secs(1),
            &[connection("c1", 1, 1), connection("c2", 9_000, 9_000)],
        );
        assert_eq!(rates.get("c1"), ConnectionRate::default());
        assert_eq!(rates.get("c2"), ConnectionRate::default());
    }

    #[test]
    fn a_vanished_connection_forgets_its_window() {
        let base = Instant::now();
        let mut differ = ConnectionRateDiffer::new();
        differ.observe(base, &[connection("c1", 0, 0)]);
        let empty: Vec<Connection> = Vec::new();
        differ.observe(base + Duration::from_secs(1), &empty);
        assert_eq!(differ.tracked(), 0);
        let rates = differ.observe(base + Duration::from_secs(2), &[connection("c1", 5, 5)]);
        assert_eq!(rates.get("c1"), ConnectionRate::default());
    }

    #[test]
    fn zero_interval_and_reset_report_zero() {
        let base = Instant::now();
        let mut differ = ConnectionRateDiffer::new();
        differ.observe(base, &[connection("c1", 1, 1)]);
        let rates = differ.observe(base, &[connection("c1", 2, 2)]);
        assert_eq!(rates.get("c1"), ConnectionRate::default());
        differ.reset();
        assert_eq!(differ.tracked(), 0);
        let rates = differ.observe(base + Duration::from_secs(1), &[connection("c1", 3, 3)]);
        assert_eq!(rates.get("c1"), ConnectionRate::default());
    }

    #[test]
    fn instantaneous_rate_rejects_bad_windows() {
        assert_eq!(instantaneous_rate(0, 1_000, 2.0), 500.0);
        assert_eq!(instantaneous_rate(1_000, 1_000, 2.0), 0.0);
        assert_eq!(instantaneous_rate(2_000, 1_000, 2.0), 0.0);
        assert_eq!(instantaneous_rate(0, 1_000, 0.0), 0.0);
        assert_eq!(instantaneous_rate(0, 1_000, -1.0), 0.0);
        assert_eq!(instantaneous_rate(0, 1_000, f64::NAN), 0.0);
    }

    #[test]
    fn threshold_and_pulse_only_react_to_real_high_throughput() {
        assert!(!is_high_throughput(0.0, 0.0));
        assert!(!is_high_throughput(
            HIGH_THROUGHPUT_THRESHOLD_BPS - 1.0,
            0.0
        ));
        assert!(is_high_throughput(0.0, HIGH_THROUGHPUT_THRESHOLD_BPS));

        // One breath at the shared frequency advances exactly one phase unit.
        let phase = (0.5_f32 + 1.25 * PULSE_BREATH_HZ).fract();
        assert!(
            (phase - 0.5).abs() < 1e-3,
            "1.25 s of breathing must wrap back to the same phase, got {phase}"
        );
        assert_eq!(pulse_intensity(1_000.0, 1_000.0, 0.0), 0.0);

        // A full breath reaches the maximum; the phase midpoint is the peak.
        let low = pulse_intensity(HIGH_THROUGHPUT_THRESHOLD_BPS, 0.0, 0.0);
        let high = pulse_intensity(HIGH_THROUGHPUT_THRESHOLD_BPS, 0.0, 0.5);
        assert_eq!(low, PULSE_MIN_INTENSITY);
        assert!((high - PULSE_MAX_INTENSITY).abs() < 1e-5);
        assert!(low < high);
        // The phase wraps: 1.0 is the same breath position as 0.0.
        assert!((pulse_intensity(HIGH_THROUGHPUT_THRESHOLD_BPS, 0.0, 1.0) - low).abs() < 1e-5);
    }

    #[test]
    fn rate_book_reports_high_throughput_and_peak_glow() {
        let empty = ConnectionRates::default();
        assert!(empty.is_empty());
        assert!(!empty.has_high_throughput());
        assert_eq!(empty.get("missing"), ConnectionRate::default());
        assert_eq!(empty.peak_pulse_intensity(0.5), 0.0);

        let book = ConnectionRates::from_pairs([
            (
                "slow".to_string(),
                ConnectionRate {
                    upload_bps: 1_000.0,
                    download_bps: 2_000.0,
                },
            ),
            (
                "fast".to_string(),
                ConnectionRate {
                    upload_bps: 100.0,
                    download_bps: HIGH_THROUGHPUT_THRESHOLD_BPS,
                },
            ),
        ]);
        assert_eq!(book.len(), 2);
        assert!(!book.is_empty());
        assert!(book.has_high_throughput());
        assert_eq!(book.ids().collect::<Vec<_>>(), vec!["fast", "slow"]);
        assert!((book.peak_pulse_intensity(0.5) - PULSE_MAX_INTENSITY).abs() < 1e-5);
        assert_eq!(book.get("fast").peak_bps(), HIGH_THROUGHPUT_THRESHOLD_BPS);
        assert!(book.get("fast").is_high_throughput());
        assert!(!book.get("slow").is_high_throughput());
    }
}

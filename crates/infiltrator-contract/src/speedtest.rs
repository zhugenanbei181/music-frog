//! Shared contracts and models for latency testing, jitter variance analysis,
//! packet loss gradient rating, and bandwidth throughput evaluation.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Packet loss gradient rating reflecting connection quality tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PacketLossRating {
    /// 0% packet loss - optimal lossless transmission.
    Excellent,
    /// (0%, 5%] - low loss within normal operating tolerance.
    Good,
    /// (5%, 20%] - moderate loss, potential stutter and retransmissions.
    Fair,
    /// > 20% - poor connection, heavy degradation.
    Poor,
    /// 100% loss - node unreachable, connection refused, or timed out.
    Dead,
}

impl PacketLossRating {
    /// Classify packet loss percent into strict gradient tiers.
    pub fn from_loss_percent(percent: f64) -> Self {
        if !percent.is_finite() || percent >= 100.0 {
            Self::Dead
        } else if percent <= 0.0001 {
            Self::Excellent
        } else if percent <= 5.0 {
            Self::Good
        } else if percent <= 20.0 {
            Self::Fair
        } else {
            Self::Poor
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Excellent => "极佳 (0%)",
            Self::Good => "良好 (<5%)",
            Self::Fair => "一般 (5-20%)",
            Self::Poor => "较差 (>20%)",
            Self::Dead => "超时 (100%)",
        }
    }

    pub const fn label_en(self) -> &'static str {
        match self {
            Self::Excellent => "Excellent (0%)",
            Self::Good => "Good (<5%)",
            Self::Fair => "Fair (5-20%)",
            Self::Poor => "Poor (>20%)",
            Self::Dead => "Timeout (100%)",
        }
    }

    pub const fn is_healthy(self) -> bool {
        matches!(self, Self::Excellent | Self::Good)
    }

    pub const fn is_alive(self) -> bool {
        !matches!(self, Self::Dead)
    }
}

/// Precise RTT statistics, sample variance, and jitter calculations.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JitterCalculation {
    pub sample_count: usize,
    pub successful_probes: usize,
    pub lost_probes: usize,
    pub loss_percent: f64,
    pub loss_rating: PacketLossRating,
    pub mean_rtt_ms: f64,
    pub min_rtt_ms: Option<u32>,
    pub max_rtt_ms: Option<u32>,
    /// Sample standard deviation of round-trip times (RTT) in milliseconds.
    pub std_dev_ms: f64,
    /// Mean absolute consecutive difference in milliseconds.
    pub jitter_ms: f64,
    /// RFC 3550 EWMA smoothed jitter estimator in milliseconds.
    pub rfc3550_jitter_ms: f64,
    /// Chronological probe results (None indicates a dropped/timed-out packet).
    pub samples: Vec<Option<u32>>,
    /// 0..=100 composite node stability score.
    pub stability_score: u8,
    /// 1..=5 star rating.
    pub star_rating: u8,
}

impl JitterCalculation {
    /// Compute exact jitter, sample standard deviation, and packet loss rating from probe samples.
    pub fn from_samples(samples: &[Option<u32>]) -> Self {
        let sample_count = samples.len();
        if sample_count == 0 {
            return Self {
                sample_count: 0,
                successful_probes: 0,
                lost_probes: 0,
                loss_percent: 0.0,
                loss_rating: PacketLossRating::Excellent,
                mean_rtt_ms: 0.0,
                min_rtt_ms: None,
                max_rtt_ms: None,
                std_dev_ms: 0.0,
                jitter_ms: 0.0,
                rfc3550_jitter_ms: 0.0,
                samples: Vec::new(),
                stability_score: 100,
                star_rating: 5,
            };
        }

        let mut valid_rtts = Vec::with_capacity(sample_count);
        let mut lost_probes = 0;

        for &s in samples {
            if let Some(rtt) = s {
                valid_rtts.push(rtt as f64);
            } else {
                lost_probes += 1;
            }
        }

        let successful_probes = valid_rtts.len();
        let loss_percent = (lost_probes as f64 / sample_count as f64) * 100.0;
        let loss_rating = PacketLossRating::from_loss_percent(loss_percent);

        if successful_probes == 0 {
            return Self {
                sample_count,
                successful_probes: 0,
                lost_probes,
                loss_percent: 100.0,
                loss_rating: PacketLossRating::Dead,
                mean_rtt_ms: 0.0,
                min_rtt_ms: None,
                max_rtt_ms: None,
                std_dev_ms: 0.0,
                jitter_ms: 0.0,
                rfc3550_jitter_ms: 0.0,
                samples: samples.to_vec(),
                stability_score: 0,
                star_rating: 1,
            };
        }

        let sum: f64 = valid_rtts.iter().sum();
        let mean_rtt_ms = sum / successful_probes as f64;
        let min_rtt_ms = valid_rtts.iter().map(|&x| x as u32).min();
        let max_rtt_ms = valid_rtts.iter().map(|&x| x as u32).max();

        // Standard deviation (sample standard deviation with Bessel's correction)
        let std_dev_ms = if successful_probes > 1 {
            let variance = valid_rtts
                .iter()
                .map(|&x| (x - mean_rtt_ms).powi(2))
                .sum::<f64>()
                / (successful_probes - 1) as f64;
            variance.sqrt()
        } else {
            0.0
        };

        // Mean consecutive difference jitter
        let jitter_ms = if successful_probes > 1 {
            let diff_sum: f64 = valid_rtts.windows(2).map(|w| (w[1] - w[0]).abs()).sum();
            diff_sum / (successful_probes - 1) as f64
        } else {
            0.0
        };

        // RFC 3550 EWMA smoothed jitter: J = J + (|D| - J) / 16.0
        let mut rfc3550 = 0.0;
        for w in valid_rtts.windows(2) {
            let diff = (w[1] - w[0]).abs();
            rfc3550 += (diff - rfc3550) / 16.0;
        }

        // Multi-dimensional stability scoring (0..=100)
        let mut score = 100.0;
        if mean_rtt_ms > 50.0 {
            score -= ((mean_rtt_ms - 50.0) / 5.0).min(40.0);
        }
        score -= (std_dev_ms * 1.5).min(30.0);
        score -= (loss_percent * 2.0).min(50.0);
        let stability_score = score.clamp(0.0, 100.0).round() as u8;

        let star_rating = match stability_score {
            85..=100 => 5,
            70..=84 => 4,
            50..=69 => 3,
            30..=49 => 2,
            _ => 1,
        };

        Self {
            sample_count,
            successful_probes,
            lost_probes,
            loss_percent,
            loss_rating,
            mean_rtt_ms,
            min_rtt_ms,
            max_rtt_ms,
            std_dev_ms,
            jitter_ms,
            rfc3550_jitter_ms: rfc3550,
            samples: samples.to_vec(),
            stability_score,
            star_rating,
        }
    }

    pub fn demo_fixture() -> Self {
        Self::from_samples(&[Some(38), Some(41), Some(36), Some(39), Some(37)])
    }
}

/// DUAL-06-12: honest comparison between the country implied by a node label
/// and the country actually reported by a real egress probe through that node.
///
/// This is a pure classification over two already-known facts; the read model
/// never derives an egress fact it does not have. Desktop hosts that cannot
/// probe through a node leave the reported country `None`, which stays
/// [`EgressCountryMatch::Unknown`] rather than being guessed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EgressCountryMatch {
    /// No egress country was reported yet (host cannot probe through a node,
    /// or the probe has not run). Never a failure — just an unknown fact.
    Unknown,
    /// Neither the label nor the probe implies a country.
    Unlabelled,
    /// The label-derived country equals the reported egress country.
    Match,
    /// Both countries are known and differ: the node's label is misleading.
    Mismatch,
}

impl EgressCountryMatch {
    /// Compare a label-derived country with a reported egress country.
    pub fn classify(label_country: Option<&str>, egress_country: Option<&str>) -> Self {
        let label = label_country.map(str::trim).filter(|s| !s.is_empty());
        let egress = egress_country.map(str::trim).filter(|s| !s.is_empty());
        match (label, egress) {
            (_, None) => Self::Unknown,
            (None, Some(_)) => Self::Unlabelled,
            (Some(l), Some(e)) if l.eq_ignore_ascii_case(e) => Self::Match,
            (Some(_), Some(_)) => Self::Mismatch,
        }
    }

    pub const fn is_mismatch(self) -> bool {
        matches!(self, Self::Mismatch)
    }

    /// One honest token for guards and compact captions.
    pub const fn as_token(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Unlabelled => "unlabelled",
            Self::Match => "match",
            Self::Mismatch => "mismatch",
        }
    }
}

/// Lifecycle phases of the speedtest engine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestPhase {
    Idle,
    ProbingLatency,
    MeasuringBandwidth,
    Completed,
    Cancelled,
    Failed,
}

/// Target scope for a speedtest run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeedtestScope {
    AllGroups,
    SingleGroup(String),
    SingleNode(String),
}

/// Real-time progress update for UI rendering.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SpeedtestProgress {
    pub completed_nodes: usize,
    pub total_nodes: usize,
    pub percent: f64,
    pub current_node: Option<String>,
    pub current_mbps: Option<f64>,
}

/// Configuration parameters for speedtesting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeedtestTargetConfig {
    pub test_url: String,
    pub timeout_ms: u32,
    pub concurrency: usize,
    pub probe_rounds: usize,
}

impl Default for SpeedtestTargetConfig {
    fn default() -> Self {
        Self {
            test_url: "http://www.gstatic.com/generate_204".to_string(),
            timeout_ms: 5000,
            concurrency: 30,
            probe_rounds: 3,
        }
    }
}

/// Measured speedtest metrics for an individual proxy node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeSpeedtestResult {
    pub node_name: String,
    pub group_name: Option<String>,
    pub proxy_type: String,
    pub delay_ms: Option<u32>,
    pub jitter: Option<JitterCalculation>,
    pub bandwidth_mbps: Option<f64>,
    pub packet_loss: PacketLossRating,
    pub star_rating: u8,
    /// DUAL-06-12: ISO country code derived from the node's own label. This is
    /// a label fact, kept separate from the reported egress fact so the two can
    /// be compared honestly instead of conflated.
    pub label_country: Option<String>,
    /// DUAL-06-12: real egress IP reported by the host's outbound probe. `None`
    /// until a probe through the node has actually succeeded.
    pub outbound_ip: Option<String>,
    /// DUAL-06-12: real egress country reported by the host's outbound probe.
    pub outbound_country: Option<String>,
    pub is_alive: bool,
    pub tested_at_epoch_ms: u64,
}

impl NodeSpeedtestResult {
    /// Classify the label-derived country against the reported egress country.
    pub fn egress_country_match(&self) -> EgressCountryMatch {
        EgressCountryMatch::classify(
            self.label_country.as_deref(),
            self.outbound_country.as_deref(),
        )
    }

    /// Honest one-line egress endpoint caption: `ip (CC)` when reported, `—`
    /// when the host has not probed through this node.
    pub fn egress_endpoint_label(&self) -> String {
        match (
            self.outbound_ip.as_deref(),
            self.outbound_country.as_deref(),
        ) {
            (Some(ip), Some(country)) if !country.is_empty() => format!("{ip} ({country})"),
            (Some(ip), _) => ip.to_string(),
            (None, Some(country)) if !country.is_empty() => format!("— ({country})"),
            _ => "—".to_string(),
        }
    }
}

/// Historical summary entry of a completed speedtest run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HistoricalSpeedtestRecord {
    pub run_id: u64,
    pub timestamp_epoch_ms: u64,
    pub scope: SpeedtestScope,
    pub target_url: String,
    pub total_nodes: usize,
    pub alive_nodes: usize,
    pub avg_latency_ms: Option<f64>,
    pub avg_jitter_ms: Option<f64>,
    pub avg_bandwidth_mbps: Option<f64>,
    pub overall_star_rating: u8,
}

/// Shared canonical read model for concurrent speedtesting and stability telemetry.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpeedtestSnapshot {
    pub generation: u64,
    pub revision: u64,
    pub phase: SpeedtestPhase,
    pub scope: SpeedtestScope,
    pub config: SpeedtestTargetConfig,
    pub progress: SpeedtestProgress,
    pub node_results: BTreeMap<String, NodeSpeedtestResult>,
    pub recent_history: Vec<HistoricalSpeedtestRecord>,
    pub failure: Option<String>,
}

impl Default for SpeedtestSnapshot {
    fn default() -> Self {
        Self {
            generation: 0,
            revision: 0,
            phase: SpeedtestPhase::Idle,
            scope: SpeedtestScope::AllGroups,
            config: SpeedtestTargetConfig::default(),
            progress: SpeedtestProgress::default(),
            node_results: BTreeMap::new(),
            recent_history: Vec::new(),
            failure: None,
        }
    }
}

impl SpeedtestSnapshot {
    pub fn is_running(&self) -> bool {
        matches!(
            self.phase,
            SpeedtestPhase::ProbingLatency | SpeedtestPhase::MeasuringBandwidth
        )
    }

    pub fn is_drawable(&self) -> bool {
        !self.node_results.is_empty() || self.is_running()
    }

    pub fn demo_fixture() -> Self {
        let mut node_results = BTreeMap::new();

        let hk_calc = JitterCalculation::from_samples(&[Some(36), Some(38), Some(35), Some(37)]);
        node_results.insert(
            "🇭🇰 香港 01 · BGP 专线".to_string(),
            NodeSpeedtestResult {
                node_name: "🇭🇰 香港 01 · BGP 专线".to_string(),
                group_name: Some("节点选择 (PROXIES)".to_string()),
                proxy_type: "Shadowsocks".to_string(),
                delay_ms: Some(36),
                jitter: Some(hk_calc),
                bandwidth_mbps: Some(184.5),
                packet_loss: PacketLossRating::Excellent,
                star_rating: 5,
                label_country: Some("HK".to_string()),
                outbound_ip: Some("103.242.175.12".to_string()),
                outbound_country: Some("HK".to_string()),
                is_alive: true,
                tested_at_epoch_ms: 1700000000000,
            },
        );

        let jp_calc = JitterCalculation::from_samples(&[Some(65), Some(68), Some(64), Some(66)]);
        node_results.insert(
            "🇯🇵 日本东京 02 · 极速".to_string(),
            NodeSpeedtestResult {
                node_name: "🇯🇵 日本东京 02 · 极速".to_string(),
                group_name: Some("节点选择 (PROXIES)".to_string()),
                proxy_type: "Vmess".to_string(),
                delay_ms: Some(65),
                jitter: Some(jp_calc),
                bandwidth_mbps: Some(125.0),
                packet_loss: PacketLossRating::Excellent,
                star_rating: 4,
                label_country: Some("JP".to_string()),
                outbound_ip: Some("133.242.18.5".to_string()),
                outbound_country: Some("JP".to_string()),
                is_alive: true,
                tested_at_epoch_ms: 1700000000000,
            },
        );

        let dead_calc = JitterCalculation::from_samples(&[None, None, None]);
        node_results.insert(
            "💀 超时不可用节点 01".to_string(),
            NodeSpeedtestResult {
                node_name: "💀 超时不可用节点 01".to_string(),
                group_name: Some("节点选择 (PROXIES)".to_string()),
                proxy_type: "Trojan".to_string(),
                delay_ms: None,
                jitter: Some(dead_calc),
                bandwidth_mbps: None,
                packet_loss: PacketLossRating::Dead,
                star_rating: 1,
                label_country: None,
                outbound_ip: None,
                outbound_country: None,
                is_alive: false,
                tested_at_epoch_ms: 1700000000000,
            },
        );

        Self {
            generation: 1,
            revision: 1,
            phase: SpeedtestPhase::Completed,
            scope: SpeedtestScope::AllGroups,
            config: SpeedtestTargetConfig::default(),
            progress: SpeedtestProgress {
                completed_nodes: 3,
                total_nodes: 3,
                percent: 100.0,
                current_node: None,
                current_mbps: None,
            },
            node_results,
            recent_history: vec![HistoricalSpeedtestRecord {
                run_id: 1,
                timestamp_epoch_ms: 1700000000000,
                scope: SpeedtestScope::AllGroups,
                target_url: "http://www.gstatic.com/generate_204".to_string(),
                total_nodes: 3,
                alive_nodes: 2,
                avg_latency_ms: Some(50.5),
                avg_jitter_ms: Some(1.5),
                avg_bandwidth_mbps: Some(154.75),
                overall_star_rating: 5,
            }],
            failure: None,
        }
    }

    pub fn node_count(&self) -> usize {
        self.node_results.len()
    }

    pub fn alive_nodes_count(&self) -> usize {
        self.node_results.values().filter(|n| n.is_alive).count()
    }

    pub fn dead_nodes(&self) -> Vec<&NodeSpeedtestResult> {
        self.node_results.values().filter(|n| !n.is_alive).collect()
    }

    /// DUAL-06-12: nodes whose label country and reported egress country are
    /// both known and disagree. Derived purely from the published results.
    pub fn egress_country_mismatches(&self) -> Vec<&NodeSpeedtestResult> {
        self.node_results
            .values()
            .filter(|n| n.egress_country_match().is_mismatch())
            .collect()
    }

    /// DUAL-06-12: how many nodes have a real reported egress IP.
    pub fn egress_reported_count(&self) -> usize {
        self.node_results
            .values()
            .filter(|n| n.outbound_ip.is_some())
            .count()
    }

    /// DUAL-06-12: compact read-model caption for the egress comparison, e.g.
    /// `出口 2/3 · 不一致 1`. Honest zero/empty facts are represented literally.
    pub fn egress_summary(&self) -> String {
        format!(
            "出口 {}/{} · 不一致 {}",
            self.egress_reported_count(),
            self.node_count(),
            self.egress_country_mismatches().len()
        )
    }

    pub fn fastest_node(&self) -> Option<&NodeSpeedtestResult> {
        self.node_results
            .values()
            .filter(|n| n.is_alive && n.delay_ms.is_some())
            .min_by_key(|n| n.delay_ms.unwrap_or(u32::MAX))
    }

    pub fn sorted_by_latency(&self) -> Vec<&NodeSpeedtestResult> {
        let mut list: Vec<&NodeSpeedtestResult> = self.node_results.values().collect();
        list.sort_by(|a, b| match (a.delay_ms, b.delay_ms) {
            (Some(da), Some(db)) => da.cmp(&db),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.node_name.cmp(&b.node_name),
        });
        list
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_loss_gradient_ratings() {
        assert_eq!(
            PacketLossRating::from_loss_percent(0.0),
            PacketLossRating::Excellent
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(0.00005),
            PacketLossRating::Excellent
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(1.5),
            PacketLossRating::Good
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(5.0),
            PacketLossRating::Good
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(5.1),
            PacketLossRating::Fair
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(15.0),
            PacketLossRating::Fair
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(20.0),
            PacketLossRating::Fair
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(20.1),
            PacketLossRating::Poor
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(50.0),
            PacketLossRating::Poor
        );
        assert_eq!(
            PacketLossRating::from_loss_percent(100.0),
            PacketLossRating::Dead
        );
    }

    #[test]
    fn test_jitter_calculation_known_samples() {
        // 4 successful probes: 100, 110, 90, 100
        // Mean = 100.0
        // Diff from mean: 0, 10, -10, 0 -> squared: 0, 100, 100, 0 = 200
        // Variance = 200 / 3 = 66.666...
        // Std Dev = sqrt(66.666...) ≈ 8.165
        let calc = JitterCalculation::from_samples(&[Some(100), Some(110), Some(90), Some(100)]);

        assert_eq!(calc.sample_count, 4);
        assert_eq!(calc.successful_probes, 4);
        assert_eq!(calc.lost_probes, 0);
        assert_eq!(calc.loss_percent, 0.0);
        assert_eq!(calc.loss_rating, PacketLossRating::Excellent);
        assert!((calc.mean_rtt_ms - 100.0).abs() < 1e-4);
        assert_eq!(calc.min_rtt_ms, Some(90));
        assert_eq!(calc.max_rtt_ms, Some(110));
        assert!((calc.std_dev_ms - 8.16496).abs() < 1e-3);

        // Jitter: (|110-100| + |90-110| + |100-90|) / 3 = (10 + 20 + 10) / 3 = 13.333...
        assert!((calc.jitter_ms - 13.3333).abs() < 1e-3);
    }

    #[test]
    fn test_jitter_calculation_with_packet_loss() {
        let calc = JitterCalculation::from_samples(&[Some(50), None, Some(60), Some(55), None]);

        assert_eq!(calc.sample_count, 5);
        assert_eq!(calc.successful_probes, 3);
        assert_eq!(calc.lost_probes, 2);
        assert_eq!(calc.loss_percent, 40.0);
        assert_eq!(calc.loss_rating, PacketLossRating::Poor);
        assert_eq!(calc.min_rtt_ms, Some(50));
        assert_eq!(calc.max_rtt_ms, Some(60));
    }

    #[test]
    fn test_speedtest_snapshot_fixture() {
        let snapshot = SpeedtestSnapshot::demo_fixture();
        assert!(snapshot.is_drawable());
        assert!(!snapshot.is_running());
        assert_eq!(snapshot.node_count(), 3);
        assert_eq!(snapshot.alive_nodes_count(), 2);
        assert_eq!(snapshot.dead_nodes().len(), 1);
        assert_eq!(
            snapshot.fastest_node().unwrap().node_name,
            "🇭🇰 香港 01 · BGP 专线"
        );
    }

    #[test]
    fn test_egress_country_match_classification() {
        assert_eq!(
            EgressCountryMatch::classify(None, None),
            EgressCountryMatch::Unknown
        );
        assert_eq!(
            EgressCountryMatch::classify(Some("HK"), None),
            EgressCountryMatch::Unknown
        );
        assert_eq!(
            EgressCountryMatch::classify(None, Some("US")),
            EgressCountryMatch::Unlabelled
        );
        assert_eq!(
            EgressCountryMatch::classify(Some("hk"), Some("HK")),
            EgressCountryMatch::Match
        );
        assert_eq!(
            EgressCountryMatch::classify(Some("HK"), Some("US")),
            EgressCountryMatch::Mismatch
        );
        assert!(EgressCountryMatch::Mismatch.is_mismatch());
        assert_eq!(EgressCountryMatch::Mismatch.as_token(), "mismatch");
    }

    #[test]
    fn test_snapshot_egress_summary_from_fixture() {
        let snapshot = SpeedtestSnapshot::demo_fixture();
        // Two nodes reported a real egress IP; neither label contradicts it.
        assert_eq!(snapshot.egress_reported_count(), 2);
        assert!(snapshot.egress_country_mismatches().is_empty());
        assert_eq!(snapshot.egress_summary(), "出口 2/3 · 不一致 0");
    }
}

//! The globe's frame and memory report, bound to the source identity.
//!
//! **Identity contract.** A performance report is only evidence about the
//! thing it measured. The report therefore carries the *source identity* of
//! the globe it was measured on: the fingerprint of the nodes, their real
//! geographic observations, the links between them, the rendering mode and
//! the raster size. The view (the globe rotates) and the flow phase (the
//! traffic animates) are deliberately *not* part of the identity — the
//! source is the source.
//!
//! A baseline carries an identity too. Comparing a report against a baseline
//! of another source is not a pass and not a failure: it is
//! [`GlobeBudgetVerdict::OtherSource`], because the two are not comparable.

use crate::globe::stage::GlobeStage;
use crate::globe::{
    GeoLocation, GeoPrecision, GeoSource, GeoUnknown, GlobeMode, GlobeNodeCategory, GlobePlate,
    GlobeSpec,
};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::time::Time;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// The identity of the globe source a report is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlobeSourceIdentity(u64);

impl GlobeSourceIdentity {
    /// The identity of one spec's source.
    pub fn of(spec: &GlobeSpec) -> Self {
        let mut hash = FNV_OFFSET;
        hash_bytes(&mut hash, &[mode_code(spec.mode)]);
        hash_bytes(&mut hash, &spec.width.to_le_bytes());
        hash_bytes(&mut hash, &spec.height.to_le_bytes());
        for node in &spec.nodes {
            hash_bytes(&mut hash, &[category_code(node.category)]);
            hash_str(&mut hash, &node.id);
            hash_str(&mut hash, &node.label);
            hash_f64(&mut hash, f64::from(node.weight));
            match node.location {
                GeoLocation::Known(point) => {
                    hash_bytes(&mut hash, &[1]);
                    hash_f64(&mut hash, point.lat_deg());
                    hash_f64(&mut hash, point.lon_deg());
                    hash_bytes(&mut hash, &[source_code(point.source())]);
                    hash_bytes(&mut hash, &[precision_code(point.precision())]);
                }
                GeoLocation::Unknown(reason) => {
                    hash_bytes(&mut hash, &[0]);
                    hash_bytes(&mut hash, &[unknown_code(reason)]);
                }
            }
        }
        for link in &spec.links {
            hash_str(&mut hash, &link.source_id);
            hash_str(&mut hash, &link.target_id);
            hash_f64(&mut hash, link.bandwidth_bps);
            hash_bytes(&mut hash, &[u8::from(link.highlighted)]);
        }
        Self(hash)
    }

    /// The fingerprint value, for a report file or a log line.
    pub fn as_u64(self) -> u64 {
        self.0
    }
}

/// One measured window of the globe's frame and memory cost.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeFrameReport {
    /// The source identity the measurement is bound to.
    pub source: GlobeSourceIdentity,
    /// The rendering mode that was measured.
    pub mode: GlobeMode,
    /// The frames the host rendered while the window was open.
    pub frames: u32,
    /// The wall-clock seconds those frames took.
    pub elapsed_seconds: f32,
    /// The peak raster bytes the enhancement owned during the window.
    pub owned_bytes: u64,
}

impl GlobeFrameReport {
    /// The report of one measured window, bound to the spec's source identity.
    pub fn from_window(spec: &GlobeSpec, window: GlobeFrameWindow) -> Self {
        Self {
            source: GlobeSourceIdentity::of(spec),
            mode: spec.mode,
            frames: window.frames,
            elapsed_seconds: window.elapsed_seconds,
            owned_bytes: window.owned_bytes,
        }
    }

    /// The measured frames per second.
    pub fn fps(&self) -> f32 {
        if self.elapsed_seconds <= 0.0 {
            0.0
        } else {
            self.frames as f32 / self.elapsed_seconds
        }
    }

    /// Whether this report describes the given source.
    pub fn describes(&self, spec: &GlobeSpec) -> bool {
        self.source == GlobeSourceIdentity::of(spec)
    }

    /// Compare this report against one measured baseline.
    pub fn compare(&self, baseline: &GlobeBaseline) -> GlobeBudgetVerdict {
        if self.source != baseline.source {
            return GlobeBudgetVerdict::OtherSource {
                report_source: self.source,
                baseline_source: baseline.source,
            };
        }
        let measured_fps = self.fps();
        if measured_fps < baseline.min_fps {
            return GlobeBudgetVerdict::FrameRateBelow {
                measured_fps,
                baseline_fps: baseline.min_fps,
            };
        }
        if self.owned_bytes > baseline.max_owned_bytes {
            return GlobeBudgetVerdict::MemoryAbove {
                measured_bytes: self.owned_bytes,
                baseline_bytes: baseline.max_owned_bytes,
            };
        }
        GlobeBudgetVerdict::WithinBudget
    }
}

/// The measured baseline a report is compared against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlobeBaseline {
    /// The source identity the baseline was measured on.
    pub source: GlobeSourceIdentity,
    /// The frame-rate floor the baseline allows.
    pub min_fps: f32,
    /// The raster ceiling the baseline allows, in bytes.
    pub max_owned_bytes: u64,
}

impl GlobeBaseline {
    /// The baseline of one spec's source.
    pub fn of(spec: &GlobeSpec, min_fps: f32, max_owned_bytes: u64) -> Self {
        Self {
            source: GlobeSourceIdentity::of(spec),
            min_fps,
            max_owned_bytes,
        }
    }
}

/// The verdict of comparing one report against one baseline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlobeBudgetVerdict {
    /// The report stays inside the baseline budget.
    WithinBudget,
    /// The measured frame rate is below the baseline floor.
    FrameRateBelow {
        measured_fps: f32,
        baseline_fps: f32,
    },
    /// The measured raster is above the baseline ceiling.
    MemoryAbove {
        measured_bytes: u64,
        baseline_bytes: u64,
    },
    /// The baseline belongs to another source identity: the two are not
    /// comparable, so the report is no verdict at all.
    OtherSource {
        report_source: GlobeSourceIdentity,
        baseline_source: GlobeSourceIdentity,
    },
}

/// The accumulating frame evidence of one measurement window.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlobeFrameWindow {
    /// The frames counted in the window.
    pub frames: u32,
    /// The wall-clock seconds the window covered.
    pub elapsed_seconds: f32,
    /// The peak raster bytes the enhancement owned during the window.
    pub owned_bytes: u64,
}

/// The globe's measurement state: one typed value.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub enum GlobeMeasurement {
    /// No window is open; nothing is counted.
    #[default]
    Idle,
    /// The open window accumulates the frames and the elapsed time.
    Open(GlobeFrameWindow),
}

impl GlobeMeasurement {
    /// Open a measurement window, discarding any previous one.
    pub fn open(self) -> Self {
        Self::Open(GlobeFrameWindow::default())
    }

    /// Close the window and take the measured evidence.
    pub fn close(self) -> Option<GlobeFrameWindow> {
        match self {
            Self::Open(window) => Some(window),
            Self::Idle => None,
        }
    }

    /// Count one frame of the open window.
    pub fn record_frame(self, delta_seconds: f32, owned_bytes: u64) -> Self {
        match self {
            Self::Idle => self,
            Self::Open(mut window) => {
                window.frames = window.frames.saturating_add(1);
                window.elapsed_seconds += delta_seconds.max(0.0);
                window.owned_bytes = window.owned_bytes.max(owned_bytes);
                Self::Open(window)
            }
        }
    }
}

/// Count the frames of the open measurement window and the raster bytes the
/// enhancement owns while it runs.
///
/// The window counts the *host's* frames, so a window measured with the
/// enhancement closed and a window measured with it displayed are directly
/// comparable: the frames are the same, the owned bytes are not.
pub fn sync_globe_measurement(
    time: Res<Time>,
    plates: Query<(&GlobePlate, Option<&GlobeStage>)>,
    mut measurement: ResMut<GlobeMeasurement>,
) {
    let mut owned_bytes = 0u64;
    for (plate, stage) in &plates {
        if stage.is_none_or(|stage| stage.is_live()) {
            owned_bytes = owned_bytes.saturating_add(plate.0.raster_bytes());
        }
    }
    let current = *measurement;
    let next = current.record_frame(time.delta_secs(), owned_bytes);
    if next != current {
        *measurement = next;
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_bytes(hash, value.as_bytes());
    hash_bytes(hash, &[0]);
}

fn hash_f64(hash: &mut u64, value: f64) {
    hash_bytes(hash, &value.to_bits().to_le_bytes());
}

fn mode_code(mode: GlobeMode) -> u8 {
    match mode {
        GlobeMode::Globe => 0,
        GlobeMode::Flat2d => 1,
        GlobeMode::Eco => 2,
    }
}

fn category_code(category: GlobeNodeCategory) -> u8 {
    match category {
        GlobeNodeCategory::Home => 0,
        GlobeNodeCategory::Relay => 1,
        GlobeNodeCategory::Exit => 2,
        GlobeNodeCategory::Direct => 3,
        GlobeNodeCategory::Reject => 4,
    }
}

fn source_code(source: GeoSource) -> u8 {
    match source {
        GeoSource::GeoIpDatabase => 0,
        GeoSource::Configured => 1,
        GeoSource::EndpointReported => 2,
    }
}

fn precision_code(precision: GeoPrecision) -> u8 {
    match precision {
        GeoPrecision::Country => 0,
        GeoPrecision::City => 1,
        GeoPrecision::Exact => 2,
    }
}

fn unknown_code(reason: GeoUnknown) -> u8 {
    match reason {
        GeoUnknown::NotObserved => 0,
        GeoUnknown::SourceEmpty => 1,
        GeoUnknown::SourceUnavailable => 2,
        GeoUnknown::CountryOnly => 3,
    }
}

//! The globe enhancement's own stage: live, asleep while idle, closed.
//!
//! One typed stage owns the enhancement's whole lifecycle, and it is the
//! single source of truth for "does this enhancement own a raster right
//! now". A stage that owns no raster releases the texture it had, so
//! idling, closing and leaving the page all release the same resources
//! through the same path — the repaint seam reads the stage and never keeps
//! a raster a stage does not own.
//!
//! The stage also carries the idle clock, so "idle" has exactly one owner
//! and no page has to keep a second timer next to it.

use crate::globe::{GlobePlate, GlobeSpec};
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res};
use bevy::time::Time;

/// The default idle budget: how long the globe may stay untouched before it
/// sleeps and releases its raster.
pub const GLOBE_IDLE_SLEEP_SECONDS: f32 = 20.0;

/// The idle budget the stage reads.
///
/// A host may lower it (a background page) or raise it (a kiosk demo); the
/// value lives in exactly one place.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct GlobeIdleBudget(pub f32);

impl Default for GlobeIdleBudget {
    fn default() -> Self {
        Self(GLOBE_IDLE_SLEEP_SECONDS)
    }
}

/// What the globe spends on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlobeActivity {
    /// The globe animates on its own (auto-rotation or flowing traffic).
    Busy,
    /// The globe is at rest and only waits for a pointer.
    Idle,
}

/// The globe enhancement's own stage.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub enum GlobeStage {
    /// The enhancement is live: it owns a raster texture and paints.
    Live { idle_seconds: f32 },
    /// The enhancement slept: it released its raster and paints nothing.
    Sleeping,
    /// The enhancement is closed: it released its raster and stays closed.
    Closed,
}

impl Default for GlobeStage {
    fn default() -> Self {
        Self::Live { idle_seconds: 0.0 }
    }
}

impl GlobeStage {
    /// Advance the idle clock.
    ///
    /// A globe that spends work on its own is not idle. An untouched globe
    /// sleeps once the idle budget is spent; a sleeping or closed enhancement
    /// stays as it is.
    pub fn tick(self, delta_seconds: f32, budget_seconds: f32, activity: GlobeActivity) -> Self {
        match self {
            Self::Live { idle_seconds } => {
                if activity == GlobeActivity::Busy {
                    return Self::Live { idle_seconds: 0.0 };
                }
                let idle = idle_seconds + delta_seconds.max(0.0);
                if idle >= budget_seconds {
                    Self::Sleeping
                } else {
                    Self::Live { idle_seconds: idle }
                }
            }
            Self::Sleeping | Self::Closed => self,
        }
    }

    /// A pointer gesture: a sleeping enhancement wakes up and the idle clock
    /// restarts. A closed enhancement stays closed — only an explicit
    /// [`GlobeStage::open`] demands it again.
    pub fn interact(self) -> Self {
        match self {
            Self::Live { .. } | Self::Sleeping => Self::Live { idle_seconds: 0.0 },
            Self::Closed => Self::Closed,
        }
    }

    /// Close the enhancement: it releases its raster and stays closed.
    pub fn close(self) -> Self {
        Self::Closed
    }

    /// Demand the enhancement again: it owns its raster once more.
    pub fn open(self) -> Self {
        Self::Live { idle_seconds: 0.0 }
    }

    /// Whether the enhancement owns a raster right now.
    pub fn is_live(self) -> bool {
        matches!(self, Self::Live { .. })
    }

    /// Whether the host closed the enhancement.
    pub fn is_closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

/// Advance the mounted globes' stage: an untouched enhancement sleeps and
/// releases its raster.
pub fn sync_globe_stage(
    time: Res<Time>,
    budget: Res<GlobeIdleBudget>,
    mut stages: Query<(&mut GlobeStage, &GlobePlate)>,
) {
    let delta = time.delta_secs();
    if delta <= 0.0 {
        return;
    }
    for (mut stage, plate) in &mut stages {
        let current = *stage;
        let next = current.tick(delta, budget.0, plate.0.activity());
        if next != current {
            *stage = next;
        }
    }
}

impl GlobeSpec {
    /// What the globe spends on its own: the auto-rotation and the flow.
    pub fn activity(&self) -> GlobeActivity {
        if !self.mode.animates() {
            return GlobeActivity::Idle;
        }
        if self.auto_rotate_speed_deg_per_sec != 0.0 || self.has_active_flow() {
            GlobeActivity::Busy
        } else {
            GlobeActivity::Idle
        }
    }

    /// Whether any link carries live traffic for the flow particles.
    pub fn has_active_flow(&self) -> bool {
        self.flow_speed > 0.0
            && self.links.iter().any(|link| {
                link.bandwidth_bps.is_finite() && (link.highlighted || link.bandwidth_bps > 0.0)
            })
    }
}

//! The animation coordinator (EFFECTS-029 #116–#118): every animated fact
//! of the Iced shell is one official [`iced::Animation`], redirected through
//! `go_mut` on update and sampled from `Instant` timestamps at view time —
//! never a second per-frame counter and never a hand-rolled phase table.
//!
//! The frame subscription (`subscription::frame_cadence_subscription`) stays
//! registered exactly while [`AnimationCoordinator::is_animating`] holds, so
//! a still shell ticks no frames at all (#117).

use iced::Animation;
use std::time::{Duration, Instant};

/// Route fade length; identical to the retired hand-rolled transition, so
/// the migration changes the driver, not the visual.
pub const ROUTE_FADE: Duration = Duration::from_millis(300);
/// Drawer fade length of the modal command palette.
pub const DRAWER_FADE: Duration = Duration::from_millis(200);

/// The presence of one animated surface: an official [`Animation`] driver
/// whose `bool` target is the business state, plus the projection facts
/// derived from it.
///
/// This type holds no timer and no second copy of any business state:
/// `value()` is the driver's target, `is_animating` is its phase, and all
/// progress comes from timestamps inside the driver.
#[derive(Debug, Clone)]
pub struct Presence {
    driver: Animation<bool>,
}

impl Presence {
    /// A presence settled at `presented`, timed by `fade`.
    pub fn settled(presented: bool, fade: Duration) -> Self {
        Self {
            driver: Animation::new(presented).duration(fade),
        }
    }

    /// Redirect the target through the official `go_mut` path (#116).
    pub fn redirect(&mut self, presented: bool, at: Instant) {
        self.driver.go_mut(presented, at);
    }

    /// The target this presence is animating toward.
    pub fn value(&self) -> bool {
        self.driver.value()
    }

    /// Whether a fade is still running at `at`.
    pub fn is_animating(&self, at: Instant) -> bool {
        self.driver.is_animating(at)
    }

    /// Faded amount in `0.0..=1.0` at `at`.
    pub fn opacity(&self, at: Instant) -> f32 {
        self.driver.interpolate(0.0, 1.0, at)
    }

    /// #118 exit retention: a surface stays presented while its exit fade is
    /// still running — a closing drawer keeps its exit animation instead of
    /// vanishing on the close message.
    pub fn is_presented(&self, at: Instant) -> bool {
        self.driver.value() || self.driver.is_animating(at)
    }
}

/// The shell's animation coordinator: one presence per animated fact.
#[derive(Debug, Clone)]
pub struct AnimationCoordinator {
    /// Presence of the presented route. A route change snaps it to hidden
    /// and redirects to presented, so the arriving page fades in alone —
    /// the single-page discipline of the root view stays untouched.
    route: Presence,
    /// Presence of the modal command-palette drawer (#118).
    drawer: Presence,
}

impl Default for AnimationCoordinator {
    fn default() -> Self {
        Self {
            route: Presence::settled(true, ROUTE_FADE),
            drawer: Presence::settled(false, DRAWER_FADE),
        }
    }
}

impl AnimationCoordinator {
    /// A route change: snap to hidden, then fade the arriving page in.
    pub fn route_changed(&mut self, at: Instant) {
        self.route = Presence::settled(false, ROUTE_FADE);
        self.route.redirect(true, at);
    }

    /// Route fade progress in `0.0..=1.0` — the arriving page's opacity.
    pub fn route_opacity(&self, at: Instant) -> f32 {
        self.route.opacity(at)
    }

    /// The drawer followed its business open state (`Toggle`/`Open`/`Close`
    /// messages own that state; this only redirects the driver).
    pub fn drawer_redirect(&mut self, open: bool, at: Instant) {
        self.drawer.redirect(open, at);
    }

    /// Whether the command-palette drawer must render: open, or closing
    /// while its exit fade still runs (#118).
    pub fn drawer_presented(&self, at: Instant) -> bool {
        self.drawer.is_presented(at)
    }

    /// The drawer's faded amount, applied to its scrim and card.
    pub fn drawer_opacity(&self, at: Instant) -> f32 {
        self.drawer.opacity(at)
    }

    /// Whether any driver is mid-fade — the on-demand frame gate (#117).
    pub fn is_animating(&self, at: Instant) -> bool {
        self.route.is_animating(at) || self.drawer.is_animating(at)
    }
}

//! The animation coordinator (EFFECTS-029 #116–#118): every animated fact
//! of the Iced shell is one official [`iced::Animation`], redirected through
//! `go_mut` on update and sampled from `Instant` timestamps at view time —
//! never a second per-frame counter and never a hand-rolled phase table.
//!
//! The frame subscription (`subscription::frame_cadence_subscription`) stays
//! registered exactly while [`AnimationCoordinator::is_animating`] holds, so
//! a still shell ticks no frames at all (#117).

use iced::Animation;
use infiltrator_contract::motion_spec::MotionPreference;
use infiltrator_contract::motion_spec::MotionTokens;
use infiltrator_contract::motion_spec::TransitionDirection;
use infiltrator_contract::motion_spec::TransitionPattern;
use infiltrator_presentation::motion_projection::RouteTransitionProjection;
use infiltrator_presentation::motion_projection::project_route_transition;
use std::time::{Duration, Instant};

/// Route fade length aligned with Material 3 Deliberate motion (250ms).
pub const ROUTE_FADE: Duration = Duration::from_millis(MotionTokens::DELIBERATE_MS as u64);
/// Drawer fade length of the modal command palette aligned with Material 3 Standard motion (200ms).
pub const DRAWER_FADE: Duration = Duration::from_millis(MotionTokens::STANDARD_MS as u64);

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
    /// When the current route transition started.
    route_started_at: Option<Instant>,
    /// Presence of the modal command-palette drawer (#118).
    drawer: Presence,
}

impl Default for AnimationCoordinator {
    fn default() -> Self {
        Self {
            route: Presence::settled(true, ROUTE_FADE),
            route_started_at: None,
            drawer: Presence::settled(false, DRAWER_FADE),
        }
    }
}

impl AnimationCoordinator {
    /// A route change: snap to hidden, then fade the arriving page in.
    pub fn route_changed(&mut self, at: Instant) {
        self.route = Presence::settled(false, ROUTE_FADE);
        self.route_started_at = Some(at);
        self.route.redirect(true, at);
    }

    /// Route fade progress in `0.0..=1.0` — the arriving page's opacity.
    pub fn route_opacity(&self, at: Instant) -> f32 {
        self.route.opacity(at)
    }

    /// Complete route transition projection including opacity, scale and subtle vertical settling.
    pub fn route_transition(&self, at: Instant) -> RouteTransitionProjection {
        if !self.route.is_animating(at) {
            return RouteTransitionProjection::default();
        }
        let elapsed_ms = self.route_started_at.map_or(0, |start| {
            at.saturating_duration_since(start).as_millis() as u32
        });
        project_route_transition(
            TransitionPattern::FadeThrough,
            TransitionDirection::Enter,
            elapsed_ms,
            MotionPreference::Full,
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_coordinator_route_transition() {
        let mut coord = AnimationCoordinator::default();
        let now = Instant::now();
        assert!(!coord.is_animating(now));

        coord.route_changed(now);
        assert!(coord.is_animating(now));
        let trans_start = coord.route_transition(now);
        assert!(trans_start.is_animating);
        assert_eq!(trans_start.opacity, 0.0);

        let half = now + Duration::from_millis(125);
        let trans_half = coord.route_transition(half);
        assert!(trans_half.is_animating);
        assert!(trans_half.opacity > 0.0);

        let completed_time = now + Duration::from_millis(300);
        let trans_done = coord.route_transition(completed_time);
        assert!(!trans_done.is_animating);
        assert_eq!(trans_done.opacity, 1.0);
        assert_eq!(trans_done.scale, 1.0);
    }
}

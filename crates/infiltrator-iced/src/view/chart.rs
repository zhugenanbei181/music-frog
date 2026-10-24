//! The three-layer chart cache architecture (EFFECTS-031 #122-#125).
//!
//! A chart draws in three layers with independent invalidation: the static
//! background (axes/grid) rebuilds only when bounds or skin move (#123), the
//! data geometry rebuilds only when the sample window slides or the bounds
//! move (#124), and the hover crosshair is never cached at all — it draws
//! immediately every frame (#125). The layer split is structural: the
//! crosshair simply has no cache to hold.

use iced::widget::canvas::Cache;
use std::cell::RefCell;

/// The facts a static chart background depends on (#123): its bounds and the
/// skin that paints it. Anything else leaves the cached grid untouched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BackgroundFacts {
    pub width: f32,
    pub height: f32,
    /// Skin identity: a theme switch must repaint cached colors.
    pub skin: &'static str,
}

/// The facts a data-geometry layer depends on (#124): bounds, skin, the
/// number of samples and the head sample of the window. A sliding window
/// changes its head even at constant length, so the head is part of the
/// identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DataFacts {
    pub width: f32,
    pub height: f32,
    pub skin: &'static str,
    pub samples: usize,
    /// The first (upload, download) sample of the raw window.
    pub head: (u64, u64),
}

/// Whether a cached layer holding `seen` must rebuild for `next`.
pub fn needs_rebuild<T: PartialEq>(seen: Option<T>, next: &T) -> bool {
    seen.as_ref() != Some(next)
}

/// The cached layers of one chart (#122). The crosshair layer is absent by
/// design: it is immediate (#125), so no field could ever cache it.
///
/// Facts are tracked in a `RefCell` because canvas `Program::draw` hands the
/// state over immutably; the caches themselves are interior-mutable the same
/// way.
#[derive(Debug, Default)]
pub struct ChartLayers {
    background: Cache,
    data: Cache,
    seen: RefCell<SeenFacts>,
}

#[derive(Debug, Default)]
struct SeenFacts {
    background: Option<BackgroundFacts>,
    data: Option<DataFacts>,
}

impl ChartLayers {
    /// Sync the static background layer (#123): clear only when its facts
    /// actually changed.
    pub fn sync_background(&self, facts: BackgroundFacts) {
        if needs_rebuild(self.seen.borrow().background, &facts) {
            self.seen.borrow_mut().background = Some(facts);
            self.background.clear();
        }
    }

    /// The background cache to draw through.
    pub fn background(&self) -> &Cache {
        &self.background
    }

    /// Sync the data-geometry layer (#124): clear only when the bounds, the
    /// skin, the window length or the window head changed.
    pub fn sync_data(&self, facts: DataFacts) {
        if needs_rebuild(self.seen.borrow().data, &facts) {
            self.seen.borrow_mut().data = Some(facts);
            self.data.clear();
        }
    }

    /// The data-geometry cache to draw through.
    pub fn data(&self) -> &Cache {
        &self.data
    }
}

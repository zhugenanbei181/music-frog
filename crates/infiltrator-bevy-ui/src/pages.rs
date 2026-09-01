//! The product pages of the Bevy frontend.
//!
//! **Page contract** (taskmanager-proven shape, one module per page):
//! a page exposes one scene constructor that takes the projection data it
//! renders plus the [`UiPalette`](infiltrator_bevy_widgets::palette::
//! UiPalette), and returns an `impl Scene` — the *only* entry a route's
//! scene table knows. Page roots carry a
//! [`PageRoot`](crate::route::PageRoot) marker naming their route.
//!
//! - Static structure composes declaratively with `bsn!` through the
//!   widget layer's `*_scene` adapters; the bsn! guard enforces it.
//! - Data never bakes into structure the page can't update: mutable text
//!   and fill nodes carry typed markers, and the page self-registers one
//!   refresh observer (via the root's `on_insert` bind hook, once per
//!   world) that restamps those components in place when its typed
//!   projection event fires. No polling, no tree rebuilds.
//! - All colors come from palette tokens; fonts come from text roles.
//!   Pages never touch bevy color/asset constructors directly.

pub mod overview;

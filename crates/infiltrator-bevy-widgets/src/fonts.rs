//! Embedded typography: the four OFL faces every widget text draws with.
//!
//! The faces are compiled into this crate (`include_bytes!`) and registered
//! as [`Font`] assets through the [`Assets<Font>`] store — the same store
//! `AssetServer`-loaded fonts land in, and the store bevy_text's font
//! collection is built from. bevy_asset 0.19 has no in-memory
//! `AssetServer` entry point, and folder-path loading would tie every host
//! (headless tests, the future Android closure) to a configured asset root,
//! so the embedding is the load path, not a preload cache.
//!
//! [`FontSources`] is the only place role → face is decided; text scenes
//! never name a face. When the resource is absent (or a host deliberately
//! unregisters it) the stamping falls back to `Handle::default()`, which
//! renders with the cosmic-text system fallback — a degradation, never a
//! panic.

use bevy::asset::{Assets, Handle};
use bevy::ecs::resource::Resource;
use bevy::text::Font;

use crate::text::Role;

/// Inter SemiBold — headings.
const HEADING_TTF: &[u8] = include_bytes!("../assets/fonts/Inter-SemiBold.ttf");
/// Inter Regular — body and control labels.
const BODY_TTF: &[u8] = include_bytes!("../assets/fonts/Inter-Regular.ttf");
/// Inter Medium — captions and idle labels.
const CAPTION_TTF: &[u8] = include_bytes!("../assets/fonts/Inter-Medium.ttf");
/// JetBrains Mono Regular — aligned telemetry values.
const MONO_TTF: &[u8] = include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf");

/// The role → face table, injected as a resource by [`crate::WidgetsPlugin`].
#[derive(Resource, Clone, Debug)]
pub struct FontSources {
    pub heading: Handle<Font>,
    pub body: Handle<Font>,
    pub caption: Handle<Font>,
    pub mono: Handle<Font>,
}

impl FontSources {
    /// Register every embedded face and return the handle table. All four
    /// faces are OFL-licensed and logged in `THIRD-PARTY-NOTICES.md`.
    pub fn embedded(fonts: &mut Assets<Font>) -> Self {
        Self {
            heading: fonts.add(Font::from_bytes(HEADING_TTF.to_vec())),
            body: fonts.add(Font::from_bytes(BODY_TTF.to_vec())),
            caption: fonts.add(Font::from_bytes(CAPTION_TTF.to_vec())),
            mono: fonts.add(Font::from_bytes(MONO_TTF.to_vec())),
        }
    }

    /// The face one typographic role draws with. Heading, display and body
    /// strong take weight contrast (SemiBold at display/heading size /
    /// Regular at body size); `BodyStrong` borrows the heading face at body
    /// size; captions drop to Medium so de-emphasis is typographic, not
    /// just alpha.
    pub fn face(&self, role: Role) -> Handle<Font> {
        match role {
            Role::Heading | Role::Display | Role::BodyStrong => self.heading.clone(),
            Role::Body => self.body.clone(),
            Role::Caption => self.caption.clone(),
            Role::Mono => self.mono.clone(),
        }
    }
}

/// The unregistered state: every role resolves to the default handle and
/// renders with the system fallback. Also the seed bsn! templates use.
impl Default for FontSources {
    fn default() -> Self {
        Self {
            heading: Handle::default(),
            body: Handle::default(),
            caption: Handle::default(),
            mono: Handle::default(),
        }
    }
}

//! Embedded typography: the four OFL faces every widget text draws with.
//!
//! The faces are compiled into this crate (`include_bytes!`) and registered
//! as [`Font`] assets through the [`Assets<Font>`] store — the same store
//! `AssetServer`-loaded fonts land in, and the store bevy_text's font
//! collection is built from. bevy_asset 0.20 has no in-memory
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
use infiltrator_contract::design_resources;

use infiltrator_contract::design_resources::{FontFamilyToken, FontWeightTier, typeface};
use infiltrator_shared::effect_assets::FontFace;

/// The shared byte owner decides which physical face belongs to each role.
const FACES: FontRoleFaces = FontRoleFaces {
    heading: FontFace::InterSemiBold,
    body: FontFace::InterRegular,
    caption: FontFace::InterMedium,
    mono: FontFace::Mono,
    cjk_regular: FontFace::CjkRegular,
    cjk_medium: FontFace::CjkMedium,
    cjk_semibold: FontFace::CjkSemiBold,
};

struct FontRoleFaces {
    heading: FontFace,
    body: FontFace,
    caption: FontFace,
    mono: FontFace,
    cjk_regular: FontFace,
    cjk_medium: FontFace,
    cjk_semibold: FontFace,
}

/// The role → face table, injected as a resource by [`crate::WidgetsPlugin`].
#[derive(Resource, Clone, Debug)]
pub struct FontSources {
    pub heading: Handle<Font>,
    pub body: Handle<Font>,
    pub caption: Handle<Font>,
    pub mono: Handle<Font>,
    pub cjk: Handle<Font>,
    pub cjk_regular: Handle<Font>,
    pub cjk_medium: Handle<Font>,
    pub cjk_semibold: Handle<Font>,
}

impl FontSources {
    /// Register every embedded face and return the handle table. All
    /// faces are OFL/Apache-licensed and logged in `THIRD-PARTY-NOTICES.md`.
    pub fn embedded(fonts: &mut Assets<Font>) -> Self {
        let heading = fonts.add(Font::from_bytes(FACES.heading.bytes().to_vec()));
        let body = fonts.add(Font::from_bytes(FACES.body.bytes().to_vec()));
        let caption = fonts.add(Font::from_bytes(FACES.caption.bytes().to_vec()));
        let mono = fonts.add(Font::from_bytes(FACES.mono.bytes().to_vec()));
        let cjk_regular = fonts.add(Font::from_bytes(FACES.cjk_regular.bytes().to_vec()));
        let cjk_medium = fonts.add(Font::from_bytes(FACES.cjk_medium.bytes().to_vec()));
        let cjk_semibold = fonts.add(Font::from_bytes(FACES.cjk_semibold.bytes().to_vec()));
        Self {
            heading,
            body,
            caption,
            mono,
            cjk: cjk_regular.clone(),
            cjk_regular,
            cjk_medium,
            cjk_semibold,
        }
    }

    /// The face one typographic role draws with.
    pub fn face(&self, role: design_resources::TextRole) -> Handle<Font> {
        let spec = typeface(role);
        match (spec.face.family, spec.face.weight) {
            (FontFamilyToken::Mono, _) => self.mono.clone(),
            (FontFamilyToken::Cjk, weight) => match weight {
                FontWeightTier::Regular => self.cjk_regular.clone(),
                FontWeightTier::Medium => self.cjk_medium.clone(),
                FontWeightTier::SemiBold => self.cjk_semibold.clone(),
            },
            (FontFamilyToken::Sans, weight) => match weight {
                FontWeightTier::Regular => self.body.clone(),
                FontWeightTier::Medium => self.caption.clone(),
                FontWeightTier::SemiBold => self.heading.clone(),
            },
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
            cjk: Handle::default(),
            cjk_regular: Handle::default(),
            cjk_medium: Handle::default(),
            cjk_semibold: Handle::default(),
        }
    }
}

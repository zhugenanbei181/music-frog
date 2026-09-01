//! Semantic icons: bitmap plates tinted at draw time. Never glyphs.
//!
//! **The tofu law** (charter): icons never ride text codepoints — embedded
//! faces guarantee no decorative Unicode coverage, and a glyph fallback is
//! exactly how tofu bugs ship. An icon is an SVG-derived RGBA bitmap drawn
//! through a tinted [`ImageNode`]; the tint is the theme ink a text sibling
//! would inherit, applied by the image widget's color multiply at draw time.
//!
//! **Bitmap pipeline**: the checked-in plates under `assets/icons/` are the
//! iced frontend's Lucide SVGs rasterized with `rsvg-convert -w 64 -h 64`
//! after substituting the SVGs' `currentColor` with white, so a plate is
//! white ink with alpha and any theme color tints it exactly. A semantic id
//! whose plate is missing (or whose [`Image`] has not resolved) degrades to
//! an invisible square — an honest absence, never a placeholder glyph, and
//! `Handle::default()` keeps the composition panic-free headless.
//!
//! Runtime shape mirrors the text contract: [`icon_scene`] stays pure and
//! emits [`IconPlate`] + [`IconTint`]; the [`stamp_icon_plate`] observer
//! joins them with the [`IconSources`] handle store and inserts the
//! `ImageNode` — its template plumbing is renderer-side, while the scene
//! declares only the sized node and the two semantic markers.

use bevy::asset::{AssetServer, Handle};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::image::Image;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, px};
use bevy::ui::widget::ImageNode;

/// The semantic ids this layer draws. M1 keeps the common chrome set; ids
/// are appended, never remapped — a plate file's name is its contract.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum IconId {
    #[default]
    Settings,
    Network,
    Activity,
    Globe,
    Zap,
    FileText,
    Plus,
    Trash,
    /// Uplink arrow (Lucide `arrow-up`) — the upload chip's semantics.
    ArrowUp,
    /// Downlink arrow (Lucide `arrow-down`) — the download chip's
    /// semantics. Rasterized from the iced frontend's own `arrow-down.svg`
    /// (a distinct Lucide glyph, not a flipped copy of `arrow-up.svg`),
    /// through the same currentColor→white pipeline as every plate above.
    ArrowDown,
}

impl IconId {
    /// Every id, in stable order (the [`IconSources`] plate order).
    pub const ALL: [IconId; 10] = [
        IconId::Settings,
        IconId::Network,
        IconId::Activity,
        IconId::Globe,
        IconId::Zap,
        IconId::FileText,
        IconId::Plus,
        IconId::Trash,
        IconId::ArrowUp,
        IconId::ArrowDown,
    ];

    /// Position in the [`IconSources`] plate table.
    pub fn index(self) -> usize {
        match self {
            IconId::Settings => 0,
            IconId::Network => 1,
            IconId::Activity => 2,
            IconId::Globe => 3,
            IconId::Zap => 4,
            IconId::FileText => 5,
            IconId::Plus => 6,
            IconId::Trash => 7,
            IconId::ArrowUp => 8,
            IconId::ArrowDown => 9,
        }
    }
}

/// The asset path one id draws from, relative to the host's asset root —
/// the crate ships the plates under `assets/icons/`. Pure mapping, so the
/// path table is headless-testable.
pub fn icon_path(icon: IconId) -> &'static str {
    match icon {
        IconId::Settings => "icons/settings.png",
        IconId::Network => "icons/network.png",
        IconId::Activity => "icons/activity.png",
        IconId::Globe => "icons/globe.png",
        IconId::Zap => "icons/zap.png",
        IconId::FileText => "icons/file-text.png",
        IconId::Plus => "icons/plus.png",
        IconId::Trash => "icons/trash-2.png",
        IconId::ArrowUp => "icons/arrow-up.png",
        IconId::ArrowDown => "icons/arrow-down.png",
    }
}

/// Handle store for every semantic plate, loaded through the host's
/// [`AssetServer`] (registered by [`crate::WidgetsPlugin`] when one is
/// present, defaulting to the empty store — and invisible icons — when
/// not). Rendering an unloaded plate is a no-op, never a panic.
#[derive(Resource, Default)]
pub struct IconSources {
    handles: [Option<Handle<Image>>; IconId::ALL.len()],
}

impl IconSources {
    /// Start loading every plate through the asset server. A plate whose
    /// file or loader is missing simply never resolves; the scene degrades
    /// to an invisible square.
    pub fn load(server: &AssetServer) -> Self {
        let mut sources = Self::default();
        for icon in IconId::ALL {
            sources.handles[icon.index()] = Some(server.load::<Image>(icon_path(icon)));
        }
        sources
    }

    /// The store handle for one semantic icon, if it was registered.
    pub fn handle(&self, icon: IconId) -> Option<Handle<Image>> {
        self.handles[icon.index()].clone()
    }
}

/// Marker naming the semantic icon a node draws. Pure scene builders emit
/// it; [`stamp_icon_plate`] is its only applier. The `Default` seed exists
/// only for the bsn! template mechanism — it is never a drawn identity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct IconPlate(pub IconId);

/// The tint the icon inherits — the icon equivalent of a text sibling's
/// `TextColor`, emitted at scene time where the palette is already in hand.
/// `Default` is the bsn! template seed only.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct IconTint(pub Color);

/// Stamp the bitmap handle and tint as a plate lands. The icon counterpart
/// of the text role observer — scenes never touch image assets. A missing
/// plate stamps `Handle::default()` (bevy's transparent image): the node
/// keeps its layout box and draws nothing.
pub fn stamp_icon_plate(
    trigger: On<Add, IconPlate>,
    sources: Option<Res<IconSources>>,
    marks: Query<&IconPlate>,
    tints: Query<&IconTint>,
    present: Query<(), With<ImageNode>>,
    mut commands: Commands,
) {
    let entity = trigger.event().entity;
    if present.get(entity).is_ok() {
        return;
    }
    let icon = marks.get(entity).ok().map(|mark| mark.0);
    let image = icon.and_then(|icon| sources.as_ref().and_then(|s| s.handle(icon)));
    let tint = tints.get(entity).map(|tint| tint.0).unwrap_or_default();
    commands.entity(entity).insert(ImageNode {
        color: tint,
        image: image.unwrap_or_default(),
        ..ImageNode::default()
    });
}

/// Mirror every live [`IconTint`] into its node's drawn image color.
/// Compare-and-set per frame: a tint restamped against a new palette (icon
/// tiles, future tinted chrome) reaches the renderer without any
/// switch-specific hook, and unchanged frames produce no write noise.
pub fn sync_icon_tints(mut icons: Query<(&IconTint, &mut ImageNode)>) {
    for (tint, mut image) in &mut icons {
        if image.color != tint.0 {
            image.color = tint.0;
        }
    }
}

/// The icon scene: a fixed square carrying the plate + tint markers; the
/// stamp observer turns it into a tinted bitmap. Plate resolution is
/// deferred to that observer, so a missing bitmap is an invisible square —
/// not a panic and not a glyph.
pub fn icon_scene(icon: IconId, size_px: f32, tint: Color) -> impl Scene + use<> {
    bsn! {
        Node {
            width: px(size_px),
            height: px(size_px),
            flex_shrink: 0.0,
        }
        IconPlate({ icon })
        IconTint({ tint })
    }
}

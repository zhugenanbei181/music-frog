//! Semantic icons: bitmap plates tinted at draw time. Never glyphs.
//!
//! **The tofu law** (charter): icons never ride text codepoints — embedded
//! faces guarantee no decorative Unicode coverage, and a glyph fallback is
//! exactly how tofu bugs ship. An icon is an SVG-derived RGBA bitmap drawn
//! through a tinted [`ImageNode`]; the tint is the theme ink a text sibling
//! would inherit, applied by the image widget's color multiply at draw time.
//!
//! **Bitmap pipeline**: the shared asset catalog exposes renderer-ready
//! 64px PNG plates rasterized from the canonical Iced Lucide SVGs after
//! substituting `currentColor` with white. A plate is white ink with alpha,
//! so any theme color tints it exactly. A semantic id
//! whose plate is missing (or whose [`Image`] has not resolved) degrades to
//! an invisible square — an honest absence, never a placeholder glyph, and
//! `Handle::default()` keeps the composition panic-free headless.
//!
//! Runtime shape mirrors the text contract: [`icon_scene`] stays pure and
//! emits [`IconPlate`] + [`IconTint`]; the [`stamp_icon_plate`] observer
//! joins them with the [`IconSources`] handle store and inserts the
//! `ImageNode` — its template plumbing is renderer-side, while the scene
//! declares only the sized node and the two semantic markers.

use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::lifecycle::Add;
use bevy::ecs::observer::On;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Node, Val, px};
use bevy::ui::widget::ImageNode;
use infiltrator_shared::assets::bevy_icon_png;

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
    // ---- the window's own three keys and the history arrows -------------
    //
    // The chrome keys are plates, never text glyphs: the embedded faces
    // guarantee no decorative Unicode coverage, so a "✕" glyph is tofu.
    /// The minimize key (Lucide `minus`).
    Minus,
    /// The maximize key (Lucide `square`).
    Square,
    /// The close key (Lucide `x`).
    Close,
    /// The history-back arrow (Lucide `chevron-left`).
    ChevronLeft,
    /// The history-forward arrow (Lucide `chevron-right`).
    ChevronRight,
    // ---- the routes' own plates ------------------------------------------
    //
    // Every route carries a distinct plate: the rail used to repeat five
    // plates for eleven routes, which made the navigation unreadable.
    /// The overview route (Lucide `layout-grid`).
    LayoutGrid,
    /// The rules route (Lucide `list-checks`).
    ListChecks,
    /// The sync route (Lucide `server` — the remote backup host).
    Server,
    /// The DNS route (Lucide `shield` — the resolver's secure path).
    Shield,
    /// The logs route (Lucide `code-2` — the console's own lines).
    Code2,
    /// The app-routing route (Lucide `target` — the targeted application).
    Target,
}

impl IconId {
    /// Every id, in stable order (the [`IconSources`] plate order).
    pub const ALL: [IconId; 21] = [
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
        IconId::Minus,
        IconId::Square,
        IconId::Close,
        IconId::ChevronLeft,
        IconId::ChevronRight,
        IconId::LayoutGrid,
        IconId::ListChecks,
        IconId::Server,
        IconId::Shield,
        IconId::Code2,
        IconId::Target,
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
            IconId::Minus => 10,
            IconId::Square => 11,
            IconId::Close => 12,
            IconId::ChevronLeft => 13,
            IconId::ChevronRight => 14,
            IconId::LayoutGrid => 15,
            IconId::ListChecks => 16,
            IconId::Server => 17,
            IconId::Shield => 18,
            IconId::Code2 => 19,
            IconId::Target => 20,
        }
    }

    /// Stable shared-catalog name for this semantic plate.
    pub const fn asset_name(self) -> &'static str {
        match self {
            IconId::Settings => "settings",
            IconId::Network => "network",
            IconId::Activity => "activity",
            IconId::Globe => "globe",
            IconId::Zap => "zap",
            IconId::FileText => "file-text",
            IconId::Plus => "plus",
            IconId::Trash => "trash-2",
            IconId::ArrowUp => "arrow-up",
            IconId::ArrowDown => "arrow-down",
            IconId::Minus => "minus",
            IconId::Square => "square",
            IconId::Close => "x",
            IconId::ChevronLeft => "chevron-left",
            IconId::ChevronRight => "chevron-right",
            IconId::LayoutGrid => "layout-grid",
            IconId::ListChecks => "list-checks",
            IconId::Server => "server",
            IconId::Shield => "shield",
            IconId::Code2 => "code-2",
            IconId::Target => "target",
        }
    }
}

/// Handle store for every semantic plate, decoded from shared embedded bytes
/// into Bevy's `Assets<Image>` store. A host without that store keeps the
/// empty resource and renders invisible icons without panicking.
#[derive(Resource, Default)]
pub struct IconSources {
    handles: [Option<Handle<Image>>; IconId::ALL.len()],
}

impl IconSources {
    /// Decode the shared PNG variants into Bevy's image store. Missing or
    /// malformed plates remain absent and stamp as transparent squares.
    pub fn embedded(images: &mut Assets<Image>) -> Self {
        let mut sources = Self::default();
        for icon in IconId::ALL {
            let Some(bytes) = bevy_icon_png(icon.asset_name()) else {
                continue;
            };
            let Ok(image) = Image::from_buffer(
                bytes,
                ImageType::Extension("png"),
                CompressedImageFormats::NONE,
                true,
                ImageSampler::nearest(),
                RenderAssetUsages::default(),
            ) else {
                continue;
            };
            sources.handles[icon.index()] = Some(images.add(image));
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
    trigger: On<Add<IconPlate>>,
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

/// The icon scene on the rem channel: the layout box travels with the root
/// font size, so the plate follows the user's font scale instead of staying
/// a physical size.
///
/// The plate itself is a physical asset (one 64px bitmap per semantic id),
/// but its **box** is a layout size: a shell that keeps the box in physical
/// pixels makes every icon the same size at every font scale, which is the
/// residual jitter of the rem migration.
pub fn icon_scene_val(icon: IconId, size: Val, tint: Color) -> impl Scene + use<> {
    bsn! {
            Node {
                width: size,
                height: size,
                flex_shrink: 0.0,
            }
            IconPlate({ icon })
            IconTint({ tint })
    }
}

/// The icon scene with a physical pixel box.
///
/// The shell's remaining call sites still state their box in physical pixels;
/// this entry keeps them compiling while they migrate to the rem channel
/// ([`icon_scene_val`]). Once the last one has moved, this entry goes with
/// it: one icon entry, one sizing channel.
pub fn icon_scene(icon: IconId, size_px: f32, tint: Color) -> impl Scene + use<> {
    icon_scene_val(icon, px(size_px), tint)
}

/// A 9×9 filled circle for text-inline status dots (capability norm ⑤):
/// spawned as an [`bevy_ui::widget::inline_image::InlineImage`] child of a
/// text block and tinted per state, so the dot flows and wraps with the
/// sentence instead of being a sibling node.
pub fn status_dot_image() -> Image {
    let size = 9;
    let radius_sq = (size as f32 / 2.0 - 0.5).powi(2);
    let mut data = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - (size as f32 - 1.0) / 2.0;
            let dy = y as f32 - (size as f32 - 1.0) / 2.0;
            let alpha = if dx * dx + dy * dy <= radius_sq {
                255u8
            } else {
                0u8
            };
            data.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    Image::new(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

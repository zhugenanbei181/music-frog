//! Single authority for typography roles, physical font faces and icon semantics.
//!
//! A surface never asks for a raw weight or a file name: it asks for a
//! [`TextRole`] and receives the one [`TypefaceSpec`] that the shipped font
//! set can actually draw. [`IconSemantic`] does the same for icons: a
//! canonical semantic id resolves to the canonical vector source, while
//! platform-native exceptions are an explicit, exhaustively matched tier.

use serde::{Deserialize, Serialize};

/// The three physical weights every bundled family may expose.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontWeightTier {
    Regular,
    Medium,
    SemiBold,
}

impl FontWeightTier {
    pub const ALL: [Self; 3] = [Self::Regular, Self::Medium, Self::SemiBold];

    /// CSS/OpenType weight value. The closed set is also the inventory bound:
    /// no surface may request a weight without a shipped physical face.
    pub const fn value(self) -> u16 {
        match self {
            Self::Regular => 400,
            Self::Medium => 500,
            Self::SemiBold => 600,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Medium => "medium",
            Self::SemiBold => "semibold",
        }
    }
}

/// The families whose bytes are registered in the shared asset catalog.
/// `Cjk` has one family name and three separate physical faces; no surface
/// may treat `font.CJK-Regular` as the whole family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FontFamilyToken {
    Sans,
    Mono,
    Cjk,
}

impl FontFamilyToken {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sans => "Inter",
            Self::Mono => "JetBrains Mono",
            Self::Cjk => "HarmonyOS Sans SC",
        }
    }
}

/// The catalog id of one physical font face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FontFaceToken {
    pub family: FontFamilyToken,
    pub weight: FontWeightTier,
}

impl FontFaceToken {
    pub const ALL: [Self; 7] = [
        Self::sans(FontWeightTier::Regular),
        Self::sans(FontWeightTier::Medium),
        Self::sans(FontWeightTier::SemiBold),
        Self::mono(),
        Self::cjk(FontWeightTier::Regular),
        Self::cjk(FontWeightTier::Medium),
        Self::cjk(FontWeightTier::SemiBold),
    ];

    pub const fn sans(weight: FontWeightTier) -> Self {
        Self {
            family: FontFamilyToken::Sans,
            weight,
        }
    }

    pub const fn mono() -> Self {
        Self {
            family: FontFamilyToken::Mono,
            weight: FontWeightTier::Regular,
        }
    }

    pub const fn cjk(weight: FontWeightTier) -> Self {
        Self {
            family: FontFamilyToken::Cjk,
            weight,
        }
    }

    /// The stable `font.*` id in `asset-catalog.json`.
    pub const fn catalog_id(self) -> &'static str {
        match (self.family, self.weight) {
            (FontFamilyToken::Sans, FontWeightTier::Regular) => "font.Inter-Regular",
            (FontFamilyToken::Sans, FontWeightTier::Medium) => "font.Inter-Medium",
            (FontFamilyToken::Sans, FontWeightTier::SemiBold) => "font.Inter-SemiBold",
            (FontFamilyToken::Mono, _) => "font.JetBrainsMono-Regular",
            (FontFamilyToken::Cjk, FontWeightTier::Regular) => "font.CJK-Regular",
            (FontFamilyToken::Cjk, FontWeightTier::Medium) => "font.CJK-Medium",
            (FontFamilyToken::Cjk, FontWeightTier::SemiBold) => "font.CJK-SemiBold",
        }
    }

    /// The Android resource name generated from the shared bytes.
    pub const fn android_resource(self) -> &'static str {
        match (self.family, self.weight) {
            (FontFamilyToken::Sans, FontWeightTier::Regular) => "inter_regular",
            (FontFamilyToken::Sans, FontWeightTier::Medium) => "inter_medium",
            (FontFamilyToken::Sans, FontWeightTier::SemiBold) => "inter_semibold",
            (FontFamilyToken::Mono, _) => "jetbrains_mono_regular",
            (FontFamilyToken::Cjk, FontWeightTier::Regular) => "cjk_regular",
            (FontFamilyToken::Cjk, FontWeightTier::Medium) => "cjk_medium",
            (FontFamilyToken::Cjk, FontWeightTier::SemiBold) => "cjk_semibold",
        }
    }
}

/// One typographic role; every surface consumes this closed vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TextRole {
    Display,
    Title,
    Subheading,
    Heading,
    BodyLarge,
    #[default]
    Body,
    BodyStrong,
    Label,
    LabelStrong,
    Caption,
    Micro,
    Tag,
    Mono,
    MonoStrong,
    /// Large live numeric readout (dashboard traffic / counters).
    NumericLarge,
    /// Dense variant of the live numeric readout for compact grid cells.
    NumericDense,
    /// Small numeric caption that sits under a live readout.
    NumericSmall,
    /// Compact CJK semibold body: 11px / 600.
    MicroStrong,
    /// Small CJK semibold label: 12px / 600.
    CaptionStrong,
    /// Extra-small CJK semibold tag: 10px / 600.
    TagStrong,
    /// Small monospaced readout: 10px / 400.
    MonoTag,
    /// Small monospaced readout: 11px / 400.
    MonoMicro,
    /// Small monospaced readout: 12px / 400.
    MonoCaption,
    /// Medium monospaced readout: 16px / 400.
    MonoBodyLarge,
    /// Large monospaced readout: 20px / 400.
    MonoTitle,
    /// A labelled fact value on the hero card: 13px / 400 in the interface
    /// family. One family and one size is the only structural guarantee that
    /// two values on the same row share a baseline; a monospaced neighbour has
    /// a different ascent and cannot align with it at any size.
    FactNamed,
}

impl TextRole {
    pub const ALL: [Self; 26] = [
        Self::Display,
        Self::Title,
        Self::Subheading,
        Self::Heading,
        Self::BodyLarge,
        Self::Body,
        Self::BodyStrong,
        Self::Label,
        Self::LabelStrong,
        Self::Caption,
        Self::Micro,
        Self::Tag,
        Self::Mono,
        Self::MonoStrong,
        Self::NumericLarge,
        Self::NumericDense,
        Self::NumericSmall,
        Self::MicroStrong,
        Self::CaptionStrong,
        Self::TagStrong,
        Self::MonoTag,
        Self::MonoMicro,
        Self::MonoCaption,
        Self::MonoBodyLarge,
        Self::MonoTitle,
        Self::FactNamed,
    ];

    /// The one column-weight vocabulary for the live connection table.
    /// Header and body read these weights instead of a page-derived length,
    /// so a column edge cannot drift with a parent layout guess.
    pub const CONNECTION_COLUMN_WEIGHTS: [u8; 7] = [8, 16, 3, 5, 6, 6, 4];

    /// Sum of [`Self::CONNECTION_COLUMN_WEIGHTS`].
    pub const CONNECTION_COLUMN_WEIGHT_TOTAL: u8 = 48;

    /// Absolute fractions for the hero card's two labelled fact columns.
    /// The phone and desktop hero both read these generated values, so a
    /// short label and a long organization name cannot claim different slack
    /// pools and pull their baselines out of the shared grid.
    pub const HERO_FACT_COLUMN_WEIGHTS: [u8; 2] = [5, 7];

    /// Sum of [`Self::HERO_FACT_COLUMN_WEIGHTS`].
    pub const HERO_FACT_COLUMN_WEIGHT_TOTAL: u8 = 12;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Display => "display",
            Self::Title => "title",
            Self::Subheading => "subheading",
            Self::Heading => "heading",
            Self::BodyLarge => "bodyLarge",
            Self::Body => "body",
            Self::BodyStrong => "bodyStrong",
            Self::Label => "label",
            Self::LabelStrong => "labelStrong",
            Self::Caption => "caption",
            Self::Micro => "micro",
            Self::Tag => "tag",
            Self::Mono => "mono",
            Self::MonoStrong => "monoStrong",
            Self::NumericLarge => "numericLarge",
            Self::NumericDense => "numericDense",
            Self::NumericSmall => "numericSmall",
            Self::MicroStrong => "microStrong",
            Self::CaptionStrong => "captionStrong",
            Self::TagStrong => "tagStrong",
            Self::MonoTag => "monoTag",
            Self::MonoMicro => "monoMicro",
            Self::MonoCaption => "monoCaption",
            Self::MonoBodyLarge => "monoBodyLarge",
            Self::MonoTitle => "monoTitle",
            Self::FactNamed => "factNamed",
        }
    }
}

/// The full typographic resolution of one role: face, weight and size.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypefaceSpec {
    pub face: FontFaceToken,
    pub size_px: f32,
}

impl TypefaceSpec {
    pub const fn weight(self) -> FontWeightTier {
        self.face.weight
    }

    pub const fn family(self) -> FontFamilyToken {
        self.face.family
    }
}

/// The single role→typeface table. Sizes come from `design_tokens::type_scale`
/// or an explicitly registered numeric/mono role; weights and families come
/// only from the seven shipped physical faces.
pub const fn typeface(role: TextRole) -> TypefaceSpec {
    use crate::design_tokens::type_scale::{
        BODY, BODY_LARGE, CAPTION, DISPLAY, HEADING, LABEL_LARGE, LABEL_SMALL, MONO, SUBHEADING,
        TAG, TITLE,
    };
    match role {
        TextRole::Display => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: DISPLAY,
        },
        TextRole::Title => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: TITLE,
        },
        TextRole::Subheading => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: SUBHEADING,
        },
        TextRole::Heading => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: HEADING,
        },
        TextRole::BodyLarge => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Regular),
            size_px: BODY_LARGE,
        },
        TextRole::Body => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Regular),
            size_px: BODY,
        },
        TextRole::BodyStrong => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: BODY,
        },
        TextRole::Label => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Medium),
            size_px: LABEL_LARGE,
        },
        TextRole::LabelStrong => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: LABEL_LARGE,
        },
        TextRole::Caption => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Medium),
            size_px: CAPTION,
        },
        TextRole::Micro => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Medium),
            size_px: LABEL_SMALL,
        },
        TextRole::Tag => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Medium),
            size_px: TAG,
        },
        TextRole::Mono => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: MONO,
        },
        TextRole::MonoStrong => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: MONO,
        },
        TextRole::NumericLarge => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 28.0,
        },
        TextRole::NumericDense => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 22.0,
        },
        TextRole::NumericSmall => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: MONO,
        },
        TextRole::MicroStrong => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: 11.0,
        },
        TextRole::CaptionStrong => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: 12.0,
        },
        TextRole::TagStrong => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::SemiBold),
            size_px: 10.0,
        },
        TextRole::MonoTag => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 10.0,
        },
        TextRole::MonoMicro => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 11.0,
        },
        TextRole::MonoCaption => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 12.0,
        },
        TextRole::MonoBodyLarge => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 16.0,
        },
        TextRole::MonoTitle => TypefaceSpec {
            face: FontFaceToken::mono(),
            size_px: 20.0,
        },
        TextRole::FactNamed => TypefaceSpec {
            face: FontFaceToken::cjk(FontWeightTier::Regular),
            size_px: MONO,
        },
    }
}

/// How one icon semantic may differ between surfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconTier {
    /// Every surface draws the same canonical vector source.
    Canonical,
    /// A documented platform-native representation is allowed.
    PlatformNative,
}

impl IconTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Canonical => "canonical",
            Self::PlatformNative => "platformNative",
        }
    }
}

/// Semantic icons shared by the four surfaces. A semantic id, not a file name
/// or toolkit symbol, is what a surface requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IconSemantic {
    Overview,
    Proxy,
    Config,
    Settings,
    Connections,
    Logs,
    Rules,
    Traffic,
    Search,
    Add,
    Remove,
    Edit,
    Delete,
    Copy,
    Refresh,
    Start,
    Stop,
    ChevronRight,
    ChevronLeft,
    ChevronUp,
    ChevronDown,
    Expand,
    Close,
    Check,
    Shield,
    Globe,
    Network,
    Server,
    Zap,
    Target,
    Activity,
    Plus,
    Minus,
    X,
    ArrowUp,
    ArrowDown,
    ListChecks,
    LayoutGrid,
    FileText,
    Code2,
    Wifi,
    Plug,
    Square,
    Pin,
}

impl IconSemantic {
    pub const ALL: [Self; 44] = [
        Self::Overview,
        Self::Proxy,
        Self::Config,
        Self::Settings,
        Self::Connections,
        Self::Logs,
        Self::Rules,
        Self::Traffic,
        Self::Search,
        Self::Add,
        Self::Remove,
        Self::Edit,
        Self::Delete,
        Self::Copy,
        Self::Refresh,
        Self::Start,
        Self::Stop,
        Self::ChevronRight,
        Self::ChevronLeft,
        Self::ChevronUp,
        Self::ChevronDown,
        Self::Expand,
        Self::Close,
        Self::Check,
        Self::Shield,
        Self::Globe,
        Self::Network,
        Self::Server,
        Self::Zap,
        Self::Target,
        Self::Activity,
        Self::Plus,
        Self::Minus,
        Self::X,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::ListChecks,
        Self::LayoutGrid,
        Self::FileText,
        Self::Code2,
        Self::Wifi,
        Self::Plug,
        Self::Square,
        Self::Pin,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Proxy => "proxy",
            Self::Config => "config",
            Self::Settings => "settings",
            Self::Connections => "connections",
            Self::Logs => "logs",
            Self::Rules => "rules",
            Self::Traffic => "traffic",
            Self::Search => "search",
            Self::Add => "add",
            Self::Remove => "remove",
            Self::Edit => "edit",
            Self::Delete => "delete",
            Self::Copy => "copy",
            Self::Refresh => "refresh",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::ChevronRight => "chevronRight",
            Self::ChevronLeft => "chevronLeft",
            Self::ChevronUp => "chevronUp",
            Self::ChevronDown => "chevronDown",
            Self::Expand => "expand",
            Self::Close => "close",
            Self::Check => "check",
            Self::Shield => "shield",
            Self::Globe => "globe",
            Self::Network => "network",
            Self::Server => "server",
            Self::Zap => "zap",
            Self::Target => "target",
            Self::Activity => "activity",
            Self::Plus => "plus",
            Self::Minus => "minus",
            Self::X => "x",
            Self::ArrowUp => "arrowUp",
            Self::ArrowDown => "arrowDown",
            Self::ListChecks => "listChecks",
            Self::LayoutGrid => "layoutGrid",
            Self::FileText => "fileText",
            Self::Code2 => "code2",
            Self::Wifi => "wifi",
            Self::Plug => "plug",
            Self::Square => "square",
            Self::Pin => "pin",
        }
    }

    /// The canonical Lucide SVG name in the shared asset catalog.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::Overview => "layout-grid",
            Self::Proxy => "globe",
            Self::Config => "file-text",
            Self::Settings => "settings",
            Self::Shield => "shield",
            Self::Connections => "network",
            Self::Logs => "file-text",
            Self::Rules => "list-checks",
            Self::Traffic => "activity",
            Self::Search => "search",
            Self::Add | Self::Plus => "plus",
            Self::Remove | Self::Delete => "trash-2",
            Self::Edit => "pencil",
            Self::Copy => "copy",
            Self::Refresh => "refresh-cw",
            Self::Start => "zap",
            Self::Stop => "square",
            Self::ChevronRight => "chevron-right",
            Self::ChevronLeft => "chevron-left",
            Self::ChevronUp => "chevron-up",
            Self::ChevronDown => "chevron-down",
            Self::Expand => "chevron-down",
            Self::Close | Self::X => "x",
            Self::Check => "square",
            Self::Globe => "globe",
            Self::Network => "network",
            Self::Server => "server",
            Self::Zap => "zap",
            Self::Target => "target",
            Self::Activity => "activity",
            Self::Minus => "minus",
            Self::ArrowUp => "arrow-up",
            Self::ArrowDown => "arrow-down",
            Self::ListChecks => "list-checks",
            Self::LayoutGrid => "layout-grid",
            Self::FileText => "file-text",
            Self::Code2 => "code-2",
            Self::Wifi => "wifi",
            Self::Plug => "plug",
            Self::Square => "square",
            Self::Pin => "pin",
        }
    }

    /// The stable `icon.*` id in `asset-catalog.json`.
    pub const fn catalog_id(self) -> &'static str {
        match self {
            Self::Overview | Self::LayoutGrid => "icon.layout-grid",
            Self::Proxy | Self::Globe => "icon.globe",
            Self::Config | Self::Logs | Self::FileText => "icon.file-text",
            Self::Settings => "icon.settings",
            Self::Shield => "icon.shield",
            Self::Connections | Self::Network => "icon.network",
            Self::Rules | Self::ListChecks => "icon.list-checks",
            Self::Traffic | Self::Activity => "icon.activity",
            Self::Search => "icon.search",
            Self::Add | Self::Plus => "icon.plus",
            Self::Remove | Self::Delete => "icon.trash-2",
            Self::Edit => "icon.pencil",
            Self::Copy => "icon.copy",
            Self::Refresh => "icon.refresh-cw",
            Self::Start | Self::Zap => "icon.zap",
            Self::Stop | Self::Square | Self::Check => "icon.square",
            Self::ChevronRight => "icon.chevron-right",
            Self::ChevronLeft => "icon.chevron-left",
            Self::ChevronUp => "icon.chevron-up",
            Self::ChevronDown | Self::Expand => "icon.chevron-down",
            Self::Close | Self::X => "icon.x",
            Self::Server => "icon.server",
            Self::Target => "icon.target",
            Self::Minus => "icon.minus",
            Self::ArrowUp => "icon.arrow-up",
            Self::ArrowDown => "icon.arrow-down",
            Self::Code2 => "icon.code-2",
            Self::Wifi => "icon.wifi",
            Self::Plug => "icon.plug",
            Self::Pin => "icon.pin",
        }
    }

    pub const fn tier(self) -> IconTier {
        IconTier::Canonical
    }
}

/// Platform-native exceptions. Every entry carries its reason; an absent
/// reason is a guard violation, not an implicit allowance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformNativeIcon {
    pub id: &'static str,
    pub reason: &'static str,
}

pub const PLATFORM_NATIVE_ICONS: &[PlatformNativeIcon] = &[
    PlatformNativeIcon {
        id: "flag.*",
        reason: "full-colour national flags are a distinct shared inventory",
    },
    PlatformNativeIcon {
        id: "app.product",
        reason: "OS launcher density buckets and ICO frames are generated variants",
    },
    PlatformNativeIcon {
        id: "android.notification",
        reason: "system notification small icons require platform monochrome rules",
    },
    PlatformNativeIcon {
        id: "slint.ping-spin",
        reason: "Slint renders a frame sequence instead of a runtime rotation",
    },
];

/// Machine-readable authority exported for non-Rust generators and guards.
#[derive(Debug, PartialEq, Serialize)]
pub struct DesignAuthorityDocument {
    pub version: u32,
    pub font_weights: Vec<u16>,
    pub font_faces: Vec<FontFaceDocument>,
    pub type_roles: Vec<TypefaceDocument>,
    /// Stretch weights of the live connection table's seven columns. Header
    /// and rows read the same generated tokens, so no surface recomputes a
    /// column width from a parent layout guess.
    pub connection_column_weights: Vec<u8>,
    /// Stretch weights of the hero card's two labelled fact columns.
    pub hero_fact_column_weights: Vec<u8>,
    pub icons: Vec<IconDocument>,
    pub platform_native_icons: Vec<PlatformNativeIcon>,
    /// The product mascot: a brand asset, not an icon semantic. Its bytes live
    /// in the shared catalog and its motion is a separate typed contract.
    pub mascot: MascotAssetDocument,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FontFaceDocument {
    pub catalog_id: &'static str,
    pub android_resource: &'static str,
    pub family: FontFamilyToken,
    pub family_name: &'static str,
    pub weight: FontWeightTier,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypefaceDocument {
    pub role: TextRole,
    pub catalog_id: &'static str,
    pub family_name: &'static str,
    pub size_px: f32,
    pub weight: FontWeightTier,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct IconDocument {
    pub semantic: IconSemantic,
    pub tier: IconTier,
    pub catalog_id: &'static str,
    pub canonical_name: &'static str,
}

/// The one exported brand identity. A second frog image, a second cutout or a
/// per-surface copy would be a second source of truth.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MascotAssetDocument {
    pub catalog_id: &'static str,
    pub path: &'static str,
    pub format: &'static str,
    /// A prefiltered plate sized for the hero's real pixel box. It is a
    /// generated variant of the same catalog identity, never a second mascot.
    pub hero_path: &'static str,
    /// Whether the licence has been cleared for shipping in a release. Local
    /// implementation and receipts may use the asset; distribution may not.
    pub redistributable: bool,
}

pub const MASCOT_ASSET: MascotAssetDocument = MascotAssetDocument {
    catalog_id: "mascot.musicfrog-cutout",
    path: "mascot/musicfrog-cutout.png",
    format: "png",
    hero_path: "mascot/musicfrog-hero-512.png",
    redistributable: false,
};

/// The one exported snapshot; generators compare against this value, never
/// against a second hand-maintained table.
pub fn design_authority_document() -> DesignAuthorityDocument {
    DesignAuthorityDocument {
        version: 1,
        font_weights: FontWeightTier::ALL.map(FontWeightTier::value).to_vec(),
        font_faces: FontFaceToken::ALL
            .map(|face| FontFaceDocument {
                catalog_id: face.catalog_id(),
                android_resource: face.android_resource(),
                family: face.family,
                family_name: face.family.as_str(),
                weight: face.weight,
            })
            .to_vec(),
        type_roles: TextRole::ALL
            .map(|role| {
                let spec = typeface(role);
                TypefaceDocument {
                    role,
                    catalog_id: spec.face.catalog_id(),
                    family_name: spec.face.family.as_str(),
                    size_px: spec.size_px,
                    weight: spec.weight(),
                }
            })
            .to_vec(),
        connection_column_weights: TextRole::CONNECTION_COLUMN_WEIGHTS.to_vec(),
        hero_fact_column_weights: TextRole::HERO_FACT_COLUMN_WEIGHTS.to_vec(),
        icons: IconSemantic::ALL
            .map(|semantic| IconDocument {
                semantic,
                tier: semantic.tier(),
                catalog_id: semantic.catalog_id(),
                canonical_name: semantic.canonical_name(),
            })
            .to_vec(),
        platform_native_icons: PLATFORM_NATIVE_ICONS.to_vec(),
        mascot: MASCOT_ASSET,
    }
}

/// JSON form consumed by the Kotlin/Slint generators and the parity guard.
pub fn design_authority_json() -> String {
    serde_json::to_string_pretty(&design_authority_document())
        .expect("design authority document must serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn shipped_weights_are_the_only_weights() {
        assert_eq!(
            FontWeightTier::ALL.map(FontWeightTier::value),
            [400, 500, 600]
        );
        for role in TextRole::ALL {
            assert!(FontWeightTier::ALL.contains(&typeface(role).weight()));
        }
    }

    #[test]
    fn every_role_and_icon_resolves_to_a_catalog_shape() {
        for role in TextRole::ALL {
            let spec = typeface(role);
            assert!(spec.face.catalog_id().starts_with("font."));
            assert!(spec.size_px > 0.0);
        }
        let mut by_triple: HashMap<(u32, u16, &'static str), Vec<TextRole>> = HashMap::new();
        for role in TextRole::ALL {
            let spec = typeface(role);
            let triple = (
                spec.size_px.to_bits(),
                spec.weight().value(),
                spec.family().as_str(),
            );
            by_triple.entry(triple).or_default().push(role);
        }
        let legacy_semantic_groups: [&[TextRole]; 2] = [
            &[TextRole::Title, TextRole::Heading],
            &[TextRole::Mono, TextRole::MonoStrong, TextRole::NumericSmall],
        ];
        for (triple, roles) in by_triple {
            if roles.len() > 1 {
                assert!(
                    legacy_semantic_groups
                        .iter()
                        .any(|group| roles.iter().all(|role| group.contains(role))),
                    "unregistered duplicate (size, weight, family) {triple:?}: {roles:?}"
                );
            }
        }
        assert_eq!(TextRole::ALL.len(), 26);
        assert_eq!(FontFaceToken::ALL.len(), 7);
        assert_eq!(typeface(TextRole::Subheading).size_px, 18.0);
        assert_eq!(typeface(TextRole::Micro).size_px, 11.0);
        assert_eq!(typeface(TextRole::Micro).weight(), FontWeightTier::Medium);
        for icon in IconSemantic::ALL {
            assert!(icon.catalog_id().starts_with("icon."));
            assert!(!icon.canonical_name().is_empty());
            assert_eq!(icon.tier(), IconTier::Canonical);
        }
        assert!(
            PLATFORM_NATIVE_ICONS
                .iter()
                .all(|entry| !entry.reason.is_empty())
        );
    }

    #[test]
    fn exported_document_is_stable_and_complete() {
        let document = design_authority_document();
        assert_eq!(document.version, 1);
        assert_eq!(document.font_faces.len(), FontFaceToken::ALL.len());
        assert_eq!(document.font_faces.len(), 7);
        assert_eq!(document.type_roles.len(), TextRole::ALL.len());
        assert_eq!(document.type_roles.len(), 26);
        let cjk_faces: Vec<_> = document
            .font_faces
            .iter()
            .filter(|face| face.family == FontFamilyToken::Cjk)
            .collect();
        assert_eq!(cjk_faces.len(), 3);
        assert!(
            cjk_faces
                .iter()
                .all(|face| face.family_name == FontFamilyToken::Cjk.as_str())
        );
        assert_eq!(
            cjk_faces
                .iter()
                .map(|face| face.weight.value())
                .collect::<Vec<_>>(),
            vec![400, 500, 600]
        );
        assert_eq!(
            document.connection_column_weights,
            TextRole::CONNECTION_COLUMN_WEIGHTS
        );
        assert_eq!(
            document.connection_column_weights.iter().sum::<u8>(),
            TextRole::CONNECTION_COLUMN_WEIGHT_TOTAL
        );
        assert_eq!(
            document.hero_fact_column_weights,
            TextRole::HERO_FACT_COLUMN_WEIGHTS
        );
        assert_eq!(
            document.hero_fact_column_weights.iter().sum::<u8>(),
            TextRole::HERO_FACT_COLUMN_WEIGHT_TOTAL
        );
        assert_eq!(document.icons.len(), IconSemantic::ALL.len());
        let json = design_authority_json();
        assert!(json.contains("\"font_weights\""));
        assert!(json.contains("\"catalog_id\": \"font.Inter-Regular\""));
        assert!(json.contains("\"Body\""));
        assert!(json.contains("\"Settings\""));
        assert!(json.contains("\"mascot.musicfrog-cutout\""));
        assert_eq!(MASCOT_ASSET.hero_path, "mascot/musicfrog-hero-512.png");
        const { assert!(!MASCOT_ASSET.redistributable) };
        assert_eq!(document.font_weights, vec![400, 500, 600]);
        assert_eq!(
            document
                .type_roles
                .iter()
                .filter(|role| role.family_name == FontFamilyToken::Cjk.as_str())
                .count(),
            16
        );
        assert!(document.type_roles.iter().all(|role| {
            document
                .font_faces
                .iter()
                .any(|face| face.catalog_id == role.catalog_id)
        }));
    }
}

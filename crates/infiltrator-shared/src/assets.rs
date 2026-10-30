//! Framework-neutral catalog for visual assets shared by MusicFrog products.
//!
//! Asset identity and license provenance live in `assets/asset-catalog.json`.
//! Renderers may request canonical files by semantic ID and format, while
//! surfaces with compile-time embedding use the small typed helpers below.

use serde::Deserialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const CATALOG_SOURCE: &str = include_str!("../assets/asset-catalog.json");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetFormat {
    Svg,
    Png,
    Ico,
    Ttf,
}

impl AssetFormat {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Svg => "svg",
            Self::Png => "png",
            Self::Ico => "ico",
            Self::Ttf => "ttf",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct AssetRecord {
    pub id: String,
    pub format: String,
    pub variant: String,
    /// Path relative to this crate's `assets/` directory.
    pub path: String,
    pub mime_type: String,
    pub license: String,
    /// Repository-relative path to the notice or license text.
    pub license_path: Option<String>,
    pub source: String,
}

#[derive(Debug, Deserialize)]
struct AssetCatalogDocument {
    version: u32,
    assets: Vec<AssetRecord>,
}

fn catalog() -> &'static AssetCatalogDocument {
    static CATALOG: OnceLock<AssetCatalogDocument> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog: AssetCatalogDocument =
            serde_json::from_str(CATALOG_SOURCE).expect("shared asset catalog must be valid JSON");
        assert_eq!(
            catalog.version, 1,
            "unsupported shared asset catalog version"
        );
        catalog
    })
}

/// Read-only asset catalog shared by the framework adapters.
pub struct AssetCatalog;

impl AssetCatalog {
    /// All catalog records, including format variants and license provenance.
    pub fn records() -> &'static [AssetRecord] {
        &catalog().assets
    }

    /// Find one semantic asset in the requested format and variant.
    pub fn find(id: &str, format: AssetFormat, variant: &str) -> Option<&'static AssetRecord> {
        Self::records().iter().find(|record| {
            record.id == id && record.format == format.as_str() && record.variant == variant
        })
    }

    /// Resolve a catalog record to its canonical source path for build tools
    /// and renderers that consume files rather than embedded bytes.
    pub fn path(id: &str, format: AssetFormat, variant: &str) -> Option<PathBuf> {
        let record = Self::find(id, format, variant)?;
        Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join(&record.path),
        )
    }

    /// Read one canonical resource file. Product binaries should prefer the
    /// embedded helpers for common UI icons and launcher art.
    pub fn read(id: &str, format: AssetFormat, variant: &str) -> io::Result<Option<Vec<u8>>> {
        Self::path(id, format, variant).map_or(Ok(None), |path| fs::read(path).map(Some))
    }
}

/// Embedded monochrome SVG source used by the Iced surface.
pub fn iced_icon_svg(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "layout-grid" => include_bytes!("../assets/ui/iced/svg/layout-grid.svg"),
        "file-text" => include_bytes!("../assets/ui/iced/svg/file-text.svg"),
        "globe" => include_bytes!("../assets/ui/iced/svg/globe.svg"),
        "activity" => include_bytes!("../assets/ui/iced/svg/activity.svg"),
        "list-checks" => include_bytes!("../assets/ui/iced/svg/list-checks.svg"),
        "network" => include_bytes!("../assets/ui/iced/svg/network.svg"),
        "refresh-cw" => include_bytes!("../assets/ui/iced/svg/refresh-cw.svg"),
        "settings" => include_bytes!("../assets/ui/iced/svg/settings.svg"),
        "code-2" => include_bytes!("../assets/ui/iced/svg/code-2.svg"),
        "search" => include_bytes!("../assets/ui/iced/svg/search.svg"),
        "target" => include_bytes!("../assets/ui/iced/svg/target.svg"),
        "zap" => include_bytes!("../assets/ui/iced/svg/zap.svg"),
        "shield" => include_bytes!("../assets/ui/iced/svg/shield.svg"),
        "wifi" => include_bytes!("../assets/ui/iced/svg/wifi.svg"),
        "chevron-down" => include_bytes!("../assets/ui/iced/svg/chevron-down.svg"),
        "chevron-right" => include_bytes!("../assets/ui/iced/svg/chevron-right.svg"),
        "chevron-left" => include_bytes!("../assets/ui/iced/svg/chevron-left.svg"),
        "chevron-up" => include_bytes!("../assets/ui/iced/svg/chevron-up.svg"),
        "x" => include_bytes!("../assets/ui/iced/svg/x.svg"),
        "plus" => include_bytes!("../assets/ui/iced/svg/plus.svg"),
        "trash-2" => include_bytes!("../assets/ui/iced/svg/trash-2.svg"),
        "copy" => include_bytes!("../assets/ui/iced/svg/copy.svg"),
        "pencil" => include_bytes!("../assets/ui/iced/svg/pencil.svg"),
        "pin" => include_bytes!("../assets/ui/iced/svg/pin.svg"),
        "arrow-up" => include_bytes!("../assets/ui/iced/svg/arrow-up.svg"),
        "arrow-down" => include_bytes!("../assets/ui/iced/svg/arrow-down.svg"),
        "server" => include_bytes!("../assets/ui/iced/svg/server.svg"),
        "plug" => include_bytes!("../assets/ui/iced/svg/plug.svg"),
        "minus" => include_bytes!("../assets/ui/iced/svg/minus.svg"),
        "square" => include_bytes!("../assets/ui/iced/svg/square.svg"),
        _ => return None,
    })
}

/// Embedded white-ink PNG plate used by the Bevy renderer.
///
/// Every plate is rasterized from the canonical Lucide SVG of the same name
/// (`assets/ui/iced/svg/<name>.svg`) through the shared currentColor→white
/// flow: substitute `currentColor` with `#ffffff`, then rasterize the
/// 24×24 stroke drawing into a 64×64 RGBA plate. A plate is white ink with
/// alpha, so any theme color tints it exactly.
pub fn bevy_icon_png(name: &str) -> Option<&'static [u8]> {
    Some(match name {
        "settings" => include_bytes!("../assets/ui/bevy/png/settings.png"),
        "network" => include_bytes!("../assets/ui/bevy/png/network.png"),
        "activity" => include_bytes!("../assets/ui/bevy/png/activity.png"),
        "globe" => include_bytes!("../assets/ui/bevy/png/globe.png"),
        "zap" => include_bytes!("../assets/ui/bevy/png/zap.png"),
        "file-text" => include_bytes!("../assets/ui/bevy/png/file-text.png"),
        "plus" => include_bytes!("../assets/ui/bevy/png/plus.png"),
        "trash-2" => include_bytes!("../assets/ui/bevy/png/trash-2.png"),
        "arrow-up" => include_bytes!("../assets/ui/bevy/png/arrow-up.png"),
        "arrow-down" => include_bytes!("../assets/ui/bevy/png/arrow-down.png"),
        // The window's own three keys and the history arrows: plates, never
        // text glyphs.
        "minus" => include_bytes!("../assets/ui/bevy/png/minus.png"),
        "square" => include_bytes!("../assets/ui/bevy/png/square.png"),
        "x" => include_bytes!("../assets/ui/bevy/png/x.png"),
        "chevron-left" => include_bytes!("../assets/ui/bevy/png/chevron-left.png"),
        "chevron-right" => include_bytes!("../assets/ui/bevy/png/chevron-right.png"),
        // The routes' own plates: one per route, so the rail never repeats.
        "layout-grid" => include_bytes!("../assets/ui/bevy/png/layout-grid.png"),
        "list-checks" => include_bytes!("../assets/ui/bevy/png/list-checks.png"),
        "server" => include_bytes!("../assets/ui/bevy/png/server.png"),
        "shield" => include_bytes!("../assets/ui/bevy/png/shield.png"),
        "code-2" => include_bytes!("../assets/ui/bevy/png/code-2.png"),
        "target" => include_bytes!("../assets/ui/bevy/png/target.png"),
        _ => return None,
    })
}

/// Shared product launcher artwork in the platform-neutral raster and Windows
/// icon-container formats.
pub fn product_icon_png() -> &'static [u8] {
    include_bytes!("../assets/product/icon.png")
}

pub fn product_icon_ico() -> &'static [u8] {
    include_bytes!("../assets/product/icon.ico")
}

/// Complete redistribution agreement that must accompany the embedded CJK
/// face. Product UIs also show its required copyright attribution.
pub fn harmonyos_font_license_text() -> &'static str {
    include_str!("../assets/fonts/HarmonyOS-Sans-LICENSE.txt")
}

#[cfg(test)]
mod tests {
    use super::{AssetCatalog, AssetFormat, harmonyos_font_license_text};
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;

    fn identity_value(key: &str) -> Option<String> {
        let registry = include_str!("../../../product-identity.properties");
        registry.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim().to_string())
        })
    }

    fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
        const PNG_MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        if bytes.len() < 24 || bytes[..8] != PNG_MAGIC {
            return None;
        }
        let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        Some((width, height))
    }

    fn ico_frame_sizes(bytes: &[u8]) -> Option<Vec<(u32, u32)>> {
        if bytes.len() < 6 {
            return None;
        }
        let count = u16::from_le_bytes([bytes[4], bytes[5]]);
        let mut frames = Vec::new();
        for index in 0..count {
            let entry = 6 + usize::from(index) * 16;
            if entry + 16 > bytes.len() {
                return None;
            }
            // 0 in an ICONDIRENTRY means the 256px slot.
            let width = if bytes[entry] == 0 {
                256
            } else {
                u32::from(bytes[entry])
            };
            let height = if bytes[entry + 1] == 0 {
                256
            } else {
                u32::from(bytes[entry + 1])
            };
            frames.push((width, height));
        }
        Some(frames)
    }

    #[test]
    fn every_catalog_identity_resolves_to_an_asset_and_its_license_source() {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root exists");
        let mut identities = HashSet::new();
        for record in AssetCatalog::records() {
            assert!(!record.id.trim().is_empty());
            assert!(!record.variant.trim().is_empty());
            assert!(identities.insert((
                record.id.as_str(),
                record.format.as_str(),
                record.variant.as_str(),
            )));
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets")
                    .join(&record.path)
                    .is_file(),
                "catalog asset is missing: {}",
                record.path
            );
            if let Some(license_path) = &record.license_path {
                let license = repository_root.join(license_path);
                assert!(
                    license.is_file(),
                    "catalog license is missing: {license_path}"
                );
                assert!(
                    fs::metadata(license).expect("license metadata").len() > 0,
                    "catalog license is empty: {license_path}"
                );
            }
        }
    }

    #[test]
    fn product_launcher_formats_share_one_identity_and_font_notice_is_complete() {
        assert!(AssetCatalog::find("app.product", AssetFormat::Png, "launcher").is_some());
        assert!(AssetCatalog::find("app.product", AssetFormat::Ico, "launcher").is_some());
        assert!(harmonyos_font_license_text().contains("HarmonyOS Sans"));
        assert!(AssetCatalog::find("font.CJK-Regular", AssetFormat::Ttf, "default").is_some());
    }

    #[test]
    fn product_icon_variants_match_the_identity_registry() {
        let ladder: Vec<u32> = identity_value("product.icon_variant_px")
            .expect("product.icon_variant_px is registered")
            .split(',')
            .map(|value| value.trim().parse().expect("pixel size must be numeric"))
            .collect();
        let densities: Vec<(String, u32)> = identity_value("product.android_icon_density")
            .expect("product.android_icon_density is registered")
            .split(',')
            .map(|entry| {
                let (density, pixels) = entry.split_once(':').expect("density:px pair");
                (
                    density.trim().to_string(),
                    pixels
                        .trim()
                        .parse()
                        .expect("density pixel size must be numeric"),
                )
            })
            .collect();
        assert!(!ladder.is_empty());
        assert!(!densities.is_empty());

        for pixels in &ladder {
            let record =
                AssetCatalog::find("app.product", AssetFormat::Png, &format!("icon-{pixels}"))
                    .unwrap_or_else(|| panic!("catalog is missing the icon-{pixels} variant"));
            let bytes = fs::read(record_path(record.path.as_str())).expect("variant readable");
            assert_eq!(
                png_dimensions(&bytes),
                Some((*pixels, *pixels)),
                "icon-{pixels} variant has the wrong dimensions"
            );
        }
        for (density, pixels) in &densities {
            let record = AssetCatalog::find(
                "app.product",
                AssetFormat::Png,
                &format!("android-{density}"),
            )
            .unwrap_or_else(|| panic!("catalog is missing the android-{density} variant"));
            let bytes = fs::read(record_path(record.path.as_str())).expect("variant readable");
            assert_eq!(
                png_dimensions(&bytes),
                Some((*pixels, *pixels)),
                "android-{density} variant has the wrong dimensions"
            );
        }

        let ico_record = AssetCatalog::find("app.product", AssetFormat::Ico, "launcher")
            .expect("product ICO record");
        let ico_bytes = fs::read(record_path(ico_record.path.as_str())).expect("ico readable");
        let mut frames = ico_frame_sizes(&ico_bytes).expect("parseable ICO directory");
        frames.sort_unstable();
        let mut expected = ladder
            .iter()
            .map(|pixels| (*pixels, *pixels))
            .collect::<Vec<_>>();
        expected.sort_unstable();
        assert_eq!(
            frames, expected,
            "ICO frames must mirror the shared px ladder"
        );
    }

    fn record_path(relative: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("assets")
            .join(relative)
    }
}

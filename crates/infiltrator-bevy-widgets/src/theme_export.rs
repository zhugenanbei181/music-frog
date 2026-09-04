//! Theme token serializer exporting to Tailwind CSS, Material You, and standalone JSON specifications.

use crate::palette::UiPalette;
use bevy::color::Color;

/// Target export format for theme design tokens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeExportFormat {
    Json,
    TailwindCss,
    MaterialYouXml,
}

/// Convert linear/sRGB Bevy Color into standard hex `#RRGGBB` or `#RRGGBBAA` string.
pub fn color_to_hex(color: Color) -> String {
    let s = color.to_srgba();
    let r = (s.red * 255.0).round() as u8;
    let g = (s.green * 255.0).round() as u8;
    let b = (s.blue * 255.0).round() as u8;
    let a = (s.alpha * 255.0).round() as u8;
    if a == 255 {
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    } else {
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, a)
    }
}

/// Export full UiPalette tokens into target format.
pub fn export_palette_tokens(palette: &UiPalette, format: ThemeExportFormat) -> String {
    match format {
        ThemeExportFormat::Json => {
            format!(
                r#"{{"accent":"{}","surface":"{}","border":"{}","ink":"{}"}}"#,
                color_to_hex(palette.accent),
                color_to_hex(palette.surface),
                color_to_hex(palette.border),
                color_to_hex(palette.ink)
            )
        }
        ThemeExportFormat::TailwindCss => {
            format!(
                ":root {{
  --color-accent: {};
  --color-surface: {};
  --color-border: {};
  --color-ink: {};
}}",
                color_to_hex(palette.accent),
                color_to_hex(palette.surface),
                color_to_hex(palette.border),
                color_to_hex(palette.ink)
            )
        }
        ThemeExportFormat::MaterialYouXml => {
            format!(
                r#"<resources>
  <color name="md_theme_primary">{}</color>
  <color name="md_theme_surface">{}</color>
</resources>"#,
                color_to_hex(palette.accent),
                color_to_hex(palette.surface)
            )
        }
    }
}

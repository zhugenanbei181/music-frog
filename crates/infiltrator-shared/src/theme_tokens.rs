use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl RgbaColor {
    pub const fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
        if hex.len() == 6 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Self::new(r, g, b, 1.0))
        } else if hex.len() == 8 {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()? as f32 / 255.0;
            Some(Self::new(r, g, b, a))
        } else {
            None
        }
    }

    pub fn to_hex(&self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn to_css_rgba(&self) -> String {
        format!("rgba({}, {}, {}, {})", self.r, self.g, self.b, self.a)
    }

    pub fn with_alpha(&self, alpha: f32) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a: alpha,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemePalette {
    pub surface: RgbaColor,
    pub surface_variant: RgbaColor,
    pub text_primary: RgbaColor,
    pub text_secondary: RgbaColor,
    pub accent: RgbaColor,
    pub success: RgbaColor,
    pub warning: RgbaColor,
    pub danger: RgbaColor,
    pub border: RgbaColor,
}

impl ThemePalette {
    pub fn with_accent(&self, accent: RgbaColor) -> Self {
        let mut palette = self.clone();
        palette.accent = accent;
        palette
    }
}

/// Predefined accent colors for user customization across platforms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccentPreset {
    Blue,
    Emerald,
    Purple,
    Amber,
    Crimson,
    Cyan,
    Rose,
}

impl AccentPreset {
    pub const ALL: [AccentPreset; 7] = [
        AccentPreset::Blue,
        AccentPreset::Emerald,
        AccentPreset::Purple,
        AccentPreset::Amber,
        AccentPreset::Crimson,
        AccentPreset::Cyan,
        AccentPreset::Rose,
    ];

    pub const fn as_str(&self) -> &'static str {
        match self {
            AccentPreset::Blue => "blue",
            AccentPreset::Emerald => "emerald",
            AccentPreset::Purple => "purple",
            AccentPreset::Amber => "amber",
            AccentPreset::Crimson => "crimson",
            AccentPreset::Cyan => "cyan",
            AccentPreset::Rose => "rose",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "blue" | "default" | "ocean" => Some(AccentPreset::Blue),
            "emerald" | "green" => Some(AccentPreset::Emerald),
            "purple" | "violet" => Some(AccentPreset::Purple),
            "amber" | "yellow" | "gold" | "orange" => Some(AccentPreset::Amber),
            "crimson" | "red" | "ruby" => Some(AccentPreset::Crimson),
            "cyan" | "teal" | "sky" => Some(AccentPreset::Cyan),
            "rose" | "pink" => Some(AccentPreset::Rose),
            _ => None,
        }
    }

    pub const fn color(&self, is_dark: bool) -> RgbaColor {
        match self {
            AccentPreset::Blue => {
                if is_dark {
                    RgbaColor::new(30, 143, 245, 1.0)
                } else {
                    RgbaColor::new(10, 112, 224, 1.0)
                }
            }
            AccentPreset::Emerald => RgbaColor::new(16, 185, 129, 1.0),
            AccentPreset::Purple => RgbaColor::new(139, 92, 246, 1.0),
            AccentPreset::Amber => RgbaColor::new(245, 158, 11, 1.0),
            AccentPreset::Crimson => RgbaColor::new(239, 68, 68, 1.0),
            AccentPreset::Cyan => RgbaColor::new(6, 182, 212, 1.0),
            AccentPreset::Rose => RgbaColor::new(244, 63, 94, 1.0),
        }
    }
}

pub struct ThemeTokens;

impl ThemeTokens {
    pub fn light_default() -> ThemePalette {
        ThemePalette {
            surface: RgbaColor::new(255, 255, 255, 1.0),
            surface_variant: RgbaColor::new(245, 245, 245, 1.0),
            text_primary: RgbaColor::new(0, 0, 0, 0.87),
            text_secondary: RgbaColor::new(0, 0, 0, 0.6),
            accent: RgbaColor::new(98, 0, 238, 1.0),
            success: RgbaColor::new(76, 175, 80, 1.0),
            warning: RgbaColor::new(255, 152, 0, 1.0),
            danger: RgbaColor::new(244, 67, 54, 1.0),
            border: RgbaColor::new(0, 0, 0, 0.12),
        }
    }

    pub fn dark_default() -> ThemePalette {
        ThemePalette {
            surface: RgbaColor::new(18, 18, 18, 1.0),
            surface_variant: RgbaColor::new(30, 30, 30, 1.0),
            text_primary: RgbaColor::new(255, 255, 255, 0.87),
            text_secondary: RgbaColor::new(255, 255, 255, 0.6),
            accent: RgbaColor::new(187, 134, 252, 1.0),
            success: RgbaColor::new(129, 199, 132, 1.0),
            warning: RgbaColor::new(255, 183, 77, 1.0),
            danger: RgbaColor::new(229, 115, 115, 1.0),
            border: RgbaColor::new(255, 255, 255, 0.12),
        }
    }

    pub fn amoled_default() -> ThemePalette {
        ThemePalette {
            surface: RgbaColor::new(0, 0, 0, 1.0),
            surface_variant: RgbaColor::new(22, 25, 28, 1.0),
            text_primary: RgbaColor::new(248, 250, 252, 1.0),
            text_secondary: RgbaColor::new(248, 250, 252, 0.68),
            accent: RgbaColor::new(30, 143, 245, 1.0),
            success: RgbaColor::new(16, 185, 129, 1.0),
            warning: RgbaColor::new(245, 158, 11, 1.0),
            danger: RgbaColor::new(245, 89, 82, 1.0),
            border: RgbaColor::new(255, 255, 255, 0.12),
        }
    }

    pub fn forest_default() -> ThemePalette {
        ThemePalette {
            surface: RgbaColor::new(239, 245, 236, 1.0),
            surface_variant: RgbaColor::new(248, 251, 245, 1.0),
            text_primary: RgbaColor::new(31, 53, 37, 1.0),
            text_secondary: RgbaColor::new(87, 112, 90, 0.75),
            accent: RgbaColor::new(48, 111, 78, 1.0),
            success: RgbaColor::new(62, 125, 80, 1.0),
            warning: RgbaColor::new(169, 112, 40, 1.0),
            danger: RgbaColor::new(179, 59, 70, 1.0),
            border: RgbaColor::new(87, 112, 90, 0.22),
        }
    }

    pub fn palette_from_name(name: &str) -> ThemePalette {
        match name.trim().to_ascii_lowercase().as_str() {
            "forest" | "eyeforest" | "eye-forest" => Self::forest_default(),
            "amoled" | "black" | "pitch-black" | "pitch_black" | "pitchblack" => {
                Self::amoled_default()
            }
            "light" => Self::light_default(),
            _ => Self::dark_default(),
        }
    }

    pub fn generate_css_variables(palette: &ThemePalette, selector: &str) -> String {
        format!(
            "{} {{\n\
             \t--color-surface: {};\n\
             \t--color-surface-variant: {};\n\
             \t--color-text-primary: {};\n\
             \t--color-text-secondary: {};\n\
             \t--color-accent: {};\n\
             \t--color-success: {};\n\
             \t--color-warning: {};\n\
             \t--color-danger: {};\n\
             \t--color-border: {};\n\
             }}",
            selector,
            palette.surface.to_css_rgba(),
            palette.surface_variant.to_css_rgba(),
            palette.text_primary.to_css_rgba(),
            palette.text_secondary.to_css_rgba(),
            palette.accent.to_css_rgba(),
            palette.success.to_css_rgba(),
            palette.warning.to_css_rgba(),
            palette.danger.to_css_rgba(),
            palette.border.to_css_rgba(),
        )
    }

    pub fn export_json(palette: &ThemePalette) -> Result<String, anyhow::Error> {
        Ok(serde_json::to_string_pretty(palette)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgba_color_to_hex() {
        let color = RgbaColor::new(255, 0, 128, 1.0);
        assert_eq!(color.to_hex(), "#FF0080");
    }

    #[test]
    fn test_rgba_color_from_hex() {
        let color = RgbaColor::from_hex("#FF0080").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 128);
        assert_eq!(color.a, 1.0);

        let color_alpha = RgbaColor::from_hex("00FF0080").unwrap();
        assert_eq!(color_alpha.r, 0);
        assert_eq!(color_alpha.g, 255);
        assert_eq!(color_alpha.b, 0);
        assert!((color_alpha.a - 0.5019).abs() < 0.01);
    }

    #[test]
    fn test_rgba_color_to_css_rgba() {
        let color = RgbaColor::new(255, 0, 128, 0.5);
        assert_eq!(color.to_css_rgba(), "rgba(255, 0, 128, 0.5)");
    }

    #[test]
    fn test_rgba_color_with_alpha() {
        let color = RgbaColor::new(255, 0, 128, 1.0);
        let new_color = color.with_alpha(0.5);
        assert_eq!(new_color.a, 0.5);
        assert_eq!(new_color.r, 255);
    }

    #[test]
    fn test_light_dark_default() {
        let light = ThemeTokens::light_default();
        let dark = ThemeTokens::dark_default();

        assert_eq!(light.surface.r, 255);
        assert_eq!(dark.surface.r, 18);
    }

    #[test]
    fn test_amoled_forest_default() {
        let amoled = ThemeTokens::amoled_default();
        let forest = ThemeTokens::forest_default();

        assert_eq!(amoled.surface.r, 0);
        assert_eq!(amoled.surface.g, 0);
        assert_eq!(amoled.surface.b, 0);

        assert_eq!(forest.surface.r, 239);
        assert_eq!(forest.surface.g, 245);
        assert_eq!(forest.surface.b, 236);
    }

    #[test]
    fn test_palette_from_name() {
        let amoled = ThemeTokens::palette_from_name("amoled");
        let forest = ThemeTokens::palette_from_name("forest");
        let light = ThemeTokens::palette_from_name("light");
        let dark = ThemeTokens::palette_from_name("dark");

        assert_eq!(amoled, ThemeTokens::amoled_default());
        assert_eq!(forest, ThemeTokens::forest_default());
        assert_eq!(light, ThemeTokens::light_default());
        assert_eq!(dark, ThemeTokens::dark_default());
    }

    #[test]
    fn test_accent_presets() {
        for preset in AccentPreset::ALL {
            let name = preset.as_str();
            assert_eq!(AccentPreset::from_name(name), Some(preset));
            let color = preset.color(true);
            assert_eq!(color.a, 1.0);
        }
    }

    #[test]
    fn test_generate_css_variables() {
        let palette = ThemeTokens::light_default();
        let css = ThemeTokens::generate_css_variables(&palette, ":root");
        assert!(css.contains(":root {"));
        assert!(css.contains("--color-surface: rgba(255, 255, 255, 1)"));
        assert!(css.contains("--color-danger: rgba(244, 67, 54, 1)"));
    }

    #[test]
    fn test_export_json() {
        let palette = ThemeTokens::light_default();
        let json = ThemeTokens::export_json(&palette).unwrap();

        let deserialized: ThemePalette = serde_json::from_str(&json).unwrap();
        assert_eq!(palette, deserialized);
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f32,
}

impl RgbaColor {
    pub fn new(r: u8, g: u8, b: u8, a: f32) -> Self {
        Self { r, g, b, a }
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

//! Bridge the shared sidebar label geometry to the shipped font metrics.

use infiltrator_contract::sidebar_label::SidebarLabelGeometry;
use infiltrator_contract::text_truncation::TextMeasure;
use infiltrator_shared::effect_assets::char_width_em;

/// One character of a shipped font face, measured in logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FontMeasure {
    size_px: f32,
}

impl FontMeasure {
    pub const fn new(size_px: f32) -> Self {
        Self { size_px }
    }
}

impl TextMeasure for FontMeasure {
    fn char_width(&self, ch: char) -> f32 {
        char_width_em(ch) * self.size_px
    }
}

/// Whether a desktop sidebar label was cut, using the geometry's own font size.
pub fn sidebar_label_is_truncated(label: &str, geometry: SidebarLabelGeometry) -> bool {
    let measure = FontMeasure::new(geometry.label_size_px());
    geometry.is_truncated(label, &measure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_chinese_label_fits() {
        assert!(!sidebar_label_is_truncated(
            "代理",
            SidebarLabelGeometry::DESKTOP
        ));
    }

    #[test]
    fn an_overlong_cyrillic_label_is_cut() {
        assert!(sidebar_label_is_truncated(
            "Подключения и активные соединения",
            SidebarLabelGeometry::DESKTOP
        ));
    }

    #[test]
    fn full_width_characters_use_their_own_advance() {
        let long = "连接与活动会话连接与活动会话连接与活动会话连接与活动会话连接与活动会话";
        assert!(sidebar_label_is_truncated(
            long,
            SidebarLabelGeometry::DESKTOP
        ));
    }

    #[test]
    fn scaling_keeps_the_same_answer() {
        let geometry = SidebarLabelGeometry {
            scale_percent: 125,
            ..SidebarLabelGeometry::DESKTOP
        };
        let label = "连接与活动会话";
        assert_eq!(
            sidebar_label_is_truncated(label, SidebarLabelGeometry::DESKTOP),
            sidebar_label_is_truncated(label, geometry)
        );
    }

    #[test]
    fn an_empty_label_is_never_cut() {
        assert!(!sidebar_label_is_truncated(
            "",
            SidebarLabelGeometry::DESKTOP
        ));
    }
}

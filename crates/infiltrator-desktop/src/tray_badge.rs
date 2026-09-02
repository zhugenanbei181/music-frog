use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BadgeStatus {
    Running,
    Stopped,
    Warning,
    Reloading,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum BadgeColor {
    Green,
    Red,
    Yellow,
    Blue,
    Gray,
}

pub struct TrayBadgeGenerator;

impl TrayBadgeGenerator {
    pub fn status_to_color(status: BadgeStatus) -> BadgeColor {
        match status {
            BadgeStatus::Running => BadgeColor::Green,
            BadgeStatus::Stopped => BadgeColor::Red,
            BadgeStatus::Warning => BadgeColor::Yellow,
            BadgeStatus::Reloading => BadgeColor::Blue,
        }
    }

    pub fn color_to_hex(color: BadgeColor) -> &'static str {
        match color {
            BadgeColor::Green => "#4CAF50",
            BadgeColor::Red => "#F44336",
            BadgeColor::Yellow => "#FFC107",
            BadgeColor::Blue => "#2196F3",
            BadgeColor::Gray => "#9E9E9E",
        }
    }

    pub fn generate_svg_badge(status: BadgeStatus, size: u32) -> String {
        let color = Self::status_to_color(status);
        let hex = Self::color_to_hex(color);
        let radius = size / 2;
        format!(
            r#"<svg width="{size}" height="{size}" xmlns="http://www.w3.org/2000/svg"><circle cx="{radius}" cy="{radius}" r="{radius}" fill="{hex}"/></svg>"#,
            size = size,
            radius = radius,
            hex = hex
        )
    }

    pub fn generate_activity_tooltip(status: BadgeStatus, up_bps: u64, down_bps: u64) -> String {
        let status_str = match status {
            BadgeStatus::Running => "Running",
            BadgeStatus::Stopped => "Stopped",
            BadgeStatus::Warning => "Warning",
            BadgeStatus::Reloading => "Reloading",
        };
        format!(
            "Status: {}\nUp: {} bps\nDown: {} bps",
            status_str, up_bps, down_bps
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_to_color() {
        assert_eq!(
            TrayBadgeGenerator::status_to_color(BadgeStatus::Running),
            BadgeColor::Green
        );
        assert_eq!(
            TrayBadgeGenerator::status_to_color(BadgeStatus::Stopped),
            BadgeColor::Red
        );
        assert_eq!(
            TrayBadgeGenerator::status_to_color(BadgeStatus::Warning),
            BadgeColor::Yellow
        );
        assert_eq!(
            TrayBadgeGenerator::status_to_color(BadgeStatus::Reloading),
            BadgeColor::Blue
        );
    }

    #[test]
    fn test_color_to_hex() {
        assert_eq!(
            TrayBadgeGenerator::color_to_hex(BadgeColor::Green),
            "#4CAF50"
        );
        assert_eq!(TrayBadgeGenerator::color_to_hex(BadgeColor::Red), "#F44336");
        assert_eq!(
            TrayBadgeGenerator::color_to_hex(BadgeColor::Yellow),
            "#FFC107"
        );
        assert_eq!(
            TrayBadgeGenerator::color_to_hex(BadgeColor::Blue),
            "#2196F3"
        );
        assert_eq!(
            TrayBadgeGenerator::color_to_hex(BadgeColor::Gray),
            "#9E9E9E"
        );
    }

    #[test]
    fn test_generate_svg_badge() {
        let svg = TrayBadgeGenerator::generate_svg_badge(BadgeStatus::Running, 24);
        assert!(svg.contains(r#"<svg width="24" height="24""#));
        assert!(svg.contains(r#"<circle cx="12" cy="12" r="12""#));
        assert!(svg.contains(r##"fill="#4CAF50""##));
    }

    #[test]
    fn test_generate_activity_tooltip() {
        let tooltip =
            TrayBadgeGenerator::generate_activity_tooltip(BadgeStatus::Running, 1024, 2048);
        assert_eq!(tooltip, "Status: Running\nUp: 1024 bps\nDown: 2048 bps");
    }
}

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum MotionPreference {
    FullMotion,
    ReducedMotion,
    NoMotion,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SkeletonState {
    Loading { estimated_ms: u64, shimmer: bool },
    Loaded,
    Failed { retry_allowed: bool },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ThemeContrast {
    Standard,
    HighContrast,
    AmoledDark,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ThemeSpecification {
    pub theme_id: String,
    pub contrast: ThemeContrast,
    pub motion: MotionPreference,
    pub accent_color: String,
    pub background_color: String,
}

pub fn build_theme_spec(
    theme_id: &str,
    contrast: ThemeContrast,
    motion: MotionPreference,
) -> ThemeSpecification {
    let (accent_color, background_color) = match contrast {
        ThemeContrast::Standard => ("#1DB954".to_string(), "#FFFFFF".to_string()),
        ThemeContrast::HighContrast => ("#00FF00".to_string(), "#000000".to_string()),
        ThemeContrast::AmoledDark => ("#1ED760".to_string(), "#000000".to_string()),
    };

    ThemeSpecification {
        theme_id: theme_id.to_string(),
        contrast,
        motion,
        accent_color,
        background_color,
    }
}

pub fn compute_duration_ms(base_ms: u64, motion: MotionPreference) -> u64 {
    match motion {
        MotionPreference::FullMotion => base_ms,
        MotionPreference::ReducedMotion => base_ms / 2,
        MotionPreference::NoMotion => 0,
    }
}

pub fn is_shimmer_enabled(state: &SkeletonState) -> bool {
    match state {
        SkeletonState::Loading { shimmer, .. } => *shimmer,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_theme_spec() {
        let standard = build_theme_spec("light", ThemeContrast::Standard, MotionPreference::FullMotion);
        assert_eq!(standard.theme_id, "light");
        assert_eq!(standard.contrast, ThemeContrast::Standard);
        assert_eq!(standard.motion, MotionPreference::FullMotion);

        let high_contrast = build_theme_spec("hc", ThemeContrast::HighContrast, MotionPreference::ReducedMotion);
        assert_eq!(high_contrast.theme_id, "hc");
        assert_eq!(high_contrast.contrast, ThemeContrast::HighContrast);

        let amoled = build_theme_spec("amoled", ThemeContrast::AmoledDark, MotionPreference::NoMotion);
        assert_eq!(amoled.theme_id, "amoled");
        assert_eq!(amoled.contrast, ThemeContrast::AmoledDark);
    }

    #[test]
    fn test_compute_duration_ms() {
        assert_eq!(compute_duration_ms(1000, MotionPreference::FullMotion), 1000);
        assert_eq!(compute_duration_ms(1000, MotionPreference::ReducedMotion), 500);
        assert_eq!(compute_duration_ms(1000, MotionPreference::NoMotion), 0);
    }

    #[test]
    fn test_is_shimmer_enabled() {
        assert!(is_shimmer_enabled(&SkeletonState::Loading { estimated_ms: 1000, shimmer: true }));
        assert!(!is_shimmer_enabled(&SkeletonState::Loading { estimated_ms: 1000, shimmer: false }));
        assert!(!is_shimmer_enabled(&SkeletonState::Loaded));
        assert!(!is_shimmer_enabled(&SkeletonState::Failed { retry_allowed: true }));
    }

    #[test]
    fn test_serialization() {
        let spec = ThemeSpecification {
            theme_id: "test".to_string(),
            contrast: ThemeContrast::Standard,
            motion: MotionPreference::FullMotion,
            accent_color: "#fff".to_string(),
            background_color: "#000".to_string(),
        };
        let serialized = serde_json::to_string(&spec).unwrap();
        let deserialized: ThemeSpecification = serde_json::from_str(&serialized).unwrap();
        assert_eq!(spec, deserialized);

        let state = SkeletonState::Loading { estimated_ms: 500, shimmer: true };
        let serialized_state = serde_json::to_string(&state).unwrap();
        let deserialized_state: SkeletonState = serde_json::from_str(&serialized_state).unwrap();
        assert_eq!(state, deserialized_state);
    }
}

//! EFFECTS-047（#192）：横竖屏旋转与窗口尺寸变化的 typed 事实。
//!
//! Kotlin 宿主在 `onConfigurationChanged` 里读取真实的窗口方向、像素尺寸、
//! 显示密度与系统字体缩放，推给本模块。本模块是「窗口形状」唯一的真源：
//! 共享外壳据此重新布局，而 Compose 表面不需要自己推导旋转结果。

/// 宿主窗口的方向。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostOrientation {
    /// 竖屏。
    Portrait,
    /// 横屏。
    Landscape,
    /// 正方形窗口（折叠屏展开等形态）。
    Square,
}

impl HostOrientation {
    /// 解码宿主上报的方向线码。
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Portrait),
            1 => Some(Self::Landscape),
            2 => Some(Self::Square),
            _ => None,
        }
    }

    /// 方向的线码（Kotlin 常量镜像这些值）。
    pub const fn as_code(self) -> i32 {
        match self {
            Self::Portrait => 0,
            Self::Landscape => 1,
            Self::Square => 2,
        }
    }
}

/// 宿主推送的一条配置事实。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigurationFact {
    /// 单调递增的 Activity 实例代号；重建后的实例代号更大。
    pub generation: u64,
    pub orientation: HostOrientation,
    pub width_px: u32,
    pub height_px: u32,
    /// 显示密度（`resources.displayMetrics.density`）。
    pub density: f32,
    /// 系统字体缩放（`resources.configuration.fontScale`）。
    pub font_scale: f32,
}

impl ConfigurationFact {
    pub const fn new(
        generation: u64,
        orientation: HostOrientation,
        width_px: u32,
        height_px: u32,
        density: f32,
        font_scale: f32,
    ) -> Self {
        Self {
            generation,
            orientation,
            width_px,
            height_px,
            density,
            font_scale,
        }
    }
}

/// 宿主当前发布的 typed 配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HostConfigurationState {
    /// 还没有收到任何配置事实。
    Unobserved,
    /// 最后一次被接受的配置。
    Configured(HostConfiguration),
}

/// 一次被接受的窗口配置。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HostConfiguration {
    pub generation: u64,
    pub orientation: HostOrientation,
    pub width_px: u32,
    pub height_px: u32,
    pub density: f32,
    pub font_scale: f32,
}

/// 两次配置之间的 typed 差异。
///
/// 一次配置变更可以同时改变多个维度（旋转会同时改方向与尺寸），因此这里
/// 给出**第一个**命中的差异，优先级从高到低是方向、尺寸、密度、字体缩放。
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConfigurationChange {
    /// 配置没有变化。
    Unchanged,
    /// 方向变了：外壳必须重新布局。
    OrientationChanged {
        from: HostOrientation,
        to: HostOrientation,
    },
    /// 尺寸变了但方向没变。
    SizeChanged { width_px: u32, height_px: u32 },
    /// 显示密度变了。
    DensityChanged { from: f32, to: f32 },
    /// 系统字体缩放变了。
    FontScaleChanged { from: f32, to: f32 },
    /// 新的 Activity 代替换了上一代：配置必须重新读取。
    Recreated,
    /// 事实来自已经退役的代，被忽略。
    Stale,
}

impl ConfigurationChange {
    /// 这次变更是否要求外壳重新布局。
    pub const fn requires_relayout(self) -> bool {
        match self {
            Self::Unchanged | Self::Stale => false,
            Self::OrientationChanged { .. }
            | Self::SizeChanged { .. }
            | Self::DensityChanged { .. }
            | Self::FontScaleChanged { .. }
            | Self::Recreated => true,
        }
    }
}

/// 跟踪最新的窗口配置，并用代号栅栏围住 Activity 重建。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConfigurationTracker {
    current: HostConfigurationState,
}

impl ConfigurationTracker {
    pub const fn new() -> Self {
        Self {
            current: HostConfigurationState::Unobserved,
        }
    }

    /// 当前的 typed 配置。
    pub const fn current(&self) -> HostConfigurationState {
        self.current
    }

    /// 应用一条配置事实，返回 typed 差异。
    pub fn apply(&mut self, fact: ConfigurationFact) -> ConfigurationChange {
        let previous = match self.current {
            HostConfigurationState::Unobserved => {
                self.current = HostConfigurationState::Configured(host_configuration(fact));
                return ConfigurationChange::Unchanged;
            }
            HostConfigurationState::Configured(previous) => previous,
        };
        if fact.generation < previous.generation {
            return ConfigurationChange::Stale;
        }
        if fact.generation > previous.generation {
            self.current = HostConfigurationState::Configured(host_configuration(fact));
            return ConfigurationChange::Recreated;
        }
        let change = configuration_change(previous, fact);
        self.current = HostConfigurationState::Configured(host_configuration(fact));
        change
    }
}

impl Default for ConfigurationTracker {
    fn default() -> Self {
        Self::new()
    }
}

fn host_configuration(fact: ConfigurationFact) -> HostConfiguration {
    HostConfiguration {
        generation: fact.generation,
        orientation: fact.orientation,
        width_px: fact.width_px,
        height_px: fact.height_px,
        density: fact.density,
        font_scale: fact.font_scale,
    }
}

/// 两次配置之间的第一个差异，优先级从高到低是方向、尺寸、密度、字体缩放。
fn configuration_change(
    previous: HostConfiguration,
    fact: ConfigurationFact,
) -> ConfigurationChange {
    if previous.orientation != fact.orientation {
        return ConfigurationChange::OrientationChanged {
            from: previous.orientation,
            to: fact.orientation,
        };
    }
    if previous.width_px != fact.width_px || previous.height_px != fact.height_px {
        return ConfigurationChange::SizeChanged {
            width_px: fact.width_px,
            height_px: fact.height_px,
        };
    }
    if previous.density != fact.density {
        return ConfigurationChange::DensityChanged {
            from: previous.density,
            to: fact.density,
        };
    }
    if previous.font_scale != fact.font_scale {
        return ConfigurationChange::FontScaleChanged {
            from: previous.font_scale,
            to: fact.font_scale,
        };
    }
    ConfigurationChange::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn portrait(width_px: u32, height_px: u32) -> ConfigurationFact {
        ConfigurationFact::new(3, HostOrientation::Portrait, width_px, height_px, 2.75, 1.0)
    }

    #[test]
    fn wire_codes_round_trip_and_unknown_is_rejected() {
        for orientation in [
            HostOrientation::Portrait,
            HostOrientation::Landscape,
            HostOrientation::Square,
        ] {
            assert_eq!(
                HostOrientation::from_code(orientation.as_code()),
                Some(orientation)
            );
        }
        assert_eq!(HostOrientation::from_code(3), None);
        assert_eq!(HostOrientation::from_code(-1), None);
    }

    #[test]
    fn a_rotation_reports_the_orientation_change_first() {
        let mut tracker = ConfigurationTracker::new();

        assert_eq!(
            tracker.apply(portrait(1080, 1920)),
            ConfigurationChange::Unchanged
        );
        let landscape =
            ConfigurationFact::new(3, HostOrientation::Landscape, 1920, 1080, 2.75, 1.0);
        let change = tracker.apply(landscape);
        assert_eq!(
            change,
            ConfigurationChange::OrientationChanged {
                from: HostOrientation::Portrait,
                to: HostOrientation::Landscape,
            }
        );
        assert!(change.requires_relayout());
        assert_eq!(
            tracker.current(),
            HostConfigurationState::Configured(HostConfiguration {
                generation: 3,
                orientation: HostOrientation::Landscape,
                width_px: 1920,
                height_px: 1080,
                density: 2.75,
                font_scale: 1.0,
            })
        );
    }

    #[test]
    fn a_resize_without_a_rotation_reports_the_size_change() {
        let mut tracker = ConfigurationTracker::new();

        tracker.apply(portrait(1080, 1920));
        assert_eq!(
            tracker.apply(portrait(1080, 1600)),
            ConfigurationChange::SizeChanged {
                width_px: 1080,
                height_px: 1600,
            }
        );
    }

    #[test]
    fn density_and_font_scale_changes_are_reported() {
        let mut tracker = ConfigurationTracker::new();

        tracker.apply(portrait(1080, 1920));
        let denser = ConfigurationFact::new(3, HostOrientation::Portrait, 1080, 1920, 3.0, 1.0);
        assert_eq!(
            tracker.apply(denser),
            ConfigurationChange::DensityChanged {
                from: 2.75,
                to: 3.0,
            }
        );
        let bigger_text =
            ConfigurationFact::new(3, HostOrientation::Portrait, 1080, 1920, 3.0, 1.3);
        assert_eq!(
            tracker.apply(bigger_text),
            ConfigurationChange::FontScaleChanged { from: 1.0, to: 1.3 }
        );
        assert_eq!(tracker.apply(bigger_text), ConfigurationChange::Unchanged);
        assert!(!ConfigurationChange::Unchanged.requires_relayout());
    }

    #[test]
    fn a_new_activity_generation_reports_a_recreation() {
        let mut tracker = ConfigurationTracker::new();

        tracker.apply(portrait(1080, 1920));
        let recreated =
            ConfigurationFact::new(4, HostOrientation::Landscape, 1920, 1080, 2.75, 1.0);
        assert_eq!(tracker.apply(recreated), ConfigurationChange::Recreated);
        assert!(ConfigurationChange::Recreated.requires_relayout());
        assert_eq!(
            tracker.current(),
            HostConfigurationState::Configured(HostConfiguration {
                generation: 4,
                orientation: HostOrientation::Landscape,
                width_px: 1920,
                height_px: 1080,
                density: 2.75,
                font_scale: 1.0,
            })
        );
    }

    #[test]
    fn a_retired_activity_generation_is_stale() {
        let mut tracker = ConfigurationTracker::new();

        tracker.apply(portrait(1080, 1920));
        let stale = ConfigurationFact::new(2, HostOrientation::Landscape, 1920, 1080, 2.75, 1.0);
        assert_eq!(tracker.apply(stale), ConfigurationChange::Stale);
        assert_eq!(
            tracker.current(),
            HostConfigurationState::Configured(HostConfiguration {
                generation: 3,
                orientation: HostOrientation::Portrait,
                width_px: 1080,
                height_px: 1920,
                density: 2.75,
                font_scale: 1.0,
            })
        );
    }
}

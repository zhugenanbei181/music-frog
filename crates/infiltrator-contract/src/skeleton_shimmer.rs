//! Standard specifications for skeleton loading shimmer waves and empty-state illustrations (FLPARITY-025).

use serde::{Deserialize, Serialize};

/// Canonical specification for skeleton shimmer animation wave.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ShimmerAnimationSpec {
    /// Full animation cycle duration in milliseconds (default 1500ms).
    pub cycle_duration_ms: u32,
    /// Width of the sweeping highlight band as a fraction of container width (0.2 .. 0.8).
    pub wave_width_ratio: f32,
    /// Angle of the diagonal gradient band in degrees (default 45.0).
    pub angle_degrees: f32,
    /// Opacity boost at the crest of the shimmer wave (0.15 .. 0.35).
    pub highlight_opacity: f32,
    /// Base background opacity of the skeleton box (0.05 .. 0.15).
    pub base_opacity: f32,
}

impl Default for ShimmerAnimationSpec {
    fn default() -> Self {
        Self {
            cycle_duration_ms: 1500,
            wave_width_ratio: 0.45,
            angle_degrees: 45.0,
            highlight_opacity: 0.22,
            base_opacity: 0.08,
        }
    }
}

impl ShimmerAnimationSpec {
    /// Calculate current wave position in normalized coordinates `[-0.5, 1.5]`.
    pub fn wave_position(&self, elapsed_ms: u64) -> f32 {
        let period = self.cycle_duration_ms.max(1) as f32;
        let progress = (elapsed_ms as f32 % period) / period;
        -0.5 + progress * 2.0
    }

    /// Calculate brightness boost factor for a point at `(x, y)` given wave crest at `center_x`.
    pub fn brightness_boost_at(&self, point_x: f32, center_x: f32) -> f32 {
        let half_width = (self.wave_width_ratio * 0.5).max(0.01);
        let dist = (point_x - center_x).abs();
        if dist >= half_width {
            0.0
        } else {
            let factor = (1.0 - dist / half_width).clamp(0.0, 1.0);
            factor * self.highlight_opacity
        }
    }
}

/// Six canonical core business empty-state illustration categories.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmptyStateIllustrationKind {
    /// No proxy nodes available in the active group or subscription.
    NoProxies,
    /// No routing rules configured or matching.
    NoRules,
    /// No active live network connections observed.
    NoConnections,
    /// No runtime logs captured yet in memory.
    NoLogs,
    /// No recent DNS queries logged.
    NoDnsQueries,
    /// No subscription profiles added to workspace.
    NoProfiles,
}

impl EmptyStateIllustrationKind {
    pub const ALL: [Self; 6] = [
        Self::NoProxies,
        Self::NoRules,
        Self::NoConnections,
        Self::NoLogs,
        Self::NoDnsQueries,
        Self::NoProfiles,
    ];

    pub const fn title_zh(self) -> &'static str {
        match self {
            Self::NoProxies => "暂无代理节点",
            Self::NoRules => "暂无分流规则",
            Self::NoConnections => "当前无活跃连接",
            Self::NoLogs => "暂无运行日志",
            Self::NoDnsQueries => "暂无 DNS 查询记录",
            Self::NoProfiles => "未添加订阅配置",
        }
    }

    pub const fn description_zh(self) -> &'static str {
        match self {
            Self::NoProxies => "当前订阅或策略组中未包含可用代理节点，请检查订阅链接或尝试刷新。",
            Self::NoRules => "核心分流规则列表为空，所有流量将按兜底直连策略流转。",
            Self::NoConnections => "当前尚无应用程序发起网络通信，发起网页浏览后将在此实时展示。",
            Self::NoLogs => "内核日志队列为空，可通过设置调整日志详细度或触发网络操作。",
            Self::NoDnsQueries => "Fake-IP 与系统 DNS 解析历史为空，网络请求发起后自动记录。",
            Self::NoProfiles => "暂未导入任何 Clash / Sing-Box 配置文件，点击下方按钮快速开始。",
        }
    }

    pub const fn primary_action_zh(self) -> &'static str {
        match self {
            Self::NoProxies => "刷新当前订阅",
            Self::NoRules => "重置默认分流规则",
            Self::NoConnections => "打开连接详情",
            Self::NoLogs => "调整日志等级",
            Self::NoDnsQueries => "刷新 DNS 缓存",
            Self::NoProfiles => "添加订阅链接",
        }
    }

    pub const fn icon_token(self) -> &'static str {
        match self {
            Self::NoProxies => "globe_network_off",
            Self::NoRules => "routing_rule_empty",
            Self::NoConnections => "ethernet_unplugged",
            Self::NoLogs => "scroll_text_blank",
            Self::NoDnsQueries => "dns_search_missing",
            Self::NoProfiles => "folder_plus_empty",
        }
    }
}

/// Standardized layout archetype for skeleton placeholder cards.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkeletonCardLayoutKind {
    /// Proxy node card: icon box + node title line + protocol badge + latency pill.
    ProxyNodeCard,
    /// Rule row: rule type pill + payload line + target badge + hit count.
    RuleRow,
    /// Connection row: app icon + remote host + throughput spark + data volume.
    ConnectionRow,
    /// Profile card: profile name + traffic usage progress bar + expiration date.
    ProfileCard,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shimmer_wave_math_computes_smooth_boost() {
        let shimmer = ShimmerAnimationSpec::default();
        let pos_0 = shimmer.wave_position(0);
        assert!((-0.51..=-0.49).contains(&pos_0));

        let pos_mid = shimmer.wave_position(750);
        assert!((0.49..=0.51).contains(&pos_mid));

        let boost_center = shimmer.brightness_boost_at(0.5, 0.5);
        assert!(boost_center > 0.20);

        let boost_far = shimmer.brightness_boost_at(0.9, 0.5);
        assert_eq!(boost_far, 0.0);
    }

    #[test]
    fn empty_state_illustrations_cover_all_six_businesses() {
        assert_eq!(EmptyStateIllustrationKind::ALL.len(), 6);
        for kind in EmptyStateIllustrationKind::ALL {
            assert!(!kind.title_zh().is_empty());
            assert!(!kind.description_zh().is_empty());
            assert!(!kind.primary_action_zh().is_empty());
            assert!(!kind.icon_token().is_empty());
        }
    }
}

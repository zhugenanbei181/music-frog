//! Windows 11 DWM 亚克力/云母扩展与 Snap Layout API 接入点（#187 / FLPARITY-018）。
//!
//! Windows 11（Build 22000 起）在无框 CSD 标题栏上引入了 Snap Layout 布局菜单能力：
//! 当在最大化/还原按钮区域处理 `WM_NCHITTEST` 并返回 `HTMAXBUTTON` 时，DWM 会自动展示
//! 系统级 Snap Layout 悬浮面板并处理窗口吸附。
//!
//! 在扩展接入之前，材质接入点恒返回 `None`；Snap Layout 支持能力按 Windows 构建版本检测。
//! 接入 `windows` crate 之后，用 `DwmSetWindowAttribute` 把窗口的 `DWMWA_SYSTEMBACKDROP_TYPE` 设为
//! `DWMSBT_MICA`（云母）或 `DWMSBT_ACRYLIC`（亚克力），并让客户区透出系统
//! 材质；窗口背景本身仍由表面层绘制，材质只负责背后的系统质感。

use crate::window_chrome::BlurMaterial;
use infiltrator_contract::window_chrome::SnapLayoutSupport;

/// 把窗口背景材质交给 DWM。
///
/// 返回实际生效的材质；`None` 表示该平台扩展尚未接入，窗口保持纯色背景。
pub fn apply_system_backdrop(_material: BlurMaterial) -> Option<BlurMaterial> {
    None
}

/// 纯函数：根据 Windows 构建版本判定是否支持 Windows 11 Snap Layout API。
///
/// Windows 11 起始 Build 号为 22000。低于 22000 的 Windows 10/8/7 不支持该 API。
pub fn snap_layout_support_for_build(build_num: u32) -> SnapLayoutSupport {
    if build_num >= 22000 {
        SnapLayoutSupport::Supported
    } else {
        SnapLayoutSupport::unsupported("windows-build-below-22000")
    }
}

/// 检测当前宿主对 Windows 11 Snap Layout API 的支持能力。
pub fn detect_snap_layout_support() -> SnapLayoutSupport {
    #[cfg(target_os = "windows")]
    {
        read_windows_build_number_and_detect()
    }
    #[cfg(not(target_os = "windows"))]
    {
        SnapLayoutSupport::unsupported("non-windows-platform")
    }
}

#[cfg(target_os = "windows")]
fn read_windows_build_number_and_detect() -> SnapLayoutSupport {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(key) = hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
        if let Ok(build_str) = key.get_value::<String, _>("CurrentBuildNumber") {
            if let Ok(build_num) = build_str.parse::<u32>() {
                return snap_layout_support_for_build(build_num);
            }
        }
    }
    SnapLayoutSupport::unsupported("failed-to-query-windows-build-number")
}

/// 自定义标题栏上最大化按钮的几何范围（用于 WM_NCHITTEST 命中测试返回 HTMAXBUTTON）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SnapLayoutButtonGeometry {
    /// 控件相对标题栏右边界的偏移（包含自身宽度）。
    pub x_offset_from_right_px: u16,
    /// 控件宽度（逻辑像素）。
    pub width_px: u16,
    /// 控件高度（逻辑像素）。
    pub height_px: u16,
}

impl SnapLayoutButtonGeometry {
    /// 标准 Windows 11 标题栏三键中最大化按钮的几何范围。
    ///
    /// 在 138px 的控制区中，右起依次是关闭按钮（46px）、最大化按钮（46px）、最小化按钮（46px）。
    /// 因此最大化按钮相对右边界的跨度是 `[46, 92]`。
    pub const STANDARD_WIN11: Self = Self {
        x_offset_from_right_px: 92,
        width_px: 46,
        height_px: 38,
    };

    /// 测试给定逻辑坐标 `(x, y)` 是否落在最大化按钮区域内。
    pub fn contains(&self, title_bar_width: u16, x: u16, y: u16) -> bool {
        let btn_left = title_bar_width.saturating_sub(self.x_offset_from_right_px);
        let btn_right = btn_left.saturating_add(self.width_px);
        x >= btn_left && x < btn_right && y < self.height_px
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_layout_support_requires_win11_build_22000() {
        assert!(snap_layout_support_for_build(22000).is_supported());
        assert!(snap_layout_support_for_build(22621).is_supported());
        assert!(snap_layout_support_for_build(26100).is_supported());

        let win10 = snap_layout_support_for_build(19045);
        assert!(!win10.is_supported());
        assert_eq!(
            win10.unsupported_reason(),
            Some("windows-build-below-22000")
        );
    }

    #[test]
    fn snap_layout_button_geometry_hit_testing() {
        let geo = SnapLayoutButtonGeometry::STANDARD_WIN11;
        let title_bar_width = 1200;
        assert!(geo.contains(title_bar_width, 1108, 10));
        assert!(geo.contains(title_bar_width, 1130, 20));
        assert!(geo.contains(title_bar_width, 1153, 37));

        assert!(!geo.contains(title_bar_width, 1107, 10));
        assert!(!geo.contains(title_bar_width, 1154, 10));
        assert!(!geo.contains(title_bar_width, 1130, 38));
        assert!(!geo.contains(title_bar_width, 1130, 50));
    }

    #[test]
    fn non_windows_platform_reports_unsupported_snap_layout() {
        #[cfg(not(target_os = "windows"))]
        {
            let support = detect_snap_layout_support();
            assert!(!support.is_supported());
            assert_eq!(support.unsupported_reason(), Some("non-windows-platform"));
        }
    }
}

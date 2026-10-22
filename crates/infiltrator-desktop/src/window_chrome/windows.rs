//! Windows 11 DWM 亚克力/云母扩展接入点（#187）。
//!
//! 该平台扩展**尚未接入**：接入 `windows` crate 之后，这里用
//! `DwmSetWindowAttribute` 把窗口的 `DWMWA_SYSTEMBACKDROP_TYPE` 设为
//! `DWMSBT_MICA`（云母）或 `DWMSBT_ACRYLIC`（亚克力），并让客户区透出系统
//! 材质；窗口背景本身仍由表面层绘制，材质只负责背后的系统质感。
//!
//! 在扩展接入之前本模块恒返回 `None`：调用方据此让窗口保持纯色背景，而不是
//! 假装已经有系统材质。

use crate::window_chrome::BlurMaterial;

/// 把窗口背景材质交给 DWM。
///
/// 返回实际生效的材质；`None` 表示该平台扩展尚未接入，窗口保持纯色背景。
pub fn apply_system_backdrop(_material: BlurMaterial) -> Option<BlurMaterial> {
    // 该平台扩展尚未接入：接入 `windows` crate 后在此调用
    // `DwmSetWindowAttribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, ...)`。
    None
}

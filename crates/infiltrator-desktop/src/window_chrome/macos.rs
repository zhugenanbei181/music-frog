//! macOS `NSVisualEffectView` 混成材质接入点（#188）。
//!
//! 该平台扩展**尚未接入**：接入 macOS 的 objc 绑定之后，这里在窗口内容视图
//! 背后插入 `NSVisualEffectView`，并按材质选择 `NSVisualEffectMaterial`
//! （`hudWindow` / `sidebar` / `menu` 等）与其 `blendingMode` /
//! `state`（`active` / `inactive`），让窗口透出系统混成质感。
//!
//! 在扩展接入之前本模块恒返回 `None`：调用方据此让窗口保持纯色背景，而不是
//! 假装已经有系统材质。

use crate::window_chrome::BlurMaterial;

/// 把混成材质挂到窗口内容视图背后。
///
/// 返回实际生效的材质；`None` 表示该平台扩展尚未接入，窗口保持纯色背景。
pub fn attach_visual_effect(_material: BlurMaterial) -> Option<BlurMaterial> {
    // 该平台扩展尚未接入：接入 objc 绑定后在此创建 `NSVisualEffectView` 并
    // 设置 `NSVisualEffectMaterial` / `blendingMode` / `state`。
    None
}

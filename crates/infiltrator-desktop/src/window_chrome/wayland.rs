//! Wayland 合成器窗口模糊扩展接入点（#189）。
//!
//! Niri 与 KWin 的窗口模糊需要客户端协议（`xdg_toplevel` 的模糊扩展、
//! `org.kde.kwin.blur`），本仓库尚未引入 Wayland 客户端库，因此应用侧暂未
//! 接入；能力探测只依据会话环境变量（见 [`crate::window_chrome`] 的纯函数
//! `capabilities_for_session`），能指名 Niri / KWin 时才确认模糊能力。
//!
//! 在扩展接入之前本模块恒返回 `None`：调用方据此让窗口保持纯色背景，而不是
//! 假装已经有系统材质。

use crate::window_chrome::BlurMaterial;

/// 把窗口模糊扩展挂到 toplevel 上。
///
/// 返回实际生效的材质；`None` 表示该平台扩展尚未接入，窗口保持纯色背景。
pub fn attach_blur_extension(_material: BlurMaterial) -> Option<BlurMaterial> {
    // 该平台扩展尚未接入：引入 Wayland 客户端库后在此协商模糊协议并把
    // toplevel 的模糊区域登记给合成器。
    None
}

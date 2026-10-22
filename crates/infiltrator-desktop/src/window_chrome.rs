//! 桌面窗口装饰与材质的唯一真源（EFFECTS-046 / #186~#189）。
//!
//! 桌面外壳统一运行「现代无边框一体化标题栏」：操作系统不再绘制标题栏，
//! 窗口自己绘制标题栏与最小化/最大化/关闭控件。至于窗口背后要不要透出系统
//! 材质（Windows 11 云母/亚克力、macOS 混成、Wayland 合成器模糊），取决于宿主
//! 平台与该平台**真实具备**的能力。
//!
//! 本模块是「平台能力 → 窗口装饰与材质」的唯一真源：
//!
//! * [`HostPlatform`]：宿主平台；
//! * [`PlatformCapabilities`]：两个**独立二值事实**（能否渲染窗口背后模糊、
//!   能否使用带 alpha 的窗口表面），它们不是流程阶段，因此用 `bool` 表达；
//! * [`plan_window_chrome`]：**纯函数**，同一组（平台、能力）永远得到同一份
//!   [`WindowChromePlan`]，全部降级分支都能被单测覆盖；
//! * 各表面只消费 [`WindowChromePlan`]，不得自行派生装饰或材质，也不得在
//!   表面层再实现一份平台判断。
//!
//! 装饰形态的词汇（系统标题栏 / 无边框）由契约层
//! `infiltrator_contract::window_chrome::WindowChromeStyle` 唯一持有，桌面层
//! 不复制第二份；本模块只负责「选哪一种」，并把它与材质、标题栏高度打包成
//! 一份各表面可直接消费的 typed 结果。
//!
//! 平台扩展现状：Windows 的 DWM 亚克力/云母、macOS 的 `NSVisualEffectView`
//! 与 Wayland 合成器模糊扩展都尚未接入（见 `windows` / `macos` / `wayland`
//! 子模块），因此 [`apply_window_material`] 目前恒为 `None`：窗口诚实地保持
//! 纯色背景，而不是假装已经有系统材质。

use infiltrator_contract::window_chrome::{
    CHROME_DRAG_STRIP_HEIGHT_PX, NativeShadow, WindowChrome, WindowChromeStyle,
};
use std::env;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "windows")]
mod windows;

/// 无边框一体化时，窗口自绘标题栏预留的高度（逻辑像素）。
///
/// 数值取自契约层共享的拖拽条高度，避免同一事实出现第二份。
pub const TITLE_BAR_HEIGHT_PX: u16 = CHROME_DRAG_STRIP_HEIGHT_PX as u16;

/// 窗口背景材质。
///
/// 材质是**平台原生**的窗口质感：云母/亚克力来自 Windows 的 DWM，混成来自
/// macOS 的 `NSVisualEffectView`，Wayland 模糊来自合成器扩展。没有材质就是
/// 纯色背景，不存在「假装有材质」的中间态。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlurMaterial {
    /// 不使用材质：窗口保持纯色背景。
    None,
    /// Windows 11 云母（Mica）：带磨砂层次的系统材质。
    WindowsMica,
    /// Windows 11 亚克力（Acrylic）：带透光感的半透明材质。
    WindowsAcrylic,
    /// macOS 混成（`NSVisualEffectView`）。
    MacVibrancy,
    /// Wayland 合成器窗口模糊扩展（Niri / KWin）。
    WaylandBlur,
}

impl BlurMaterial {
    /// 该材质是否要求窗口表面带 alpha 通道。
    ///
    /// 亚克力与 Wayland 合成器模糊都要靠带 alpha 的客户区透出系统材质；云母与
    /// macOS 混成由系统材质自身提供混成效果，不需要整窗透明。
    pub const fn needs_transparent_surface(self) -> bool {
        match self {
            Self::None => false,
            Self::WindowsMica => false,
            Self::WindowsAcrylic => true,
            Self::MacVibrancy => false,
            Self::WaylandBlur => true,
        }
    }
}

/// 宿主平台。
///
/// `LinuxWayland` 表示跑在 Wayland 合成器上（Wayland 才是现代桌面质感的目标
/// 会话），`LinuxX11` 表示经典 X11 会话。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostPlatform {
    /// Windows（含 Windows 10/11）。
    Windows,
    /// macOS。
    MacOs,
    /// Linux 上的 Wayland 合成器会话。
    LinuxWayland,
    /// Linux 上的 X11 会话。
    LinuxX11,
    /// 其它平台（未列出的目标）。
    Other,
}

/// 宿主平台的两项独立能力。
///
/// 这两个字段是两个**独立二值事实**（能力在不在），不是流程阶段：窗口处于
/// 哪个阶段由 [`WindowChromePlan`] 的 typed 字段表达，因此这里用 `bool` 是
/// 合规的，而不是用标志位藏状态机。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlatformCapabilities {
    /// 宿主能否在窗口背后渲染系统模糊（云母/亚克力/混成/合成器模糊）。
    pub blur: bool,
    /// 宿主是否接受带 alpha 通道的窗口表面。
    pub transparent_background: bool,
}

/// 显示会话的环境信号快照。
///
/// 平台探测只读这几个信号，因此判定过程可以完全离线复现：单测用参数化输入
/// 驱动 [`platform_for_session`] 与 [`capabilities_for_session`]，不依赖运行
/// 测试机器的真实环境。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DisplaySessionEnv {
    /// `WAYLAND_DISPLAY`：Wayland 会话套接字名，存在即说明跑在 Wayland 上。
    pub wayland_display: Option<String>,
    /// `DISPLAY`：X11 会话显示名。
    pub x11_display: Option<String>,
    /// `NIRI_SOCKET`：Niri 合成器控制套接字，存在即说明是 Niri。
    pub niri_socket: Option<String>,
    /// `XDG_CURRENT_DESKTOP`：当前桌面环境（如 `KDE`）。
    pub current_desktop: Option<String>,
}

impl DisplaySessionEnv {
    /// 从当前进程环境读取显示会话信号；缺失的信号记为 `None`。
    pub fn from_process() -> Self {
        Self {
            wayland_display: env::var("WAYLAND_DISPLAY").ok(),
            x11_display: env::var("DISPLAY").ok(),
            niri_socket: env::var("NIRI_SOCKET").ok(),
            current_desktop: env::var("XDG_CURRENT_DESKTOP").ok(),
        }
    }
}

/// 窗口装饰与材质计划：各表面直接消费的唯一结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowChromePlan {
    /// 窗口装饰形态（系统标题栏 / 无边框一体化）。
    pub chrome: WindowChromeStyle,
    /// 窗口背景材质。
    pub material: BlurMaterial,
    /// 自绘标题栏预留高度（逻辑像素）；系统标题栏由系统绘制，此处为 `0`。
    pub title_bar_height: u16,
}

impl WindowChromePlan {
    /// 窗口是否需要自绘标题栏与窗口控件。
    pub const fn needs_custom_controls(self) -> bool {
        matches!(self.chrome, WindowChromeStyle::Frameless)
    }

    /// 把计划投影成契约层共享的窗口形态。
    ///
    /// 各表面据此设置自己的窗口参数（装饰开关、拖拽条高度），不需要也不允许
    /// 各自再判断一次平台。
    pub fn contract_chrome(self) -> WindowChrome {
        match self.chrome {
            WindowChromeStyle::Frameless => WindowChrome {
                style: WindowChromeStyle::Frameless,
                drag_strip_height_px: u32::from(self.title_bar_height),
                double_click_maximizes: true,
                native_shadow: NativeShadow::Absent,
            },
            WindowChromeStyle::SystemDecorated => WindowChrome {
                style: WindowChromeStyle::SystemDecorated,
                drag_strip_height_px: 0,
                double_click_maximizes: true,
                native_shadow: NativeShadow::PlatformDefault,
            },
        }
    }
}

/// 纯函数：按宿主平台与平台能力选出最终的窗口装饰与材质。
///
/// 规则（每条降级路径都落在 typed 结果里，没有隐式分支）：
///
/// * Windows：无边框一体化；能力齐备时用亚克力，只有模糊能力时用云母，
///   没有模糊能力则无材质；
/// * macOS：无边框一体化；有模糊能力时用混成，否则无材质；
/// * Wayland：无边框一体化；模糊能力与 alpha 表面同时具备时用合成器模糊，
///   否则无材质；
/// * X11：标准标题栏、无材质（X11 没有可靠的窗口模糊协议）；
/// * 其它平台：标准标题栏、无材质。
///
/// 同一组输入永远得到同一份结果：这是纯函数，不读环境、不读时钟。
pub fn plan_window_chrome(host: HostPlatform, caps: PlatformCapabilities) -> WindowChromePlan {
    let (chrome, material) = match host {
        HostPlatform::Windows => (WindowChromeStyle::Frameless, windows_material(caps)),
        HostPlatform::MacOs => (WindowChromeStyle::Frameless, macos_material(caps)),
        HostPlatform::LinuxWayland => (WindowChromeStyle::Frameless, wayland_material(caps)),
        HostPlatform::LinuxX11 => (WindowChromeStyle::SystemDecorated, BlurMaterial::None),
        HostPlatform::Other => (WindowChromeStyle::SystemDecorated, BlurMaterial::None),
    };
    WindowChromePlan {
        chrome,
        material,
        title_bar_height: match chrome {
            WindowChromeStyle::Frameless => TITLE_BAR_HEIGHT_PX,
            WindowChromeStyle::SystemDecorated => 0,
        },
    }
}

/// Windows 的材质选择：亚克力优先，其次云母。
fn windows_material(caps: PlatformCapabilities) -> BlurMaterial {
    match (caps.blur, caps.transparent_background) {
        (true, true) => BlurMaterial::WindowsAcrylic,
        (true, false) => BlurMaterial::WindowsMica,
        (false, _) => BlurMaterial::None,
    }
}

/// macOS 的材质选择：混成。
fn macos_material(caps: PlatformCapabilities) -> BlurMaterial {
    match caps.blur {
        true => BlurMaterial::MacVibrancy,
        false => BlurMaterial::None,
    }
}

/// Wayland 的材质选择：合成器模糊需要模糊能力与带 alpha 的表面同时成立。
fn wayland_material(caps: PlatformCapabilities) -> BlurMaterial {
    match (caps.blur, caps.transparent_background) {
        (true, true) => BlurMaterial::WaylandBlur,
        _ => BlurMaterial::None,
    }
}

/// 探测当前宿主平台。
///
/// Windows / macOS 由编译目标决定；Linux 由显示会话信号决定：有 Wayland 会话
/// 就是 Wayland，没有 Wayland 会话时按 X11 保守降级（标准标题栏、无材质）。
pub fn detect_platform() -> HostPlatform {
    platform_for_target()
}

/// 编译目标决定的平台（Linux 上再按显示会话细分）。
#[cfg(target_os = "windows")]
fn platform_for_target() -> HostPlatform {
    HostPlatform::Windows
}

/// 编译目标决定的平台（Linux 上再按显示会话细分）。
#[cfg(target_os = "macos")]
fn platform_for_target() -> HostPlatform {
    HostPlatform::MacOs
}

/// 编译目标决定的平台（Linux 上再按显示会话细分）。
#[cfg(target_os = "linux")]
fn platform_for_target() -> HostPlatform {
    platform_for_session(&DisplaySessionEnv::from_process())
}

/// 编译目标决定的平台（未列出的目标）。
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn platform_for_target() -> HostPlatform {
    HostPlatform::Other
}

/// 纯函数：按显示会话信号判定平台归属（Linux 语义）。
///
/// 有 Wayland 会话就是 Wayland 合成器会话；否则按 X11 保守降级。
pub fn platform_for_session(session: &DisplaySessionEnv) -> HostPlatform {
    match session.wayland_display.is_some() {
        true => HostPlatform::LinuxWayland,
        false => HostPlatform::LinuxX11,
    }
}

/// 纯函数：按显示会话信号判定平台能力。
///
/// Wayland 会话普遍接受带 alpha 的窗口表面；窗口模糊扩展只有能指名道姓的
/// 合成器才算确认支持（见 [`wayland_blur_confirmed`]）。X11 会话没有可靠的
/// 窗口模糊协议，模糊一律按不支持处理。
pub fn capabilities_for_session(session: &DisplaySessionEnv) -> PlatformCapabilities {
    match session.wayland_display.is_some() {
        true => PlatformCapabilities {
            blur: wayland_blur_confirmed(session),
            transparent_background: true,
        },
        false => PlatformCapabilities {
            blur: false,
            transparent_background: session.x11_display.is_some(),
        },
    }
}

/// 纯函数：判定 Wayland 合成器是否**可确认**支持窗口模糊扩展。
///
/// 只有能指名道姓的合成器才算确认支持：Niri（`NIRI_SOCKET`）与 KWin
/// （`XDG_CURRENT_DESKTOP` 含 `KDE` / `plasma`）。其余 Wayland 合成器的模糊
/// 扩展无法从环境变量确认，一律按不支持处理，绝不假装支持。
fn wayland_blur_confirmed(session: &DisplaySessionEnv) -> bool {
    if session.niri_socket.is_some() {
        return true;
    }
    let desktop = session.current_desktop.as_deref().unwrap_or_default();
    desktop.split(':').any(is_kde_desktop)
}

/// KDE / KWin 的桌面标识（`XDG_CURRENT_DESKTOP` 可能写成 `KDE:Plasma`）。
fn is_kde_desktop(entry: &str) -> bool {
    entry.eq_ignore_ascii_case("kde") || entry.eq_ignore_ascii_case("plasma")
}

/// 探测宿主平台的真实能力。
///
/// Windows / macOS 的原生能力探测随平台扩展一起接入（`windows` crate 的 DWM
/// 调用、macOS 的 objc 绑定）；在接入之前一律报告「不支持」，绝不假装支持。
/// Linux 侧按显示会话信号判定：能指名 Wayland 合成器时确认模糊扩展，其余按
/// 不支持处理。
pub fn detect_capabilities(host: HostPlatform) -> PlatformCapabilities {
    match host {
        HostPlatform::Windows => PlatformCapabilities::default(),
        HostPlatform::MacOs => PlatformCapabilities::default(),
        HostPlatform::LinuxWayland | HostPlatform::LinuxX11 => {
            capabilities_for_session(&DisplaySessionEnv::from_process())
        }
        HostPlatform::Other => PlatformCapabilities::default(),
    }
}

/// 把计划选中的材质交给平台扩展。
///
/// 返回实际生效的材质；`None` 表示该平台扩展尚未接入（或该平台不支持该
/// 材质），窗口会保持纯色背景。
pub fn apply_window_material(material: BlurMaterial) -> Option<BlurMaterial> {
    if material == BlurMaterial::None {
        return None;
    }
    apply_platform_material(material)
}

/// Windows 上的材质接入点。
#[cfg(target_os = "windows")]
fn apply_platform_material(material: BlurMaterial) -> Option<BlurMaterial> {
    windows::apply_system_backdrop(material)
}

/// macOS 上的材质接入点。
#[cfg(target_os = "macos")]
fn apply_platform_material(material: BlurMaterial) -> Option<BlurMaterial> {
    macos::attach_visual_effect(material)
}

/// Linux 上的材质接入点。
#[cfg(target_os = "linux")]
fn apply_platform_material(material: BlurMaterial) -> Option<BlurMaterial> {
    wayland::attach_blur_extension(material)
}

/// 未列出的目标：没有材质接入点。
#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply_platform_material(_material: BlurMaterial) -> Option<BlurMaterial> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities(blur: bool, transparent_background: bool) -> PlatformCapabilities {
        PlatformCapabilities {
            blur,
            transparent_background,
        }
    }

    fn session(
        wayland_display: Option<&str>,
        x11_display: Option<&str>,
        niri_socket: Option<&str>,
        current_desktop: Option<&str>,
    ) -> DisplaySessionEnv {
        DisplaySessionEnv {
            wayland_display: wayland_display.map(str::to_owned),
            x11_display: x11_display.map(str::to_owned),
            niri_socket: niri_socket.map(str::to_owned),
            current_desktop: current_desktop.map(str::to_owned),
        }
    }

    #[test]
    fn windows_prefers_acrylic_and_falls_back_to_mica() {
        let acrylic = plan_window_chrome(HostPlatform::Windows, capabilities(true, true));
        assert_eq!(acrylic.material, BlurMaterial::WindowsAcrylic);
        assert_eq!(acrylic.chrome, WindowChromeStyle::Frameless);

        let mica = plan_window_chrome(HostPlatform::Windows, capabilities(true, false));
        assert_eq!(mica.material, BlurMaterial::WindowsMica);
        assert_eq!(mica.chrome, WindowChromeStyle::Frameless);

        let plain = plan_window_chrome(HostPlatform::Windows, capabilities(false, false));
        assert_eq!(plain.material, BlurMaterial::None);
        assert_eq!(plain.chrome, WindowChromeStyle::Frameless);
    }

    #[test]
    fn macos_prefers_vibrancy_when_blur_is_available() {
        let vibrancy = plan_window_chrome(HostPlatform::MacOs, capabilities(true, true));
        assert_eq!(vibrancy.material, BlurMaterial::MacVibrancy);
        assert_eq!(vibrancy.chrome, WindowChromeStyle::Frameless);

        let plain = plan_window_chrome(HostPlatform::MacOs, capabilities(false, false));
        assert_eq!(plain.material, BlurMaterial::None);
    }

    #[test]
    fn wayland_without_capability_degrades_to_no_material() {
        let degraded = plan_window_chrome(HostPlatform::LinuxWayland, capabilities(false, false));
        assert_eq!(degraded.material, BlurMaterial::None);
        assert_eq!(degraded.chrome, WindowChromeStyle::Frameless);
        assert_eq!(degraded.title_bar_height, TITLE_BAR_HEIGHT_PX);
    }

    #[test]
    fn wayland_with_blur_and_alpha_selects_the_compositor_blur() {
        let plan = plan_window_chrome(HostPlatform::LinuxWayland, capabilities(true, true));
        assert_eq!(plan.material, BlurMaterial::WaylandBlur);
        assert_eq!(plan.chrome, WindowChromeStyle::Frameless);

        // 没有 alpha 表面时合成器模糊无法成立，诚实地退回无材质。
        let host = HostPlatform::LinuxWayland;
        let without_alpha = plan_window_chrome(host, capabilities(true, false));
        assert_eq!(without_alpha.material, BlurMaterial::None);
    }

    #[test]
    fn x11_keeps_the_system_title_bar_and_drops_the_material() {
        let plan = plan_window_chrome(HostPlatform::LinuxX11, capabilities(true, true));
        assert_eq!(plan.chrome, WindowChromeStyle::SystemDecorated);
        assert_eq!(plan.material, BlurMaterial::None);
        assert_eq!(plan.title_bar_height, 0);
        assert!(!plan.needs_custom_controls());
    }

    #[test]
    fn an_unknown_platform_keeps_the_system_title_bar() {
        let plan = plan_window_chrome(HostPlatform::Other, capabilities(true, true));
        assert_eq!(plan.chrome, WindowChromeStyle::SystemDecorated);
        assert_eq!(plan.material, BlurMaterial::None);
        assert_eq!(plan.title_bar_height, 0);
    }

    #[test]
    fn the_frameless_plan_reserves_the_shared_title_bar_height() {
        let plan = plan_window_chrome(HostPlatform::Windows, capabilities(false, false));
        assert!(plan.needs_custom_controls());
        assert_eq!(plan.title_bar_height, TITLE_BAR_HEIGHT_PX);
        // 标题栏高度必须与契约层共享的拖拽条高度同源。
        let strip_height = u32::from(plan.title_bar_height);
        assert_eq!(strip_height, CHROME_DRAG_STRIP_HEIGHT_PX);
    }

    #[test]
    fn the_plan_is_pure_for_the_same_inputs() {
        let first = plan_window_chrome(HostPlatform::LinuxWayland, capabilities(true, true));
        let second = plan_window_chrome(HostPlatform::LinuxWayland, capabilities(true, true));
        assert_eq!(first, second);
    }

    #[test]
    fn the_contract_projection_carries_the_planned_shape() {
        let plan = plan_window_chrome(HostPlatform::Windows, capabilities(true, true));
        let projected = plan.contract_chrome();
        let strip_height = u32::from(plan.title_bar_height);
        assert_eq!(projected.style, plan.chrome);
        assert_eq!(projected.drag_strip_height_px, strip_height);
        assert!(projected.needs_custom_controls());

        let system = plan_window_chrome(HostPlatform::LinuxX11, capabilities(false, false));
        let projected = system.contract_chrome();
        assert_eq!(projected.style, system.chrome);
        assert_eq!(projected.drag_strip_height_px, 0);
        assert!(projected.os_decorations());
    }

    #[test]
    fn a_wayland_session_without_a_named_compositor_claims_no_blur() {
        let env = session(Some("wayland-0"), None, None, Some("sway"));
        assert_eq!(platform_for_session(&env), HostPlatform::LinuxWayland);
        let caps = capabilities_for_session(&env);
        assert!(!caps.blur, "无法确认的合成器不得声称支持模糊");
        assert!(caps.transparent_background);
    }

    #[test]
    fn a_niri_session_reports_blur_and_transparency() {
        let env = session(Some("wayland-0"), None, Some("/run/niri.sock"), None);
        assert_eq!(platform_for_session(&env), HostPlatform::LinuxWayland);
        let caps = capabilities_for_session(&env);
        assert!(caps.blur);
        assert!(caps.transparent_background);
    }

    #[test]
    fn a_kde_session_reports_blur_and_transparency() {
        let env = session(Some("wayland-0"), None, None, Some("KDE:Plasma"));
        assert_eq!(platform_for_session(&env), HostPlatform::LinuxWayland);
        let caps = capabilities_for_session(&env);
        assert!(caps.blur);
        assert!(caps.transparent_background);
    }

    #[test]
    fn an_x11_session_has_no_blur_but_keeps_alpha_surfaces() {
        let env = session(None, Some(":0"), None, Some("GNOME"));
        assert_eq!(platform_for_session(&env), HostPlatform::LinuxX11);
        let caps = capabilities_for_session(&env);
        assert!(!caps.blur);
        assert!(caps.transparent_background);
    }

    #[test]
    fn a_headless_session_reports_no_capability_at_all() {
        let env = session(None, None, None, None);
        assert_eq!(platform_for_session(&env), HostPlatform::LinuxX11);
        let caps = capabilities_for_session(&env);
        assert_eq!(caps, PlatformCapabilities::default());
    }

    #[test]
    fn the_detected_capabilities_match_the_pure_session_verdict() {
        let host = detect_platform();
        let caps = detect_capabilities(host);
        match host {
            HostPlatform::LinuxWayland | HostPlatform::LinuxX11 => {
                assert_eq!(
                    caps,
                    capabilities_for_session(&DisplaySessionEnv::from_process()),
                    "运行期探测必须与纯函数判定同源"
                );
            }
            HostPlatform::Windows | HostPlatform::MacOs | HostPlatform::Other => {
                assert_eq!(
                    caps,
                    PlatformCapabilities::default(),
                    "原生扩展接入之前一律报告不支持，绝不假装支持"
                );
            }
        }
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn the_detected_platform_matches_the_compile_target() {
        assert_eq!(detect_platform(), HostPlatform::Windows);
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn the_detected_platform_matches_the_compile_target() {
        assert_eq!(detect_platform(), HostPlatform::MacOs);
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn the_detected_platform_matches_the_compile_target() {
        assert!(matches!(
            detect_platform(),
            HostPlatform::LinuxWayland | HostPlatform::LinuxX11
        ));
    }

    #[test]
    fn the_material_is_only_reported_when_the_platform_extension_is_wired() {
        for material in [
            BlurMaterial::WindowsMica,
            BlurMaterial::WindowsAcrylic,
            BlurMaterial::MacVibrancy,
            BlurMaterial::WaylandBlur,
        ] {
            let applied = apply_window_material(material);
            assert!(
                applied.is_none() || applied == Some(material),
                "平台扩展未接入时不得谎报材质生效"
            );
        }
        assert_eq!(apply_window_material(BlurMaterial::None), None);
    }

    #[test]
    fn the_material_reports_whether_it_needs_an_alpha_surface() {
        assert!(!BlurMaterial::None.needs_transparent_surface());
        assert!(!BlurMaterial::WindowsMica.needs_transparent_surface());
        assert!(BlurMaterial::WindowsAcrylic.needs_transparent_surface());
        assert!(!BlurMaterial::MacVibrancy.needs_transparent_surface());
        assert!(BlurMaterial::WaylandBlur.needs_transparent_surface());
    }
}

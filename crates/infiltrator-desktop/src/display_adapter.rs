use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct DisplayBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
}

impl DisplayBounds {
    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    pub fn contains_point(&self, px: i32, py: i32) -> bool {
        px >= self.x && px < self.right() && py >= self.y && py < self.bottom()
    }

    pub fn intersection_area(&self, win: &WindowPosition) -> u64 {
        let (ix1, iy1) = (self.x.max(win.x), self.y.max(win.y));
        let (ix2, iy2) = (
            self.right().min(win.right()),
            self.bottom().min(win.bottom()),
        );
        if ix2 > ix1 && iy2 > iy1 {
            (ix2 - ix1) as u64 * (iy2 - iy1) as u64
        } else {
            0
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl WindowPosition {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> i32 {
        self.x.saturating_add(self.width as i32)
    }

    pub fn bottom(&self) -> i32 {
        self.y.saturating_add(self.height as i32)
    }

    pub fn center(&self) -> (i32, i32) {
        (
            self.x + (self.width as i32 / 2),
            self.y + (self.height as i32 / 2),
        )
    }

    pub fn area(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PersistedWindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_maximized: bool,
    pub screen_index: Option<usize>,
}

impl Default for PersistedWindowGeometry {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            width: 800,
            height: 600,
            is_maximized: false,
            screen_index: None,
        }
    }
}

impl PersistedWindowGeometry {
    pub fn to_position(&self) -> WindowPosition {
        WindowPosition::new(self.x, self.y, self.width, self.height)
    }

    pub fn from_position(
        pos: &WindowPosition,
        is_maximized: bool,
        screen_index: Option<usize>,
    ) -> Self {
        Self {
            x: pos.x,
            y: pos.y,
            width: pos.width,
            height: pos.height,
            is_maximized,
            screen_index,
        }
    }

    pub fn validate_and_restore(
        &self,
        displays: &[DisplayBounds],
        min_size: (u32, u32),
        default_size: (u32, u32),
    ) -> WindowPosition {
        if displays.is_empty() {
            return WindowPosition::new(
                self.x,
                self.y,
                self.width.max(min_size.0),
                self.height.max(min_size.1),
            );
        }

        let target = self
            .screen_index
            .and_then(|i| displays.get(i))
            .or_else(|| {
                let p = self.to_position();
                displays
                    .iter()
                    .max_by_key(|d| d.intersection_area(&p))
                    .filter(|d| d.intersection_area(&p) > 0)
            })
            .unwrap_or(&displays[0]);

        let w = if self.width < min_size.0 || self.width > target.width * 2 {
            default_size.0
        } else {
            self.width
        };
        let h = if self.height < min_size.1 || self.height > target.height * 2 {
            default_size.1
        } else {
            self.height
        };

        let candidate = WindowPosition::new(self.x, self.y, w, h);
        let vis = target.intersection_area(&candidate);
        let area = candidate.area().max(1);

        if vis == 0 || (vis * 100 / area) < 30 {
            DisplayAdapter::center_window(w, h, target)
        } else {
            DisplayAdapter::constrain_to_screen(candidate, target)
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowMaterial {
    Opaque,
    Mica,
    MicaAlt,
    Acrylic,
    Vibrancy,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Win11BackdropType {
    None,
    Mica,
    MicaAlt,
    Acrylic,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Win11MaterialConfig {
    pub backdrop: Win11BackdropType,
    pub dark_mode: bool,
    pub border_color: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosVibrancyMaterial {
    Titlebar,
    Selection,
    Menu,
    Popover,
    Sidebar,
    HeaderView,
    Sheet,
    WindowBackground,
    HudWindow,
    FullScreenUI,
    ToolTip,
    ContentBackground,
    UnderWindowBackground,
    UnderPageBackground,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosBlendingMode {
    BehindWindow,
    WithinWindow,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacosAppearance {
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MacosMaterialConfig {
    pub material: MacosVibrancyMaterial,
    pub blending_mode: MacosBlendingMode,
    pub appearance: MacosAppearance,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct DesktopMaterialContract {
    pub target_material: WindowMaterial,
    pub win11_config: Option<Win11MaterialConfig>,
    pub macos_config: Option<MacosMaterialConfig>,
    pub fallback_material: WindowMaterial,
}

impl DesktopMaterialContract {
    pub fn new(target_material: WindowMaterial) -> Self {
        Self {
            target_material,
            win11_config: None,
            macos_config: None,
            fallback_material: WindowMaterial::Opaque,
        }
    }

    pub fn resolve_effective_material(&self, os: &str, win_build: Option<u32>) -> WindowMaterial {
        match self.target_material {
            WindowMaterial::Opaque => WindowMaterial::Opaque,
            WindowMaterial::Mica => match (os.eq_ignore_ascii_case("windows"), win_build) {
                (true, Some(b)) if b >= 22000 => WindowMaterial::Mica,
                _ => self.fallback_material,
            },
            WindowMaterial::MicaAlt => match (os.eq_ignore_ascii_case("windows"), win_build) {
                (true, Some(b)) if b >= 22621 => WindowMaterial::MicaAlt,
                (true, Some(b)) if b >= 22000 => WindowMaterial::Mica,
                _ => self.fallback_material,
            },
            WindowMaterial::Acrylic => match (os.eq_ignore_ascii_case("windows"), win_build) {
                (true, Some(b)) if b >= 17134 => WindowMaterial::Acrylic,
                _ => self.fallback_material,
            },
            WindowMaterial::Vibrancy => {
                if os.eq_ignore_ascii_case("macos") || os.eq_ignore_ascii_case("darwin") {
                    WindowMaterial::Vibrancy
                } else {
                    self.fallback_material
                }
            }
        }
    }

    pub fn supports_transparency(&self) -> bool {
        self.target_material != WindowMaterial::Opaque
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAnchor {
    Top,
    Bottom,
    Left,
    Right,
    Auto,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapResult {
    pub position: WindowPosition,
    pub snapped_left: bool,
    pub snapped_right: bool,
    pub snapped_top: bool,
    pub snapped_bottom: bool,
}

impl SnapResult {
    pub fn has_snapped(&self) -> bool {
        self.snapped_left || self.snapped_right || self.snapped_top || self.snapped_bottom
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct MiniWidgetConfig {
    pub x: i32,
    pub y: i32,
    pub is_pinned: bool,
    pub is_visible: bool,
}

impl Default for MiniWidgetConfig {
    fn default() -> Self {
        Self {
            x: 100,
            y: 100,
            is_pinned: false,
            is_visible: false,
        }
    }
}

pub struct DisplayAdapter;

impl DisplayAdapter {
    pub fn scale_dimension(base: u32, scale_factor: f64) -> u32 {
        if scale_factor <= 0.0 {
            0
        } else {
            (base as f64 * scale_factor).round() as u32
        }
    }

    pub fn logical_to_physical(logical: u32, scale: f64) -> u32 {
        Self::scale_dimension(logical, scale)
    }

    pub fn physical_to_logical(physical: u32, scale: f64) -> u32 {
        if scale <= 0.0 {
            0
        } else {
            ((physical as f64) / scale).round() as u32
        }
    }

    pub fn center_window(window_w: u32, window_h: u32, display: &DisplayBounds) -> WindowPosition {
        WindowPosition::new(
            display.x + (display.width as i32 - window_w as i32) / 2,
            display.y + (display.height as i32 - window_h as i32) / 2,
            window_w,
            window_h,
        )
    }

    pub fn constrain_to_screen(window: WindowPosition, display: &DisplayBounds) -> WindowPosition {
        let max_x = display.right() - window.width as i32;
        let x = if window.x < display.x {
            display.x
        } else if window.x > max_x {
            max_x.max(display.x)
        } else {
            window.x
        };
        let max_y = display.bottom() - window.height as i32;
        let y = if window.y < display.y {
            display.y
        } else if window.y > max_y {
            max_y.max(display.y)
        } else {
            window.y
        };
        WindowPosition::new(x, y, window.width, window.height)
    }

    pub fn tray_popup_position(
        tray_x: i32,
        tray_y: i32,
        window_w: u32,
        window_h: u32,
        display: &DisplayBounds,
    ) -> WindowPosition {
        let raw = WindowPosition::new(
            tray_x - (window_w as i32 / 2),
            tray_y.saturating_sub(window_h as i32),
            window_w,
            window_h,
        );
        Self::constrain_to_screen(raw, display)
    }

    pub fn tray_popup_position_with_anchor(
        tray_x: i32,
        tray_y: i32,
        window_w: u32,
        window_h: u32,
        display: &DisplayBounds,
        anchor: TrayAnchor,
    ) -> WindowPosition {
        let resolved = match anchor {
            TrayAnchor::Auto => {
                let (db, dt) = (
                    (display.bottom() - tray_y).abs(),
                    (tray_y - display.y).abs(),
                );
                let (dl, dr) = ((tray_x - display.x).abs(), (display.right() - tray_x).abs());
                let min_d = db.min(dt).min(dl).min(dr);
                if min_d == db {
                    TrayAnchor::Bottom
                } else if min_d == dt {
                    TrayAnchor::Top
                } else if min_d == dr {
                    TrayAnchor::Right
                } else {
                    TrayAnchor::Left
                }
            }
            other => other,
        };

        let raw = match resolved {
            TrayAnchor::Bottom | TrayAnchor::Auto => WindowPosition::new(
                tray_x - (window_w as i32 / 2),
                tray_y.saturating_sub(window_h as i32),
                window_w,
                window_h,
            ),
            TrayAnchor::Top => {
                WindowPosition::new(tray_x - (window_w as i32 / 2), tray_y, window_w, window_h)
            }
            TrayAnchor::Left => {
                WindowPosition::new(tray_x, tray_y - (window_h as i32 / 2), window_w, window_h)
            }
            TrayAnchor::Right => WindowPosition::new(
                tray_x.saturating_sub(window_w as i32),
                tray_y - (window_h as i32 / 2),
                window_w,
                window_h,
            ),
        };

        Self::constrain_to_screen(raw, display)
    }

    pub fn snap_to_screen_edges(
        window: WindowPosition,
        display: &DisplayBounds,
        threshold: u32,
    ) -> SnapResult {
        let t = threshold as i32;
        let mut x = window.x;
        let mut y = window.y;
        let (mut sl, mut sr, mut st, mut sb) = (false, false, false, false);

        if (window.x - display.x).abs() <= t {
            x = display.x;
            sl = true;
        } else if ((window.right()) - display.right()).abs() <= t {
            x = display.right() - window.width as i32;
            sr = true;
        }

        if (window.y - display.y).abs() <= t {
            y = display.y;
            st = true;
        } else if ((window.bottom()) - display.bottom()).abs() <= t {
            y = display.bottom() - window.height as i32;
            sb = true;
        }

        SnapResult {
            position: WindowPosition::new(x, y, window.width, window.height),
            snapped_left: sl,
            snapped_right: sr,
            snapped_top: st,
            snapped_bottom: sb,
        }
    }

    pub fn find_display_for_point(x: i32, y: i32, displays: &[DisplayBounds]) -> Option<usize> {
        displays.iter().position(|d| d.contains_point(x, y))
    }

    pub fn find_best_display_for_window(
        win: &WindowPosition,
        displays: &[DisplayBounds],
    ) -> Option<usize> {
        displays
            .iter()
            .enumerate()
            .max_by_key(|(_, d)| d.intersection_area(win))
            .filter(|(_, d)| d.intersection_area(win) > 0)
            .map(|(idx, _)| idx)
    }

    pub fn migrate_window_between_displays(
        window: &WindowPosition,
        source: &DisplayBounds,
        target: &DisplayBounds,
    ) -> WindowPosition {
        let rx = (window.x - source.x) as f64 / source.width.max(1) as f64;
        let ry = (window.y - source.y) as f64 / source.height.max(1) as f64;
        let ratio = target.scale_factor / source.scale_factor.max(0.01);
        let nw = (window.width as f64 * ratio).round() as u32;
        let nh = (window.height as f64 * ratio).round() as u32;
        let nx = target.x + (rx * target.width as f64).round() as i32;
        let ny = target.y + (ry * target.height as f64).round() as i32;

        Self::constrain_to_screen(WindowPosition::new(nx, ny, nw, nh), target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fhd() -> DisplayBounds {
        DisplayBounds {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        }
    }

    fn wp(x: i32, y: i32, w: u32, h: u32) -> WindowPosition {
        WindowPosition::new(x, y, w, h)
    }

    #[test]
    fn test_scale_and_dimensions() {
        assert_eq!(DisplayAdapter::scale_dimension(100, 1.0), 100);
        assert_eq!(DisplayAdapter::scale_dimension(100, 1.25), 125);
        assert_eq!(DisplayAdapter::scale_dimension(100, 1.5), 150);
        assert_eq!(DisplayAdapter::scale_dimension(100, 2.0), 200);
        assert_eq!(DisplayAdapter::scale_dimension(33, 1.5), 50);
        assert_eq!(DisplayAdapter::scale_dimension(100, -0.5), 0);
        assert_eq!(DisplayAdapter::logical_to_physical(100, 1.5), 150);
        assert_eq!(DisplayAdapter::physical_to_logical(150, 1.5), 100);
        assert_eq!(DisplayAdapter::physical_to_logical(150, 0.0), 0);
    }

    #[test]
    fn test_centering_and_constraining() {
        let d = fhd();
        assert_eq!(
            DisplayAdapter::center_window(800, 600, &d),
            wp(560, 240, 800, 600)
        );
        let d4 = DisplayBounds {
            x: 1920,
            y: 0,
            width: 3840,
            height: 2160,
            scale_factor: 2.0,
        };
        assert_eq!(
            DisplayAdapter::center_window(1000, 1000, &d4),
            wp(3340, 580, 1000, 1000)
        );

        assert_eq!(
            DisplayAdapter::constrain_to_screen(wp(-100, -50, 800, 600), &d),
            wp(0, 0, 800, 600)
        );
        assert_eq!(
            DisplayAdapter::constrain_to_screen(wp(1800, 1000, 800, 600), &d),
            wp(1120, 480, 800, 600)
        );
        assert_eq!(
            DisplayAdapter::constrain_to_screen(wp(100, 100, 2000, 2000), &d),
            wp(0, 0, 2000, 2000)
        );
    }

    #[test]
    fn test_tray_popup_positioning() {
        let d = fhd();
        let pos = DisplayAdapter::tray_popup_position(1900, 1040, 320, 480, &d);
        assert!(pos.right() <= 1920 && pos.bottom() <= 1080 && pos.y == 560);

        let pt =
            DisplayAdapter::tray_popup_position_with_anchor(500, 10, 300, 400, &d, TrayAnchor::Top);
        assert_eq!(pt, wp(350, 10, 300, 400));
        let pl = DisplayAdapter::tray_popup_position_with_anchor(
            20,
            500,
            300,
            400,
            &d,
            TrayAnchor::Left,
        );
        assert_eq!(pl, wp(20, 300, 300, 400));
        let pr = DisplayAdapter::tray_popup_position_with_anchor(
            1900,
            500,
            300,
            400,
            &d,
            TrayAnchor::Right,
        );
        assert_eq!(pr, wp(1600, 300, 300, 400));
    }

    #[test]
    fn test_window_geometry_persistence() {
        let displays = vec![
            fhd(),
            DisplayBounds {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
                scale_factor: 1.25,
            },
        ];
        let valid = PersistedWindowGeometry {
            x: 200,
            y: 150,
            width: 1000,
            height: 700,
            is_maximized: false,
            screen_index: Some(0),
        };
        let restored = valid.validate_and_restore(&displays, (400, 300), (800, 600));
        assert_eq!(restored, wp(200, 150, 1000, 700));

        let off = PersistedWindowGeometry {
            x: 5000,
            y: 5000,
            width: 800,
            height: 600,
            is_maximized: false,
            screen_index: Some(5),
        };
        assert_eq!(
            off.validate_and_restore(&displays, (400, 300), (800, 600)),
            wp(560, 240, 800, 600)
        );
        assert_eq!(
            valid
                .validate_and_restore(&[], (400, 300), (800, 600))
                .width,
            1000
        );

        let from_pos = PersistedWindowGeometry::from_position(&restored, false, Some(0));
        assert_eq!(from_pos.to_position(), restored);
        let json = serde_json::to_string(&valid).unwrap();
        assert_eq!(
            serde_json::from_str::<PersistedWindowGeometry>(&json).unwrap(),
            valid
        );
    }

    #[test]
    fn test_edge_snapping() {
        let d = fhd();
        let s1 = DisplayAdapter::snap_to_screen_edges(wp(8, 12, 600, 400), &d, 16);
        assert!(
            s1.has_snapped()
                && s1.snapped_left
                && s1.snapped_top
                && s1.position == wp(0, 0, 600, 400)
        );

        let s2 = DisplayAdapter::snap_to_screen_edges(wp(1310, 675, 600, 400), &d, 16);
        assert!(
            s2.has_snapped()
                && s2.snapped_right
                && s2.snapped_bottom
                && s2.position == wp(1320, 680, 600, 400)
        );

        let s3 = DisplayAdapter::snap_to_screen_edges(wp(500, 300, 600, 400), &d, 16);
        assert!(!s3.has_snapped() && s3.position == wp(500, 300, 600, 400));
    }

    #[test]
    fn test_desktop_material_contracts() {
        let cm = DesktopMaterialContract::new(WindowMaterial::Mica);
        assert!(cm.supports_transparency());
        assert_eq!(
            cm.resolve_effective_material("windows", Some(22621)),
            WindowMaterial::Mica
        );
        assert_eq!(
            cm.resolve_effective_material("windows", Some(22000)),
            WindowMaterial::Mica
        );
        assert_eq!(
            cm.resolve_effective_material("windows", Some(19044)),
            WindowMaterial::Opaque
        );
        assert_eq!(
            cm.resolve_effective_material("linux", None),
            WindowMaterial::Opaque
        );

        let cma = DesktopMaterialContract::new(WindowMaterial::MicaAlt);
        assert_eq!(
            cma.resolve_effective_material("windows", Some(22621)),
            WindowMaterial::MicaAlt
        );
        assert_eq!(
            cma.resolve_effective_material("windows", Some(22000)),
            WindowMaterial::Mica
        );

        let ca = DesktopMaterialContract::new(WindowMaterial::Acrylic);
        assert_eq!(
            ca.resolve_effective_material("windows", Some(19044)),
            WindowMaterial::Acrylic
        );
        assert_eq!(
            ca.resolve_effective_material("windows", Some(15063)),
            WindowMaterial::Opaque
        );

        let cv = DesktopMaterialContract::new(WindowMaterial::Vibrancy);
        assert_eq!(
            cv.resolve_effective_material("macos", None),
            WindowMaterial::Vibrancy
        );
        assert_eq!(
            cv.resolve_effective_material("windows", Some(22621)),
            WindowMaterial::Opaque
        );
    }

    #[test]
    fn test_display_helpers_and_migration() {
        let (d1, d2) = (
            fhd(),
            DisplayBounds {
                x: 1920,
                y: 0,
                width: 2560,
                height: 1440,
                scale_factor: 1.25,
            },
        );
        let list = vec![d1.clone(), d2.clone()];
        assert_eq!(
            DisplayAdapter::find_display_for_point(500, 500, &list),
            Some(0)
        );
        assert_eq!(
            DisplayAdapter::find_display_for_point(2000, 500, &list),
            Some(1)
        );
        assert_eq!(
            DisplayAdapter::find_display_for_point(-500, 500, &list),
            None
        );

        let win = wp(100, 100, 800, 600);
        assert_eq!(
            DisplayAdapter::find_best_display_for_window(&win, &list),
            Some(0)
        );
        let mig = DisplayAdapter::migrate_window_between_displays(&win, &d1, &d2);
        assert!(
            mig.x >= d2.x && mig.right() <= d2.right() && mig.width == 1000 && mig.height == 750
        );

        let c = MiniWidgetConfig::default();
        assert_eq!(
            c,
            MiniWidgetConfig {
                x: 100,
                y: 100,
                is_pinned: false,
                is_visible: false
            }
        );
    }
}

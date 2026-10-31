//! Desktop adapter for host appearance and dynamic accent observations.
//!
//! Linux GNOME queries `org.gnome.desktop.interface` (color-scheme and
//! accent-color) via `gsettings`; Windows queries DWM and Personalize in the
//! registry; unknown platforms honestly report `Unobserved` through the port.

use infiltrator_contract::theme::{AppearanceMode, SystemAppearance};
use infiltrator_contract::theme_palette::{
    DynamicColorSeed, SystemDynamicColor, SystemThemeObservation,
};
use infiltrator_ports::system_theme::SystemThemeObservationPort;
use std::process::Command;
use std::sync::Arc;

/// Dynamic accent seed extractor for desktop environments.
pub trait SystemThemeRunner: Send + Sync {
    /// Read the host's appearance mode if exposed.
    fn read_appearance_mode(&self) -> Option<AppearanceMode>;
    /// Read the host's primary accent seed as sRGB if exposed.
    fn read_accent_seed(&self) -> Option<DynamicColorSeed>;
}

/// Production desktop system theme adapter.
pub struct DesktopSystemTheme {
    runner: Arc<dyn SystemThemeRunner>,
}

impl Default for DesktopSystemTheme {
    fn default() -> Self {
        Self::new(Arc::new(NativeDesktopThemeRunner))
    }
}

impl DesktopSystemTheme {
    pub fn new(runner: Arc<dyn SystemThemeRunner>) -> Self {
        Self { runner }
    }
}

impl SystemThemeObservationPort for DesktopSystemTheme {
    fn observe(&self) -> SystemThemeObservation {
        let appearance = self
            .runner
            .read_appearance_mode()
            .map(SystemAppearance::Observed)
            .unwrap_or(SystemAppearance::Unobserved);
        let dynamic_color = self
            .runner
            .read_accent_seed()
            .map(SystemDynamicColor::Seed)
            .unwrap_or(SystemDynamicColor::Unobserved);
        SystemThemeObservation {
            appearance,
            dynamic_color,
        }
    }
}

/// Host runner reading the live desktop platform.
pub struct NativeDesktopThemeRunner;

impl SystemThemeRunner for NativeDesktopThemeRunner {
    fn read_appearance_mode(&self) -> Option<AppearanceMode> {
        #[cfg(target_os = "linux")]
        {
            read_gnome_appearance_mode()
        }
        #[cfg(target_os = "windows")]
        {
            read_windows_appearance_mode()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }

    fn read_accent_seed(&self) -> Option<DynamicColorSeed> {
        #[cfg(target_os = "linux")]
        {
            read_gnome_accent_seed()
        }
        #[cfg(target_os = "windows")]
        {
            read_windows_accent_seed()
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }
}

// ---- Linux GNOME ---------------------------------------------------------

#[cfg(target_os = "linux")]
fn read_gnome_appearance_mode() -> Option<AppearanceMode> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "color-scheme"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_gnome_color_scheme(&text)
}

#[cfg(target_os = "linux")]
fn read_gnome_accent_seed() -> Option<DynamicColorSeed> {
    let output = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "accent-color"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_gnome_accent_color(&text)
}

/// Parse GNOME's `org.gnome.desktop.interface color-scheme` value.
pub fn parse_gnome_color_scheme(raw: &str) -> Option<AppearanceMode> {
    let cleaned = raw.trim().trim_matches('\'').trim_matches('"');
    match cleaned {
        "prefer-dark" => Some(AppearanceMode::Dark),
        "prefer-light" | "default" => Some(AppearanceMode::Light),
        _ => None,
    }
}

/// Parse GNOME 47+ / libadwaita named accent colors into their canonical sRGB seed.
pub fn parse_gnome_accent_color(raw: &str) -> Option<DynamicColorSeed> {
    let cleaned = raw.trim().trim_matches('\'').trim_matches('"');
    let (r, g, b) = match cleaned {
        "blue" => (0x35, 0x84, 0xe4),
        "teal" => (0x21, 0x90, 0xa4),
        "green" => (0x3a, 0x94, 0x4c),
        "yellow" => (0xe5, 0xa5, 0x0a),
        "orange" => (0xed, 0x5b, 0x00),
        "red" => (0xe6, 0x2d, 0x42),
        "pink" => (0xd5, 0x61, 0x99),
        "purple" => (0x91, 0x41, 0xac),
        "slate" => (0x6f, 0x83, 0x96),
        _ => return None,
    };
    DynamicColorSeed::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0).ok()
}

// ---- Windows (DWM / Personalize) ----------------------------------------

#[cfg(target_os = "windows")]
fn read_windows_appearance_mode() -> Option<AppearanceMode> {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize",
            "/v",
            "AppsUseLightTheme",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_windows_apps_use_light_theme(&text)
}

#[cfg(target_os = "windows")]
fn read_windows_accent_seed() -> Option<DynamicColorSeed> {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\DWM",
            "/v",
            "ColorizationColor",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    parse_windows_colorization_color(&text)
}

pub fn parse_windows_apps_use_light_theme(raw: &str) -> Option<AppearanceMode> {
    for line in raw.lines() {
        if line.contains("AppsUseLightTheme") {
            if line.contains("0x1") {
                return Some(AppearanceMode::Light);
            } else if line.contains("0x0") {
                return Some(AppearanceMode::Dark);
            }
        }
    }
    None
}

pub fn parse_windows_colorization_color(raw: &str) -> Option<DynamicColorSeed> {
    for line in raw.lines() {
        if line.contains("ColorizationColor") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if let Some(hex) = parts.last() {
                let stripped = hex.trim_start_matches("0x");
                if let Ok(val) = u32::from_str_radix(stripped, 16) {
                    // DWM format is 0xAARRGGBB
                    let r = ((val >> 16) & 0xFF) as f32 / 255.0;
                    let g = ((val >> 8) & 0xFF) as f32 / 255.0;
                    let b = (val & 0xFF) as f32 / 255.0;
                    return DynamicColorSeed::new(r, g, b).ok();
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockRunner {
        appearance: Option<AppearanceMode>,
        seed: Option<DynamicColorSeed>,
    }

    impl SystemThemeRunner for MockRunner {
        fn read_appearance_mode(&self) -> Option<AppearanceMode> {
            self.appearance
        }
        fn read_accent_seed(&self) -> Option<DynamicColorSeed> {
            self.seed
        }
    }

    #[test]
    fn parse_gnome_accent_and_appearance() {
        assert_eq!(
            parse_gnome_color_scheme("'prefer-dark'\n"),
            Some(AppearanceMode::Dark)
        );
        assert_eq!(
            parse_gnome_color_scheme("'prefer-light'"),
            Some(AppearanceMode::Light)
        );
        assert_eq!(parse_gnome_color_scheme("'other'"), None);

        let blue = parse_gnome_accent_color("'blue'\n").expect("gnome blue accent");
        assert!((blue.color().r - 0.208).abs() < 0.01);
        assert!((blue.color().b - 0.894).abs() < 0.01);
        assert!(parse_gnome_accent_color("'unknown'").is_none());
    }

    #[test]
    fn parse_windows_registry_values() {
        let personalize =
            "HKEY_CURRENT_USER\\...\\Personalize\n    AppsUseLightTheme    REG_DWORD    0x0\n";
        assert_eq!(
            parse_windows_apps_use_light_theme(personalize),
            Some(AppearanceMode::Dark)
        );

        let dwm = "HKEY_CURRENT_USER\\Software\\Microsoft\\Windows\\DWM\n    ColorizationColor    REG_DWORD    0xc40078d7\n";
        let seed = parse_windows_colorization_color(dwm).expect("windows accent");
        assert!((seed.color().r - 0.0).abs() < 0.01);
        assert!((seed.color().b - 0.843).abs() < 0.01);
    }

    #[test]
    fn desktop_adapter_composes_runner_facts() {
        let seed = DynamicColorSeed::new(0.2, 0.4, 0.8).unwrap();
        let adapter = DesktopSystemTheme::new(Arc::new(MockRunner {
            appearance: Some(AppearanceMode::Dark),
            seed: Some(seed),
        }));
        let observation = adapter.observe();
        assert_eq!(
            observation.appearance,
            SystemAppearance::Observed(AppearanceMode::Dark)
        );
        assert_eq!(observation.dynamic_color, SystemDynamicColor::Seed(seed));
    }
}

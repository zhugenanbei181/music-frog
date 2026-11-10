//! Host projection of the shell's own chrome into the contract's receipt facts.
//!
//! The bindings in `.slint` decide what the frame paints: which navigation the
//! shell mounted, how much column it reserved, what margin and page air the
//! chrome keeps around the body, whether a bar reserved a band, and where the
//! sidebar's layout says its rows end. This module is the only place those
//! bindings are read back as numbers, so the capture marker and any future
//! consumer state the same facts the surface is wearing - no second copy of a
//! padding lives in a script, which is what `SPACEUTIL-007` retired.
//!
//! Lengths come back in the window's own pixels; the receipt states logical
//! pixels, so each is divided by the ui unit the bindings were multiplied by
//! (`page-scale-percent / 100`), which is the one scale fact the shell owns.

use crate::AppWindow;
use crate::mobile_dock_driver::to_contract_form;
use infiltrator_contract::shell_receipt::ShellChromeFacts;
use infiltrator_contract::shell_sidebar_preference::SidebarExpansionPreference;

/// One painted length, stated in the logical units the shell bound it in.
///
/// `LumenUiScale.unit` is `1px * page-scale-percent / 100`, so a binding that
/// reads `16 * unit` is 32 device pixels at 200% and still 16 logical units.
/// The receipt speaks the second language, because that is the language the
/// thresholds, the exported column and the judge's box are stated in. Kept
/// separate so the arithmetic has its own cases: a headless window never
/// renders, so it cannot be exercised through the bindings themselves.
pub fn logical_units(length_px: f32, scale_percent: i32) -> u16 {
    // A frame that cannot state a scale is read as unscaled: there is no such
    // window, and the judge refuses any receipt that does not say 100%, so the
    // only job here is never to divide by zero.
    let percent = if scale_percent < 1 {
        100.0
    } else {
        scale_percent as f32
    };
    (length_px / (percent / 100.0)).round().max(0.0) as u16
}

fn to_contract_preference(
    preference: crate::SidebarExpansionPreference,
) -> SidebarExpansionPreference {
    match preference {
        crate::SidebarExpansionPreference::Expanded => SidebarExpansionPreference::Expanded,
        crate::SidebarExpansionPreference::Collapsed => SidebarExpansionPreference::Collapsed,
    }
}

/// The chrome this window is painting right now.
pub fn shell_chrome_facts(window: &AppWindow) -> ShellChromeFacts {
    let chrome = window.get_receipt_chrome();
    let scale = chrome.scale_percent;
    let length_px = |value: f32| logical_units(value, scale);
    ShellChromeFacts {
        nav_form: to_contract_form(chrome.nav_form),
        nav_column_px: length_px(chrome.nav_column),
        nav_stack_bottom_px: length_px(chrome.nav_stack_bottom),
        nav_content_offset_px: length_px(chrome.nav_content_offset),
        page_margin_px: length_px(chrome.page_margin),
        shell_top_air_px: length_px(chrome.shell_top_air),
        shell_bottom_air_px: length_px(chrome.shell_bottom_air),
        dock_band_px: length_px(chrome.dock_band),
        scale_percent: chrome.scale_percent.max(0) as u16,
        sidebar_expansion: to_contract_preference(chrome.sidebar_expansion),
        touch_surface: chrome.touch_surface,
        sidebar_toggle_px: length_px(chrome.sidebar_toggle),
    }
}

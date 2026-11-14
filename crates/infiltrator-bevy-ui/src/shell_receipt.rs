//! The chrome geometry a captured frame really painted.
//!
//! The space-utilization judge measures pixels, so the numbers it builds its
//! measurement box from have to be the numbers that drew those pixels
//! ([`infiltrator_contract::shell_receipt`]). The Bevy surface published
//! nothing: every captured frame carried a readiness marker and no
//! `CAPTURE_CHROME` line, so the judge had no box to measure and this
//! surface stayed outside the pixel leg (`SPACEUTIL-022`).
//!
//! This module is the Bevy half of that receipt, and it is made of two
//! pieces:
//!
//! * [`ShellChromeMeasurement`] — the boxes the live layout measured, read
//!   out of the same tree that painted the frame: the sidebar panel's own
//!   box, the last destination row's own bottom edge, the page body's own
//!   box and the compact bar's own height.
//! * [`shell_chrome_facts`] — the typed receipt, refused while any box the
//!   mounted form needs is still unmeasured. A partial line is a frame the
//!   judge may not measure, and a remembered number is exactly what the
//!   receipt exists to retire.
//!
//! Every number is measured, never remembered. The column is the panel's own
//! width; the stack end is the last row's own bottom edge; the page margin
//! is the gap the content column actually left between the column's edge and
//! the page body; the shell air is the distance from the window's edge to
//! the body the frame painted; and the leading is the ground between the
//! column's left edge and the first mark the first destination row painted.

use crate::app::{BottomNavBar, ContentSlot, SidebarNavItem, SidebarPanel};
use crate::interaction_capture::geometry::{CaptureGeometry, GeometryRejections};
use crate::shell_scene::nav::NavIndicator;
use bevy::app::{App, Plugin, PostUpdate};
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::SystemParam;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::text::RemSize;
use bevy::ui::UiSystems;
use bevy::ui::prelude::{Node, Val};
use bevy::window::{PrimaryWindow, Window};
use infiltrator_bevy_widgets::theme::REM_BASE;
use infiltrator_contract::responsive_viewport::{SidebarForm, ViewportTier};
use infiltrator_contract::shell_receipt::ShellChromeFacts;
use infiltrator_contract::shell_sidebar_preference::SidebarExpansionPreference;

/// The baseline root font size the rem channel scales from.
pub const BASELINE_FONT_PX: f32 = REM_BASE;

/// The boxes one frame's shell measured, in window pixels.
///
/// A field stays `None` until the layout measured the box it names, so a
/// frame that is not laid out yet publishes no receipt rather than a
/// half-described one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ShellChromeMeasurement {
    /// The navigation column's own box: `[x, y, width, height]`.
    pub nav_column: Option<[f32; 4]>,
    /// The bottom edge of the last destination row the column painted.
    pub nav_stack_bottom: Option<f32>,
    /// The left edge of the first mark the first destination row painted.
    pub nav_first_mark_left: Option<f32>,
    /// The page body's own box: `[x, y, width, height]`.
    pub page_body: Option<[f32; 4]>,
    /// The compact bar's own height, when the shell mounted one.
    pub dock_band: Option<f32>,
}

/// The measured chrome of the frame being captured.
///
/// The resource exists so the marker writer can publish what the layout
/// measured without measuring the tree a second time: the measurement runs
/// after the UI layout, the marker writer runs after the measurement.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ShellChromeReceipt(pub ShellChromeMeasurement);

/// The read-only views of the shell tree the measurement needs.
#[derive(SystemParam)]
pub struct ShellChromeNodes<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    panels: Query<'w, 's, Entity, With<SidebarPanel>>,
    slots: Query<'w, 's, Entity, With<ContentSlot>>,
    docks: Query<'w, 's, Entity, With<BottomNavBar>>,
    rows: Query<'w, 's, (Entity, &'static Node, &'static Children), With<SidebarNavItem>>,
    indicators: Query<'w, 's, Entity, With<NavIndicator>>,
}

/// The receipt of the frame the shell really painted.
///
/// The form comes from the shared viewport ladder — the same ladder the
/// judge reads the tier from — so the shell and the judge cannot disagree
/// about which form a width owns. The column, the stack end, the leading,
/// the page margin and the two air bands come from the measured tree.
///
/// `None` means the frame cannot describe itself yet: a form that owns a
/// column without a measured stack end, a column without a measured page
/// body, or a page body without a measured window. Such a frame is reported
/// by the judge, never measured against a number the shell remembered.
pub fn shell_chrome_facts(
    window: (f32, f32),
    scale_percent: u16,
    expansion: SidebarExpansionPreference,
    measurement: &ShellChromeMeasurement,
) -> Option<ShellChromeFacts> {
    let (width, height) = window;
    if !(width.is_finite() && height.is_finite()) || width <= 0.0 || height <= 0.0 {
        return None;
    }
    let form = ViewportTier::from_width(width).sidebar_form();
    let page_body = measurement.page_body?;
    let column = measurement.nav_column;
    let column_width = match column {
        Some([_, _, column_width, _]) => column_width,
        None => 0.0,
    };
    // The column and the stack end are one fact seen from two edges: a form
    // that reserves a column must say where its rows stop, and a form that
    // reserves none owns no vertical stack.
    if column_width > 0.0 && measurement.nav_stack_bottom.is_none() {
        return None;
    }
    let dock_band = measurement.dock_band.unwrap_or(0.0);
    let bottom_air = height - (page_body[1] + page_body[3]) - dock_band;
    if !(bottom_air.is_finite() && bottom_air >= 0.0) {
        return None;
    }
    // The page body's horizontal inset: the gutter between the navigation
    // column's edge and the body it paints. A form with no column measures
    // the same gutter from the window's own edge.
    let margin = match column {
        Some([x, _, column_width, _]) => page_body[0] - (x + column_width),
        None => page_body[0],
    };
    if !(margin.is_finite() && margin >= 0.0) {
        return None;
    }
    let leading = match (column, measurement.nav_first_mark_left) {
        (Some([x, _, _, _]), Some(mark)) => mark - x,
        _ => 0.0,
    };
    if !(leading.is_finite() && leading >= 0.0) {
        return None;
    }
    Some(ShellChromeFacts {
        nav_form: form,
        nav_column_px: round_px(column_width),
        nav_stack_bottom_px: round_px(measurement.nav_stack_bottom.unwrap_or(0.0)),
        // A rail centres its glyphs in the column, so it has no leading to
        // state; a labelled column indents its rows and states the ground it
        // gives away. The pair is the contract's rule, read here.
        nav_content_offset_px: if matches!(form, SidebarForm::Standard | SidebarForm::Wide) {
            round_px(leading)
        } else {
            0
        },
        page_margin_px: round_px(margin),
        shell_top_air_px: round_px(page_body[1]),
        shell_bottom_air_px: round_px(bottom_air),
        dock_band_px: round_px(dock_band),
        scale_percent,
        sidebar_expansion: expansion,
        // The Bevy desktop is a pointing-device surface; a touch build
        // resolves the compact form at every width instead.
        touch_surface: false,
        // The shell paints no collapse affordance above its destinations, so
        // it owns no toggle band to publish.
        sidebar_toggle_px: 0,
    })
}

fn round_px(value: f32) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    value.round().max(0.0).min(u16::MAX as f32) as u16
}

/// Measure the shell's chrome for the frame being captured.
///
/// The measurement runs after the UI layout, so every box it reads is the
/// box that painted this frame's pixels. A frame whose tree is not laid out
/// yet measures nothing and therefore publishes no receipt.
pub fn measure_shell_chrome(
    geometry: CaptureGeometry,
    nodes: ShellChromeNodes,
    rem_size: Option<Res<RemSize>>,
    mut receipt: ResMut<ShellChromeReceipt>,
) {
    let Ok(_window) = nodes.windows.single() else {
        return;
    };
    let root_font_px = rem_size.map_or(BASELINE_FONT_PX, |size| size.0);

    let mut measurement = ShellChromeMeasurement::default();

    // The navigation column: the sidebar panel's own box. A compact shell
    // hides the panel and mounts the bottom bar instead, so the column is
    // absent rather than zero-width.
    if let Ok(panel) = nodes.panels.single()
        && let Some(rect) = geometry.rect(panel)
    {
        measurement.nav_column = Some(rect);
    }

    // The destination stack: the last row's own bottom edge, and the first
    // row's first painted mark. The rows are the ones the column painted,
    // so a row the layout has not placed yet contributes nothing.
    let mut last_bottom: Option<f32> = None;
    let mut first_row: Option<(f32, f32)> = None;
    for (entity, node, children) in &nodes.rows {
        let Some(rect) = geometry.rect(entity) else {
            continue;
        };
        let bottom = rect[1] + rect[3];
        if last_bottom.is_none_or(|known| bottom > known) {
            last_bottom = Some(bottom);
        }
        // The first row is the topmost one; its content origin is where its
        // glyph box starts, and the indicator it paints at the row's own edge
        // is the first mark when it is the current destination.
        let content_left = rect[0] + resolve_px(node.padding.left, root_font_px).unwrap_or(0.0);
        let mut mark_left = content_left;
        for child in children.iter() {
            if !nodes.indicators.contains(*child) || !geometry.visible(*child) {
                continue;
            }
            if let Some(indicator) = geometry.rect(*child) {
                mark_left = mark_left.min(indicator[0]);
            }
        }
        if first_row.is_none_or(|(top, _)| rect[1] < top) {
            first_row = Some((rect[1], mark_left));
        }
    }
    measurement.nav_stack_bottom = last_bottom;
    if let Some((_, mark_left)) = first_row {
        measurement.nav_first_mark_left = Some(mark_left);
    }

    // The page body: the content slot's own box. The shell air above and
    // below it is measured against the window's edges, so the judge reads
    // the room the frame really left.
    if let Ok(slot) = nodes.slots.single() {
        measurement.page_body = geometry.rect(slot);
    }

    // The compact bar: the bottom navigation's own height, when the shell
    // mounted one.
    if let Ok(dock) = nodes.docks.single()
        && geometry.visible(dock)
        && let Some(rect) = geometry.rect(dock)
    {
        measurement.dock_band = Some(rect[3]);
    }

    receipt.0 = measurement;
}

fn resolve_px(value: Val, root_font_px: f32) -> Option<f32> {
    match value {
        Val::Px(px) => Some(px),
        Val::Rem(rem) => Some(rem * root_font_px),
        _ => None,
    }
}

/// The page scale this frame was painted at, in percent.
///
/// The rem channel is the surface's only scaling entry, so the root font
/// size is the scale the pixels were painted at. A frame painted at another
/// scale says so, and the judge refuses to measure it against a box that
/// does not fit its pixels.
pub fn page_scale_percent(rem_size: Option<&RemSize>) -> u16 {
    let root_font_px = rem_size.map_or(BASELINE_FONT_PX, |size| size.0);
    let percent = (root_font_px / BASELINE_FONT_PX) * 100.0;
    if !percent.is_finite() {
        return 100;
    }
    percent.round().clamp(1.0, 1000.0) as u16
}

/// Installs the chrome measurement for the capture pipeline.
pub struct ShellChromePlugin;

impl Plugin for ShellChromePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShellChromeReceipt>();
        // The geometry reader logs a rejection through this resource, and a
        // plain page capture installs neither the interaction capture nor a
        // capture section that would otherwise register it. Without this the
        // measurement would panic on exactly the frames it exists to describe.
        app.init_resource::<GeometryRejections>();
        app.add_systems(
            PostUpdate,
            measure_shell_chrome.after(UiSystems::PostLayout),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::shell_receipt::CHROME_KEYS;

    fn desktop_measurement() -> ShellChromeMeasurement {
        ShellChromeMeasurement {
            // A 240px labelled column at the window's left edge.
            nav_column: Some([0.0, 38.0, 240.0, 742.0]),
            // Seven destination rows, the last one ending at 382.
            nav_stack_bottom: Some(382.0),
            // The first row's indicator is painted at the column's edge.
            nav_first_mark_left: Some(12.0),
            // The page body sits 16px past the column's edge and 54px below
            // the window's top, and ends 16px above the window's bottom.
            page_body: Some([256.0, 54.0, 908.0, 710.0]),
            dock_band: None,
        }
    }

    #[test]
    fn a_desktop_frame_describes_the_geometry_it_painted() {
        let facts = shell_chrome_facts(
            (1180.0, 780.0),
            100,
            SidebarExpansionPreference::Expanded,
            &desktop_measurement(),
        )
        .expect("a measured desktop frame describes itself");
        assert_eq!(facts.nav_form, SidebarForm::Standard);
        assert_eq!(facts.nav_column_px, 240);
        assert_eq!(facts.nav_stack_bottom_px, 382);
        assert_eq!(facts.nav_content_offset_px, 12);
        assert_eq!(facts.page_margin_px, 16);
        assert_eq!(facts.shell_top_air_px, 54);
        assert_eq!(facts.shell_bottom_air_px, 16);
        assert_eq!(facts.dock_band_px, 0);
        assert_eq!(facts.scale_percent, 100);
        assert_eq!(
            facts.sidebar_expansion,
            SidebarExpansionPreference::Expanded
        );
        assert!(!facts.touch_surface);
        assert_eq!(facts.sidebar_toggle_px, 0);
    }

    #[test]
    fn the_published_line_carries_exactly_the_declared_key_set() {
        let facts = shell_chrome_facts(
            (1180.0, 780.0),
            100,
            SidebarExpansionPreference::Expanded,
            &desktop_measurement(),
        )
        .expect("a measured desktop frame describes itself");
        let line = facts.to_marker_line();
        let keys: Vec<&str> = line
            .split_whitespace()
            .skip(1)
            .map(|pair| pair.split('=').next().expect("key=value"))
            .collect();
        assert_eq!(keys, CHROME_KEYS.to_vec());
        // The contract's own reader reads back what this shell wrote.
        assert_eq!(
            ShellChromeFacts::parse_marker_line(&line),
            Some(facts),
            "the receipt a frame publishes must read back as the same facts"
        );
    }

    #[test]
    fn a_frame_that_measured_nothing_publishes_no_receipt() {
        assert!(
            shell_chrome_facts(
                (1180.0, 780.0),
                100,
                SidebarExpansionPreference::Expanded,
                &ShellChromeMeasurement::default(),
            )
            .is_none(),
            "a frame with no measured page body describes nothing"
        );
    }

    #[test]
    fn a_column_without_a_measured_stack_end_publishes_no_receipt() {
        let mut measurement = desktop_measurement();
        measurement.nav_stack_bottom = None;
        assert!(
            shell_chrome_facts(
                (1180.0, 780.0),
                100,
                SidebarExpansionPreference::Expanded,
                &measurement,
            )
            .is_none(),
            "the column and the stack end are one fact seen from two edges"
        );
    }

    #[test]
    fn a_compact_frame_publishes_a_bar_and_no_column() {
        let measurement = ShellChromeMeasurement {
            nav_column: None,
            nav_stack_bottom: None,
            nav_first_mark_left: None,
            page_body: Some([12.0, 50.0, 376.0, 682.0]),
            dock_band: Some(58.0),
        };
        let facts = shell_chrome_facts(
            (400.0, 800.0),
            100,
            SidebarExpansionPreference::Expanded,
            &measurement,
        )
        .expect("a measured compact frame describes itself");
        assert_eq!(facts.nav_form, SidebarForm::BottomNav);
        assert_eq!(facts.nav_column_px, 0);
        assert_eq!(facts.nav_stack_bottom_px, 0);
        assert_eq!(facts.nav_content_offset_px, 0);
        assert_eq!(facts.dock_band_px, 58);
        assert_eq!(facts.page_margin_px, 12);
        assert_eq!(facts.shell_bottom_air_px, 10);
    }

    #[test]
    fn an_icon_rail_states_no_leading() {
        let measurement = ShellChromeMeasurement {
            nav_column: Some([0.0, 38.0, 64.0, 442.0]),
            nav_stack_bottom: Some(398.0),
            nav_first_mark_left: Some(8.0),
            page_body: Some([80.0, 54.0, 624.0, 410.0]),
            dock_band: None,
        };
        let facts = shell_chrome_facts(
            (720.0, 480.0),
            100,
            SidebarExpansionPreference::Expanded,
            &measurement,
        )
        .expect("a measured rail frame describes itself");
        assert_eq!(facts.nav_form, SidebarForm::Rail);
        assert_eq!(facts.nav_column_px, 64);
        assert_eq!(facts.nav_content_offset_px, 0);
    }

    #[test]
    fn the_page_scale_follows_the_root_font_size() {
        assert_eq!(page_scale_percent(None), 100);
        assert_eq!(page_scale_percent(Some(&RemSize(BASELINE_FONT_PX))), 100);
        assert_eq!(
            page_scale_percent(Some(&RemSize(BASELINE_FONT_PX * 1.5))),
            150
        );
    }

    /// Every capture frame measures its own chrome, so the plugin's system
    /// must run on a shell that mounted no interaction and no section.
    #[test]
    fn the_measurement_system_runs_on_a_plain_capture() {
        let mut app = App::new();
        app.add_plugins(bevy::MinimalPlugins);
        app.add_plugins(ShellChromePlugin);
        app.update();
        assert_eq!(
            app.world().resource::<ShellChromeReceipt>().0.page_body,
            None,
            "a shell with no laid-out page body publishes no measurement"
        );
    }
}

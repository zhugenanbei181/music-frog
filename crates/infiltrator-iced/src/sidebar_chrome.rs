//! The desktop sidebar's resolved form, its own geometry, and the frame receipt.
//!
//! The space-utilization judge measures pixels, so every number it builds its
//! measurement box from has to be the number that drew those pixels
//! (`infiltrator_contract::shell_receipt`). Until this module existed the Iced
//! pane published nothing, so the whole pixel leg covered Slint alone, and the
//! two facts the pane could not state - where its navigation stack really ends
//! and whether it offers a collapse affordance - were the numbers that leg
//! needs most (`SPACEUTIL-011`).
//!
//! Three things live here, and only here, for this surface:
//!
//! * [`SidebarPresentation`] - the one resolution of the user's persisted
//!   preference against the viewport's capability. The pane paints what it
//!   returns, and the receipt publishes the same answer, so a form cannot be
//!   drawn and described differently by two code paths.
//! * [`SidebarChromeReceipt`] - the boxes the live layout measured, named by
//!   the widget ids the pane mounts.
//! * [`shell_chrome_facts`] - the receipt the frame writes, refused while any
//!   box of the mounted form is still unmeasured. A partial line is not a
//!   frame the judge may measure, and a remembered number is exactly what the
//!   receipt exists to retire.

use iced::Rectangle;
use infiltrator_contract::design_tokens::space;
use infiltrator_contract::responsive_viewport::{SidebarForm, ViewportTier};
use infiltrator_contract::shell_receipt::ShellChromeFacts;
use infiltrator_contract::shell_sidebar_preference::{
    SidebarExpansionCapability, SidebarExpansionPreference, SidebarExpansionPresentation,
    resolve_sidebar_expansion,
};

/// The widget id of the box that ends the navigation stack.
///
/// It wraps the destination rows and nothing else - the free air below the
/// stack and the collapse affordance above it are outside it - so its own
/// bottom edge IS the last row's bottom edge, in window pixels. The Slint pane
/// publishes the same fact from the last row itself
/// (`DesktopSideBar.publish-stack-bottom`); both surfaces state "where the last
/// destination ends", which is what lets the judge tell eight painted rows from
/// a chrome card parked under them.
pub const SIDEBAR_STACK_END_ID: &str = "sidebar-stack-end";

/// The widget id of the collapse/expand affordance, when the resolved variant
/// offers one. Its band is what the judge skips before it reads the labelled
/// rows' leading, so the affordance's own glyph inset cannot be misread as the
/// destinations' inset.
pub const SIDEBAR_TOGGLE_ID: &str = "sidebar-toggle";

/// The widget id of the compact bottom bar, whose own height is the band it
/// reserves at the bottom of the page.
pub const SIDEBAR_DOCK_ID: &str = "sidebar-dock";

/// The labelled row's content leading: the shared layout ladder's default gap,
/// the same rung the Slint pane's `rail-content-leading` reads. The row both
/// pads its content by this and states it in the receipt, so the declared and
/// the painted leading are one fact.
pub const NAV_CONTENT_OFFSET_PX: u16 = space::SM as u16;

/// The selected row's indicator: a narrow bar at the column's left edge, sized
/// as the physical fact it is (the Slint pane's `nav-indicator-width/height`
/// carry the same pair). It is painted AT x0, so the judge's leading scan -
/// which starts past the indicator - still reads the content inset.
pub const NAV_INDICATOR_WIDTH_PX: f32 = 3.0;
pub const NAV_INDICATOR_HEIGHT_PX: f32 = 16.0;

/// The gap between the collapse affordance and the first destination row.
pub const TOGGLE_GAP_PX: u16 = space::XS as u16;

/// The pane's page scale. The capture readback refuses any screenshot whose
/// scale factor is not 1.0 (`AppState::finish_capture_frame`), and the compositor
/// runs at scale 1, so the honest value is unscaled: the receipt's numbers are
/// the pixels the judge measures, and the judge refuses any other percentage.
pub const SCALE_PERCENT: u16 = 100;

/// The one resolution of the persisted preference against the viewport.
///
/// The preference is the user's (owner: the settings readback), the capability
/// is the viewport's, and this type is the single place the two become a
/// painted form. It is deliberately not a view-layer boolean: a surface that
/// asked "am I collapsed" locally would be a second answer to a question the
/// contract already answers (`resolve_sidebar_expansion`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SidebarPresentation {
    tier: ViewportTier,
    preference: SidebarExpansionPreference,
}

impl SidebarPresentation {
    pub const fn resolve(tier: ViewportTier, preference: SidebarExpansionPreference) -> Self {
        Self { tier, preference }
    }

    /// What the pane paints after preference and capability are folded together.
    pub const fn presentation(self) -> SidebarExpansionPresentation {
        resolve_sidebar_expansion(
            self.preference,
            SidebarExpansionCapability::from_tier(self.tier),
        )
    }

    /// The navigation form the pane mounts.
    pub const fn form(self) -> SidebarForm {
        self.presentation().form()
    }

    /// The preference this surface last read from its owner.
    pub const fn preference(self) -> SidebarExpansionPreference {
        self.preference
    }

    /// Whether this viewport may offer the collapse/expand affordance.
    ///
    /// This is the same fact the exported `sidebar_variants` row carries as
    /// `toggle_available`: a labelled-capable viewport offers the affordance
    /// whether the column is currently expanded or collapsed, and a viewport
    /// that cannot honour expansion (the icon rail of the medium tier, the
    /// compact bottom bar) offers none.
    pub const fn toggle_available(self) -> bool {
        SidebarExpansionCapability::from_tier(self.tier).can_expand()
    }

    /// Whether the mounted rows carry text labels.
    pub const fn labelled(self) -> bool {
        self.form().shows_labels()
    }

    /// The content leading the labelled rows give away inside the column.
    pub const fn nav_content_offset_px(self) -> u16 {
        if self.labelled() {
            NAV_CONTENT_OFFSET_PX
        } else {
            0
        }
    }
}

/// The boxes the live layout measured for the mounted sidebar, before they are
/// folded into a receipt.
///
/// Each field is `None` until the geometry probe has walked the widget tree
/// once, and a frame whose form needs a box that is still `None` never
/// publishes a receipt: the line would otherwise claim a geometry nobody
/// measured, which is the defect the receipt exists to prevent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SidebarChromeReceipt {
    pub stack_end: Option<Rectangle>,
    pub toggle: Option<Rectangle>,
    pub dock: Option<Rectangle>,
}

impl SidebarChromeReceipt {
    /// Whether the measured boxes can account for everything the form paints.
    pub fn is_complete(self, presentation: SidebarPresentation) -> bool {
        let column = presentation.form().has_column();
        // The column and the stack end are one fact seen from two edges: a form
        // with no column owns no stack, and a form with one must have measured
        // where it ends.
        let stack = !column || self.stack_end.is_some();
        let toggle = !presentation.toggle_available() || self.toggle.is_some();
        let dock = column || self.dock.is_some();
        stack && toggle && dock
    }

    /// Where the navigation stack ends, in window pixels.
    ///
    /// The column and the stack end are one fact seen from two edges, and the
    /// receipt states `0` for both when the form reserves no column.
    fn stack_bottom_px(self, presentation: SidebarPresentation) -> u16 {
        if !presentation.form().has_column() {
            return 0;
        }
        self.stack_end
            .map(|box_| round_px(box_.y + box_.height))
            .unwrap_or(0)
    }

    /// The band the collapse affordance occupies, including the gap below it.
    fn toggle_px(self, presentation: SidebarPresentation) -> u16 {
        if !presentation.toggle_available() {
            return 0;
        }
        self.toggle
            .map(|box_| round_px(box_.y + box_.height) + TOGGLE_GAP_PX)
            .unwrap_or(0)
    }

    /// The band the compact bottom bar reserves at the bottom of the page.
    fn dock_band_px(self, presentation: SidebarPresentation) -> u16 {
        if presentation.form().has_column() {
            return 0;
        }
        self.dock.map(|box_| round_px(box_.height)).unwrap_or(0)
    }
}

fn round_px(value: f32) -> u16 {
    value.round().max(0.0) as u16
}

/// The chrome line this frame's shell wrote, or `None` while it cannot account
/// for the form it mounted.
pub fn shell_chrome_facts(
    presentation: SidebarPresentation,
    receipt: SidebarChromeReceipt,
    viewport: ViewportTier,
) -> Option<ShellChromeFacts> {
    if !receipt.is_complete(presentation) {
        return None;
    }
    let form = presentation.form();
    Some(ShellChromeFacts {
        nav_form: form,
        nav_column_px: form.width_px().unwrap_or(0),
        nav_stack_bottom_px: receipt.stack_bottom_px(presentation),
        nav_content_offset_px: presentation.nav_content_offset_px(),
        // The page body's own margin: the outer padding the page container
        // paints, straight from the tier the shell laid the window out with.
        page_margin_px: viewport.content_padding_px(),
        // The shell reserves no air above the page body of its own: the strip
        // that carries the title and the window controls is window chrome, not
        // page air, and the page's own padding is already stated as its margin.
        shell_top_air_px: 0,
        shell_bottom_air_px: 0,
        dock_band_px: receipt.dock_band_px(presentation),
        scale_percent: SCALE_PERCENT,
        sidebar_expansion: presentation.preference(),
        // The Iced desktop is a pointing-device surface: a touch build resolves
        // the compact form at every width instead (`shell_navigation`).
        touch_surface: false,
        sidebar_toggle_px: receipt.toggle_px(presentation),
    })
}

#[cfg(test)]
#[path = "../tests/gui/sidebar_chrome_tests.rs"]
mod tests;

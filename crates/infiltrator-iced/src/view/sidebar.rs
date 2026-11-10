//! Desktop navigation sidebar: destinations and nothing else.
//!
//! The rows come from the shared typed registry
//! ([`infiltrator_contract::shell_navigation::desktop_items`]), the same table
//! the Slint rail and the Bevy rail mount, so a destination's id, order,
//! grouping, label and semantic icon are stated once for every surface. This
//! pane used to declare its own primary/secondary route tables *and* carry the
//! shell's status: a brand tile with the package version, a routing-intent
//! segmented control, the system-proxy/TUN switch cards, the active profile
//! card with its usage bar, a shortcut matrix with count badges and a live
//! speed footer with a sparkline. Every one of those facts already had an
//! owner - the overview page publishes the master switches and the intent
//! segment, the window chrome strip owns the title and the live rates, the
//! About page owns the version, the overview quota card owns the profile and
//! its usage - so the rail was a second truth source for five facts, and it
//! pushed navigation below a wall of cards. The reference desktop sidebar is
//! navigation alone (`docs/reference/FLCLASH_DESKTOP_REFERENCE.md` §2), and
//! `docs/SPACE_UTILIZATION_STANDARD.md` holds the rail at a zero non-navigation
//! band, which is what `chrome_band` 0% enforces on every judged frame.
//!
//! A destination that the registry folds under another one (DNS, Doctor, App
//! Routing, Sync, Tools, About, Disclaimer, Licenses) gets no row of its own:
//! it is reached from the Settings page's index, which is built from
//! `pages_folded_under` rather than from a literal list.

use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::components::{nav_button, nav_rail_icon};
use crate::view::svg_icons::{icon_for_semantic, icon_themed};
use crate::view::theme;
use iced::widget::{Space, button, column, container, row};
use iced::{Alignment, Border, Color, Element, Length, Theme, border};
use infiltrator_contract::responsive_viewport::{SidebarForm, ViewportTier};
use infiltrator_contract::shell_navigation::{
    NavigationItem, NavigationSurface, NavigationTarget, desktop_items,
};
use infiltrator_contract::sidebar_material::SidebarMaterialKind;
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_contract::window_chrome::DesktopPlatformKind;
use infiltrator_presentation::sidebar_material_projection::{
    project_sidebar_material, project_sidebar_surface_wash,
};
use infiltrator_shared::locales::{Lang, Localizer};

/// Standard labelled sidebar width. Mirrors
/// `responsive_viewport::SidebarForm::Standard`, the authoritative value.
pub(crate) const SIDEBAR_WIDTH: f32 = 240.0;

/// Compact 64px rail sidebar for responsive tablet or narrow desktop views.
pub const RAIL_WIDTH: f32 = 64.0;

/// The rows this desktop surface mounts, in the registry's own order.
fn sidebar_rows() -> Vec<NavigationItem> {
    desktop_items(SurfaceKind::IcedDesktop)
}

/// Map a registry destination onto this surface's route.
///
/// The one surface-local row is the traffic page: Slint paints it as its own
/// page, and Iced folds it into the runtime page (which hosts the log and
/// traffic sections), so `Runtime` is the route that owns that destination
/// here. A surface-local id this surface does not own cannot arrive - the
/// registry filters those out through `owns_surface_local` - so reaching the
/// fallback arm is a registry/surface disagreement, not a user state.
fn route_for_destination(target: NavigationTarget) -> Route {
    match target {
        NavigationTarget::Shared(page) => Route::from_shell_page(page),
        NavigationTarget::SurfaceLocal("traffic") => Route::Runtime,
        NavigationTarget::SurfaceLocal(id) => unreachable!(
            "the Iced desktop mounts no surface-local row, but `{id}` reached the pane"
        ),
    }
}

/// The localized label of one row. A shared destination is named by the copy
/// table the contract points at ([`ShellPage::sidebar_key`]); the
/// surface-local row has no shared key by design and is named by this
/// surface's own entry for the page it opens.
fn label_for(item: NavigationItem, lang: &Lang<'_>) -> String {
    match item.target {
        NavigationTarget::Shared(page) => lang.tr(page.sidebar_key()).into_owned(),
        NavigationTarget::SurfaceLocal(_) => lang.tr("nav_runtime").into_owned(),
    }
}

/// Full labelled sidebar at the default expanded width. Kept as a stable
/// entry point for callers that do not have a tier (tests, mini HUD shell).
pub fn sidebar(state: &AppState) -> Element<'_, Message> {
    sidebar_with_form(state, SidebarForm::Standard)
}

/// The navigation column: one row per registry destination, then the free
/// air that belongs below the stack.
fn sidebar_inner(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let mut rows = column![].spacing(theme::SP_XS).width(Length::Fill);
    for item in sidebar_rows() {
        rows = rows.push(nav_button(
            label_for(item, &lang),
            route_for_destination(item.target),
            &state.shell.current_route,
            icon_for_semantic(item.icon),
        ));
    }
    rows.push(Space::new().height(Length::Fill)).into()
}

/// Compact bottom navigation bar for the `< 600px` tier: a horizontal strip of
/// icon + label entries replacing the vertical sidebar entirely. It mounts the
/// registry's dock rows, which are the destinations a four-item bar can carry.
fn sidebar_bottom_nav(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let mut bar = row![]
        .spacing(theme::SP_XS)
        .padding([theme::SP_XS, theme::SP_SM]);

    for item in NavigationItem::items(NavigationSurface::Dock) {
        let route = route_for_destination(item.target);
        let label = label_for(*item, &lang);
        let is_active = route == state.shell.current_route;
        let glyph = icon_themed(icon_for_semantic(item.icon), 18.0, move |t: &Theme| {
            if is_active {
                theme::tokens(t).accent
            } else {
                theme::tokens(t).sidebar_text_muted
            }
        });
        bar = bar.push(
            button(
                column![glyph, theme::role_body_text(label)]
                    .spacing(2)
                    .align_x(Alignment::Center),
            )
            .width(Length::Fill)
            .padding(6)
            .on_press(Message::Navigate(route))
            .style(move |t: &Theme, status| {
                let tk = theme::tokens(t);
                button::Style {
                    background: if is_active {
                        Some(tk.accent_soft.into())
                    } else if matches!(status, button::Status::Hovered) {
                        Some(tk.control_bg.into())
                    } else {
                        None
                    },
                    text_color: tk.text_primary,
                    border: Border {
                        radius: border::Radius::from(theme::R_CONTROL),
                        ..Default::default()
                    },
                    ..Default::default()
                }
            }),
        );
    }

    container(bar)
        .width(Length::Fill)
        .style(|t: &Theme| container::Style {
            background: Some(theme::tokens(t).sidebar.into()),
            ..Default::default()
        })
        .into()
}

/// Pure tier → navigation-form mapping shared with tests. Keeps the dropdown
/// width and the rendered widget in sync from one decision point.
pub fn sidebar_form_for_width(width_px: f32) -> SidebarForm {
    ViewportTier::from_width(width_px).sidebar_form()
}

/// Select and render the sidebar form for the shell's current responsive tier.
///
/// This is the single entry point the view root uses, so window-resize tier
/// changes actually re-lay out navigation instead of leaving a static sidebar.
pub fn sidebar_for_tier(state: &AppState) -> Element<'_, Message> {
    match state.shell.viewport.tier {
        ViewportTier::Compact => sidebar_bottom_nav(state),
        ViewportTier::Medium => sidebar_rail(state),
        ViewportTier::Expanded | ViewportTier::Ultra => {
            let form = state.shell.viewport.tier.sidebar_form();
            sidebar_with_form(state, form)
        }
    }
}

/// Standard / wide labelled sidebar; width is taken from the shared tier form.
fn sidebar_background_color(state: &AppState, t: &Theme) -> Color {
    let base = theme::tokens(t).sidebar;
    let is_dark = theme::tokens(t).canvas.r < 0.3;
    let material_state = project_sidebar_material(
        SidebarMaterialKind::OpaqueFallback,
        false,
        state.shell.window_focused,
        DesktopPlatformKind::Linux,
        None,
    );
    let (r, g, b, a) =
        project_sidebar_surface_wash(&material_state, (base.r, base.g, base.b), is_dark);
    Color::from_rgba(r, g, b, a)
}

fn sidebar_with_form(state: &AppState, form: SidebarForm) -> Element<'_, Message> {
    let width = form.width_px().map(f32::from).unwrap_or(SIDEBAR_WIDTH);
    let inner = sidebar_inner(state);
    let bg = sidebar_background_color(state, &state.shell.theme);
    container(inner)
        .width(width)
        .height(Length::Fill)
        .padding([theme::SP_LG, theme::SP_MD])
        .style(move |_t: &Theme| container::Style {
            background: Some(bg.into()),
            ..Default::default()
        })
        .into()
}

/// Compact vertical rail with icon-only navigation (medium tier).
///
/// A rail is navigation by glyph alone: the same registry rows the labelled
/// column mounts, with the label left to the hover tooltip the icon button
/// carries. It used to paint a brand tile of its own and a second, longer
/// route list, which is how the two forms of one pane drifted apart.
pub fn sidebar_rail(state: &AppState) -> Element<'_, Message> {
    let mut items = column![].spacing(theme::SP_XS).align_x(Alignment::Center);
    for item in sidebar_rows() {
        items = items.push(nav_rail_icon(
            route_for_destination(item.target),
            &state.shell.current_route,
            icon_for_semantic(item.icon),
        ));
    }

    let bg = sidebar_background_color(state, &state.shell.theme);
    container(items)
        .width(RAIL_WIDTH)
        .height(Length::Fill)
        .padding([theme::SP_MD, theme::SP_XS])
        .style(move |_t: &Theme| container::Style {
            background: Some(bg.into()),
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_sidebar_tests.rs"]
mod tests;

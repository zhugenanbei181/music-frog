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

use crate::sidebar_chrome::{
    NAV_CONTENT_OFFSET_PX, SIDEBAR_DOCK_ID, SIDEBAR_STACK_END_ID, SIDEBAR_TOGGLE_ID,
    SidebarPresentation,
};
use crate::state::AppState;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::view::components::{nav_button, nav_rail_icon};
use crate::view::svg_icons::{Icon, icon_for_semantic, icon_themed};
use crate::view::theme;
use iced::advanced::widget::Id;
use iced::widget::{Space, button, column, container, row, tooltip};
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

/// The rows this desktop surface mounts, in the registry's own order.
fn sidebar_rows() -> Vec<NavigationItem> {
    desktop_items(SurfaceKind::IcedDesktop)
}

/// Map a registry destination onto this surface's route.
///
/// The one surface-local row is the traffic page; both desktop surfaces mount
/// it as its own destination. Connections and logs use their shared dedicated
/// routes.
/// A surface-local id this surface does not own cannot arrive - the registry
/// filters those out through `owns_surface_local` - so reaching the fallback
/// arm is a registry/surface disagreement, not a user state.
fn route_for_destination(target: NavigationTarget) -> Route {
    match target {
        NavigationTarget::Shared(page) => Route::from_shell_page(page),
        NavigationTarget::SurfaceLocal("traffic") => Route::Traffic,
        NavigationTarget::SurfaceLocal(id) => unreachable!(
            "the Iced desktop mounts no surface-local row except traffic, but `{id}` reached the pane"
        ),
    }
}

/// The one navigation row selected by the contract's owner for this route.
///
/// Some routes are nested under a shared destination: Iced's runtime page is
/// the `Network` child of Settings, while Traffic owns its own surface-local
/// destination. Resolve both directions through the shared contract before
/// painting the active row.
pub(crate) fn selected_navigation_route(route: Route) -> Route {
    route_for_destination(route.navigation_target())
}

/// How many registry destinations resolve to this route's contract owner.
///
/// The capture gate reads it: a frame that claims to be the connections page
/// is only publishable while exactly one destination is selected, so a folded
/// route (two destinations, one selection) cannot masquerade as its own page.
/// `Runtime` is a nested Settings route and selects the Settings row;
/// `Traffic` resolves to the separate local traffic row.
/// Production rendering never needs the count, so the entry is gated with its
/// consumers instead of being kept alive by an `#[allow(dead_code)]`.
#[cfg(any(feature = "capture-tools", test))]
pub(crate) fn selected_navigation_destination_count(route: Route) -> usize {
    let selected_route = selected_navigation_route(route);
    sidebar_rows()
        .into_iter()
        .filter(|item| route_for_destination(item.target) == selected_route)
        .count()
}

/// The localized label of one row. A shared destination is named by the copy
/// table the contract points at ([`ShellPage::sidebar_key`]); the surface-local
/// traffic row has no shared key by design and is named by this surface's own
/// entry for the page it opens.
fn label_for(item: NavigationItem, lang: &Lang<'_>) -> String {
    match item.target {
        NavigationTarget::Shared(page) => lang.tr(page.sidebar_key()).into_owned(),
        NavigationTarget::SurfaceLocal(_) => lang.tr("nav_traffic").into_owned(),
    }
}

/// Full labelled sidebar at the default expanded width. Kept as a stable
/// entry point for callers that do not have a tier (tests, mini HUD shell).
pub fn sidebar(state: &AppState) -> Element<'_, Message> {
    sidebar_with_form(
        state,
        SidebarPresentation::resolve(ViewportTier::Expanded, state.shell.sidebar_expansion),
    )
}

/// The one resolution of the persisted preference against the viewport.
///
/// The pane paints this answer, the frame receipt publishes it, and the
/// capture gate holds the marker until the boxes it names are measured, so the
/// three cannot describe different sidebars.
pub fn sidebar_presentation(state: &AppState) -> SidebarPresentation {
    SidebarPresentation::resolve(state.shell.viewport.tier, state.shell.sidebar_expansion)
}

/// The navigation column: one row per registry destination, then the free
/// air that belongs below the stack.
fn sidebar_inner(state: &AppState, presentation: SidebarPresentation) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let selected_route = selected_navigation_route(state.shell.current_route);
    let mut rows = column![].spacing(theme::SP_XS).width(Length::Fill);
    for item in sidebar_rows() {
        rows = rows.push(nav_button(
            label_for(item, &lang),
            route_for_destination(item.target),
            &selected_route,
            icon_for_semantic(item.icon),
        ));
    }
    // The stack's own box: it wraps the destination rows and nothing else, so
    // its measured bottom edge IS the last row's bottom edge - the fact the
    // frame receipt publishes and the judge reads to tell painted navigation
    // from a chrome block parked under it.
    let stack = container(rows)
        .id(Id::new(SIDEBAR_STACK_END_ID))
        .width(Length::Fill);

    let mut pane = column![].spacing(theme::SP_XS).width(Length::Fill);
    if presentation.toggle_available() {
        pane = pane.push(sidebar_toggle(state, presentation));
    }
    pane.push(stack)
        .push(Space::new().height(Length::Fill))
        .into()
}

/// The one collapse/expand affordance of a viewport that can honour the
/// preference: a single glyph, with the word in its hover tooltip.
///
/// The exported variant decides whether the viewport offers it at all, and the
/// glyph's own box is what the receipt measures as the frame's toggle band - so
/// the judge skips the affordance before it reads the destinations' leading
/// instead of mistaking one for the other.
fn sidebar_toggle(state: &AppState, presentation: SidebarPresentation) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let labelled = presentation.labelled();
    let glyph = icon_themed(
        if labelled {
            Icon::ChevronLeft
        } else {
            Icon::ChevronRight
        },
        theme::NAV_ICON_SIZE,
        |t: &Theme| theme::tokens(t).text_secondary,
    );
    // A labelled column starts its glyph box where its destinations start
    // theirs; a rail centres the one glyph in the column, exactly as the
    // reference's toggle does.
    let content = if labelled {
        row![glyph, Space::new().width(Length::Fill)].width(Length::Fill)
    } else {
        row![
            Space::new().width(Length::Fill),
            glyph,
            Space::new().width(Length::Fill)
        ]
        .width(Length::Fill)
    };
    let face = container(content)
        .width(Length::Fill)
        .padding(iced::Padding {
            top: theme::NAV_ROW_PADDING,
            right: theme::SP_LG,
            bottom: theme::NAV_ROW_PADDING,
            left: if labelled {
                NAV_CONTENT_OFFSET_PX as f32
            } else {
                0.0
            },
        })
        .align_y(Alignment::Center);
    let control = button(face)
        .width(Length::Fill)
        .padding(0)
        .style(|t: &Theme, status| {
            let tk = theme::tokens(t);
            button::Style {
                background: match status {
                    button::Status::Hovered => Some(tk.hover.into()),
                    button::Status::Pressed => Some(tk.pressed.into()),
                    _ => None,
                },
                border: Border {
                    radius: border::Radius::from(theme::R_CONTROL),
                    ..Default::default()
                },
                text_color: tk.text_secondary,
                ..Default::default()
            }
        })
        .on_press(Message::ToggleSidebarExpansion);
    // One product-copy key per direction. The word travels in the tooltip and
    // nowhere else: a permanent label made the affordance read as a destination
    // row and bought a text run no reference paints.
    let word = if labelled {
        lang.tr("nav_sidebar_toggle_collapse")
    } else {
        lang.tr("nav_sidebar_toggle_expand")
    }
    .to_string();
    let tip = container(theme::role_body_text(word))
        .padding(6)
        .style(container::rounded_box);

    container(tooltip(control, tip, tooltip::Position::Right).gap(4.0))
        .id(Id::new(SIDEBAR_TOGGLE_ID))
        .width(Length::Fill)
        .into()
}

/// Compact bottom navigation bar for the `< 600px` tier: a horizontal strip of
/// icon + label entries replacing the vertical sidebar entirely. It mounts the
/// registry's dock rows, which are the destinations a four-item bar can carry.
fn sidebar_bottom_nav(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let selected_route = selected_navigation_route(state.shell.current_route);
    let mut bar = row![]
        .spacing(theme::SP_XS)
        .padding([theme::SP_XS, theme::SP_SM]);

    for item in NavigationItem::items(NavigationSurface::Dock) {
        let route = route_for_destination(item.target);
        let label = label_for(*item, &lang);
        let is_active = route == selected_route;
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

    // The bar's own box is the band it reserves at the bottom of the page; the
    // receipt publishes its measured height so the judge's page body stops
    // above the bar instead of counting its rows as page content.
    container(bar)
        .id(Id::new(SIDEBAR_DOCK_ID))
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
    // The form is the resolution of the user's persisted preference against the
    // viewport's capability, never the tier's own form alone: a collapsed
    // preference paints the icon rail on a labelled-capable viewport, and the
    // receipt publishes the same answer the pane painted.
    let presentation = sidebar_presentation(state);
    match presentation.form() {
        SidebarForm::BottomNav => sidebar_bottom_nav(state),
        SidebarForm::Rail => sidebar_rail_with(state, presentation),
        SidebarForm::Standard | SidebarForm::Wide => sidebar_with_form(state, presentation),
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

fn sidebar_with_form(state: &AppState, presentation: SidebarPresentation) -> Element<'_, Message> {
    let form = presentation.form();
    let width = form
        .width_px()
        .map(f32::from)
        .expect("a labelled sidebar form always reserves a column");
    let inner = sidebar_inner(state, presentation);
    let bg = sidebar_background_color(state, &state.shell.theme);
    container(inner)
        .width(width)
        .height(Length::Fill)
        // The column reserves no horizontal padding of its own: the rows are
        // full-bleed so the selected wash and the indicator can start at the
        // column's edge, and the leading the receipt publishes is the row's.
        .padding([theme::SP_LG, 0.0])
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
    sidebar_rail_with(state, sidebar_presentation(state))
}

/// The icon rail: one glyph per registry destination, labels in the tooltips.
///
/// This is the form a viewport that cannot honour expansion always paints, and
/// the form a collapsed preference paints on a labelled-capable viewport - in
/// which case it carries the same collapse/expand affordance the labelled
/// column does, because the way back is what the preference's owner offers.
fn sidebar_rail_with(state: &AppState, presentation: SidebarPresentation) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let mut items = column![].spacing(theme::SP_XS).align_x(Alignment::Center);
    let mut stack = column![].spacing(theme::SP_XS).align_x(Alignment::Center);
    for item in sidebar_rows() {
        stack = stack.push(nav_rail_icon(
            label_for(item, &lang),
            route_for_destination(item.target),
            &state.shell.current_route,
            icon_for_semantic(item.icon),
        ));
    }
    let stack = container(stack)
        .id(Id::new(SIDEBAR_STACK_END_ID))
        .width(Length::Fill)
        .align_x(Alignment::Center);
    if presentation.toggle_available() {
        items = items.push(sidebar_toggle(state, presentation));
    }
    items = items.push(stack);
    let items = items.push(Space::new().height(Length::Fill));

    let bg = sidebar_background_color(state, &state.shell.theme);
    container(items)
        .width(f32::from(
            SidebarForm::Rail
                .width_px()
                .expect("the rail always reserves a column"),
        ))
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

//! The compact dock projection of the Slint surface.
//!
//! The dock is a navigation surface, so it reads the one typed
//! `NavigationSurface::Dock` registry. Its route vocabulary lives here, next
//! to the projection that owns them: the bar consumes the published rows and
//! never derives an active row from a chain of page-string comparisons.
//!
//! The projection publishes navigation FACTS - destination, route and
//! semantic icon - and no words. The word on a dock tab is copy, and the
//! surface's compact vocabulary (`PageTitles`) is the one owner of it: the
//! same owner the compact page headers read, so a tab can never be named
//! differently from the header of the page it opens. Publishing a label
//! here would make the host spell a second copy of that dialect.

use crate::slint_model;
use crate::{AppWindow, DockSlot};
use infiltrator_contract::command_catalogue::ShellPage;
use infiltrator_contract::shell_navigation::{NavigationItem, NavigationSurface, NavigationTarget};

/// Publish the compact dock rows the bar mounts.
pub(super) fn project_dock(window: &AppWindow) {
    let rows = NavigationItem::items(NavigationSurface::Dock)
        .iter()
        .map(|item| DockSlot {
            route: dock_route(*item).into(),
            destination: item.id().into(),
            icon: item.icon.canonical_name().into(),
        })
        .collect::<Vec<_>>();
    window.set_dock_items(slint_model(rows));
}

/// The route vocabulary the dock's own destinations speak.
fn dock_route(item: NavigationItem) -> &'static str {
    match item.target {
        NavigationTarget::Shared(ShellPage::Overview) => "Home",
        NavigationTarget::Shared(ShellPage::Proxies) => "Proxy",
        NavigationTarget::Shared(ShellPage::Profiles) => "Config",
        NavigationTarget::Shared(ShellPage::Settings) => "Settings",
        _ => unreachable!("the dock registry carries only four primary destinations"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::shell_navigation::destination_for_route;

    #[test]
    fn the_dock_projection_is_the_registrys_four_typed_destinations() {
        let projected: Vec<(String, String, String)> =
            NavigationItem::items(NavigationSurface::Dock)
                .iter()
                .map(|item| {
                    (
                        dock_route(*item).to_owned(),
                        item.id().to_owned(),
                        item.icon.canonical_name().to_owned(),
                    )
                })
                .collect();
        assert_eq!(
            projected,
            [
                (
                    "Home".to_owned(),
                    "overview".to_owned(),
                    "layout-grid".to_owned()
                ),
                ("Proxy".to_owned(), "proxies".to_owned(), "globe".to_owned()),
                (
                    "Config".to_owned(),
                    "profiles".to_owned(),
                    "file-text".to_owned()
                ),
                (
                    "Settings".to_owned(),
                    "settings".to_owned(),
                    "settings".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn every_dock_route_is_a_primary_page_the_surface_can_open() {
        for item in NavigationItem::items(NavigationSurface::Dock) {
            let route = dock_route(*item);
            assert!(
                matches!(route, "Home" | "Proxy" | "Config" | "Settings"),
                "{route} is not a primary route"
            );
            assert!(
                destination_for_route(route) == Some(item.target),
                "{route} does not resolve back to {:?}",
                item.target
            );
        }
    }
}

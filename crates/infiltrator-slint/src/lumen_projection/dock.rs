//! The compact dock projection of the Slint surface.
//!
//! The dock is a navigation surface, so it reads the one typed
//! `NavigationSurface::Dock` registry. Its route vocabulary and its compact
//! label dialect live here, next to the projection that owns them: the bar
//! consumes the published rows and never derives an active row from a chain
//! of page-string comparisons.

use crate::slint_model;
use crate::{AppWindow, DockSlot};
use infiltrator_contract::command_catalogue::ShellPage;
use infiltrator_contract::shell_navigation::{NavigationItem, NavigationSurface, NavigationTarget};
use infiltrator_shared::locales::{Lang, Localizer};

/// Publish the compact dock rows the bar mounts.
pub(super) fn project_dock(window: &AppWindow, lang: &Lang<'_>) {
    let rows = NavigationItem::items(NavigationSurface::Dock)
        .iter()
        .map(|item| DockSlot {
            route: dock_route(*item).into(),
            destination: item.id().into(),
            label: dock_label(*item, lang).into(),
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

/// The compact label the dock paints for one destination.
fn dock_label(item: NavigationItem, lang: &Lang<'_>) -> String {
    let key = match item.target {
        NavigationTarget::Shared(ShellPage::Overview) => "仪表盘",
        NavigationTarget::Shared(ShellPage::Proxies) => "代理",
        NavigationTarget::Shared(ShellPage::Profiles) => "配置",
        NavigationTarget::Shared(ShellPage::Settings) => "工具",
        _ => unreachable!("the dock registry carries only four primary destinations"),
    };
    lang.tr(key).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dock_projection_is_the_registrys_four_typed_destinations() {
        let lang = Lang("zh-CN");
        let projected: Vec<(String, String, String, String)> =
            NavigationItem::items(NavigationSurface::Dock)
                .iter()
                .map(|item| {
                    (
                        dock_route(*item).to_owned(),
                        item.id().to_owned(),
                        dock_label(*item, &lang),
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
                    "仪表盘".to_owned(),
                    "layout-grid".to_owned()
                ),
                (
                    "Proxy".to_owned(),
                    "proxies".to_owned(),
                    "代理".to_owned(),
                    "globe".to_owned()
                ),
                (
                    "Config".to_owned(),
                    "profiles".to_owned(),
                    "配置".to_owned(),
                    "file-text".to_owned()
                ),
                (
                    "Settings".to_owned(),
                    "settings".to_owned(),
                    "工具".to_owned(),
                    "settings".to_owned()
                ),
            ]
        );
        let english = Lang("en-US");
        assert_eq!(
            dock_label(
                *NavigationItem::items(NavigationSurface::Dock)
                    .first()
                    .unwrap(),
                &english,
            ),
            "Dashboard"
        );
    }
}

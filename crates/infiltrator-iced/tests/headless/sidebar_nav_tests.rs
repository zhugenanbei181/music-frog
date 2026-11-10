//! The desktop sidebar IS the shared navigation registry's list.
//!
//! The pane used to declare its own primary/secondary route tables and its own
//! route-to-icon matches (twice, once per morphology), so a destination could
//! exist in the rail and nowhere else. These tests pin the replacement: the
//! rows are the registry's, every mounted destination is one the registry
//! answers for, and each row's label resolves in the shared copy table - which
//! is what the pane renders instead of a literal.
//! test-intent: behavior

use infiltrator_contract::shell_navigation::{
    NavigationItem, NavigationSurface, NavigationTarget, desktop_items, destination_for,
};
use infiltrator_contract::surface::SurfaceKind;
use infiltrator_shared::locales::{Lang, Localizer};

fn mounted_ids(surface: SurfaceKind) -> Vec<&'static str> {
    desktop_items(surface)
        .iter()
        .map(|item| item.id())
        .collect()
}

#[test]
fn the_iced_pane_mounts_the_registry_rows() {
    let ids = mounted_ids(SurfaceKind::IcedDesktop);
    assert_eq!(
        ids.first().copied(),
        Some("overview"),
        "Overview leads the stack: {ids:?}"
    );

    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(
        unique.len(),
        ids.len(),
        "no destination is mounted twice: {ids:?}"
    );
}

#[test]
fn every_mounted_row_is_a_destination_the_registry_answers_for() {
    for item in desktop_items(SurfaceKind::IcedDesktop) {
        match item.target {
            // A row that folds under another destination would be a row that
            // navigates nowhere distinct: DNS, Doctor, Sync, Tools, About,
            // Disclaimer and Licenses are reached from the Settings index.
            NavigationTarget::Shared(page) => assert_eq!(
                destination_for(page),
                page,
                "{} must be a destination of its own",
                item.id()
            ),
            NavigationTarget::SurfaceLocal(id) => {
                assert_eq!(id, "traffic", "Iced owns exactly one surface-local row")
            }
        }
    }
}

#[test]
fn the_dock_is_a_subset_of_the_desktop_rows() {
    let desktop = mounted_ids(SurfaceKind::IcedDesktop);
    let dock: Vec<&'static str> = NavigationItem::items(NavigationSurface::Dock)
        .iter()
        .map(|item| item.id())
        .collect();
    assert_eq!(dock.len(), 4, "the compact bar carries four destinations");
    for id in dock {
        assert!(
            desktop.contains(&id),
            "{id} is on the dock but not on the desktop registry"
        );
    }
}

#[test]
fn every_mounted_label_resolves_in_the_shared_copy_table() {
    for item in desktop_items(SurfaceKind::IcedDesktop) {
        let Some(key) = item.target.label_key() else {
            continue;
        };
        for locale in ["zh-CN", "en-US"] {
            let lang = Lang(locale);
            assert_ne!(
                lang.tr(key).as_ref(),
                key,
                "{key} has no copy for {locale}, so the row would render its own key"
            );
        }
    }
}

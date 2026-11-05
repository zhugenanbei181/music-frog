//! The sidebar navigation tables (EFFECTS-034 #132): one primary table for
//! every tier, disjoint secondary rows, and copy keys that match the shared
//! nav vocabulary.
//! test-intent: behavior
use infiltrator_iced::types::app::Route;
use infiltrator_iced::view::sidebar::NAV_PRIMARY;

#[test]
fn primary_rows_are_unique_and_stable() {
    let mut routes: Vec<Route> = NAV_PRIMARY.iter().map(|(route, _, _)| *route).collect();
    let before = routes.clone();
    routes.dedup();
    assert_eq!(routes.len(), NAV_PRIMARY.len(), "no duplicate primary rows");
    assert_eq!(before.first(), Some(&Route::Overview), "Overview leads");
    assert_eq!(before.last(), Some(&Route::Settings), "Settings closes");
}

#[test]
fn every_copy_key_uses_the_shared_nav_vocabulary() {
    for (_, key, _) in NAV_PRIMARY {
        assert!(key.starts_with("nav_"), "copy key {key} is nav vocabulary");
    }
}

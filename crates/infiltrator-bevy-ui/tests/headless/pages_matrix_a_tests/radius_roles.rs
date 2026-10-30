//! Behavior cases for the contract's radius roles.
//! test-intent: behavior
//!
//! The contract's radius ladder gives every rung one role: a centered
//! dialog is the modal rung, a panel-style dialog card is the card rung and a
//! bottom sheet is the bottom-sheet rung. These cases drive the real mounted
//! pages and read the painted corner radius back, so a page cannot drift onto
//! the wrong rung.

use super::{navigate_to, setup_matrix_a_app};
use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ui::prelude::{BorderRadius, Node};
use infiltrator_bevy_ui::command::DemoCommandSink;
use infiltrator_bevy_ui::pages::business_panel::BusinessPanelCard;
use infiltrator_bevy_ui::pages::connections_confirm::CloseAllConfirmationCard;
use infiltrator_bevy_ui::pages::proxy_inspection::ProxyInspectionCard;
use infiltrator_bevy_ui::route::Route;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::theme::radius;
use std::sync::Arc;

/// The painted corner radii of every mounted node carrying the marker.
fn radii_of<M: Component>(app: &mut App) -> Vec<BorderRadius> {
    let entities: Vec<Entity> = {
        let mut query = app.world_mut().query_filtered::<Entity, With<M>>();
        query.iter(app.world()).collect()
    };
    entities
        .into_iter()
        .map(|entity| {
            app.world()
                .get::<Node>(entity)
                .expect("the marked card node")
                .border_radius
        })
        .collect()
}

/// Every mounted card of the marker paints the one expected radius.
fn assert_radius<M: Component>(app: &mut App, expected: BorderRadius, what: &str) {
    let radii = radii_of::<M>(app);
    assert!(!radii.is_empty(), "{what} is mounted");
    assert!(
        radii.iter().all(|painted| *painted == expected),
        "{what} must paint {expected:?}, found {radii:?}"
    );
}

#[test]
fn the_palette_carries_the_contract_radius_rungs() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let app = setup_matrix_a_app(sink);
    let palette = *app.world().resource::<UiPalette>();
    assert_eq!(palette.modal_radius_px, radius::MODAL);
    assert_eq!(palette.card_radius_px, radius::CARD);
    assert_eq!(palette.bottom_sheet_radius_px, radius::BOTTOM_SHEET);
    assert!(
        palette.modal_radius_px > palette.card_radius_px,
        "the modal rung must be distinguishable from the card rung"
    );
}

#[test]
fn a_centered_dialog_card_takes_the_modal_rung() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let palette = *app.world().resource::<UiPalette>();

    // The close-all confirmation is a centered modal dialog.
    navigate_to(&mut app, Route::Connections);
    assert_radius::<CloseAllConfirmationCard>(
        &mut app,
        BorderRadius::all(rem(palette.modal_radius_px)),
        "the close-all confirmation card",
    );

    // The proxy inspection is a centered modal dialog too.
    navigate_to(&mut app, Route::Proxies);
    assert_radius::<ProxyInspectionCard>(
        &mut app,
        BorderRadius::all(rem(palette.modal_radius_px)),
        "the proxy inspection card",
    );
}

#[test]
fn a_panel_style_dialog_card_keeps_the_card_rung() {
    let sink = Arc::new(DemoCommandSink::accepting());
    let mut app = setup_matrix_a_app(sink);
    let palette = *app.world().resource::<UiPalette>();

    // The business panel is the contract's panel-style dialog: it keeps the
    // card rung even though it is presented over a scrim.
    navigate_to(&mut app, Route::Profiles);
    assert_radius::<BusinessPanelCard>(
        &mut app,
        BorderRadius::all(rem(palette.card_radius_px)),
        "the business panel card",
    );
}

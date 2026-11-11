//! test-intent: behavior
//! Standalone observation lists own their remaining page-body height, and a
//! narrow connection card keeps its destination label on one readable line.
use crate::state::AppState;
use crate::view::observation_layout::{ObservationListOwner, connection_rows_use_stacked_headline};
use iced::Length;
use infiltrator_contract::responsive_viewport::ResponsiveViewportSnapshot;

#[test]
fn page_body_observation_list_never_falls_back_to_the_runtime_card_height() {
    assert_eq!(ObservationListOwner::PageBody.list_height(), Length::Fill);
    assert_eq!(
        ObservationListOwner::RuntimeCard.list_height(),
        Length::Fixed(260.0)
    );
}

#[test]
fn only_narrow_viewports_stack_the_connection_headline() {
    let (mut state, _) = AppState::new();
    state.shell.viewport = ResponsiveViewportSnapshot::from_dimensions(1180.0, 780.0);
    assert!(!connection_rows_use_stacked_headline(&state));
    state.shell.viewport = ResponsiveViewportSnapshot::from_dimensions(720.0, 480.0);
    assert!(connection_rows_use_stacked_headline(&state));
}

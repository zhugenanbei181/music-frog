//! Behavior cases for overview card.
//! test-intent: behavior

use super::*;
use infiltrator_contract::overview_layout::OverviewCardKind;

#[test]
fn overview_card_reorder_actions_submit_commands() {
    let sink = Arc::new(DemoCommandSink::accepting());
    sink.submit(UiCommand::MoveOverviewCardUp(OverviewCardKind::Traffic));
    sink.submit(UiCommand::MoveOverviewCardDown(OverviewCardKind::Metrics));
    sink.submit(UiCommand::ResetOverviewCardOrder);

    let items = sink.submitted();
    assert_eq!(items.len(), 3);
    assert_eq!(
        items[0],
        UiCommand::MoveOverviewCardUp(OverviewCardKind::Traffic)
    );
    assert_eq!(
        items[1],
        UiCommand::MoveOverviewCardDown(OverviewCardKind::Metrics)
    );
    assert_eq!(items[2], UiCommand::ResetOverviewCardOrder);
}

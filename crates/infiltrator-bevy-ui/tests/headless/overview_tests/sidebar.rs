//! Behavior cases for sidebar.
//! test-intent: behavior

use super::*;

/// The foot names the data source: the demo milestone caption under the
/// fixture, the real core version under the live pump (and an honest
/// placeholder while the version has not been read yet).
#[test]
fn sidebar_foot_follows_the_source_kind() {
    let mut app = mounted_app_with(DemoOverviewSource::running());
    assert_eq!(
        foot_text(app.world_mut()),
        "演示数据 · 未接实时内核",
        "demo keeps its caption"
    );

    let mut app = mounted_app_with(LiveFootStub {
        version: Some("v1.19.18"),
    });
    assert_eq!(
        foot_text(app.world_mut()),
        "实时内核 · v1.19.18",
        "a live core names the version it reported"
    );

    let mut app = mounted_app_with(LiveFootStub { version: None });
    assert_eq!(
        foot_text(app.world_mut()),
        "实时内核 · 版本未观测",
        "an unread version stays honest"
    );
}

use super::*;

#[test]
fn overview_routing_intent_segment_renders_cleanly() {
    let state = AppState::empty();
    let lang = Lang("zh-CN");
    let _elem = overview_routing_intent_segment(&state, &lang);
}

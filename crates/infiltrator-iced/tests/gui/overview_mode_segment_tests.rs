use super::*;

#[test]
fn overview_mode_segment_renders_cleanly() {
    let state = AppState::empty();
    let lang = Lang("zh-CN");
    let _elem = overview_mode_segment(&state, &lang);
}

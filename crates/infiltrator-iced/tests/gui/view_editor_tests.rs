use super::*;

#[test]
fn test_format_short_sha() {
    assert_eq!(format_short_sha("1a2b3c4d5e6f7890"), "1a2b3c4d");
    assert_eq!(format_short_sha("short"), "short");
    assert_eq!(format_short_sha(""), "");
}

#[test]
fn test_format_syntax_line_pill() {
    assert_eq!(format_syntax_line_pill(10), "Line 10");
    assert_eq!(format_syntax_line_pill(1), "Line 1");
}

#[test]
fn test_editor_view_render_all_panes() {
    {
        let (mut state, _) = AppState::new();
        state.editor.editor_pane = EditorPane::Profile;
        let _v = view(&state);
    }
    {
        let (mut state, _) = AppState::new();
        state.editor.editor_pane = EditorPane::Mixin;
        let _v = view(&state);
    }
    {
        let (mut state, _) = AppState::new();
        state.editor.editor_pane = EditorPane::Filter;
        let _v = view(&state);
    }
}

#[test]
fn test_editor_view_with_syntax_error() {
    let (mut state, _) = AppState::new();
    state.editor.syntax_error = Some("Mapping values are not allowed here".into());
    state.editor.syntax_error_line = Some(14);
    let _v = view(&state);
}

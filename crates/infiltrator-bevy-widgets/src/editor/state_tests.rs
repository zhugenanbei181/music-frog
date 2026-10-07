//! test-intent: behavior
use super::CodeEditorState;

#[test]
fn unicode_graphemes_move_and_delete_as_complete_units_with_valid_byte_offsets() {
    let mut editor = CodeEditorState::new("中e\u{301}👨‍👩‍👧‍👦🇨🇳\n尾\n");
    assert_eq!(editor.full_text(), "中e\u{301}👨‍👩‍👧‍👦🇨🇳\n尾\n");
    assert!(editor.move_right());
    assert_eq!(editor.cursor_col, "中".len());
    assert!(editor.move_right());
    assert_eq!(editor.cursor_col, "中e\u{301}".len());
    editor.delete_forward();
    assert_eq!(editor.lines[0], "中e\u{301}🇨🇳");
    editor.delete_backwards();
    assert_eq!(editor.lines[0], "中🇨🇳");
    assert_eq!(editor.cursor_col, "中".len());
    editor.move_end();
    editor.delete_backwards();
    assert_eq!(editor.lines[0], "中");
    assert!(editor.undo());
    assert_eq!(editor.lines[0], "中🇨🇳");
    assert_eq!(editor.cursor_col, "中🇨🇳".len());
}
#[test]
fn replacement_paste_is_one_undo_transaction_and_restores_original_selection() {
    let mut editor = CodeEditorState::new("前\r\n后\r\n");
    editor.select_all();
    assert_eq!(editor.selected_text().as_deref(), Some("前\r\n后\r\n"));
    editor.insert_text("替换\n🙂\n");
    assert_eq!(editor.full_text(), "替换\n🙂\n");
    assert_eq!(editor.line_count(), 3);
    assert_eq!((editor.cursor_row, editor.cursor_col), (2, 0));
    assert_eq!(editor.undo_stack.len(), 1);
    assert!(editor.undo());
    assert_eq!(editor.full_text(), "前\r\n后\r\n");
    assert_eq!(editor.selected_text().as_deref(), Some("前\r\n后\r\n"));
    assert!(editor.redo());
    assert_eq!(editor.full_text(), "替换\n🙂\n");
}
#[test]
fn crlf_navigation_join_and_enter_preserve_line_endings_and_tail_bytes() {
    let mut editor = CodeEditorState::new("甲\r\n乙\r\n");
    editor.move_end();
    assert_eq!(editor.cursor_col, "甲".len());
    editor.move_right();
    assert_eq!((editor.cursor_row, editor.cursor_col), (1, 0));
    editor.delete_backwards();
    assert_eq!(editor.full_text(), "甲乙\r\n");
    assert!(editor.undo());
    assert_eq!(editor.full_text(), "甲\r\n乙\r\n");
    editor.insert_newline();
    assert_eq!(editor.full_text(), "甲\r\n\r\n乙\r\n");
    editor.set_text("\n");
    assert_eq!(editor.lines, ["", ""]);
    assert_eq!(CodeEditorState::default().lines, [""]);
}
#[test]
fn invalid_external_caret_positions_are_normalized_before_mutation() {
    let mut editor = CodeEditorState::new("👨‍👩‍👧‍👦字");
    editor.cursor_col = 5;
    editor.insert_text("A");
    assert_eq!(editor.full_text(), "A👨‍👩‍👧‍👦字");
    editor.cursor_row = usize::MAX;
    editor.cursor_col = usize::MAX;
    editor.insert_char('中');
    assert_eq!(editor.full_text(), "A👨‍👩‍👧‍👦字中");
}

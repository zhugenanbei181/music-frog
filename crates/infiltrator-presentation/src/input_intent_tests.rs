//! 统一导航意图与组合态保护闸门的单测。
//!
//! 验收点：中文输入法输入未完成时，模型不强行刷新文本框。

use super::*;
use infiltrator_contract::ime::{ImeCompositionAction, ImeCompositionEvent, ImeCompositionTracker};

#[test]
fn the_tab_keys_move_the_focus_and_enter_activates() {
    assert_eq!(key_intent(ShellKey::Tab), Some(KeyIntent::FocusNext));
    assert_eq!(
        key_intent(ShellKey::ShiftTab),
        Some(KeyIntent::FocusPrevious)
    );
    assert_eq!(key_intent(ShellKey::Enter), Some(KeyIntent::Activate));
    assert_eq!(key_intent(ShellKey::Escape), Some(KeyIntent::Dismiss));
}

#[test]
fn the_arrow_keys_move_in_their_own_direction() {
    assert_eq!(
        key_intent(ShellKey::ArrowUp),
        Some(KeyIntent::Scroll(ScrollDirection::Up))
    );
    assert_eq!(
        key_intent(ShellKey::ArrowDown),
        Some(KeyIntent::Scroll(ScrollDirection::Down))
    );
    assert_eq!(
        key_intent(ShellKey::ArrowLeft),
        Some(KeyIntent::Scroll(ScrollDirection::Left))
    );
    assert_eq!(
        key_intent(ShellKey::ArrowRight),
        Some(KeyIntent::Scroll(ScrollDirection::Right))
    );
}

#[test]
fn a_key_the_shell_does_not_react_to_has_no_intent() {
    assert_eq!(key_intent(ShellKey::Backspace), None);
    assert_eq!(key_intent(ShellKey::Character('a')), None);
    assert_eq!(key_intent(ShellKey::Character('你')), None);
}

#[test]
fn every_intent_is_produced_by_exactly_one_key() {
    let keys = [
        ShellKey::Tab,
        ShellKey::ShiftTab,
        ShellKey::Enter,
        ShellKey::Escape,
        ShellKey::ArrowUp,
        ShellKey::ArrowDown,
        ShellKey::ArrowLeft,
        ShellKey::ArrowRight,
    ];
    let produced: [Option<KeyIntent>; 8] = keys.map(key_intent);
    for intent in KeyIntent::ALL {
        let hits = produced
            .iter()
            .filter(|candidate| **candidate == Some(intent))
            .count();
        assert_eq!(
            hits, 1,
            "意图 {intent:?} 必须由恰好一个按键产生，不能有两个来源"
        );
    }
}

#[test]
fn only_the_directional_intents_carry_a_direction() {
    for intent in KeyIntent::ALL {
        match intent {
            KeyIntent::Scroll(direction) => {
                assert_eq!(intent.scroll_direction(), Some(direction));
                assert!(!intent.moves_focus());
            }
            KeyIntent::FocusNext | KeyIntent::FocusPrevious => {
                assert_eq!(intent.scroll_direction(), None);
                assert!(intent.moves_focus());
            }
            KeyIntent::Activate | KeyIntent::Dismiss => {
                assert_eq!(intent.scroll_direction(), None);
                assert!(!intent.moves_focus());
            }
        }
    }
}

#[test]
fn the_four_phases_answer_the_write_back_question() {
    assert!(FieldComposition::Idle.admits_snapshot());
    assert!(!FieldComposition::Composing.admits_snapshot());
    assert!(!FieldComposition::Committed.admits_snapshot());
    assert!(FieldComposition::Cancelled.admits_snapshot());

    assert!(FieldComposition::Composing.is_composing());
    assert!(!FieldComposition::Idle.is_composing());
    assert!(!FieldComposition::Committed.is_composing());

    assert!(FieldComposition::Committed.has_ended());
    assert!(FieldComposition::Cancelled.has_ended());
    assert!(!FieldComposition::Idle.has_ended());
    assert!(!FieldComposition::Composing.has_ended());
}

#[test]
fn a_composing_field_does_not_take_the_published_text() {
    let mut guard = CompositionGuard::new();
    assert_eq!(guard.phase(), FieldComposition::Idle);
    assert!(guard.admits_snapshot());
    assert_eq!(guard.admit(), SnapshotAdmission::Apply);

    guard.apply(&ImeCompositionAction::Begin);
    assert!(guard.is_composing());
    assert_eq!(guard.phase(), FieldComposition::Composing);
    assert!(!guard.admits_snapshot(), "输入法正在组合时，字段不属于模型");
    assert_eq!(
        guard.admit(),
        SnapshotAdmission::Suppress,
        "中文输入法输入未完成时，模型不能强行刷新文本框"
    );

    guard.apply(&ImeCompositionAction::UpdatePreedit("ni hao".to_owned()));
    assert!(guard.is_composing());
    assert_eq!(guard.admit(), SnapshotAdmission::Suppress);
    assert_eq!(
        guard.suppressed(),
        2,
        "被抑制的发布值必须被记下来，不能只靠界面现象推断"
    );
}

#[test]
fn a_committed_field_keeps_the_users_text_until_it_settles() {
    let mut guard = CompositionGuard::new();
    guard.apply(&ImeCompositionAction::Begin);
    guard.apply(&ImeCompositionAction::Commit("你好".to_owned()));
    assert_eq!(guard.phase(), FieldComposition::Committed);
    assert!(
        !guard.admits_snapshot(),
        "用户刚提交的文本比发布值新，发布值不能覆盖它"
    );
    assert_eq!(guard.admit(), SnapshotAdmission::Suppress);

    guard.settle();
    assert_eq!(guard.phase(), FieldComposition::Idle);
    assert!(guard.admits_snapshot());
    assert_eq!(guard.admit(), SnapshotAdmission::Apply);
}

#[test]
fn a_cancelled_field_takes_the_published_text_again() {
    let mut guard = CompositionGuard::new();
    guard.apply(&ImeCompositionAction::Begin);
    guard.apply(&ImeCompositionAction::Cancel);
    assert_eq!(guard.phase(), FieldComposition::Cancelled);
    assert!(
        guard.admits_snapshot(),
        "组合被取消后字段回滚到模型的文本，发布值又可以写进去"
    );
    assert_eq!(guard.admit(), SnapshotAdmission::Apply);

    guard.settle();
    assert_eq!(guard.phase(), FieldComposition::Idle);
    assert_eq!(guard.admit(), SnapshotAdmission::Apply);
}

#[test]
fn a_commit_without_a_begin_still_reaches_the_committed_phase() {
    let mut guard = CompositionGuard::new();
    guard.apply(&ImeCompositionAction::Commit("好".to_owned()));
    assert_eq!(guard.phase(), FieldComposition::Committed);
    assert!(!guard.admits_snapshot());
}

#[test]
fn a_new_composition_starts_over_from_a_settled_field() {
    let mut guard = CompositionGuard::new();
    guard.apply(&ImeCompositionAction::Begin);
    guard.apply(&ImeCompositionAction::Commit("你好".to_owned()));
    guard.settle();
    assert_eq!(guard.admit(), SnapshotAdmission::Apply);

    guard.apply(&ImeCompositionAction::Begin);
    assert_eq!(guard.phase(), FieldComposition::Composing);
    assert_eq!(guard.admit(), SnapshotAdmission::Suppress);
}

#[test]
fn a_surface_feeds_the_shared_tracker_and_the_guard_the_same_action() {
    // 表面已经有一套共享的输入法会话：事件先过会话，再把同一份动作交给
    // 字段和闸门。闸门因此不需要自己再造一套会话词汇。
    let mut tracker = ImeCompositionTracker::new();
    let mut guard = CompositionGuard::new();

    let action = tracker.apply(ImeCompositionEvent::Opened);
    guard.apply(&action);
    assert!(guard.is_composing());
    assert!(
        !guard.admits_snapshot(),
        "输入法刚开始组合，模型就不能刷新文本框"
    );

    let action = tracker.apply(ImeCompositionEvent::Preedit("ni hao".to_owned()));
    guard.apply(&action);
    assert_eq!(guard.admit(), SnapshotAdmission::Suppress);

    let action = tracker.apply(ImeCompositionEvent::Commit("你好".to_owned()));
    guard.apply(&action);
    assert_eq!(guard.phase(), FieldComposition::Committed);
    assert_eq!(guard.admit(), SnapshotAdmission::Suppress);

    guard.settle();
    assert_eq!(guard.admit(), SnapshotAdmission::Apply);
}

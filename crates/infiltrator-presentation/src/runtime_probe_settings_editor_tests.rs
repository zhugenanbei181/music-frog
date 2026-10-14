use crate::runtime_probe_settings_editor::{
    RuntimeProbeSettingsEditor, RuntimeProbeSettingsWriteOutcome,
};
use infiltrator_contract::error::Failure;
use infiltrator_contract::runtime_settings::{RuntimeProbeSettings, RuntimeProbeSettingsState};

fn available(settings: RuntimeProbeSettings) -> RuntimeProbeSettingsState {
    RuntimeProbeSettingsState::Available { settings }
}

#[test]
fn invalid_draft_and_stale_write_receipt_never_change_the_applied_value() {
    let original = RuntimeProbeSettings::default();
    let mut editor = RuntimeProbeSettingsEditor::default();
    editor.observe(&available(original.clone()));
    editor.edit_timeout("0".into());
    assert!(editor.validation().is_some());
    assert!(!editor.can_apply());
    assert_eq!(editor.applied(), Some(&original));

    editor.cancel();
    editor.edit_url("https://probe.example.test/check".into());
    let pending = editor.begin().unwrap();
    assert!(editor.begin().is_err());
    assert_eq!(
        editor.finish(pending.token + 1, Ok(())),
        RuntimeProbeSettingsWriteOutcome::Stale
    );
    assert_eq!(editor.applied(), Some(&original));
    assert_eq!(
        editor.finish(pending.token, Err(Failure::unsupported("write denied"))),
        RuntimeProbeSettingsWriteOutcome::Failed
    );
    assert_eq!(editor.applied(), Some(&original));
    assert!(editor.failure().is_some());
}

#[test]
fn successful_write_waits_for_matching_owner_readback_before_becoming_ready() {
    let mut editor = RuntimeProbeSettingsEditor::default();
    editor.observe(&available(RuntimeProbeSettings::default()));
    editor.edit_url("https://probe.example.test/check".into());
    let pending = editor.begin().unwrap();
    assert_eq!(
        editor.finish(pending.token, Ok(())),
        RuntimeProbeSettingsWriteOutcome::Saved
    );
    assert!(editor.is_pending());
    editor.observe(&available(RuntimeProbeSettings::default()));
    assert!(
        editor.is_pending(),
        "a stale snapshot cannot undo the write"
    );
    editor.observe(&available(pending.settings.clone()));
    assert!(!editor.is_pending());
    assert_eq!(editor.applied(), Some(&pending.settings));
}

#[test]
fn unavailable_state_preserves_draft_as_a_distinct_typed_state_and_recovers_for_retry() {
    let original = RuntimeProbeSettings::default();
    let mut editor = RuntimeProbeSettingsEditor::default();
    editor.observe(&available(original.clone()));
    editor.edit_url("https://probe.example.test/check".into());
    let draft = editor.draft().cloned().unwrap();

    editor.observe(&RuntimeProbeSettingsState::Unavailable {
        failure: Failure::unsupported("settings store temporarily unavailable"),
    });
    assert!(matches!(
        &editor.state,
        super::EditorState::UnavailableWithCheckpoint { .. }
    ));
    assert_eq!(editor.draft(), Some(&draft));
    assert!(editor.begin().is_err());

    editor.observe(&available(original.clone()));
    assert!(matches!(&editor.state, super::EditorState::Editing { .. }));
    assert_eq!(editor.draft(), Some(&draft));
    let pending = editor.begin().unwrap();
    assert!(matches!(&editor.state, super::EditorState::Saving { .. }));

    assert_eq!(
        editor.finish(pending.token, Err(Failure::unsupported("write denied"))),
        RuntimeProbeSettingsWriteOutcome::Failed
    );
    assert!(matches!(
        &editor.state,
        super::EditorState::WriteFailed { .. }
    ));
    assert!(editor.can_apply());

    let retry = editor.begin().unwrap();
    assert_eq!(retry.settings, pending.settings);
    assert_eq!(
        editor.finish(retry.token, Ok(())),
        RuntimeProbeSettingsWriteOutcome::Saved
    );
    assert!(matches!(
        &editor.state,
        super::EditorState::AwaitingReadback { .. }
    ));
}

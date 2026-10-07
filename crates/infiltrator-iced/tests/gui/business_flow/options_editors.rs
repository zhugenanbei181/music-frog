//! Journeys 4/5 — the Editor three-pane mixin editor and the per-profile
//! subscription-filter editor, driven through the routed message surface
//! with the real sidecar/merge/pipeline product functions executed for the
//! async legs, verified against the temp-HOME filesystem.
//!
//! test-intent: behavior
use crate::test_mounts::command_harness::recording_application;
use crate::test_mounts::profile_edit_fixture;
use crate::test_mounts::script_workbench_tests::complete;
use crate::types::script::ScriptAction;
use infiltrator_application::command_application::CommandApplication;
use infiltrator_application::script_application::ScriptApplication;
use infiltrator_domain::filter_policy_form;

use super::support::{SAMPLE_PROFILE_YAML, TempHome, block_on, feed, fresh_state, last_toast};
use crate::configs_dir::config_manager;
use crate::types::app::Route;
use crate::types::message::Message;
use crate::types::options::EditorPane;
use crate::view::mixin_studio::{cascade_strip, preflight_banner, three_column_row, toggle_row};
use crate::view::script_export::export_section;
use futures_util::StreamExt;
use iced::Task;
use iced::widget::text_editor::{Action, Edit};
use iced_runtime::task::into_stream;
use infiltrator_application::core_application::CoreApplication;
use infiltrator_application::language_choice_fixtures::LanguageCaptureProcess;
use infiltrator_application::profile_application::ProfileApplication;
use infiltrator_application::profile_options_application::ProfileOptionsApplication;
use infiltrator_application::script_export_application::ScriptExportApplication;
use infiltrator_composition::tokio_application_runtime;
use infiltrator_contract::error::ErrorCode;
use infiltrator_contract::subscription_filter_form::FilterObservation;
use infiltrator_desktop::script_export::DesktopScriptExportPort;
use infiltrator_domain::filter::NodeSortOrder;
use infiltrator_domain::mixin_studio::mixin_editor_columns;
use mihomo_config::profile_option_store;
use std::fs::read_to_string;
use std::sync::Arc;

fn filter_terminal(task: Task<Message>) -> Message {
    block_on(async {
        let mut stream = into_stream(task).expect("production filter command task");
        let Some(iced_runtime::Action::Output(message)) = stream.next().await else {
            panic!("filter command must produce a terminal message");
        };
        assert!(stream.next().await.is_none());
        message
    })
}

/// Journey 4 — Editor 三 pane：打开 profile → Mixin pane 懒加载 → 编辑 →
/// 非法 YAML 被校验门拒绝 → 修正 → 保存 → mixin 合并落盘 + options sidecar。
#[test]
fn mixin_three_pane_journey_rejects_bad_yaml_then_persists_sidecar_and_merge() {
    let home = TempHome::acquire("mixin-panes");
    home.seed_profile("alpha", SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();

    let store = block_on(config_manager()).unwrap();
    let profiles = ProfileApplication::new(store.clone());
    let commands = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    );
    commands.install_command_handler(Arc::new(CommandApplication::new().with_profile(profiles)));
    state.commands = Some(commands);
    // Native TEA load and save tasks execute the actual application and store boundary.
    let task = state.update(Message::EditProfile(profile_path.clone()));
    let reply = filter_terminal(task);
    feed(&mut state, reply);
    feed(&mut state, Message::Navigate(Route::Editor));
    assert_eq!(
        state.editor.editor_path.as_deref(),
        Some(profile_path.as_path())
    );
    assert_eq!(state.shell.current_route, Route::Editor);
    assert_eq!(state.editor.editor_pane, EditorPane::Profile);
    let task = state.update(Message::SetEditorPane(EditorPane::Mixin));
    let reply = filter_terminal(task);
    feed(&mut state, reply);
    assert_eq!(state.editor.mixin_loaded_for.as_deref(), Some("alpha"));
    feed(&mut state, Message::MixinEditorAction(Action::SelectAll));
    feed(
        &mut state,
        Message::MixinEditorAction(Action::Edit(Edit::Paste(Arc::new(
            "log-level: [broken".into(),
        )))),
    );
    let units = feed(&mut state, Message::SaveMixin);
    assert_eq!(
        units, 1,
        "preflight rejection exposes the validation failure"
    );
    assert!(!state.editor.is_saving_mixin);
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or_default()
            .contains("Mixin")
    );
    assert!(
        !home.configs().join("options/alpha.yaml").exists(),
        "rejected draft never writes a sidecar"
    );
    feed(&mut state, Message::MixinEditorAction(Action::SelectAll));
    feed(
        &mut state,
        Message::MixinEditorAction(Action::Edit(Edit::Paste(Arc::new("mode: global\n".into())))),
    );
    let task = state.update(Message::SaveMixin);
    assert!(state.editor.is_saving_mixin);
    let before_wait = state.editor.mixin_content.text();
    feed(
        &mut state,
        Message::MixinEditorAction(Action::Edit(Edit::Insert('x'))),
    );
    assert_eq!(
        state.editor.mixin_content.text(),
        before_wait,
        "the pending transaction freezes its reviewed input"
    );
    let reply = filter_terminal(task);
    let units = feed(&mut state, reply);
    assert!(!state.editor.is_saving_mixin);
    assert!(state.editor.mixin_session.saved);
    assert!(state.editor.mixin_session.failure.is_none());
    assert_eq!(
        units, 1,
        "only history refresh follows the actual save receipt"
    );
    assert!(state.editor.editor_content.text().contains("mode: global"));

    // Disk truth: the merged document AND the sidecar round-trip.
    let on_disk = read_to_string(home.configs().join("alpha.yaml")).unwrap();
    assert!(
        on_disk.contains("mode: global"),
        "mixin merged into profile: {on_disk}"
    );
    assert!(on_disk.contains("HK-1"), "proxies survive the merge");
    let sidecar = block_on(profile_option_store::load_options(&home.configs(), "alpha")).unwrap();
    assert_eq!(
        sidecar.mixin.mode.as_deref(),
        Some("global"),
        "sidecar persisted"
    );
}

/// Journey 5 — Filter pane：LoadProfileFilter → include/exclude 编辑 →
/// 保存（过滤管道真实执行）→ sidecar 落盘 → 重开回读。
#[test]
fn filter_pane_journey_persists_sidecar_and_filters_proxies_on_disk() {
    let home = TempHome::acquire("filter-panes");
    home.seed_profile("alpha", SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();
    let commands = CoreApplication::new(
        Arc::new(LanguageCaptureProcess),
        Arc::new(LanguageCaptureProcess),
        tokio_application_runtime().unwrap(),
    );
    commands.install_command_handler(Arc::new(
        CommandApplication::new()
            .with_profile(ProfileApplication::new(block_on(config_manager()).unwrap())),
    ));
    state.commands = Some(commands);

    let reply =
        profile_edit_fixture::document(&mut state, profile_path, SAMPLE_PROFILE_YAML.into());
    feed(&mut state, reply);
    let read = state.update(Message::LoadProfileFilter);
    assert_eq!(read.units(), 1, "lazy filter load armed");
    assert_eq!(state.editor.filter_load.as_ref().unwrap().1, "alpha");
    let loaded = filter_terminal(read);
    assert!(
        matches!(&loaded, Message::ProfileFilterLoaded { profile, result: Ok(_), .. } if profile == "alpha")
    );
    let _ = state.update(loaded);

    // User edits the draft.
    feed(&mut state, Message::UpdateFilterInclude("HK".into()));
    feed(&mut state, Message::UpdateFilterExclude("US".into()));

    // A malformed rename is caught by the synchronous compile gate.
    feed(
        &mut state,
        Message::UpdateFilterRenames("没有箭头的规则".into()),
    );
    let units = feed(&mut state, Message::SaveProfileFilter);
    assert_eq!(
        units, 0,
        "invalid filter remains in the inline form without a persistence task"
    );
    assert_eq!(
        state.editor.filter_editor.failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    assert!(!state.editor.filter_editor.pending.is_some());

    // Valid draft → save arms the task; run its body for real.
    feed(&mut state, Message::UpdateFilterRenames(String::new()));
    feed(
        &mut state,
        Message::UpdateFilterAdvancedPolicy("{remove-emojis-typo: true}".into()),
    );
    assert_eq!(feed(&mut state, Message::SaveProfileFilter), 0);
    assert_eq!(
        state.editor.filter_editor.failure.as_ref().unwrap().code,
        ErrorCode::InvalidInput
    );
    assert!(state.editor.filter_editor.pending.is_none());
    feed(
        &mut state,
        Message::UpdateFilterAdvancedPolicy("{remove-emojis: true, sort-by: name-desc}".into()),
    );
    let task = state.update(Message::SaveProfileFilter);
    assert!(state.editor.filter_editor.pending.is_some());
    assert_eq!(task.units(), 1, "single persistence task armed");

    let pending = state.editor.filter_editor.pending.clone().unwrap();
    let terminal = filter_terminal(task);
    let Message::ProfileFilterSaved {
        token,
        result: Ok(report),
    } = terminal
    else {
        panic!("filter command must return an applied source and report");
    };
    assert_eq!(token, pending.token);
    assert_eq!(
        report.report.total_input, 3,
        "three seed proxies entered the pipeline"
    );
    assert_eq!(
        report.source,
        block_on(async {
            config_manager()
                .await
                .unwrap()
                .load_workspace("alpha")
                .await
                .unwrap()
                .source
        })
    );
    feed(
        &mut state,
        Message::ProfileFilterSaved {
            token: pending.token,
            result: Ok(report),
        },
    );
    assert!(state.editor.filter_editor.pending.is_none());

    // Disk truth: the pipeline changes proxies, preserves the sidecar and can reopen.
    let on_disk = read_to_string(home.configs().join("alpha.yaml")).unwrap();
    let kept_names: Vec<String> = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&on_disk)
        .unwrap()
        .get("proxies")
        .and_then(|value| value.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|proxy| {
                    proxy
                        .get("name")
                        .and_then(|name| name.as_str())
                        .map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        kept_names,
        vec!["HK-1".to_string()],
        "whitelist applied: {kept_names:?}"
    );
    let sidecar = block_on(profile_option_store::load_options(&home.configs(), "alpha")).unwrap();
    let stored_spec = sidecar.filter.expect("filter spec persisted");
    assert_eq!(stored_spec.include_keywords, vec!["HK".to_string()]);
    assert!(stored_spec.remove_emojis);
    assert_eq!(stored_spec.sort_by, NodeSortOrder::NameDesc);
    let reopened = filter_policy_form::filter_spec_to_draft(&stored_spec).unwrap();
    feed(&mut state, Message::LoadProfileFilter);
    let token = state.editor.filter_load.as_ref().unwrap().0;
    let snapshot = block_on(async {
        ProfileOptionsApplication::new(ProfileApplication::new(config_manager().await.unwrap()))
            .load(Some("alpha"))
            .await
            .unwrap()
    });
    assert_eq!(snapshot.filter, reopened);
    feed(
        &mut state,
        Message::ProfileFilterLoaded {
            token,
            profile: "alpha".into(),
            result: Ok(FilterObservation {
                source: snapshot.source,
                filter: snapshot.filter,
            }),
        },
    );
    assert_eq!(state.editor.filter_editor.draft.include, "HK");
    assert_eq!(state.editor.filter_editor.draft.exclude, "US");
}

/// An unbound filter form cannot submit, change pending state or write.
#[test]
fn filter_save_without_an_open_profile_is_an_inert_noop() {
    let mut state = fresh_state();
    state.editor.filter_editor.draft.include = "HK".into();
    state.editor.filter_editor.draft.renames = String::new();
    let units = feed(&mut state, Message::SaveProfileFilter);
    assert_eq!(units, 0, "no editor_path → nothing to save");
    assert!(state.editor.filter_editor.pending.is_none());
    assert!(last_toast(&state).is_none());
}

/// DUAL-10-10/11/08 — the Iced Mixin pane consumes the shared studio: the
/// preset toggles flip the buffer through the real codec, the preflight
/// banner renders the shared verdict, the cascade strip renders the shared
/// stage report, and a broken overlay is blocked before any write.
#[test]
fn mixin_studio_toggles_preflight_and_cascade_ride_the_shared_module() {
    let home = TempHome::acquire("mixin-studio");
    home.seed_profile("alpha", SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();
    let reply =
        profile_edit_fixture::document(&mut state, profile_path, SAMPLE_PROFILE_YAML.into());
    feed(&mut state, reply);
    feed(&mut state, Message::Navigate(Route::Editor));
    feed(&mut state, Message::SetEditorPane(EditorPane::Mixin));
    let reply = profile_edit_fixture::options(&mut state, "{}\n".into());
    feed(&mut state, reply);

    // DUAL-10-11: toggling flips a real MixinConfig field, not a text splice.
    feed(&mut state, Message::ToggleMixinPreset("ipv6".into(), true));
    assert!(
        state.editor.mixin_content.text().contains("ipv6: true"),
        "toggle wrote the real field: {}",
        state.editor.mixin_content.text()
    );
    feed(
        &mut state,
        Message::ToggleMixinPreset("dns-fake-ip".into(), true),
    );
    assert!(state.editor.mixin_content.text().contains("fake-ip"));
    assert!(
        state.editor.syntax_error.is_none(),
        "valid overlay preflight"
    );

    // DUAL-10-10/08: the pane builds the shared preflight banner and the real
    // cascade strip over the open document.
    {
        let _banner = preflight_banner(&state);
        let _strip = cascade_strip(&state);
        let _chips = toggle_row(&state);
        // DUAL-10-09: the three-column row compiles and reads the shared
        // reduction over the open base document.
        let _columns = three_column_row(&state);
    }
    let columns = mixin_editor_columns(
        &state.editor.editor_content.text(),
        &state.editor.mixin_content.text(),
    );
    assert_eq!(columns.base.content, state.editor.editor_content.text());
    assert!(columns.is_composed());
    assert!(columns.composed.content.contains("ipv6: true"));
    assert!(
        columns.composed.content.contains("HK-1"),
        "the composed column carries the real base document content"
    );

    // A broken overlay is blocked by the shared preflight before any write.
    feed(&mut state, Message::MixinEditorAction(Action::SelectAll));
    feed(
        &mut state,
        Message::MixinEditorAction(Action::Edit(Edit::Paste(Arc::new(
            "mode: [unterminated\n".into(),
        )))),
    );
    let units = feed(&mut state, Message::SaveMixin);
    assert_eq!(units, 1, "gate rejection arms only the error toast");
    assert!(!state.editor.is_saving_mixin);
    assert!(
        !home.configs().join("options/alpha.yaml").exists(),
        "blocked overlay must not write a sidecar"
    );
    // DUAL-10-09: a blocked overlay empties the composed column with the real
    // reason instead of a mock document.
    let blocked = mixin_editor_columns(
        &state.editor.editor_content.text(),
        &state.editor.mixin_content.text(),
    );
    assert!(blocked.is_blocked());
    assert!(blocked.composed.content.is_empty());
}

/// DUAL-10-12 — the Mixin pane export is a real file: the routed message arms
/// the shared use-case, the desktop host adapter writes the overlay YAML into
/// the host-owned exports directory, and the result message publishes the
/// shared projection both surfaces render. A host without a save-file adapter
/// answers a typed unsupported outcome instead of a fake path.
#[test]
fn mixin_export_writes_a_real_yaml_file_and_reports_the_host_outcome() {
    use infiltrator_contract::script_export::{ScriptExportKind, ScriptExportOutcome};
    use std::sync::Arc;

    let home = TempHome::acquire("mixin-export");
    home.seed_profile("alpha", SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();
    let reply =
        profile_edit_fixture::document(&mut state, profile_path, SAMPLE_PROFILE_YAML.into());
    feed(&mut state, reply);
    feed(&mut state, Message::SetEditorPane(EditorPane::Mixin));
    let reply = profile_edit_fixture::options(&mut state, "ipv6: true\n".into());
    feed(&mut state, reply);

    let exports =
        ScriptExportApplication::new(Some(Arc::new(DesktopScriptExportPort::new(home.configs()))));
    let (commands, _) = recording_application();
    commands.install_command_handler(Arc::new(
        CommandApplication::new().with_scripts(ScriptApplication::new(), exports.clone()),
    ));
    state.commands = Some(commands);
    let task = state.update(Message::Script(ScriptAction::Export(
        ScriptExportKind::MixinOverlayYaml,
    )));
    assert_eq!(task.units(), 1, "review task is armed");
    complete(&mut state, task);
    assert!(
        !home.configs().join("exports/alpha.mixin.yaml").exists(),
        "review does not write"
    );
    let task = state.update(Message::Script(ScriptAction::ConfirmExport));
    complete(&mut state, task);
    let snapshot = exports.snapshot().expect("actual export result");
    let exported_path = home.configs().join("exports/alpha.mixin.yaml");
    assert!(
        exported_path.exists(),
        "the host wrote a real overlay export at {exported_path:?}"
    );
    let written = read_to_string(&exported_path).expect("read export");
    assert!(written.contains("ipv6: true"));
    assert!(written.contains("非 JavaScript"));
    match &snapshot.outcome {
        ScriptExportOutcome::Saved {
            path,
            bytes_written,
        } => {
            assert_eq!(path, &exported_path.to_string_lossy());
            assert_eq!(*bytes_written, written.len());
        }
        other => panic!("expected a saved outcome, got {other:?}"),
    }

    // The result message clears the busy flag and publishes the shared fact.
    assert!(!state.editor.script_sandbox.is_exporting());
    assert_eq!(state.editor.script_sandbox.export.as_ref(), Some(&snapshot));
    let _panel = export_section(&state, &[ScriptExportKind::MixinOverlayYaml]);

    // A host with no save-file adapter is a typed unsupported, not a fake path.
    let hostless = ScriptExportApplication::without_host_port()
        .export_directive_dsl(
            Some("alpha"),
            "function main(config) { return config; }",
            None,
        )
        .expect("compose for a hostless export");
    assert!(hostless.outcome.is_unsupported());
    assert!(hostless.content.contains("不是 JavaScript"));
}

//! Journeys 4/5 — the Editor three-pane mixin editor and the per-profile
//! subscription-filter editor, driven through the routed message surface
//! with the real sidecar/merge/pipeline product functions executed for the
//! async legs, verified against the temp-HOME filesystem.
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, last_toast};
use crate::types::message::Message;
use crate::types::options::EditorPane;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_core::profile_options_io;
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::profile_options::{self, ProfileOptions};

/// Journey 4 — Editor 三 pane：打开 profile → Mixin pane 懒加载 → 编辑 →
/// 非法 YAML 被校验门拒绝 → 修正 → 保存 → mixin 合并落盘 + options sidecar。
#[test]
fn mixin_three_pane_journey_rejects_bad_yaml_then_persists_sidecar_and_merge() {
    let home = TempHome::acquire("mixin-panes");
    home.seed_profile("alpha", super::support::SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();

    // Profiles page → open editor (EditProfile routes; the async read is fed
    // back as its result message, exactly what the runtime would deliver).
    let units = feed(&mut state, Message::EditProfile(profile_path.clone()));
    assert_eq!(units, 1);
    feed(
        &mut state,
        Message::ProfileContentLoaded(Ok((
            profile_path.clone(),
            super::support::SAMPLE_PROFILE_YAML.into(),
        ))),
    );
    // ProfileContentLoaded chains Navigate(Editor) as a lazy done-task; the
    // runtime delivers it on the next hop — replay that hop here.
    feed(
        &mut state,
        Message::Navigate(crate::types::app::Route::Editor),
    );
    assert_eq!(
        state.editor.editor_path.as_deref(),
        Some(profile_path.as_path())
    );
    assert_eq!(state.shell.current_route, crate::types::app::Route::Editor);
    assert_eq!(state.editor.editor_pane, EditorPane::Profile);

    // Switch to the Mixin pane: lazy load arms (task dropped) → the loaded
    // overlay 回灌 fills the editor.
    let units = feed(&mut state, Message::SetEditorPane(EditorPane::Mixin));
    assert_eq!(units, 1, "lazy overlay load armed");
    assert_eq!(state.editor.mixin_loaded_for.as_deref(), Some("alpha"));
    feed(
        &mut state,
        Message::MixinLoaded(Ok("log-level: silent\n".into())),
    );
    assert_eq!(state.editor.mixin_content.text(), "log-level: silent\n");

    // Typing flows through the editor action; a broken overlay is rejected
    // by the synchronous validation gate before any state or task moves.
    for ch in "log-level: [broken".chars() {
        feed(
            &mut state,
            Message::MixinEditorAction(iced::widget::text_editor::Action::Edit(
                iced::widget::text_editor::Edit::Insert(ch),
            )),
        );
    }
    let units = feed(&mut state, Message::SaveMixin);
    assert_eq!(units, 1, "gate rejection arms only the error toast");
    assert!(!state.editor.is_saving_mixin);
    assert!(
        state
            .shell
            .error_msg
            .as_deref()
            .unwrap_or("")
            .contains("Mixin"),
        "validation error surfaces through the single sink"
    );
    assert!(
        !home.configs().join("options/alpha.yaml").exists(),
        "rejected overlay must not write a sidecar"
    );

    // Fix the overlay (fresh load 回灌 instead of retyping), then save.
    feed(
        &mut state,
        Message::MixinLoaded(Ok("mode: global\n".into())),
    );
    let units = feed(&mut state, Message::SaveMixin);
    assert!(state.editor.is_saving_mixin, "gate passed → task in flight");
    assert_eq!(units, 1, "single persistence task armed");

    // Task body for real: load old options → strip old mixin rules → merge →
    // validate → commit profile → persist sidecar.
    let mixin: MixinConfig = serde_yaml_ng::from_str("mode: global\n").unwrap();
    block_on(async {
        let config_dir = crate::configs_dir::configs_dir().await.unwrap();
        let old = profile_options_io::load_options(&config_dir, "alpha")
            .await
            .unwrap();
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let content = manager.load("alpha").await.unwrap();
        let removals: Vec<String> = old
            .mixin
            .rules
            .iter()
            .flat_map(|rules| rules.prepend.iter().chain(rules.append.iter()).cloned())
            .collect();
        let base = profile_options::strip_rule_lines(&content, &removals);
        let merged = infiltrator_domain::mixin::merge_profile_with_config(&base, &mixin).unwrap();
        infiltrator_domain::config::validate_yaml(&merged).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            merged,
            infiltrator_domain::apply::ApplyStrategy::PreferReload,
        )
        .await
        .unwrap();
        profile_options_io::save_options(
            &config_dir,
            "alpha",
            &ProfileOptions {
                mixin,
                filter: old.filter,
            },
        )
        .await
        .unwrap();
    });

    let units = feed(&mut state, Message::MixinSaved(Ok(())));
    assert!(!state.editor.is_saving_mixin);
    assert!(units >= 2, "snapshots + editor reload + toast chained");

    // Disk truth: the merged document AND the sidecar round-trip.
    let on_disk = std::fs::read_to_string(home.configs().join("alpha.yaml")).unwrap();
    assert!(
        on_disk.contains("mode: global"),
        "mixin merged into profile: {on_disk}"
    );
    assert!(on_disk.contains("HK-1"), "proxies survive the merge");
    let sidecar = block_on(profile_options_io::load_options(&home.configs(), "alpha")).unwrap();
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
    home.seed_profile("alpha", super::support::SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();

    feed(
        &mut state,
        Message::ProfileContentLoaded(Ok((
            profile_path,
            super::support::SAMPLE_PROFILE_YAML.into(),
        ))),
    );
    let units = feed(&mut state, Message::LoadProfileFilter);
    assert_eq!(units, 1, "lazy filter load armed");
    assert_eq!(state.editor.filter_loaded_for.as_deref(), Some("alpha"));

    // Empty store → default draft 回灌.
    feed(
        &mut state,
        Message::ProfileFilterLoaded(Ok(SubscriptionFilterDraft::default())),
    );

    // User edits the draft.
    feed(&mut state, Message::UpdateFilterInclude("HK".into()));
    feed(&mut state, Message::UpdateFilterExclude("US".into()));

    // A malformed rename is caught by the synchronous compile gate.
    feed(
        &mut state,
        Message::UpdateFilterRenames("没有箭头的规则".into()),
    );
    let units = feed(&mut state, Message::SaveProfileFilter);
    assert_eq!(units, 1, "compile gate arms only the error toast");
    assert!(!state.editor.is_saving_filter);

    // Valid draft → save arms the task; run its body for real.
    feed(&mut state, Message::UpdateFilterRenames(String::new()));
    let units = feed(&mut state, Message::SaveProfileFilter);
    assert!(state.editor.is_saving_filter);
    assert_eq!(units, 1, "single persistence task armed");

    let spec =
        infiltrator_domain::profile_options::filter_spec_from_draft(&state.editor.filter_draft)
            .unwrap();
    let report = block_on(async {
        let rule = spec.to_rule().unwrap();
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let content = manager.load("alpha").await.unwrap();
        let (filtered, report) = infiltrator_domain::filter::SubscriptionFilterPipeline::new(rule)
            .apply_to_yaml(&content)
            .unwrap();
        infiltrator_domain::config::validate_yaml(&filtered).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            filtered,
            infiltrator_domain::apply::ApplyStrategy::PreferReload,
        )
        .await
        .unwrap();
        let config_dir = crate::configs_dir::configs_dir().await.unwrap();
        let old = profile_options_io::load_options(&config_dir, "alpha")
            .await
            .unwrap();
        profile_options_io::save_options(
            &config_dir,
            "alpha",
            &ProfileOptions {
                mixin: old.mixin,
                filter: Some(spec.clone()),
            },
        )
        .await
        .unwrap();
        report
    });

    assert_eq!(
        report.total_input, 3,
        "three seed proxies entered the pipeline"
    );
    feed(&mut state, Message::ProfileFilterSaved(Ok(report)));
    assert!(!state.editor.is_saving_filter);

    // Disk truth: the proxies list keeps exactly the nodes that passed (the
    // pipeline filters the `proxies` sequence, not group references), the
    // sidecar stores the compiled spec, and reopening the pane reads it back.
    let on_disk = std::fs::read_to_string(home.configs().join("alpha.yaml")).unwrap();
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
    let sidecar = block_on(profile_options_io::load_options(&home.configs(), "alpha")).unwrap();
    let stored_spec = sidecar.filter.expect("filter spec persisted");
    assert_eq!(stored_spec.include_keywords, vec!["HK".to_string()]);

    // 重开回读: the lazy loader would compile the spec back into a draft.
    let reopened = infiltrator_domain::profile_options::filter_spec_to_draft(&stored_spec);
    feed(&mut state, Message::ProfileFilterLoaded(Ok(reopened)));
    assert_eq!(state.editor.filter_draft.include, "HK");
    assert_eq!(state.editor.filter_draft.exclude, "US");
}

/// Compile-gate negative: the filter editor bound to no profile is a no-op
/// (no task, no flag, no write).
#[test]
fn filter_save_without_an_open_profile_is_an_inert_noop() {
    let mut state = fresh_state();
    state.editor.filter_draft.include = "HK".into();
    state.editor.filter_draft.renames = String::new();

    let units = feed(&mut state, Message::SaveProfileFilter);
    assert_eq!(units, 0, "no editor_path → nothing to save");
    assert!(!state.editor.is_saving_filter);
    assert!(last_toast(&state).is_none());
}

/// DUAL-10-10/11/08 — the Iced Mixin pane consumes the shared studio: the
/// preset toggles flip the buffer through the real codec, the preflight
/// banner renders the shared verdict, the cascade strip renders the shared
/// stage report, and a broken overlay is blocked before any write.
#[test]
fn mixin_studio_toggles_preflight_and_cascade_ride_the_shared_module() {
    let home = TempHome::acquire("mixin-studio");
    home.seed_profile("alpha", super::support::SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();
    feed(
        &mut state,
        Message::ProfileContentLoaded(Ok((
            profile_path,
            super::support::SAMPLE_PROFILE_YAML.into(),
        ))),
    );
    feed(
        &mut state,
        Message::Navigate(crate::types::app::Route::Editor),
    );
    feed(&mut state, Message::SetEditorPane(EditorPane::Mixin));
    feed(&mut state, Message::MixinLoaded(Ok("{}\n".into())));

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
        let _banner = crate::view::mixin_studio::preflight_banner(&state);
        let _strip = crate::view::mixin_studio::cascade_strip(&state);
        let _chips = crate::view::mixin_studio::toggle_row(&state);
        // DUAL-10-09: the three-column row compiles and reads the shared
        // reduction over the open base document.
        let _columns = crate::view::mixin_studio::three_column_row(&state);
    }
    let columns = infiltrator_domain::mixin_studio::mixin_editor_columns(
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
    feed(
        &mut state,
        Message::MixinLoaded(Ok("mode: [unterminated\n".into())),
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
    let blocked = infiltrator_domain::mixin_studio::mixin_editor_columns(
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
    home.seed_profile("alpha", super::support::SAMPLE_PROFILE_YAML);
    let profile_path = home.configs().join("alpha.yaml");
    let mut state = fresh_state();
    feed(
        &mut state,
        Message::ProfileContentLoaded(Ok((
            profile_path,
            super::support::SAMPLE_PROFILE_YAML.into(),
        ))),
    );
    feed(&mut state, Message::SetEditorPane(EditorPane::Mixin));
    feed(&mut state, Message::MixinLoaded(Ok("ipv6: true\n".into())));

    // The user action arms the shared export task (no local composition).
    let units = feed(
        &mut state,
        Message::ExportScriptDraft(ScriptExportKind::MixinOverlayYaml),
    );
    assert_eq!(units, 1, "the export task is armed");
    assert!(state.editor.script_sandbox.is_exporting);

    // The same task body the update arm runs, with the real desktop adapter
    // over this journey's temp home: a real file must land on disk.
    let port =
        Arc::new(infiltrator_desktop::script_export::DesktopScriptExportPort::new(home.configs()));
    let snapshot =
        infiltrator_application::script_export_application::ScriptExportApplication::new(Some(
            port,
        ))
        .export_mixin_overlay(
            "alpha",
            &state.editor.editor_content.text(),
            &state.editor.mixin_content.text(),
        )
        .expect("real export");
    let exported_path = home.configs().join("exports/alpha.mixin.yaml");
    assert!(
        exported_path.exists(),
        "the host wrote a real overlay export at {exported_path:?}"
    );
    let written = std::fs::read_to_string(&exported_path).expect("read export");
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
    feed(
        &mut state,
        Message::ScriptExportFinished(Ok(snapshot.clone())),
    );
    assert!(!state.editor.script_sandbox.is_exporting);
    assert_eq!(
        state.editor.script_sandbox.export.as_ref(),
        Some(&snapshot),
        "the pane renders the shared export projection"
    );
    assert!(
        last_toast(&state)
            .map(|(text, _)| text.contains("alpha.mixin.yaml"))
            .unwrap_or(false),
        "the toast names the real file"
    );
    let _panel =
        crate::view::script_export::export_section(&state, &[ScriptExportKind::MixinOverlayYaml]);

    // A host with no save-file adapter is a typed unsupported, not a fake path.
    let hostless = infiltrator_application::script_export_application::ScriptExportApplication::without_host_port()
        .export_directive_dsl(Some("alpha"), "function main(config) { return config; }", None)
        .expect("compose for a hostless export");
    assert!(hostless.outcome.is_unsupported());
    assert!(hostless.content.contains("不是 JavaScript"));
}

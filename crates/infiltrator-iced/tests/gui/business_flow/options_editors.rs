//! Journeys 4/5 — the Editor three-pane mixin editor and the per-profile
//! subscription-filter editor, driven through the routed message surface
//! with the real sidecar/merge/pipeline product functions executed for the
//! async legs, verified against the temp-HOME filesystem.
//!
//! test-intent: behavior

use super::support::{TempHome, block_on, feed, fresh_state, last_toast};
use crate::types::message::Message;
use crate::types::options::{EditorPane, FilterDraft};
use infiltrator_core::mixin::MixinConfig;
use infiltrator_core::profile_options::{self, ProfileOptions};

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
        Message::ProfileContentLoaded(Ok((profile_path.clone(), super::support::SAMPLE_PROFILE_YAML.into()))),
    );
    // ProfileContentLoaded chains Navigate(Editor) as a lazy done-task; the
    // runtime delivers it on the next hop — replay that hop here.
    feed(&mut state, Message::Navigate(crate::types::app::Route::Editor));
    assert_eq!(state.editor.editor_path.as_deref(), Some(profile_path.as_path()));
    assert_eq!(state.shell.current_route, crate::types::app::Route::Editor);
    assert_eq!(state.editor.editor_pane, EditorPane::Profile);

    // Switch to the Mixin pane: lazy load arms (task dropped) → the loaded
    // overlay 回灌 fills the editor.
    let units = feed(&mut state, Message::SetEditorPane(EditorPane::Mixin));
    assert_eq!(units, 1, "lazy overlay load armed");
    assert_eq!(state.editor.mixin_loaded_for.as_deref(), Some("alpha"));
    feed(&mut state, Message::MixinLoaded(Ok("log-level: silent\n".into())));
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
    feed(&mut state, Message::MixinLoaded(Ok("mode: global\n".into())));
    let units = feed(&mut state, Message::SaveMixin);
    assert!(state.editor.is_saving_mixin, "gate passed → task in flight");
    assert_eq!(units, 1, "single persistence task armed");

    // Task body for real: load old options → strip old mixin rules → merge →
    // validate → commit profile → persist sidecar.
    let mixin: MixinConfig = serde_yaml_ng::from_str("mode: global\n").unwrap();
    block_on(async {
        let config_dir = crate::configs_dir::configs_dir().await.unwrap();
        let old = profile_options::load_options(&config_dir, "alpha").await.unwrap();
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let content = manager.load("alpha").await.unwrap();
        let removals: Vec<String> = old
            .mixin
            .rules
            .iter()
            .flat_map(|rules| rules.prepend.iter().chain(rules.append.iter()).cloned())
            .collect();
        let base = profile_options::strip_rule_lines(&content, &removals);
        let merged = infiltrator_core::mixin::merge_profile_with_config(&base, &mixin).unwrap();
        infiltrator_core::config::validate_yaml(&merged).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            merged,
            infiltrator_core::apply::ApplyStrategy::PreferReload,
        )
        .await
        .unwrap();
        profile_options::save_options(
            &config_dir,
            "alpha",
            &ProfileOptions { mixin, filter: old.filter },
        )
        .await
        .unwrap();
    });

    let units = feed(&mut state, Message::MixinSaved(Ok(())));
    assert!(!state.editor.is_saving_mixin);
    assert!(units >= 2, "snapshots + editor reload + toast chained");

    // Disk truth: the merged document AND the sidecar round-trip.
    let on_disk = std::fs::read_to_string(home.configs().join("alpha.yaml")).unwrap();
    assert!(on_disk.contains("mode: global"), "mixin merged into profile: {on_disk}");
    assert!(on_disk.contains("HK-1"), "proxies survive the merge");
    let sidecar = block_on(profile_options::load_options(&home.configs(), "alpha")).unwrap();
    assert_eq!(sidecar.mixin.mode.as_deref(), Some("global"), "sidecar persisted");
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
        Message::ProfileContentLoaded(Ok((profile_path, super::support::SAMPLE_PROFILE_YAML.into()))),
    );
    let units = feed(&mut state, Message::LoadProfileFilter);
    assert_eq!(units, 1, "lazy filter load armed");
    assert_eq!(state.editor.filter_loaded_for.as_deref(), Some("alpha"));

    // Empty store → default draft 回灌.
    feed(&mut state, Message::ProfileFilterLoaded(Ok(FilterDraft::default())));

    // User edits the draft.
    feed(&mut state, Message::UpdateFilterInclude("HK".into()));
    feed(&mut state, Message::UpdateFilterExclude("US".into()));

    // A malformed rename is caught by the synchronous compile gate.
    feed(&mut state, Message::UpdateFilterRenames("没有箭头的规则".into()));
    let units = feed(&mut state, Message::SaveProfileFilter);
    assert_eq!(units, 1, "compile gate arms only the error toast");
    assert!(!state.editor.is_saving_filter);

    // Valid draft → save arms the task; run its body for real.
    feed(&mut state, Message::UpdateFilterRenames(String::new()));
    let units = feed(&mut state, Message::SaveProfileFilter);
    assert!(state.editor.is_saving_filter);
    assert_eq!(units, 1, "single persistence task armed");

    let spec = state.editor.filter_draft.to_spec().unwrap();
    let report = block_on(async {
        let rule = spec.to_rule().unwrap();
        let manager = crate::configs_dir::config_manager().await.unwrap();
        let content = manager.load("alpha").await.unwrap();
        let (filtered, report) =
            infiltrator_core::filter::SubscriptionFilterPipeline::new(rule)
                .apply_to_yaml(&content)
                .unwrap();
        infiltrator_core::config::validate_yaml(&filtered).unwrap();
        crate::update::core::profile_apply::save_profile_content(
            None,
            "alpha".into(),
            filtered,
            infiltrator_core::apply::ApplyStrategy::PreferReload,
        )
        .await
        .unwrap();
        let config_dir = crate::configs_dir::configs_dir().await.unwrap();
        let old = profile_options::load_options(&config_dir, "alpha").await.unwrap();
        profile_options::save_options(
            &config_dir,
            "alpha",
            &ProfileOptions { mixin: old.mixin, filter: Some(spec.clone()) },
        )
        .await
        .unwrap();
        report
    });

    assert_eq!(report.total_input, 3, "three seed proxies entered the pipeline");
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
                    proxy.get("name").and_then(|name| name.as_str()).map(str::to_string)
                })
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(kept_names, vec!["HK-1".to_string()], "whitelist applied: {kept_names:?}");
    let sidecar = block_on(profile_options::load_options(&home.configs(), "alpha")).unwrap();
    let stored_spec = sidecar.filter.expect("filter spec persisted");
    assert_eq!(stored_spec.include_keywords, vec!["HK".to_string()]);

    // 重开回读: the lazy loader would compile the spec back into a draft.
    let reopened = FilterDraft::from_spec(Some(&stored_spec));
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

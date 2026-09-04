//! Profile-options handlers: the mixin overlay editor (Editor page second
//! pane) and the per-profile subscription filter editor (Profiles page card).
//!
//! Both editors commit through the shared apply transaction. The mixin save
//! strips the outgoing mixin's prepend/append rule lines first so repeated
//! edits stay idempotent; the filter save re-runs the pipeline in place (the
//! next subscription update recomposes from the raw source anyway).

use crate::state::AppState;
use crate::types::app::ToastStatus;
use crate::types::message::Message;
use crate::types::options::{EditorPane, FilterDraft};
use iced::Task;
use iced::widget::text_editor;
use infiltrator_domain::apply::ApplyStrategy;
use infiltrator_contract::error::InfiltratorError;
use infiltrator_core::profile_options_io;
use infiltrator_domain::filter::SubscriptionFilterPipeline;
use infiltrator_domain::mixin::MixinConfig;
use infiltrator_domain::profile_options::FilterSpec;
use infiltrator_domain::profile_options::{self, ProfileOptions};
use infiltrator_shared::locales::{Lang, Localizer};

impl AppState {
    pub(super) fn update_options(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SetEditorPane(pane) => {
                self.editor.editor_pane = pane;
                match pane {
                    EditorPane::Mixin => self.ensure_mixin_loaded(),
                    EditorPane::Filter => self.ensure_filter_loaded(),
                    EditorPane::Profile | EditorPane::Script => Task::none(),
                }
            }
            Message::MixinEditorAction(action) => {
                self.editor.mixin_content.perform(action);
                let text = self.editor.mixin_content.text();
                match infiltrator_domain::config::preflight_yaml_syntax(&text) {
                    Ok(()) => {
                        self.editor.syntax_error = None;
                        self.editor.syntax_error_line = None;
                    }
                    Err(diag) => {
                        self.editor.syntax_error = Some(diag.message);
                        self.editor.syntax_error_line = Some(diag.line);
                    }
                }
                Task::none()
            }
            Message::MixinLoaded(result) => {
                match result {
                    Ok(text) => {
                        self.editor.mixin_content = text_editor::Content::with_text(&text);
                    }
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            Message::SaveMixin => self.save_mixin(),
            Message::MixinSaved(result) => {
                self.editor.is_saving_mixin = false;
                match result {
                    Ok(()) => {
                        if let Some(runtime) = self.runtime.runtime.clone() {
                            self.sync_runtime_slot(Some(runtime));
                        }
                        self.invalidate_rules_dns_views();
                        let lang = Lang(&self.shell.lang);
                        let mut tasks = vec![
                            Task::done(Message::LoadProfileSnapshots),
                            Task::done(Message::ShowToast(
                                lang.tr("toast_mixin_applied").to_string(),
                                ToastStatus::Success,
                            )),
                        ];
                        // Reload the profile YAML pane so it shows the merged
                        // document instead of the stale pre-mixin content.
                        if let Some(path) = self.editor.editor_path.clone() {
                            tasks.push(Task::done(Message::EditProfile(path)));
                        }
                        Task::batch(tasks)
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            Message::LoadProfileFilter => self.ensure_filter_loaded(),
            Message::ProfileFilterLoaded(result) => {
                match result {
                    Ok(draft) => {
                        self.editor.filter_draft = draft;
                        self.editor.filter_loaded_for = self
                            .editor
                            .editor_path
                            .as_ref()
                            .and_then(|path| path.file_stem())
                            .and_then(|name| name.to_str())
                            .map(str::to_string);
                    }
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            Message::UpdateFilterInclude(value) => {
                self.editor.filter_draft.include = value;
                Task::none()
            }
            Message::UpdateFilterExclude(value) => {
                self.editor.filter_draft.exclude = value;
                Task::none()
            }
            Message::UpdateFilterExcludeTypes(value) => {
                self.editor.filter_draft.exclude_types = value;
                Task::none()
            }
            Message::UpdateFilterRenames(value) => {
                self.editor.filter_draft.renames = value;
                Task::none()
            }
            Message::UpdateFilterDedup(index) => {
                self.editor.filter_draft.dedup_index = index;
                Task::none()
            }
            Message::SaveProfileFilter => self.save_profile_filter(),
            Message::ProfileFilterSaved(result) => {
                self.editor.is_saving_filter = false;
                match result {
                    Ok(report) => {
                        self.invalidate_rules_dns_views();
                        let lang = Lang(&self.shell.lang);
                        Task::done(Message::ShowToast(
                            format!(
                                "{}：{} / {} {}（{} {}，{} {}）",
                                lang.tr("toast_filter_applied"),
                                report.passed,
                                report.total_input,
                                lang.tr("filter_stat_passed"),
                                lang.tr("filter_stat_renamed"),
                                report.renamed,
                                lang.tr("filter_stat_dedup"),
                                report.deduplicated
                            ),
                            ToastStatus::Success,
                        ))
                    }
                    Err(error) => {
                        self.set_error(&error);
                        Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error))
                    }
                }
            }
            _ => Task::none(),
        }
    }

    /// Lazily load the profile's mixin overlay the first time the Mixin pane
    /// is opened for it.
    pub(super) fn ensure_mixin_loaded(&mut self) -> Task<Message> {
        let Some(path) = self.editor.editor_path.clone() else {
            return Task::none();
        };
        let Some(profile) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            return Task::none();
        };
        if self.editor.mixin_loaded_for.as_deref() == Some(profile.as_str()) {
            return Task::none();
        }
        self.editor.mixin_loaded_for = Some(profile.clone());
        Task::perform(
            async move {
                let config_dir = crate::configs_dir::configs_dir().await?;
                let options = profile_options_io::load_options(&config_dir, &profile)
                    .await
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                serde_yaml_ng::to_string(&options.mixin)
                    .map_err(|error| InfiltratorError::Config(error.to_string()))
            },
            Message::MixinLoaded,
        )
    }

    /// Lazily load the per-profile subscription filter the first time the
    /// Filter pane is opened for the profile currently open in the editor.
    pub(super) fn ensure_filter_loaded(&mut self) -> Task<Message> {
        let Some(path) = self.editor.editor_path.clone() else {
            return Task::none();
        };
        let Some(profile) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            return Task::none();
        };
        if self.editor.filter_loaded_for.as_deref() == Some(profile.as_str()) {
            return Task::none();
        }
        self.editor.filter_loaded_for = Some(profile.clone());
        Task::perform(
            async move {
                let config_dir = crate::configs_dir::configs_dir().await?;
                let options = profile_options_io::load_options(&config_dir, &profile)
                    .await
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                Ok(FilterDraft::from_spec(options.filter.as_ref()))
            },
            Message::ProfileFilterLoaded,
        )
    }

    fn save_mixin(&mut self) -> Task<Message> {
        if self.editor.is_saving_mixin {
            return Task::none();
        }
        let Some(path) = self.editor.editor_path.clone() else {
            return Task::none();
        };
        let Some(profile) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            return Task::none();
        };
        // Validation gate: a malformed overlay is rejected before any state
        // flips or task spawns, so the editor keeps its content for fixing.
        let lang = Lang(&self.shell.lang);
        let mixin: MixinConfig = match serde_yaml_ng::from_str(&self.editor.mixin_content.text()) {
            Ok(mixin) => mixin,
            Err(error) => {
                let error = InfiltratorError::Config(format!(
                    "{}: {error}",
                    lang.tr("toast_mixin_invalid")
                ));
                self.set_error(&error);
                return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
            }
        };
        self.editor.is_saving_mixin = true;
        let runtime = self.runtime.runtime.clone();
        Task::perform(
            async move {
                let config_dir = crate::configs_dir::configs_dir().await?;
                let old = profile_options_io::load_options(&config_dir, &profile)
                    .await
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                let manager = crate::configs_dir::config_manager().await?;
                let content = manager
                    .load(&profile)
                    .await
                    .map_err(infiltrator_contract::error::from_mihomo)?;
                let removals: Vec<String> = old
                    .mixin
                    .rules
                    .iter()
                    .flat_map(|rules| rules.prepend.iter().chain(rules.append.iter()).cloned())
                    .collect();
                let base = profile_options::strip_rule_lines(&content, &removals);
                let merged = infiltrator_domain::mixin::merge_profile_with_config(&base, &mixin)
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                infiltrator_domain::config::validate_yaml(&merged)
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                crate::update::core::profile_apply::save_profile_content(
                    runtime,
                    profile.clone(),
                    merged,
                    ApplyStrategy::PreferReload,
                )
                .await?;
                profile_options_io::save_options(
                    &config_dir,
                    &profile,
                    &ProfileOptions {
                        mixin,
                        filter: old.filter,
                    },
                )
                .await
                .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                Ok(())
            },
            Message::MixinSaved,
        )
    }

    fn save_profile_filter(&mut self) -> Task<Message> {
        if self.editor.is_saving_filter {
            return Task::none();
        }
        let Some(path) = self.editor.editor_path.clone() else {
            return Task::none();
        };
        let Some(profile) = path
            .file_stem()
            .and_then(|name| name.to_str())
            .map(str::to_string)
        else {
            return Task::none();
        };
        // Validation gate: compile every pattern before spawning the task.
        let spec = match parse_filter_draft(&self.editor.filter_draft) {
            Ok(spec) => spec,
            Err(error) => {
                let error = InfiltratorError::Config(error.to_string());
                self.set_error(&error);
                return Task::done(Message::ShowToast(error.to_string(), ToastStatus::Error));
            }
        };
        self.editor.is_saving_filter = true;
        let runtime = self.runtime.runtime.clone();
        Task::perform(
            async move {
                let rule = spec
                    .to_rule()
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                let manager = crate::configs_dir::config_manager().await?;
                let content = manager
                    .load(&profile)
                    .await
                    .map_err(infiltrator_contract::error::from_mihomo)?;
                let (filtered, report) = SubscriptionFilterPipeline::new(rule)
                    .apply_to_yaml(&content)
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                infiltrator_domain::config::validate_yaml(&filtered)
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                crate::update::core::profile_apply::save_profile_content(
                    runtime,
                    profile.clone(),
                    filtered,
                    ApplyStrategy::PreferReload,
                )
                .await?;
                let config_dir = crate::configs_dir::configs_dir().await?;
                let old = profile_options_io::load_options(&config_dir, &profile)
                    .await
                    .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                profile_options_io::save_options(
                    &config_dir,
                    &profile,
                    &ProfileOptions {
                        mixin: old.mixin,
                        filter: Some(spec),
                    },
                )
                .await
                .map_err(|error| InfiltratorError::Config(error.to_string()))?;
                Ok(report)
            },
            Message::ProfileFilterSaved,
        )
    }
}

/// Compile the free-text filter draft into a stored spec (delegates to the
/// type's own parser, which owns the splitting/format rules).
pub(super) fn parse_filter_draft(draft: &FilterDraft) -> anyhow::Result<FilterSpec> {
    draft.to_spec()
}

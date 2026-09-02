//! MRS rule-provider metadata scan for the Rules page providers tab.
//!
//! Provider names come from the current profile's `rule-providers` section
//! (so the scan works without a live core) unioned with the live controller
//! list. Each candidate cache file is read from disk; only bytes carrying the
//! MRS magic parse into header metadata via `infiltrator_core::mrs`.

use crate::state::AppState;
use crate::types::message::Message;
use crate::types::options::MrsProviderDetail;
use iced::Task;
use infiltrator_core::error::InfiltratorError;
use infiltrator_shared::locales::{Lang, Localizer};
use serde_yaml_ng::Value;
use std::collections::HashMap;
use std::path::PathBuf;

impl AppState {
    pub(super) fn update_core_mrs(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ScanMrsProviders => {
                if self.editor.is_scanning_mrs {
                    return Task::none();
                }
                self.editor.is_scanning_mrs = true;
                let live: Vec<(String, String)> = self
                    .editor
                    .rule_providers
                    .iter()
                    .map(|provider| (provider.name.clone(), provider.behavior.clone()))
                    .collect();
                let lang = self.shell.lang.clone();
                Task::perform(scan_mrs_providers(live, lang), Message::MrsDetailsReady)
            }
            Message::MrsDetailsReady(result) => {
                self.editor.is_scanning_mrs = false;
                match result {
                    Ok(details) => self.editor.mrs_details = details,
                    Err(error) => self.set_error(&error),
                }
                Task::none()
            }
            other => self.update_core_advanced(other),
        }
    }
}

async fn scan_mrs_providers(
    live: Vec<(String, String)>,
    lang_code: String,
) -> Result<Vec<MrsProviderDetail>, InfiltratorError> {
    let lang = Lang(&lang_code);
    // 与 manager 同源的 configs 目录（env > settings `configs_dir`），
    // 重定向后相对 `path` 覆盖才能解析到真实 provider 文件。
    let config_dir = crate::configs_dir::configs_dir().await?;
    let live_behaviors: HashMap<String, String> = live.into_iter().collect();

    // Collect provider names + explicit `path` overrides from the profile YAML.
    let mut names: Vec<String> = Vec::new();
    let mut explicit_paths: HashMap<String, PathBuf> = HashMap::new();
    let manager = crate::configs_dir::config_manager().await?;
    if let Ok(current) = manager.get_current().await
        && let Ok(content) = manager.load(&current).await
        && let Ok(doc) = serde_yaml_ng::from_str::<Value>(&content)
        && let Some(providers) = doc
            .get("rule-providers")
            .and_then(|value| value.as_mapping())
    {
        for (key, value) in providers {
            let Some(name) = key.as_str().map(str::to_string) else {
                continue;
            };
            if !names.contains(&name) {
                names.push(name.clone());
            }
            if let Some(path) = value.get("path").and_then(|path| path.as_str()) {
                let path = PathBuf::from(path);
                let path = if path.is_absolute() {
                    path
                } else {
                    config_dir.join(path)
                };
                explicit_paths.insert(name, path);
            }
        }
    }
    for name in live_behaviors.keys() {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }

    let mut details = Vec::new();
    for name in names {
        let mut candidates = Vec::new();
        if let Some(path) = explicit_paths.get(&name) {
            candidates.push(path.clone());
        }
        // mihomo's default cache location for rule providers.
        candidates.push(
            config_dir
                .join("providers")
                .join("rules")
                .join(format!("{name}.mrs")),
        );
        candidates.push(
            config_dir
                .join("providers")
                .join("rules")
                .join(format!("{name}.yaml")),
        );

        let mut detail = MrsProviderDetail {
            behavior: live_behaviors.get(&name).cloned().unwrap_or_default(),
            name,
            file: None,
            metadata: None,
            errors: Vec::new(),
        };
        let mut found = false;
        for path in candidates {
            match tokio::fs::read(&path).await {
                Ok(bytes) => {
                    detail.file = Some(path.clone());
                    match infiltrator_core::mrs::parse_mrs_header(&bytes) {
                        Ok(meta) => detail.metadata = Some(meta),
                        Err(error) => {
                            detail.errors.push(format!("{}: {error}", path.display()));
                        }
                    }
                    found = true;
                    break;
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(error) => {
                    detail.errors.push(format!(
                        "{} {}: {error}",
                        lang.tr("mrs_read_failed"),
                        path.display()
                    ));
                    found = true;
                    break;
                }
            }
        }
        if !found && detail.errors.is_empty() {
            detail.errors.push(lang.tr("mrs_cache_missing").to_string());
        }
        details.push(detail);
    }
    Ok(details)
}

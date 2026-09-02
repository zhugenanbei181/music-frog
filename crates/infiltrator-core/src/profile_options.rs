//! Per-profile option sidecars: the subscription filter spec and the mihomo
//! mixin overlay, stored under `<config-dir>/options/<profile>.yaml`.
//!
//! Every profile write path (subscription fetch/import in core, admin
//! handlers and scheduler) runs the stored options through
//! [`apply_saved_options_for`] before saving, so all frontends observe the
//! same composed document regardless of which surface triggered the update.
//! The filter pipeline runs first (it reshapes the raw subscription's
//! `proxies` sequence), then the mixin overlay deep-merges over the result.

use crate::filter::{
    ContentDedupStrategy, DeduplicationStrategy, FilterReport, FilterRule, MultiplierRule,
    NodeMutatorConfig, NodeSortOrder, RenameRule, SubscriptionFilterPipeline,
};
use crate::mixin::MixinConfig;
use anyhow::{Context, anyhow};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Serializable mirror of [`FilterRule`]: the regex fields are stored as
/// strings and compiled on demand by [`FilterSpec::to_rule`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct FilterSpec {
    #[serde(default)]
    pub include_keywords: Vec<String>,
    #[serde(default)]
    pub exclude_keywords: Vec<String>,
    #[serde(default)]
    pub rename_rules: Vec<RenameSpec>,
    #[serde(default)]
    pub exclude_types: Vec<String>,
    #[serde(default)]
    pub deduplication: FilterDedup,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub normalize_country_code: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remove_emojis: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_ports: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_ports: Option<Vec<u16>>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub drop_private_ip: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub multiplier_rules: Vec<MultiplierSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_mutator: Option<NodeMutatorConfig>,
    #[serde(default, skip_serializing_if = "is_default_sort_order")]
    pub sort_by: NodeSortOrder,
    #[serde(default, skip_serializing_if = "is_default_content_dedup")]
    pub content_dedup: ContentDedupStrategy,
}

fn is_default_sort_order(order: &NodeSortOrder) -> bool {
    *order == NodeSortOrder::Preserve
}

fn is_default_content_dedup(dedup: &ContentDedupStrategy) -> bool {
    *dedup == ContentDedupStrategy::Disabled
}

/// Stored form of a multiplier override rule.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MultiplierSpec {
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub multiplier: f64,
}

/// Stored form of a rename rule: `pattern` is a regular expression, and
/// `replacement` may reference its capture groups.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct RenameSpec {
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub replacement: String,
}

/// Stored form of [`DeduplicationStrategy`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilterDedup {
    #[default]
    Disabled,
    KeepFirst,
    KeepLast,
    AppendIndex,
}

impl FilterSpec {
    /// True when the spec would not change anything, so the compose pipeline
    /// can skip re-serializing the document (and keep its comments intact).
    pub fn is_empty(&self) -> bool {
        self.include_keywords.is_empty()
            && self.exclude_keywords.is_empty()
            && self.rename_rules.is_empty()
            && self.exclude_types.is_empty()
            && self.deduplication == FilterDedup::Disabled
            && !self.normalize_country_code
            && !self.remove_emojis
            && self.allowed_ports.as_ref().is_none_or(|v| v.is_empty())
            && self.blocked_ports.as_ref().is_none_or(|v| v.is_empty())
            && !self.drop_private_ip
            && self.multiplier_rules.is_empty()
            && self.node_mutator.is_none()
            && self.sort_by == NodeSortOrder::Preserve
            && self.content_dedup == ContentDedupStrategy::Disabled
    }

    /// Compile the stored strings into a runtime [`FilterRule`]. Invalid
    /// patterns fail here with the offending pattern named, so a broken
    /// stored spec surfaces as an actionable error instead of silently
    /// passing every proxy through.
    pub fn to_rule(&self) -> anyhow::Result<FilterRule> {
        let mut include = Vec::new();
        for pattern in &self.include_keywords {
            include.push(
                Regex::new(pattern)
                    .with_context(|| format!("包含关键字不是有效的正则: {pattern}"))?,
            );
        }
        let mut exclude = Vec::new();
        for pattern in &self.exclude_keywords {
            exclude.push(
                Regex::new(pattern)
                    .with_context(|| format!("排除关键字不是有效的正则: {pattern}"))?,
            );
        }
        let mut renames = Vec::new();
        for spec in &self.rename_rules {
            renames.push(RenameRule {
                pattern: Regex::new(&spec.pattern)
                    .with_context(|| format!("重命名规则不是有效的正则: {}", spec.pattern))?,
                replacement: spec.replacement.clone(),
            });
        }
        let mut multipliers = Vec::new();
        for spec in &self.multiplier_rules {
            multipliers.push(MultiplierRule {
                pattern: Regex::new(&spec.pattern)
                    .with_context(|| format!("倍率重写规则不是有效的正则: {}", spec.pattern))?,
                multiplier: spec.multiplier,
            });
        }
        let allowed_ports = self.allowed_ports.as_ref().map(|v| v.iter().copied().collect::<HashSet<_>>());
        let blocked_ports = self.blocked_ports.as_ref().map(|v| v.iter().copied().collect::<HashSet<_>>());

        Ok(FilterRule {
            include_keywords: include,
            exclude_keywords: exclude,
            rename_rules: renames,
            exclude_types: self.exclude_types.clone(),
            deduplication: match self.deduplication {
                FilterDedup::Disabled => DeduplicationStrategy::Disabled,
                FilterDedup::KeepFirst => DeduplicationStrategy::KeepFirst,
                FilterDedup::KeepLast => DeduplicationStrategy::KeepLast,
                FilterDedup::AppendIndex => DeduplicationStrategy::AppendIndex,
            },
            normalize_country_code: self.normalize_country_code,
            remove_emojis: self.remove_emojis,
            allowed_ports,
            blocked_ports,
            drop_private_ip: self.drop_private_ip,
            multiplier_rules: multipliers,
            node_mutator: self.node_mutator.clone(),
            sort_order: self.sort_by,
            content_dedup: self.content_dedup,
        })
    }
}

/// Per-profile option sidecar document.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", default)]
pub struct ProfileOptions {
    #[serde(default)]
    pub mixin: MixinConfig,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter: Option<FilterSpec>,
}

impl ProfileOptions {
    /// True when neither a filter nor a non-default mixin is configured, so
    /// composition can return the source document untouched.
    pub fn is_empty(&self) -> bool {
        mixin_is_default(&self.mixin) && self.filter.as_ref().is_none_or(|spec| spec.is_empty())
    }
}

fn mixin_is_default(mixin: &MixinConfig) -> bool {
    mixin.mode.is_none()
        && mixin.log_level.is_none()
        && mixin.ipv6.is_none()
        && mixin.allow_lan.is_none()
        && mixin.mixed_port.is_none()
        && mixin.secret.is_none()
        && mixin.external_controller.is_none()
        && mixin.external_ui.is_none()
        && mixin.dns.is_none()
        && mixin.tun.is_none()
        && mixin.sniffer.is_none()
        && mixin.rules.is_none()
        && mixin.proxies.is_none()
        && mixin.proxy_groups.is_none()
        && mixin.proxy_providers.is_none()
        && mixin.rule_providers.is_none()
        && mixin.custom_yaml.is_none()
}

/// Sidecar location for one profile: `<config-dir>/options/<profile>.yaml`.
pub fn options_path(config_dir: &Path, profile: &str) -> PathBuf {
    config_dir.join("options").join(format!("{profile}.yaml"))
}

/// Load the sidecar. A missing file yields the default (empty) options; a
/// malformed one is an error so a broken hand-edit cannot silently drop the
/// user's filter/mixin on the next subscription update.
pub async fn load_options(config_dir: &Path, profile: &str) -> anyhow::Result<ProfileOptions> {
    let path = options_path(config_dir, profile);
    let Ok(text) = tokio::fs::read_to_string(&path).await else {
        return Ok(ProfileOptions::default());
    };
    serde_yaml_ng::from_str(&text)
        .with_context(|| format!("解析配置选项文件失败: {}", path.display()))
}

/// Persist the sidecar atomically. Saving empty options removes the file so
/// stale sidecars cannot resurrect onto a future profile of the same name.
pub async fn save_options(
    config_dir: &Path,
    profile: &str,
    options: &ProfileOptions,
) -> anyhow::Result<()> {
    let path = options_path(config_dir, profile);
    if options.is_empty() {
        let _ = tokio::fs::remove_file(&path).await;
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let text = serde_yaml_ng::to_string(options)?;
    let temp = path.with_file_name(format!(
        ".{}.options-tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("profile")
    ));
    tokio::fs::write(&temp, text).await?;
    tokio::fs::rename(&temp, &path).await?;
    Ok(())
}

/// Best-effort sidecar removal when a profile is deleted; a leftover file
/// would otherwise be picked up by a profile recreated with the same name.
pub async fn delete_options(config_dir: &Path, profile: &str) {
    let _ = tokio::fs::remove_file(options_path(config_dir, profile)).await;
}

/// Convenience wrapper resolving the shared config directory through the
/// same chain as [`mihomo_config::manager::ConfigManager`]: the
/// `INFILTRATOR_CONFIGS_DIR` override, then the settings `configs_dir`
/// field, then the default `<home>/configs`.
pub async fn apply_saved_options_for(
    profile: &str,
    content: &str,
) -> anyhow::Result<(String, Option<FilterReport>)> {
    let home = mihomo_platform::paths::get_home_dir()?;
    let settings_file = crate::settings::settings_path(&home)?;
    let settings = crate::settings::load_settings(&settings_file).await?;
    let config_dir =
        mihomo_config::manager::paths::resolve_configs_dir(settings.configs_dir.as_deref())?;
    apply_saved_options(&config_dir, profile, content).await
}

/// Load the sidecar for `profile` and compose it onto freshly fetched
/// subscription content. Composition failures (bad stored regex, invalid
/// mixin YAML) abort the update with the cause attached.
pub async fn apply_saved_options(
    config_dir: &Path,
    profile: &str,
    content: &str,
) -> anyhow::Result<(String, Option<FilterReport>)> {
    let options = load_options(config_dir, profile).await?;
    compose_content(content, &options)
}

/// Pure composition: filter the subscription's `proxies`, then deep-merge
/// the mixin overlay. Empty options return the source unchanged (no
/// re-serialization, so comments in hand-written profiles survive).
pub fn compose_content(
    content: &str,
    options: &ProfileOptions,
) -> anyhow::Result<(String, Option<FilterReport>)> {
    if options.is_empty() {
        return Ok((content.to_string(), None));
    }
    let mut current = content.to_string();
    let mut report = None;
    if let Some(spec) = &options.filter
        && !spec.is_empty()
    {
        let pipeline = SubscriptionFilterPipeline::new(spec.to_rule()?);
        let (filtered, filtered_report) = pipeline
            .apply_to_yaml(&current)
            .map_err(|error| anyhow!("订阅过滤管道执行失败: {error}"))?;
        current = filtered;
        report = Some(filtered_report);
    }
    if !mixin_is_default(&options.mixin) {
        if report.is_none()
            && crate::yaml_edit::mixin_fidelity::can_apply_mixin_via_fidelity(&options.mixin)
            && let Ok(mut doc) = crate::yaml_edit::SourceDoc::parse(&current)
            && crate::yaml_edit::mixin_fidelity::apply_mixin_to_doc(&mut doc, &options.mixin).is_ok()
        {
            return Ok((doc.render(), None));
        }
        current = crate::mixin::merge_profile_with_config(&current, &options.mixin)?;
    }
    Ok((current, report))
}

/// Remove exact rule lines injected by a previous mixin's prepend/append
/// lists. Mixin rule injection is cumulative by design (`prepend ++ base ++
/// append`), so re-applying an edited mixin onto an already-composed profile
/// would duplicate the old lines; the editor strips the outgoing mixin's
/// lines first to keep repeated edits idempotent. Unparseable documents are
/// returned unchanged.
pub fn strip_rule_lines(content: &str, removals: &[String]) -> String {
    if removals.is_empty() {
        return content.to_string();
    }
    if let Ok(mut doc) = crate::yaml_edit::SourceDoc::parse(content) {
        let mut any_removed = false;
        for target in removals {
            while doc.remove_rule(target).is_ok() {
                any_removed = true;
            }
        }
        if any_removed {
            return doc.render();
        }
    }
    let mut doc: Value = match serde_yaml_ng::from_str(content) {
        Ok(doc) => doc,
        Err(_) => return content.to_string(),
    };
    let Some(rules) = doc
        .get_mut("rules")
        .and_then(|rules| rules.as_sequence_mut())
    else {
        return content.to_string();
    };
    rules.retain(|rule| {
        !rule
            .as_str()
            .is_some_and(|line| removals.iter().any(|target| target == line))
    });
    match serde_yaml_ng::to_string(&doc) {
        Ok(out) => out,
        Err(_) => content.to_string(),
    }
}

#[cfg(test)]
#[path = "profile_options_test.rs"]
mod profile_options_test;

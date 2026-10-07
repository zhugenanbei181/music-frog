//! DUAL-10-08/10/11: shared Mixin-editor pure logic.
//!
//! Both surfaces edit the same Mixin overlay document, so the *rules* an
//! editor needs must live once and be called by both:
//!
//! * [`preflight_mixin`] is the syntax/schema + dry-run merge + output
//!   validation gate. A malformed overlay is blocked before any state flips,
//!   which is what keeps the kernel from ever receiving a half-merged config.
//! * [`MIXIN_PRESET_TOGGLES`] is the fixed catalogue of common overlay
//!   switches; [`set_toggle`] flips one field through the real
//!   [`MixinConfig`] codec so the stored bytes stay canonical.
//! * [`preview_cascade`] runs the real [`CascadeOverlayPipeline`] stage by
//!   stage and reports what each stage contributed — it never fabricates a
//!   stage output.
//!
//! These functions are pure (no I/O, no executor) so a surface may call them
//! on every edit, and the shared application re-runs the same rules when the
//! document is finally committed.

use crate::config::validate_yaml;
use crate::mixin::{CascadeOverlayPipeline, MixinConfig, merge_profile_with_config_fidelity};
use serde_yaml_ng::Value;

/// Verdict of the shared Mixin preflight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixinPreflightReport {
    /// Whether the overlay may be committed.
    pub valid: bool,
    /// The blocking reason when [`Self::valid`] is false.
    pub error: Option<String>,
    /// The composed document the overlay would produce (present even when the
    /// merge succeeded but the final validation refused it).
    pub merged_preview: Option<String>,
    /// Non-blocking note (never a fabricated success).
    pub note: Option<String>,
}

impl MixinPreflightReport {
    /// True when a surface must refuse to submit the overlay.
    pub fn is_blocking(&self) -> bool {
        !self.valid
    }

    fn invalid(error: impl Into<String>) -> Self {
        Self {
            valid: false,
            error: Some(error.into()),
            merged_preview: None,
            note: None,
        }
    }
}

/// Run the real parse → fidelity merge → output-validation pipeline over an
/// overlay draft, without persisting anything.
pub fn preflight_mixin(base_yaml: &str, mixin_yaml: &str) -> MixinPreflightReport {
    let config = match serde_yaml_ng::from_str::<MixinConfig>(mixin_yaml) {
        Ok(config) => config,
        Err(error) => {
            return MixinPreflightReport::invalid(format!(
                "Mixin overlay is not valid YAML: {error}"
            ));
        }
    };
    let merged = match merge_profile_with_config_fidelity(base_yaml, &config) {
        Ok(merged) => merged,
        Err(error) => {
            return MixinPreflightReport::invalid(format!("Mixin merge failed: {error}"));
        }
    };
    match validate_yaml(&merged) {
        Ok(()) => MixinPreflightReport {
            valid: true,
            error: None,
            merged_preview: Some(merged),
            note: None,
        },
        Err(error) => MixinPreflightReport {
            valid: false,
            error: Some(format!("Composed configuration failed validation: {error}")),
            merged_preview: Some(merged),
            note: None,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixinPresetId {
    Ipv6,
    AllowLan,
    DnsFakeIp,
    Tun,
    Sniffer,
    DebugLog,
}
impl MixinPresetId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ipv6 => "ipv6",
            Self::AllowLan => "allow-lan",
            Self::DnsFakeIp => "dns-fake-ip",
            Self::Tun => "tun",
            Self::Sniffer => "sniffer",
            Self::DebugLog => "log-level-debug",
        }
    }
}

/// One common Mixin overlay switch offered by both editors.
#[derive(Clone, Copy)]
pub struct MixinPresetToggle {
    /// Stable id every surface passes around.
    pub id: MixinPresetId,
    enabled: fn(&MixinConfig) -> bool,
    apply: fn(&mut MixinConfig, bool),
}

impl MixinPresetToggle {
    /// Whether the stored overlay currently requests this switch.
    pub fn is_enabled(&self, config: &MixinConfig) -> bool {
        (self.enabled)(config)
    }

    /// Apply the switch to an in-memory overlay.
    pub fn apply_to(&self, config: &mut MixinConfig, enabled: bool) {
        (self.apply)(config, enabled)
    }
}

fn dns_fake_ip() -> Value {
    serde_yaml_ng::from_str(
        "enable: true\nenhanced-mode: fake-ip\nfake-ip-range: 198.18.0.1/16\nnameserver:\n  - 223.5.5.5\n  - 119.29.29.29\n",
    )
    .expect("static dns preset yaml")
}

fn tun_preset() -> Value {
    serde_yaml_ng::from_str("enable: true\nstack: system\nauto-route: true\n").expect("static tun")
}

fn sniffer_preset() -> Value {
    serde_yaml_ng::from_str("enable: true\n").expect("static sniffer")
}

/// The fixed catalogue of common overlay switches, in stable order.
pub const MIXIN_PRESET_TOGGLES: &[MixinPresetToggle] = &[
    MixinPresetToggle {
        id: MixinPresetId::Ipv6,
        enabled: |config| config.ipv6 == Some(true),
        apply: |config, enabled| config.ipv6 = Some(enabled),
    },
    MixinPresetToggle {
        id: MixinPresetId::AllowLan,
        enabled: |config| config.allow_lan == Some(true),
        apply: |config, enabled| config.allow_lan = Some(enabled),
    },
    MixinPresetToggle {
        id: MixinPresetId::DnsFakeIp,
        enabled: |config| config.dns.is_some(),
        apply: |config, enabled| {
            config.dns = if enabled { Some(dns_fake_ip()) } else { None };
        },
    },
    MixinPresetToggle {
        id: MixinPresetId::Tun,
        enabled: |config| config.tun.is_some(),
        apply: |config, enabled| {
            config.tun = if enabled { Some(tun_preset()) } else { None };
        },
    },
    MixinPresetToggle {
        id: MixinPresetId::Sniffer,
        enabled: |config| config.sniffer.is_some(),
        apply: |config, enabled| {
            config.sniffer = if enabled {
                Some(sniffer_preset())
            } else {
                None
            };
        },
    },
    MixinPresetToggle {
        id: MixinPresetId::DebugLog,
        enabled: |config| config.log_level.as_deref() == Some("debug"),
        apply: |config, enabled| {
            config.log_level = if enabled {
                Some("debug".to_string())
            } else {
                None
            };
        },
    },
];

/// Look up a catalogue toggle by id.
pub fn preset_toggle(id: &str) -> Option<&'static MixinPresetToggle> {
    MIXIN_PRESET_TOGGLES
        .iter()
        .find(|toggle| toggle.id.as_str() == id)
}

/// Whether the overlay draft requests the toggle. An unparsable draft is an
/// error (the surface shows the preflight verdict instead of guessing).
pub fn toggle_enabled(mixin_yaml: &str, id: &str) -> Result<bool, String> {
    let toggle = preset_toggle(id).ok_or_else(|| format!("Unknown Mixin preset toggle: {id}"))?;
    let config: MixinConfig =
        serde_yaml_ng::from_str(mixin_yaml).map_err(|error| error.to_string())?;
    Ok(toggle.is_enabled(&config))
}

/// Flip one catalogue toggle in the overlay draft and return the canonical
/// bytes. The result is a real [`MixinConfig`] serialization, not text splicing.
pub fn set_toggle(mixin_yaml: &str, id: &str, enabled: bool) -> Result<String, String> {
    let toggle = preset_toggle(id).ok_or_else(|| format!("Unknown Mixin preset toggle: {id}"))?;
    let mut config: MixinConfig = if mixin_yaml.trim().is_empty() {
        MixinConfig::default()
    } else {
        serde_yaml_ng::from_str(mixin_yaml).map_err(|error| error.to_string())?
    };
    toggle.apply_to(&mut config, enabled);
    serde_yaml_ng::to_string(&config).map_err(|error| error.to_string())
}

/// Which column of the three-column Mixin editor a value belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixinColumnRole {
    /// The base profile document, read-only in this editor.
    Base,
    /// The Mixin overlay draft the user edits.
    Overlay,
    /// The composed document produced by the real cascade pipeline.
    Composed,
}

/// One column of the three-column editor, carrying its real content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixinColumn {
    pub role: MixinColumnRole,
    /// The real document bytes this column shows.
    pub content: String,
    /// Whether the column is the user's editable buffer.
    pub editable: bool,
    pub line_count: usize,
}

/// DUAL-10-09: the three-column Mixin editor model.
///
/// The composed column is always the real output of the shared
/// [`preview_cascade`] run: when the pipeline blocks (malformed base or
/// overlay) the column is empty and carries the real reason, never a mock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixinEditorColumns {
    pub base: MixinColumn,
    pub overlay: MixinColumn,
    pub composed: MixinColumn,
    /// The real error when the composed column could not be produced.
    pub error: Option<String>,
}

impl MixinEditorColumns {
    /// Whether the composed column carries a real pipeline output.
    pub fn is_composed(&self) -> bool {
        self.error.is_none() && !self.composed.content.is_empty()
    }

    /// `true` when the pipeline refused to compose (the real reason is in
    /// [`Self::error`]).
    pub fn is_blocked(&self) -> bool {
        self.error.is_some()
    }

    pub fn composed_line_count(&self) -> usize {
        self.composed.line_count
    }
}

fn column(role: MixinColumnRole, content: String, editable: bool) -> MixinColumn {
    MixinColumn {
        role,
        line_count: line_count(&content),
        content,
        editable,
    }
}

/// Build the three-column editor model for the open base document and the
/// overlay draft currently in the middle column.
pub fn mixin_editor_columns(base_yaml: &str, mixin_yaml: &str) -> MixinEditorColumns {
    let base = column(MixinColumnRole::Base, base_yaml.to_string(), false);
    let overlay = column(MixinColumnRole::Overlay, mixin_yaml.to_string(), true);
    let report = preview_cascade_from_yaml(base_yaml, mixin_yaml);
    let (composed_text, error) = match (&report.blocked, &report.error) {
        (true, error) => (String::new(), error.clone()),
        (false, _) => (report.merged_yaml.clone().unwrap_or_default(), None),
    };
    let composed = column(MixinColumnRole::Composed, composed_text, false);
    MixinEditorColumns {
        base,
        overlay,
        composed,
        error,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CascadeStageId {
    Base,
    Subscription,
    Merge,
    PreMixin,
    PostMixin,
}
impl CascadeStageId {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Subscription => "subscription",
            Self::Merge => "merge",
            Self::PreMixin => "pre_mixin",
            Self::PostMixin => "post_mixin",
        }
    }
}

/// One stage of the cascade overlay preview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeStageReport {
    pub id: CascadeStageId,
    /// Whether this stage had an input (a missing stage is shown as 未声明).
    pub applied: bool,
    /// Line count of the real stage output.
    pub line_count: usize,
}

/// Stage-by-stage result of the real cascade overlay pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CascadeOverlayReport {
    pub stages: Vec<CascadeStageReport>,
    pub merged_yaml: Option<String>,
    pub blocked: bool,
    pub error: Option<String>,
}

impl CascadeOverlayReport {
    pub fn merged_line_count(&self) -> usize {
        self.merged_yaml.as_deref().map_or(0, line_count)
    }
}

fn line_count(text: &str) -> usize {
    text.lines().count()
}

fn stage(id: CascadeStageId, applied: bool, output: &str) -> CascadeStageReport {
    CascadeStageReport {
        id,
        applied,
        line_count: line_count(output),
    }
}

/// Parse a Mixin overlay and run [`preview_cascade`]; a surface that has no
/// YAML codec of its own (Bevy) calls this so the overlay bytes are parsed by
/// the same codec the application uses.
pub fn preview_cascade_from_yaml(base_yaml: &str, mixin_yaml: &str) -> CascadeOverlayReport {
    let pre_mixin = if mixin_yaml.trim().is_empty() {
        None
    } else {
        match serde_yaml_ng::from_str::<MixinConfig>(mixin_yaml) {
            Ok(config) => Some(config),
            Err(error) => {
                return CascadeOverlayReport {
                    stages: Vec::new(),
                    merged_yaml: None,
                    blocked: true,
                    error: Some(format!("Mixin overlay is not valid YAML: {error}")),
                };
            }
        }
    };
    preview_cascade(base_yaml, None, None, pre_mixin.as_ref(), None)
}

/// Run `Base → Subscription → Merge(自定义) → Pre-Mixin → Post-Mixin` through
/// the real [`CascadeOverlayPipeline`], capturing each stage's actual output.
pub fn preview_cascade(
    base_yaml: &str,
    subscription_yaml: Option<&str>,
    custom_yaml: Option<&str>,
    pre_mixin: Option<&MixinConfig>,
    post_mixin: Option<&MixinConfig>,
) -> CascadeOverlayReport {
    // The pipeline treats an unparsable input as an empty mapping; a *preview*
    // must instead refuse to show a fabricated merge, so validate every input
    // document up front and block honestly.
    for (label, document) in [
        ("Base", Some(base_yaml)),
        ("subscription", subscription_yaml),
        ("Merge", custom_yaml),
    ] {
        let Some(document) = document.filter(|text| !text.trim().is_empty()) else {
            continue;
        };
        if let Err(error) = serde_yaml_ng::from_str::<Value>(document) {
            return CascadeOverlayReport {
                stages: Vec::new(),
                merged_yaml: None,
                blocked: true,
                error: Some(format!("{label} document is not valid YAML: {error}")),
            };
        }
    }

    let mut stages = Vec::with_capacity(5);
    // Each stage's line count comes from a real pipeline run with exactly the
    // stages up to and including it, so the preview never guesses.
    let mut cumulative = CascadeOverlayPipeline::new().with_base_profile(base_yaml.to_string());
    match cumulative.execute() {
        Ok(output) => stages.push(stage(CascadeStageId::Base, true, &output)),
        Err(error) => {
            return CascadeOverlayReport {
                stages: Vec::new(),
                merged_yaml: None,
                blocked: true,
                error: Some(error.to_string()),
            };
        }
    }

    let subscription_present = subscription_yaml.is_some_and(|s| !s.trim().is_empty());
    if let Some(subscription) = subscription_yaml.filter(|s| !s.trim().is_empty()) {
        cumulative = cumulative.with_subscription(subscription.to_string());
    }
    match cumulative.execute() {
        Ok(output) => stages.push(stage(
            CascadeStageId::Subscription,
            subscription_present,
            &output,
        )),
        Err(error) => {
            return CascadeOverlayReport {
                stages,
                merged_yaml: None,
                blocked: true,
                error: Some(error.to_string()),
            };
        }
    }

    let custom_present = custom_yaml.is_some_and(|s| !s.trim().is_empty());
    if let Some(custom) = custom_yaml.filter(|s| !s.trim().is_empty()) {
        cumulative = cumulative.with_custom_overwrites(custom.to_string());
    }
    match cumulative.execute() {
        Ok(output) => stages.push(stage(CascadeStageId::Merge, custom_present, &output)),
        Err(error) => {
            return CascadeOverlayReport {
                stages,
                merged_yaml: None,
                blocked: true,
                error: Some(error.to_string()),
            };
        }
    }

    if let Some(pre) = pre_mixin {
        cumulative = cumulative.with_pre_mixin(pre.clone());
    }
    match cumulative.execute() {
        Ok(output) => stages.push(stage(
            CascadeStageId::PreMixin,
            pre_mixin.is_some(),
            &output,
        )),
        Err(error) => {
            return CascadeOverlayReport {
                stages,
                merged_yaml: None,
                blocked: true,
                error: Some(error.to_string()),
            };
        }
    }

    if let Some(post) = post_mixin {
        cumulative = cumulative.with_post_mixin(post.clone());
    }
    match cumulative.execute() {
        Ok(merged) => {
            stages.push(stage(
                CascadeStageId::PostMixin,
                post_mixin.is_some(),
                &merged,
            ));
            CascadeOverlayReport {
                stages,
                merged_yaml: Some(merged),
                blocked: false,
                error: None,
            }
        }
        Err(error) => CascadeOverlayReport {
            stages,
            merged_yaml: None,
            blocked: true,
            error: Some(error.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preflight_blocks_a_malformed_overlay() {
        let report = preflight_mixin("mode: rule\n", "mode: [unterminated\n");
        assert!(report.is_blocking());
        assert!(report.error.is_some());
        assert!(report.merged_preview.is_none());
    }

    #[test]
    fn preflight_merges_a_valid_overlay_and_previews_the_output() {
        let report = preflight_mixin("mode: rule\nport: 7890\n", "mode: global\n");
        assert!(report.valid);
        let preview = report.merged_preview.expect("preview");
        assert!(preview.contains("mode: global"));
        assert!(preview.contains("port: 7890"));
    }

    #[test]
    fn toggles_round_trip_through_the_real_codec() {
        let enabled = set_toggle("{}", "ipv6", true).expect("enable");
        assert!(toggle_enabled(&enabled, "ipv6").expect("read"));
        let disabled = set_toggle(&enabled, "ipv6", false).expect("disable");
        assert!(!toggle_enabled(&disabled, "ipv6").expect("read"));
        let with_tun = set_toggle(&disabled, "tun", true).expect("tun");
        assert!(toggle_enabled(&with_tun, "tun").expect("read"));
        assert!(with_tun.contains("stack: system"));
    }

    #[test]
    fn unknown_toggle_is_rejected() {
        assert!(set_toggle("{}", "does-not-exist", true).is_err());
    }

    #[test]
    fn cascade_preview_reports_every_stage_and_the_real_output() {
        let pre = MixinConfig {
            mode: Some("script".to_string()),
            ..Default::default()
        };
        let post = MixinConfig {
            mode: Some("global".to_string()),
            ..Default::default()
        };
        let report = preview_cascade(
            "mode: rule\nport: 7890\n",
            Some("port: 8080\n"),
            None,
            Some(&pre),
            Some(&post),
        );
        assert!(!report.blocked, "{report:?}");
        assert_eq!(report.stages.len(), 5);
        assert_eq!(report.stages[0].id, CascadeStageId::Base);
        assert!(report.stages[1].applied);
        assert!(!report.stages[2].applied);
        assert!(report.stages[4].applied);
        let merged = report.merged_yaml.expect("merged");
        assert!(merged.contains("port: 8080"));
        assert!(merged.contains("mode: global"));
    }

    #[test]
    fn cascade_preview_reports_a_blocked_stage_without_a_merge() {
        let report = preview_cascade("mode: [bad\n", None, None, None, None);
        assert!(report.blocked);
        assert!(report.merged_yaml.is_none());
    }

    #[test]
    fn three_columns_carry_the_real_base_overlay_and_composed_output() {
        let columns = mixin_editor_columns("mode: rule\nport: 7890\n", "mode: global\n");
        assert_eq!(columns.base.content, "mode: rule\nport: 7890\n");
        assert_eq!(columns.base.line_count, 2);
        assert!(!columns.base.editable);
        assert_eq!(columns.overlay.content, "mode: global\n");
        assert!(columns.overlay.editable);
        assert!(columns.is_composed());
        assert!(!columns.is_blocked());
        // The composed column is the real pipeline output, not the overlay.
        assert!(columns.composed.content.contains("port: 7890"));
        assert!(columns.composed.content.contains("mode: global"));
        assert!(!columns.composed.editable);
        assert_eq!(
            columns.composed.line_count,
            columns.composed.content.lines().count()
        );
    }

    #[test]
    fn an_invalid_overlay_blocks_the_composed_column_instead_of_mocking_it() {
        let columns = mixin_editor_columns("mode: rule\n", "mode: [unterminated\n");
        assert!(columns.is_blocked());
        assert!(!columns.is_composed());
        assert!(columns.composed.content.is_empty());
        assert!(columns.error.is_some());
        // The user's own bytes are never dropped from the middle column.
        assert_eq!(columns.overlay.content, "mode: [unterminated\n");
    }
}

//! [缺口02] Mihomo kernel capability matrix.
//!
//! Maps an installed (or candidate) kernel version to the feature set this
//! workspace may rely on (Sniffer, `.mrs` rule sets, VLESS-Reality, TUIC v5,
//! Hysteria2, WireGuard outbound, short `script:`), so callers can decide
//! before writing a config whether the target kernel actually supports it.
//!
//! The thresholds below are **best-effort** data, locked by the fixture tests
//! in this module; they are refreshed alongside the UP-002 fixture updates
//! (the fixtures pin real mihomo releases, so a threshold drift shows up as a
//! test failure rather than silent misclassification). Do not treat them as
//! authoritative upstream documentation.
//!
//! Version-tag policy: `v1.19.18`, `1.19.18` and `v1.19-beta-xxx` all parse
//! to a numeric core (major, minor, patch) with any suffix ignored. Tags with
//! **no numeric core** (e.g. `Alpha-geosite`, bare commit tags) cannot be
//! dated; per the documented decision we treat such a prerelease tag as the
//! newest available build and report **every capability as supported**
//! (`assumed_latest: true`), instead of failing the snapshot. Only input that
//! is neither numeric nor prerelease-shaped (empty, garbage) is an `Err`.

use serde::Serialize;

/// Kernel features tracked by the workspace capability ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    /// Domain/IP sniffer (`sniffer:` config section).
    Sniffer,
    /// `.mrs` compiled rule-set format (`rule-set` with `type: mrs`).
    MrsRuleSet,
    /// VLESS inbound/outbound with Reality security transport.
    VlessReality,
    /// TUIC v5 outbound protocol.
    TuicV5,
    /// Hysteria2 outbound protocol.
    Hysteria2,
    /// WireGuard outbound proxy.
    WireGuardOutbound,
    /// Short script expressions (`script: short:`) for rule matching.
    ShortScript,
}

impl Capability {
    /// All tracked capabilities, stable order (used for serialization and
    /// for building [`CapabilitySet::supported`]).
    pub const ALL: [Capability; 7] = [
        Capability::Sniffer,
        Capability::MrsRuleSet,
        Capability::VlessReality,
        Capability::TuicV5,
        Capability::Hysteria2,
        Capability::WireGuardOutbound,
        Capability::ShortScript,
    ];

    /// Lowercase human-readable name (snake_case, matches the serialized
    /// form used by the admin API).
    pub fn name(&self) -> &'static str {
        match self {
            Capability::Sniffer => "sniffer",
            Capability::MrsRuleSet => "mrs_rule_set",
            Capability::VlessReality => "vless_reality",
            Capability::TuicV5 => "tuic_v5",
            Capability::Hysteria2 => "hysteria2",
            Capability::WireGuardOutbound => "wire_guard_outbound",
            Capability::ShortScript => "short_script",
        }
    }

    /// Minimum `(major, minor, patch)` kernel version providing this
    /// capability. Best-effort thresholds — see the module docs.
    const fn min_version(&self) -> (u64, u64, u64) {
        match self {
            Capability::Sniffer => (1, 14, 0),
            Capability::MrsRuleSet => (1, 18, 1),
            Capability::VlessReality => (1, 18, 0),
            Capability::TuicV5 => (1, 16, 0),
            Capability::Hysteria2 => (1, 16, 2),
            Capability::WireGuardOutbound => (1, 18, 6),
            Capability::ShortScript => (1, 17, 0),
        }
    }
}

/// Capability snapshot of one kernel version.
///
/// `capabilities` lists every supported feature (precomputed so the struct is
/// directly `Serialize`-able for the admin API); `assumed_latest` marks the
/// documented "unparseable prerelease tag" fallback.
#[derive(Debug, Clone, Serialize)]
pub struct CapabilitySet {
    /// Raw version string the snapshot was built from (as stored/tagged).
    pub version: String,
    /// Numeric core of the version, when it could be parsed
    /// (`[major, minor, patch]` in JSON); `null` when assumed latest.
    pub core: Option<[u64; 3]>,
    /// True when the version tag had no numeric core and every capability
    /// was assumed supported (see module docs).
    pub assumed_latest: bool,
    /// Every capability the kernel provides.
    pub capabilities: Vec<Capability>,
}

impl CapabilitySet {
    /// Whether this kernel version provides `cap`.
    pub fn supports(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Whether the kernel provides all of `caps`.
    pub fn supports_all(&self, caps: &[Capability]) -> bool {
        caps.iter().all(|cap| self.supports(*cap))
    }
}

/// Build the capability snapshot for `version`.
///
/// Errors only when the version string is neither numeric (`v1.19.18`,
/// `1.19.18`, `v1.19-beta-xxx` …) nor a recognizable prerelease tag
/// (`alpha`, `beta`, `nightly`, `rc`, `dev` anywhere in the tag).
pub fn capability_snapshot(version: &str) -> Result<CapabilitySet, String> {
    let core = parse_core_version(version);
    let assumed_latest = core.is_none();

    if core.is_none() && !looks_like_prerelease(version) {
        return Err(format!(
            "cannot parse kernel version {version:?} into a capability snapshot"
        ));
    }

    let supported: Vec<Capability> = match core {
        // Unparseable prerelease tag: documented fallback — treat as newest
        // build, everything supported.
        None => Capability::ALL.to_vec(),
        Some(c) => Capability::ALL
            .iter()
            .copied()
            .filter(|cap| c >= cap.min_version())
            .collect(),
    };

    Ok(CapabilitySet {
        version: version.to_string(),
        core: core.map(|(maj, min, pat)| [maj, min, pat]),
        assumed_latest,
        capabilities: supported,
    })
}

/// Parse the numeric core of a mihomo version tag into `(major, minor, patch)`.
///
/// Accepts `v1.19.18`, `1.19.18`, `v1.19-beta-xxx` (→ `(1, 19, 0)`) and
/// similar: an optional leading `v`, then dot-separated components whose
/// leading digit run is the number and whose suffix is ignored. Missing
/// components default to `0`. Returns `None` when the first component carries
/// no digits at all (e.g. `alpha-xxx`).
fn parse_core_version(version: &str) -> Option<(u64, u64, u64)> {
    let trimmed = version.trim();
    let body = trimmed.strip_prefix('v').unwrap_or(trimmed);

    let mut components = body.split('.');
    let major = leading_number(components.next()?)?;
    let minor = components
        .next()
        .and_then(leading_number)
        .unwrap_or(0);
    let patch = components
        .next()
        .and_then(leading_number)
        .unwrap_or(0);

    Some((major, minor, patch))
}

/// Digits prefix of `s` as a number; `None` when `s` does not start with a
/// digit (`19-beta` → `Some(19)`, `beta` → `None`).
fn leading_number(s: &str) -> Option<u64> {
    let digits: &str = &s[..s.chars().take_while(char::is_ascii_digit).count()];
    if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    }
}

/// Heuristic: does the tag look like a prerelease/channel tag rather than
/// plain garbage? Used by [`capability_snapshot`] to decide between the
/// "assume latest" fallback and a hard error.
fn looks_like_prerelease(version: &str) -> bool {
    let lower = version.to_lowercase();
    ["alpha", "beta", "nightly", "rc", "dev"]
        .iter()
        .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- version parsing ----

    #[test]
    fn parse_core_handles_all_tag_shapes() {
        assert_eq!(parse_core_version("v1.19.18"), Some((1, 19, 18)));
        assert_eq!(parse_core_version("1.19.18"), Some((1, 19, 18)));
        assert_eq!(parse_core_version(" v1.19.18 "), Some((1, 19, 18)));
        assert_eq!(parse_core_version("v1.19-beta-xxx"), Some((1, 19, 0)));
        assert_eq!(parse_core_version("v1.19.3-alpha-cb6ac1e"), Some((1, 19, 3)));
        assert_eq!(parse_core_version("v2"), Some((2, 0, 0)));
    }

    #[test]
    fn parse_core_rejects_non_numeric_tags() {
        assert_eq!(parse_core_version("alpha-cb6ac1e"), None);
        assert_eq!(parse_core_version("Alpha-geosite"), None);
        assert_eq!(parse_core_version(""), None);
    }

    // ---- capability_snapshot error path ----

    #[test]
    fn snapshot_errors_on_garbage_version() {
        assert!(capability_snapshot("").is_err());
        assert!(capability_snapshot("not-a-version").is_err());
    }

    // ---- fixture group 1: v1.18.x ----

    #[test]
    fn snapshot_v1_18_1_boundary_matrix() {
        let set = capability_snapshot("v1.18.1").unwrap();
        assert_eq!(set.core, Some([1, 18, 1]));
        assert!(!set.assumed_latest);
        // Exactly at threshold → supported.
        assert!(set.supports(Capability::MrsRuleSet));
        assert!(set.supports(Capability::Sniffer));
        assert!(set.supports(Capability::VlessReality));
        assert!(set.supports(Capability::TuicV5));
        assert!(set.supports(Capability::Hysteria2));
        assert!(set.supports(Capability::ShortScript));
        // 1.18.1 < 1.18.6 → not yet.
        assert!(!set.supports(Capability::WireGuardOutbound));
    }

    #[test]
    fn snapshot_v1_18_0_lacks_mrs() {
        let set = capability_snapshot("v1.18.0").unwrap();
        assert!(set.supports(Capability::VlessReality)); // exactly at threshold
        assert!(!set.supports(Capability::MrsRuleSet)); // one below threshold
    }

    // ---- fixture group 2: v1.19.18 ----

    #[test]
    fn snapshot_v1_19_18_supports_everything() {
        let set = capability_snapshot("v1.19.18").unwrap();
        assert_eq!(set.core, Some([1, 19, 18]));
        assert!(!set.assumed_latest);
        for cap in Capability::ALL {
            assert!(set.supports(cap), "{} should be supported", cap.name());
        }
        assert!(set.supports_all(&Capability::ALL));
    }

    // ---- fixture group 3: alpha tags ----

    #[test]
    fn snapshot_alpha_without_numeric_core_assumes_latest() {
        let set = capability_snapshot("Alpha-geosite-cb6ac1e").unwrap();
        assert!(set.assumed_latest);
        assert_eq!(set.core, None);
        for cap in Capability::ALL {
            assert!(
                set.supports(cap),
                "assumed-latest alpha should support {}",
                cap.name()
            );
        }
    }

    #[test]
    fn snapshot_alpha_with_numeric_core_uses_thresholds() {
        let set = capability_snapshot("v1.19.3-alpha-cb6ac1e").unwrap();
        assert!(!set.assumed_latest);
        assert_eq!(set.core, Some([1, 19, 3]));
        assert!(set.supports(Capability::WireGuardOutbound));
    }

    // ---- serialization for the admin API ----

    #[test]
    fn snapshot_serializes_capability_names() {
        let set = capability_snapshot("v1.18.6").unwrap();
        let json = serde_json::to_value(&set).unwrap();
        assert_eq!(json["version"], "v1.18.6");
        let caps = json["capabilities"].as_array().unwrap();
        let names: Vec<&str> = caps
            .iter()
            .map(|c| c.as_str().unwrap())
            .collect();
        // WireGuard sits exactly on its threshold → included; snake_case names.
        assert!(names.contains(&"wire_guard_outbound"));
        assert!(names.contains(&"sniffer"));
        assert!(!names.contains(&"unknown_capability"));
    }
}

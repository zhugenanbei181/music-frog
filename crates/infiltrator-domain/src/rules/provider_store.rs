//! Rule-provider declarations, kernel cache layout and honest payload
//! deconstruction (DUAL-11-06/07).
//!
//! mihomo resolves a provider's on-disk file exactly like this module does:
//! `type: http` without an explicit `path` caches the download under
//! `<home>/rules/<md5(url)>` (`constant.Path.GetPathByHash("rules", url)`),
//! an explicit `path` (and every `type: file` entry) resolves relative to the
//! same home directory, and `type: inline` never touches the filesystem.
//!
//! Everything here is pure: the caller supplies the profile declaration and
//! the bytes it read, and gets back real [`RuleEntry`] values with an honest
//! skipped count. Nothing fabricates a rule when a source is missing.

use super::{RuleEntry, RuleProviders};
use crate::mrs::unpack_mrs_to_rule_entries;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Directory mihomo uses for downloaded rule-provider files.
pub const PROVIDER_CACHE_DIR_NAME: &str = "rules";

/// Refuse to load an absurd cache file instead of exhausting memory.
pub const MAX_PROVIDER_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// `behavior:` of a rule provider, which decides how a payload line becomes a
/// routing rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderBehavior {
    Domain,
    IpCidr,
    Classical,
    Unknown,
}

impl ProviderBehavior {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "domain" => Self::Domain,
            "ipcidr" | "ip-cidr" => Self::IpCidr,
            "classical" => Self::Classical,
            _ => Self::Unknown,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Domain => "domain",
            Self::IpCidr => "ipcidr",
            Self::Classical => "classical",
            Self::Unknown => "unknown",
        }
    }
}

/// `format:` of a provider payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderFormat {
    Yaml,
    Text,
    Mrs,
    Unknown,
}

impl ProviderFormat {
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "yaml" => Self::Yaml,
            "text" => Self::Text,
            "mrs" => Self::Mrs,
            _ => Self::Unknown,
        }
    }

    /// mihomo defaults `http`/`file` providers to YAML and `inline` payloads
    /// to already-parsed text lines.
    pub fn default_for(provider_type: &str) -> Self {
        match provider_type.trim().to_ascii_lowercase().as_str() {
            "inline" => Self::Text,
            "mrs" => Self::Mrs,
            _ => Self::Yaml,
        }
    }
}

/// One `rule-providers.<name>` declaration, flattened to typed fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleProviderDeclaration {
    pub name: String,
    /// `http` | `file` | `inline` (never invented: an absent `type` stays
    /// empty and the provider is treated as undeclared).
    pub provider_type: String,
    pub behavior: ProviderBehavior,
    pub format: ProviderFormat,
    pub url: Option<String>,
    pub path: Option<String>,
    pub payload: Vec<String>,
}

impl RuleProviderDeclaration {
    pub fn from_value(name: &str, value: &Value) -> Self {
        let provider_type = value
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase();
        let behavior = value
            .get("behavior")
            .and_then(Value::as_str)
            .map(ProviderBehavior::parse)
            .unwrap_or(ProviderBehavior::Unknown);
        let format = value
            .get("format")
            .and_then(Value::as_str)
            .map(ProviderFormat::parse)
            .unwrap_or_else(|| ProviderFormat::default_for(&provider_type));
        let url = value
            .get("url")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_owned);
        let path = value
            .get("path")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned);
        let payload = value
            .get("payload")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::trim)
                    .filter(|line| !line.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            name: name.to_owned(),
            provider_type,
            behavior,
            format,
            url,
            path,
            payload,
        }
    }

    /// Whether the kernel keeps this provider's rules in the profile itself.
    pub fn is_inline(&self) -> bool {
        self.provider_type == "inline"
    }

    /// Whether the kernel caches this provider's rules on disk itself.
    pub fn is_remote(&self) -> bool {
        self.provider_type == "http"
    }
}

/// Flatten the profile's `rule-providers` mapping in declaration order.
pub fn parse_rule_provider_declarations(providers: &RuleProviders) -> Vec<RuleProviderDeclaration> {
    providers
        .iter()
        .map(|(name, value)| RuleProviderDeclaration::from_value(name, value))
        .collect()
}

/// mihomo's `GetPathByHash("rules", url)`: the lowercase hex MD5 of the URL.
pub fn provider_cache_file_name(url: &str) -> String {
    format!("{:x}", md5::compute(url.as_bytes()))
}

/// Which real location a candidate path came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderSourceKind {
    DeclaredFile,
    KernelCacheFile,
}

/// One on-disk location a provider's rules may live in, in read order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSourceCandidate {
    pub path: PathBuf,
    pub kind: ProviderSourceKind,
}

fn resolve_declared(home: &Path, path: &str) -> PathBuf {
    let declared = Path::new(path);
    if declared.is_absolute() {
        declared.to_path_buf()
    } else {
        home.join(declared)
    }
}

/// On-disk candidates for `declaration`, resolved against the kernel home
/// (`-d <config dir>`, the directory that also holds the profiles).
///
/// An inline provider has no file; a `file`/`http` provider with an explicit
/// `path` reads that path first, then the kernel's `rules/<md5(url)>` cache.
pub fn provider_source_candidates(
    declaration: &RuleProviderDeclaration,
    home: &Path,
) -> Vec<ProviderSourceCandidate> {
    if declaration.is_inline() {
        return Vec::new();
    }
    let mut candidates = Vec::new();
    if let Some(path) = declaration.path.as_deref() {
        candidates.push(ProviderSourceCandidate {
            path: resolve_declared(home, path),
            kind: ProviderSourceKind::DeclaredFile,
        });
    }
    if declaration.is_remote()
        && let Some(url) = declaration.url.as_deref()
    {
        let cached = home
            .join(PROVIDER_CACHE_DIR_NAME)
            .join(provider_cache_file_name(url));
        if !candidates.iter().any(|candidate| candidate.path == cached) {
            candidates.push(ProviderSourceCandidate {
                path: cached,
                kind: ProviderSourceKind::KernelCacheFile,
            });
        }
    }
    candidates
}

/// The real rules read out of one provider payload.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProviderDeconstruction {
    pub entries: Vec<RuleEntry>,
    /// Non-comment payload lines that were rejected (empty/unsupported).
    pub skipped: usize,
    /// Rules the payload contained before mapping.
    pub considered: usize,
}

/// Map provider payload lines to routing rules for `behavior`.
///
/// * `domain` payloads carry bare domains → `DOMAIN-SUFFIX,<domain>,<target>`
/// * `ipcidr` payloads carry CIDRs → `IP-CIDR[6],<cidr>,<target>`
/// * `classical` payloads carry complete rules → the target is appended when
///   the line has exactly one field, otherwise the rule is kept verbatim
///
/// Returns the mapped entries and the number of lines that produced nothing.
pub fn unpack_provider_rules_with_behavior(
    rules: &[String],
    behavior: ProviderBehavior,
    target: &str,
) -> (Vec<RuleEntry>, usize) {
    let target = target.trim();
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for raw in rules {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            skipped += 1;
            continue;
        }
        let rule = match behavior {
            ProviderBehavior::Domain => format!("DOMAIN-SUFFIX,{line},{target}"),
            ProviderBehavior::IpCidr => {
                if line.contains(':') {
                    format!("IP-CIDR6,{line},{target}")
                } else {
                    format!("IP-CIDR,{line},{target}")
                }
            }
            ProviderBehavior::Classical | ProviderBehavior::Unknown => {
                if line.contains(',') {
                    let fields = line.split(',').count();
                    if fields == 2 {
                        format!("{line},{target}")
                    } else {
                        line.to_owned()
                    }
                } else {
                    format!("DOMAIN-SUFFIX,{line},{target}")
                }
            }
        };
        entries.push(RuleEntry {
            rule,
            enabled: true,
        });
    }
    (entries, skipped)
}

fn payload_lines_from_yaml(bytes: &[u8]) -> Result<Vec<String>, String> {
    let doc: serde_yaml_ng::Value =
        serde_yaml_ng::from_slice(bytes).map_err(|error| format!("provider yaml: {error}"))?;
    let sequences = ["payload", "rules"];
    let mut lines = Vec::new();
    let mapping = doc
        .as_mapping()
        .ok_or_else(|| "provider yaml is not a mapping with a payload field".to_owned())?;
    for key in sequences {
        let Some(value) = mapping.get(serde_yaml_ng::Value::String(key.to_owned())) else {
            continue;
        };
        let Some(items) = value.as_sequence() else {
            continue;
        };
        for item in items {
            if let Some(line) = item.as_str() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_owned());
                }
            }
        }
    }
    if lines.is_empty() {
        return Err("provider yaml declares no payload/rules entries".to_owned());
    }
    Ok(lines)
}

fn payload_lines_from_text(bytes: &[u8]) -> Result<Vec<String>, String> {
    let text = std::str::from_utf8(bytes).map_err(|error| format!("provider text: {error}"))?;
    let lines: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#') && !line.starts_with("//"))
        .map(str::to_owned)
        .collect();
    if lines.is_empty() {
        return Err("provider text contains no rules".to_owned());
    }
    Ok(lines)
}

/// Turn a provider's raw bytes into real rule entries for `target`.
///
/// `format: mrs` goes through the binary deconstructor; `yaml`/`text` go
/// through the behaviour-aware mapping. An unknown format is retried as YAML
/// and then as text so a mis-declared `format` still reads real rules.
pub fn deconstruct_provider_payload(
    bytes: &[u8],
    declaration: &RuleProviderDeclaration,
    target: &str,
) -> Result<ProviderDeconstruction, String> {
    if bytes.is_empty() {
        return Err(format!(
            "provider {} has an empty payload",
            declaration.name
        ));
    }
    if bytes.len() as u64 > MAX_PROVIDER_FILE_BYTES {
        return Err(format!(
            "provider {} payload is {} bytes, above the {} byte safety cap",
            declaration.name,
            bytes.len(),
            MAX_PROVIDER_FILE_BYTES
        ));
    }
    let lines = match declaration.format {
        ProviderFormat::Mrs => {
            let entries = unpack_mrs_to_rule_entries(bytes, target)
                .map_err(|error| format!("provider mrs: {error}"))?;
            return Ok(ProviderDeconstruction {
                considered: entries.len(),
                entries,
                skipped: 0,
            });
        }
        ProviderFormat::Yaml => payload_lines_from_yaml(bytes)?,
        ProviderFormat::Text => payload_lines_from_text(bytes)?,
        ProviderFormat::Unknown => match payload_lines_from_yaml(bytes) {
            Ok(lines) => lines,
            Err(yaml_error) => payload_lines_from_text(bytes)
                .map_err(|text_error| format!("{yaml_error}; {text_error}"))?,
        },
    };
    let considered = lines.len();
    let (entries, skipped) =
        unpack_provider_rules_with_behavior(&lines, declaration.behavior, target);
    if entries.is_empty() {
        return Err(format!(
            "provider {} payload produced no usable rules",
            declaration.name
        ));
    }
    Ok(ProviderDeconstruction {
        entries,
        skipped,
        considered,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn declaration(name: &str, value: Value) -> RuleProviderDeclaration {
        RuleProviderDeclaration::from_value(name, &value)
    }

    #[test]
    fn declaration_flattens_real_profile_fields() {
        let decl = declaration(
            "ads",
            json!({
                "type": "http",
                "behavior": "domain",
                "format": "mrs",
                "url": "https://example.com/ads.mrs",
                "interval": 86400
            }),
        );
        assert_eq!(decl.provider_type, "http");
        assert_eq!(decl.behavior, ProviderBehavior::Domain);
        assert_eq!(decl.format, ProviderFormat::Mrs);
        assert_eq!(decl.url.as_deref(), Some("https://example.com/ads.mrs"));
        assert!(!decl.is_inline());
        assert!(decl.is_remote());

        let inline = declaration(
            "local",
            json!({ "type": "inline", "payload": ["a.com", " ", "b.com"] }),
        );
        assert!(inline.is_inline());
        assert_eq!(inline.format, ProviderFormat::Text);
        assert_eq!(inline.payload, vec!["a.com".to_owned(), "b.com".to_owned()]);
    }

    #[test]
    fn cache_name_matches_mihomo_url_hash() {
        // Reference values produced by mihomo's `utils.MakeHash` (hex MD5).
        assert_eq!(
            provider_cache_file_name("https://example.com/ads.mrs"),
            "a14446282dba250d073427778acf560b"
        );
        assert_eq!(
            provider_cache_file_name(
                "https://raw.githubusercontent.com/MetaCubeX/meta-rules-dat/meta/geo/geosite/cn.yaml"
            ),
            "07225f3cebe3fe2955706742c6cbe5b0"
        );
        assert_ne!(
            provider_cache_file_name("https://example.com/ads.mrs"),
            provider_cache_file_name("https://example.com/other.mrs")
        );
    }

    #[test]
    fn candidates_follow_mihomo_resolution_order() {
        let home = Path::new("/kernel");
        let remote = declaration("ads", json!({ "type": "http", "url": "https://e.com/a" }));
        let candidates = provider_source_candidates(&remote, home);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, ProviderSourceKind::KernelCacheFile);
        assert_eq!(
            candidates[0].path,
            home.join("rules")
                .join(provider_cache_file_name("https://e.com/a"))
        );

        let with_path = declaration(
            "ads",
            json!({ "type": "http", "url": "https://e.com/a", "path": "cache/ads.yaml" }),
        );
        let candidates = provider_source_candidates(&with_path, home);
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].kind, ProviderSourceKind::DeclaredFile);
        assert_eq!(candidates[0].path, home.join("cache/ads.yaml"));

        let local = declaration(
            "local",
            json!({ "type": "file", "path": "assets/local.yaml" }),
        );
        let candidates = provider_source_candidates(&local, home);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].kind, ProviderSourceKind::DeclaredFile);

        let inline = declaration("inline", json!({ "type": "inline", "payload": ["a.com"] }));
        assert!(provider_source_candidates(&inline, home).is_empty());
    }

    #[test]
    fn behaviour_maps_payload_lines_to_real_rules() {
        let (domain, skipped) = unpack_provider_rules_with_behavior(
            &["a.com".to_owned(), "# c".to_owned(), "".to_owned()],
            ProviderBehavior::Domain,
            "PROXY",
        );
        assert_eq!(domain.len(), 1);
        assert_eq!(domain[0].rule, "DOMAIN-SUFFIX,a.com,PROXY");
        assert_eq!(skipped, 2);

        let (cidr, _) = unpack_provider_rules_with_behavior(
            &["10.0.0.0/8".to_owned(), "2001:db8::/32".to_owned()],
            ProviderBehavior::IpCidr,
            "DIRECT",
        );
        assert_eq!(cidr[0].rule, "IP-CIDR,10.0.0.0/8,DIRECT");
        assert_eq!(cidr[1].rule, "IP-CIDR6,2001:db8::/32,DIRECT");

        let (classical, _) = unpack_provider_rules_with_behavior(
            &[
                "DOMAIN-SUFFIX,a.com".to_owned(),
                "IP-CIDR,1.1.1.1/32,DIRECT,no-resolve".to_owned(),
            ],
            ProviderBehavior::Classical,
            "PROXY",
        );
        assert_eq!(classical[0].rule, "DOMAIN-SUFFIX,a.com,PROXY");
        assert_eq!(classical[1].rule, "IP-CIDR,1.1.1.1/32,DIRECT,no-resolve");
    }

    #[test]
    fn plain_payload_deconstructs_without_fabrication() {
        let decl = declaration(
            "cn",
            json!({ "type": "http", "behavior": "domain", "format": "text", "url": "https://e.com/cn" }),
        );
        let payload = b"# comment\nexample.cn\nhupu.com\n";
        let result = deconstruct_provider_payload(payload, &decl, "PROXY").expect("deconstruct");
        assert_eq!(result.considered, 2);
        assert_eq!(result.skipped, 0);
        assert_eq!(result.entries[0].rule, "DOMAIN-SUFFIX,example.cn,PROXY");
        assert_eq!(result.entries[1].rule, "DOMAIN-SUFFIX,hupu.com,PROXY");
        assert!(result.entries.iter().all(|entry| entry.enabled));

        assert!(
            deconstruct_provider_payload(b"", &decl, "PROXY")
                .expect_err("empty payload")
                .contains("empty")
        );
        assert!(deconstruct_provider_payload(b"# only\n", &decl, "PROXY").is_err());
    }

    #[test]
    fn yaml_payload_and_mrs_payload_deconstruct() {
        let decl = declaration(
            "ads",
            json!({ "type": "file", "behavior": "classical", "format": "yaml", "path": "ads.yaml" }),
        );
        let yaml = b"payload:\n  - DOMAIN-SUFFIX,ads.com\n  - DOMAIN-KEYWORD,tracker\n";
        let result = deconstruct_provider_payload(yaml, &decl, "REJECT").expect("yaml");
        assert_eq!(result.entries.len(), 2);
        assert_eq!(result.entries[0].rule, "DOMAIN-SUFFIX,ads.com,REJECT");

        let bytes = crate::mrs::build_mrs_bytes(
            crate::mrs::Behavior::Domain,
            1,
            2,
            "test",
            b"ads.com\ntracker.net\n",
            Some(crate::mrs::MAGIC_STANDARD_MRS),
        );
        let decl = declaration(
            "ads",
            json!({ "type": "file", "behavior": "domain", "format": "mrs", "path": "ads.mrs" }),
        );
        let result = deconstruct_provider_payload(&bytes, &decl, "REJECT").expect("mrs");
        assert_eq!(result.entries.len(), 2);
        assert!(
            result
                .entries
                .iter()
                .all(|entry| entry.rule.ends_with(",REJECT"))
        );
    }
}

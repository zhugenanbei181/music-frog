//! Conversion between profile YAML documents and typed node lists.
//!
//! These are the entry points consumers use: parse a profile into
//! [`RawNode`]s, serialize nodes back into a profile document, or splice an
//! edited node list back into an existing profile without touching its other
//! sections.

use anyhow::{Context, anyhow};
use serde_yaml_ng::{Mapping, Value};

use super::model::RawNode;

/// Parse a profile YAML document and return its `proxies:` list.
///
/// Profiles without a `proxies:` key (or with an empty/null one) yield an
/// empty vec. Nodes the typed models cannot represent degrade to
/// [`ProxyNode`](super::model::ProxyNode) instead of failing; only a
/// non-mapping document, a non-list `proxies:` key, or an entry without a
/// string `type` is an error.
pub fn parse_profile_yaml(text: &str) -> anyhow::Result<Vec<RawNode>> {
    let doc: Value = serde_yaml_ng::from_str(text).context("parse profile yaml")?;
    extract_nodes_from_doc(&doc)
}

/// Extract nodes from an already parsed profile document.
pub fn extract_nodes_from_doc(doc: &Value) -> anyhow::Result<Vec<RawNode>> {
    if !doc.is_mapping() {
        return Err(anyhow!("profile yaml must be a top-level mapping"));
    }
    match doc.get("proxies") {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Sequence(entries)) => {
            let mut nodes = Vec::with_capacity(entries.len());
            for (index, entry) in entries.iter().enumerate() {
                let node: RawNode = serde_yaml_ng::from_value(entry.clone())
                    .with_context(|| format!("failed to decode proxies[{index}]"))?;
                nodes.push(node);
            }
            Ok(nodes)
        }
        Some(_) => Err(anyhow!("`proxies` must be a list of node mappings")),
    }
}

/// Serialize nodes into a minimal profile document containing only the
/// `proxies:` section. The output re-parses to exactly `nodes`.
pub fn nodes_to_profile_yaml(nodes: &[RawNode]) -> anyhow::Result<String> {
    let mut doc = Mapping::new();
    let proxies = serde_yaml_ng::to_value(nodes).context("encode proxies nodes")?;
    doc.insert(Value::String("proxies".to_string()), proxies);
    serde_yaml_ng::to_string(&Value::Mapping(doc)).context("serialize proxies yaml")
}

/// Replace (or insert) the `proxies:` section of a profile document with
/// `nodes`, leaving every other section untouched. Useful to write parsed
/// and edited nodes back without losing the rest of the profile.
pub fn replace_proxies_in_profile(text: &str, nodes: &[RawNode]) -> anyhow::Result<String> {
    let mut doc: Value = serde_yaml_ng::from_str(text).context("parse profile yaml")?;
    if !doc.is_mapping() {
        return Err(anyhow!("profile yaml must be a top-level mapping"));
    }
    let proxies = serde_yaml_ng::to_value(nodes).context("encode proxies nodes")?;
    if let Some(map) = doc.as_mapping_mut() {
        map.insert(Value::String("proxies".to_string()), proxies);
    }
    serde_yaml_ng::to_string(&doc).context("serialize profile yaml")
}

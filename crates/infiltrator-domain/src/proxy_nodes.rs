//! Typed models for Meta-specific proxy node fields ([缺口04], CORE-003).
//!
//! mihomo (Meta) profiles carry a `proxies:` list whose entries mix a handful
//! of universal keys (`name`, `type`, `server`, `port`, `udp`, ...) with
//! protocol-specific fields (VLESS REALITY options, Hysteria2 obfs and
//! bandwidth caps, TUIC congestion control, WireGuard keys and reserved
//! bytes, ...). This module gives those fields strong Rust types while
//! guaranteeing **zero data loss** on a YAML roundtrip, so a parsed profile
//! can always be written back unchanged.
//!
//! # Zero-loss mechanism
//!
//! [`ProxyNode`] is a `#[serde(untagged)]` enum tried in declaration order:
//!
//! 1. [`VlessNode`], [`Hysteria2Node`], [`TuicNode`] and [`WireGuardNode`]
//!    strongly type the known fields. Each of them carries a discriminant
//!    field `node_type` (serialized as the `type` key) plus
//!    `#[serde(flatten)] extra: BTreeMap<String, serde_yaml_ng::Value>`.
//!    The flatten catch-all is *the* key mechanism for losslessness: serde
//!    routes every key the model does not know about into `extra`, and
//!    serialization emits `extra` back into the mapping. Unknown fields
//!    (e.g. a future `fake-field: 123`) therefore survive the
//!    `YAML -> typed -> YAML` roundtrip untouched.
//! 2. If every typed variant fails to decode — an unknown `type` value such
//!    as `ss`/`vmess`/`trojan`, or a known field holding a value the model
//!    rejects (e.g. `port: "abc"`) — the node degrades to [`OtherNode`],
//!    which keeps the raw `type` string plus **every** other key verbatim in
//!    a single map. Degrading never drops data; run [`validate`] on the
//!    parsed nodes to surface the underlying problems instead.
//! 3. Only entries that are not mappings, or that lack a string `type` key,
//!    are rejected outright (mihomo rejects them too).
//!
//! Note on representation: `#[serde(tag = "type")]` (internally tagged enum)
//! would consume the `type` key, which makes a lossless fallback variant for
//! unknown `type` values impossible — an untagged variant inside a tagged
//! enum is serialized without its tag, so the `type` key would be lost on
//! the way back to YAML. Modeling the tag as an ordinary discriminating
//! field per variant keeps the exact same wire shape (`type: vless`, ...) as
//! mihomo expects while remaining fully reversible.
//!
//! Guarantees (all covered by this module's tests):
//! * `parse_profile_yaml(text)` followed by `nodes_to_profile_yaml(&nodes)`
//!   reproduces the `proxies:` section with **semantic equivalence** at the
//!   `serde_yaml_ng::Value` level (key order may differ; values do not).
//! * `typed -> YAML -> typed` roundtrip is a fixed point, and repeated
//!   serialization is byte-stable.
//! * [`Reserved`] keeps both wire shapes — the `[1, 2, 3]` list form and the
//!   base64 string form — exactly as written.
//!
//! Layout (semantic submodules):
//! * `model` — typed node/field structs and their accessors
//! * `profile_yaml` — profile YAML document <-> node list conversion
//! * `validate` — advisory per-protocol validation rules

pub mod model;
pub mod profile_yaml;
#[cfg(test)]
mod proxy_nodes_test;
pub mod validate;

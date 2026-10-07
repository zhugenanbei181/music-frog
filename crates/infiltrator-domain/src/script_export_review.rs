//! Stable content identity and draft package construction without host writes.
use crate::script_engine::{ExtensionPackage, ScriptEngine};
use sha2::{Digest, Sha256};

pub fn content_sha256(content: &str) -> String {
    Sha256::digest(content.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn draft_package(
    name: &str,
    profile: Option<&str>,
    script_code: &str,
    mixin_yaml: Option<&str>,
    preset: Option<&str>,
) -> ExtensionPackage {
    ExtensionPackage {
        name: name.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        author: "MusicFrog".to_owned(),
        description: profile
            .map(|name| format!("Directive DSL export from {name}"))
            .unwrap_or_else(|| "Directive DSL sandbox export".to_owned()),
        stage: preset
            .and_then(ScriptEngine::find_preset)
            .map(|preset| preset.stage)
            .unwrap_or_default(),
        script_code: script_code.to_owned(),
        mixin_yaml: mixin_yaml
            .filter(|yaml| !yaml.trim().is_empty())
            .map(str::to_owned),
        tags: vec!["music-frog".to_owned(), "directive-dsl".to_owned()],
    }
}

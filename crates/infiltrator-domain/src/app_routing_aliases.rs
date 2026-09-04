//! Cross-platform process alias registration and canonicalization.

use std::collections::HashMap;

use super::{BUILTIN_ALIAS_TABLE, ProcessAliasRegistry};

impl Default for ProcessAliasRegistry {
    fn default() -> Self {
        let mut registry = Self {
            aliases: HashMap::new(),
        };
        registry.load_builtins();
        registry
    }
}

impl ProcessAliasRegistry {
    pub fn empty() -> Self {
        Self {
            aliases: HashMap::new(),
        }
    }

    pub fn new() -> Self {
        Self::default()
    }

    fn normalize_key(key: &str) -> String {
        key.trim().to_ascii_lowercase()
    }

    pub fn register_alias(&mut self, alias: &str, canonical: &str) {
        let key = Self::normalize_key(alias);
        let val = canonical.trim().to_ascii_lowercase();
        if !key.is_empty() && !val.is_empty() {
            self.aliases.insert(key, val);
        }
    }

    pub fn register_aliases<I, S>(&mut self, aliases: I, canonical: &str)
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for alias in aliases {
            self.register_alias(alias.as_ref(), canonical);
        }
    }

    pub fn get_canonical(&self, name: &str) -> Option<&str> {
        let key = Self::normalize_key(name);
        self.aliases.get(&key).map(|s| s.as_str())
    }

    pub fn canonicalize(&self, raw_name: &str) -> String {
        let trimmed = raw_name.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        if let Some(canonical) = self.get_canonical(trimmed) {
            return canonical.to_string();
        }

        let filename = trimmed.rsplit(['/', '\\']).next().unwrap_or(trimmed).trim();

        if let Some(canonical) = self.get_canonical(filename) {
            return canonical.to_string();
        }

        let stem = filename
            .strip_suffix(".exe")
            .or_else(|| filename.strip_suffix(".EXE"))
            .or_else(|| filename.strip_suffix(".app"))
            .or_else(|| filename.strip_suffix(".APP"))
            .or_else(|| filename.strip_suffix(".bin"))
            .or_else(|| filename.strip_suffix(".real"))
            .unwrap_or(filename);

        if let Some(canonical) = self.get_canonical(stem) {
            return canonical.to_string();
        }

        if stem.contains('.')
            && let Some(last_segment) = stem.rsplit('.').next()
            && let Some(canonical) = self.get_canonical(last_segment)
        {
            return canonical.to_string();
        }

        let lower = stem.to_ascii_lowercase();

        for suffix in [
            "-stable", "-beta", "-nightly", "-dev", "-bin", "-desktop", "-browser",
        ] {
            if let Some(stripped) = lower.strip_suffix(suffix)
                && let Some(canonical) = self.get_canonical(stripped)
            {
                return canonical.to_string();
            }
        }

        lower
    }

    pub fn canonicalize_name(raw_name: &str) -> String {
        let registry = Self::default();
        registry.canonicalize(raw_name)
    }

    pub fn len(&self) -> usize {
        self.aliases.len()
    }

    pub fn is_empty(&self) -> bool {
        self.aliases.is_empty()
    }

    fn load_builtins(&mut self) {
        for (aliases, canonical) in BUILTIN_ALIAS_TABLE {
            for alias in *aliases {
                self.register_alias(alias, canonical);
            }
        }
    }
}

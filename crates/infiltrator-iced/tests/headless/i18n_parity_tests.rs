//! Headless checks for the shared locale tables and Iced view references.

use infiltrator_shared::locales::{Lang, Localizer};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

fn shared_source(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../infiltrator-shared/src")
        .join(file)
}

fn keys_of(file: &str) -> BTreeSet<String> {
    let content = fs::read_to_string(shared_source(file)).expect("locale source must be readable");
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let rest = trimmed.strip_prefix('"')?;
            let end = rest.find('"')?;
            let key = &rest[..end];
            rest[end + 1..].contains("=>").then(|| key.to_string())
        })
        .collect()
}

fn all_locale_keys() -> BTreeSet<String> {
    [
        keys_of("locales_table.rs"),
        keys_of("locales_table_legacy.rs"),
        keys_of("locales_table_ext.rs"),
        keys_of("locales_table_en.rs"),
        keys_of("locales_table_en_legacy.rs"),
        keys_of("locales_table_en_ext.rs"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn contains_cjk(value: &str) -> bool {
    value
        .chars()
        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
}

fn collect_rust_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn collect_translation_refs(path: &Path, refs: &mut BTreeSet<String>) {
    let content = fs::read_to_string(path).expect("view source must be readable");
    let needle = ".tr(\"";
    let mut offset = 0;
    while let Some(found) = content[offset..].find(needle) {
        let start = offset + found + needle.len();
        let Some(end) = content[start..].find('"') else {
            break;
        };
        refs.insert(content[start..start + end].to_string());
        offset = start + end + 1;
    }
}

#[test]
fn zh_and_en_tables_have_exact_key_parity() {
    let zh = [
        keys_of("locales_table.rs"),
        keys_of("locales_table_legacy.rs"),
        keys_of("locales_table_ext.rs"),
    ]
    .into_iter()
    .flatten()
    .collect::<BTreeSet<_>>();
    let en = [
        keys_of("locales_table_en.rs"),
        keys_of("locales_table_en_legacy.rs"),
        keys_of("locales_table_en_ext.rs"),
    ]
    .into_iter()
    .flatten()
    .collect::<BTreeSet<_>>();

    assert_eq!(zh, en, "zh-CN and en-US locale keys must remain identical");
}

#[test]
fn every_locale_key_resolves_in_both_locales() {
    let zh = Lang("zh-CN");
    let en = Lang("en-US");
    for key in all_locale_keys() {
        assert_ne!(zh.tr(&key), key, "zh-CN key {key} fell back to its key");
        assert_ne!(en.tr(&key), key, "en-US key {key} fell back to its key");
    }
}

#[test]
fn english_locale_copy_contains_no_cjk() {
    let en = Lang("en-US");
    for key in all_locale_keys() {
        let copy = en.tr(&key);
        assert!(!contains_cjk(&copy), "en-US key {key} contains CJK: {copy}");
    }
}

#[test]
fn every_static_view_translation_reference_exists() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rust_files(&manifest_dir.join("src/view"), &mut files);
    collect_rust_files(&manifest_dir.join("src/view_root"), &mut files);

    let mut refs = BTreeSet::new();
    for file in files {
        collect_translation_refs(&file, &mut refs);
    }

    let known = all_locale_keys();
    let missing: Vec<_> = refs.difference(&known).cloned().collect();
    assert!(
        missing.is_empty(),
        "view references missing locale keys: {missing:?}"
    );
}

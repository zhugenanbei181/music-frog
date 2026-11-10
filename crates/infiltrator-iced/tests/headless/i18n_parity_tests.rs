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
    // Every catalog table in the shared crate, discovered rather than listed:
    // the hand-kept list this test started with drifted behind the catalog
    // (`locales_table_shared_copy_*` and `locales_table_errors_*` were added
    // later), so a view could reference a key the test could not see and still
    // pass. Only the test-support table is skipped: it holds no product copy.
    catalog_tables()
        .into_iter()
        .flat_map(|path| keys_of(&path))
        .collect()
}

fn catalog_tables() -> Vec<String> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../infiltrator-shared/src");
    let mut tables: Vec<String> = fs::read_dir(&directory)
        .expect("the shared locale catalog must be readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| {
            name.starts_with("locales_table")
                && name.ends_with(".rs")
                && name != "locales_table_tests.rs"
        })
        .collect();
    tables.sort();
    assert!(
        tables.len() >= 15,
        "the catalog discovery found only {} tables: {tables:?}",
        tables.len()
    );
    tables
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
    // Both sides come from the same discovery as the reference check, so a new
    // table cannot be compared in one direction only: the un-suffixed tables
    // and the `_zh` ones are one side, the `_en` family the other.
    // The English family names its locale either right after the stem
    // (`locales_table_en_legacy.rs`) or as the file's last segment
    // (`locales_table_workflow_en.rs`); the Chinese family uses the
    // un-suffixed stem or `_zh`. Both forms must be caught, or one side absorbs
    // the other's keys and the parity check silently compares a set with
    // itself.
    let (en_tables, zh_tables): (Vec<String>, Vec<String>) = catalog_tables()
        .into_iter()
        .partition(|name| name.starts_with("locales_table_en") || name.ends_with("_en.rs"));
    assert!(
        !en_tables.is_empty() && !zh_tables.is_empty(),
        "both sides of the parity check must find tables: zh={zh_tables:?} en={en_tables:?}"
    );
    let zh: BTreeSet<String> = zh_tables.iter().flat_map(|name| keys_of(name)).collect();
    let en: BTreeSet<String> = en_tables.iter().flat_map(|name| keys_of(name)).collect();

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

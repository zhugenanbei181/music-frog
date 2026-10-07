//! Shared localized command labels; renderers retain their native row layout.
use infiltrator_contract::command_catalogue::{CommandCatalogue, CommandEntry};

pub fn localized_title(entry: &CommandEntry, translate: &impl Fn(&str) -> String) -> String {
    match entry.profile_name() {
        Some(name) => format!("{}: {name}", translate(entry.title_key)),
        None => translate(entry.title_key),
    }
}

pub fn filtered_indices(
    catalogue: &CommandCatalogue,
    query: &str,
    translate: &impl Fn(&str) -> String,
    fuzzy: &impl Fn(&str, &str) -> bool,
) -> Vec<usize> {
    let query = query.trim();
    if query.is_empty() {
        return (0..catalogue.len()).collect();
    }
    catalogue
        .entries()
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            entry.matches(query)
                || fuzzy(&entry.title_zh, query)
                || fuzzy(&translate(entry.title_key), query)
                || fuzzy(&translate(entry.category.label_key()), query)
                || fuzzy(&entry.id, query)
        })
        .map(|(index, _)| index)
        .collect()
}

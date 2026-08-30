//! Tray menu item ID generation and ID-to-entity mappings.
//!
//! Dynamic entries (profiles, proxy nodes) get hash-based menu IDs; the maps
//! built here let the menu event handler resolve an ID back to the entity it
//! selects.

use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
};

pub(crate) fn build_menu_id(prefix: &str, key: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();
    format!("{prefix}-{hash:016x}")
}

pub(crate) fn insert_profile_menu_id(
    profile_map: &mut HashMap<String, String>,
    profile_name: &str,
) -> String {
    let base_id = build_menu_id("profile-switch", profile_name);
    let mut menu_id = base_id.clone();
    let mut counter = 1u32;
    while profile_map.contains_key(&menu_id) {
        menu_id = format!("{base_id}-{counter}");
        counter = counter.saturating_add(1);
    }
    profile_map.insert(menu_id.clone(), profile_name.to_string());
    menu_id
}

pub(crate) fn insert_proxy_menu_id(
    proxy_map: &mut HashMap<String, (String, String)>,
    group_name: &str,
    node_name: &str,
) -> String {
    let base_id = build_menu_id("proxy", &format!("{group_name}\n{node_name}"));
    let mut menu_id = base_id.clone();
    let mut counter = 1u32;
    while proxy_map.contains_key(&menu_id) {
        menu_id = format!("{base_id}-{counter}");
        counter = counter.saturating_add(1);
    }
    proxy_map.insert(
        menu_id.clone(),
        (group_name.to_string(), node_name.to_string()),
    );
    menu_id
}

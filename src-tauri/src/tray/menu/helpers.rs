//! Shared label formatting and submenu manipulation helpers for the tray menu.

use tauri::{
    Wry,
    menu::{IsMenuItem, Submenu},
};

pub(crate) fn truncate_label(value: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return "...".to_string();
    }
    let take_len = max_chars.saturating_sub(3);
    let mut truncated: String = chars.into_iter().take(take_len).collect();
    truncated.push_str("...");
    truncated
}

pub(super) fn clear_submenu_items(submenu: &Submenu<Wry>) -> tauri::Result<()> {
    loop {
        let items = submenu.items()?;
        if items.is_empty() {
            break;
        }
        let _ = submenu.remove_at(0)?;
    }
    Ok(())
}

pub(super) fn append_items_to_submenu(
    submenu: &Submenu<Wry>,
    items: &[Box<dyn IsMenuItem<Wry>>],
) -> tauri::Result<()> {
    for item in items {
        submenu.append(item.as_ref())?;
    }
    Ok(())
}

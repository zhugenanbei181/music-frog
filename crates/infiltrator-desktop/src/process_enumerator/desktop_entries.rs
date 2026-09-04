use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Linux `.desktop` entry representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopEntry {
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub categories: Vec<String>,
}

/// Scanner for Linux desktop entries to enrich process metadata with application icons and names.
#[derive(Debug, Clone, Default)]
pub struct DesktopEntryScanner {
    entries: HashMap<String, DesktopEntry>,
}

impl DesktopEntryScanner {
    /// Creates a scanner and scans standard XDG application directories.
    pub fn new() -> Self {
        let mut scanner = Self {
            entries: HashMap::new(),
        };
        scanner.scan_standard_paths();
        scanner
    }

    /// Parses a raw `.desktop` file content.
    pub fn parse_desktop_file(content: &str) -> Option<DesktopEntry> {
        let mut name = None;
        let mut generic_name = None;
        let mut comment = None;
        let mut exec = None;
        let mut icon = None;
        let mut categories = Vec::new();
        let mut in_desktop_entry = false;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                in_desktop_entry = trimmed == "[Desktop Entry]";
                continue;
            }
            if !in_desktop_entry || trimmed.starts_with('#') || !trimmed.contains('=') {
                continue;
            }

            if let Some((key, val)) = trimmed.split_once('=') {
                let k = key.trim();
                let v = val.trim();
                match k {
                    "Name" => {
                        if name.is_none() {
                            name = Some(v.to_string());
                        }
                    }
                    "GenericName" => {
                        if generic_name.is_none() {
                            generic_name = Some(v.to_string());
                        }
                    }
                    "Comment" => {
                        if comment.is_none() {
                            comment = Some(v.to_string());
                        }
                    }
                    "Exec" => {
                        if exec.is_none() {
                            let clean_exec = v
                                .split_whitespace()
                                .next()
                                .unwrap_or(v)
                                .trim_matches('"')
                                .to_string();
                            exec = Some(clean_exec);
                        }
                    }
                    "Icon" => {
                        if icon.is_none() {
                            icon = Some(v.to_string());
                        }
                    }
                    "Categories" => {
                        categories = v
                            .split(';')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect();
                    }
                    _ => {}
                }
            }
        }

        let name = name?;
        let exec = exec.unwrap_or_else(|| name.to_lowercase());

        Some(DesktopEntry {
            name,
            generic_name,
            comment,
            exec,
            icon,
            categories,
        })
    }

    fn scan_standard_paths(&mut self) {
        let paths = [
            "/usr/share/applications",
            "/usr/local/share/applications",
            "/var/lib/flatpak/exports/share/applications",
        ];

        for path_str in paths {
            let path = Path::new(path_str);
            if !path.exists() || !path.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(path) {
                for entry in entries.filter_map(|r| r.ok()) {
                    let file_path = entry.path();
                    if file_path.extension().and_then(|e| e.to_str()) == Some("desktop")
                        && let Ok(content) = std::fs::read_to_string(&file_path)
                            && let Some(desktop_entry) = Self::parse_desktop_file(&content) {
                                let key = desktop_entry
                                    .exec
                                    .rsplit('/')
                                    .next()
                                    .unwrap_or(&desktop_entry.exec)
                                    .to_ascii_lowercase();
                                self.entries.insert(key, desktop_entry);
                            }
                }
            }
        }
    }

    /// Looks up desktop metadata by binary name.
    pub fn lookup(&self, binary_name: &str) -> Option<&DesktopEntry> {
        let clean = binary_name.trim().to_ascii_lowercase();
        let stem = clean.strip_suffix(".exe").unwrap_or(&clean);
        self.entries.get(stem)
    }
}

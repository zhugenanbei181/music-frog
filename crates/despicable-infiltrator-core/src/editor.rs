use anyhow::anyhow;
use std::env;
use std::path::PathBuf;
use std::process::Command;

use crate::profiles;

#[cfg(target_os = "windows")]
use std::path::Path;

pub async fn open_profile_in_editor(
    editor_path: Option<String>,
    name: &str,
) -> anyhow::Result<()> {
    let profile = profiles::load_profile_info(name).await?;
    let auto_detect = editor_path
        .as_ref()
        .map(|value| value.trim().is_empty())
        .unwrap_or(true);
    let (editor, args) = resolve_editor_command(editor_path.as_deref())?;
    let profile_path = profile.path.clone();
    let mut final_args = args;
    final_args.push(profile_path.clone());
    if let Err(err) = open_in_editor(&editor, &final_args) {
        if auto_detect {
            if let Some((fallback, mut fallback_args)) = fallback_editor_command(&editor) {
                fallback_args.push(profile_path);
                if open_in_editor(&fallback, &fallback_args).is_ok() {
                    return Ok(());
                }
            }
        }
        return Err(err);
    }
    Ok(())
}

fn open_in_editor(editor: &str, args: &[String]) -> anyhow::Result<()> {
    let mut command = build_editor_command(editor, args);
    command
        .spawn()
        .map_err(|e| anyhow!("无法打开编辑器 {editor}: {e}"))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn build_editor_command(editor: &str, args: &[String]) -> Command {
    let extension = Path::new(editor)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    if matches!(extension.as_deref(), Some("cmd") | Some("bat")) {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(editor).args(args);
        return command;
    }
    let mut command = Command::new(editor);
    command.args(args);
    command
}

#[cfg(not(target_os = "windows"))]
fn build_editor_command(editor: &str, args: &[String]) -> Command {
    let mut command = Command::new(editor);
    command.args(args);
    command
}

fn resolve_editor_command(editor_path: Option<&str>) -> anyhow::Result<(String, Vec<String>)> {
    if let Some(path) = editor_path {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return default_editor_command();
        }
        let expanded = expand_env_vars(trimmed);
        let (command, args) = parse_command_line(&expanded);
        if command.trim().is_empty() {
            return Err(anyhow!(
                "编辑器命令为空（可清空以自动使用 VSCode/记事本）"
            ));
        }
        if is_path_like(&command) {
            let candidate = PathBuf::from(&command);
            if !candidate.is_file() {
                return Err(anyhow!(
                    "未找到编辑器路径: {command}（可清空以自动使用 VSCode/记事本）"
                ));
            }
        } else if let Some(resolved) = resolve_command_path(&command) {
            return Ok((resolved.to_string_lossy().to_string(), args));
        } else {
            return Err(anyhow!(
                "未找到编辑器命令: {command}（可清空以自动使用 VSCode/记事本）"
            ));
        }
        return Ok((command, args));
    }
    default_editor_command()
}

fn default_editor_command() -> anyhow::Result<(String, Vec<String>)> {
    if let Some((command, args)) = resolve_vscode_command() {
        return Ok((command, args));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(("notepad.exe".to_string(), Vec::new()));
    }
    #[cfg(target_os = "macos")]
    {
        return Ok(("open".to_string(), Vec::new()));
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        Ok(("xdg-open".to_string(), Vec::new()))
    }
}

fn fallback_editor_command(current: &str) -> Option<(String, Vec<String>)> {
    #[cfg(target_os = "windows")]
    {
        let lower = current.to_ascii_lowercase();
        if lower.ends_with("notepad.exe") {
            return None;
        }
        return Some(("notepad.exe".to_string(), Vec::new()));
    }
    #[cfg(target_os = "macos")]
    {
        let lower = current.to_ascii_lowercase();
        if lower == "open" || lower.ends_with("/open") {
            return None;
        }
        return Some(("open".to_string(), Vec::new()));
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let lower = current.to_ascii_lowercase();
        if lower == "xdg-open" || lower.ends_with("/xdg-open") {
            return None;
        }
        Some(("xdg-open".to_string(), Vec::new()))
    }
}

fn resolve_command_path(command: &str) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let output = Command::new("where").arg(command).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let candidate = stdout.lines().next()?.trim();
        if candidate.is_empty() {
            return None;
        }
        return Some(PathBuf::from(candidate));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let output = Command::new("which").arg(command).output().ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let candidate = stdout.lines().next()?.trim();
        if candidate.is_empty() {
            return None;
        }
        Some(PathBuf::from(candidate))
    }
}

fn resolve_vscode_command() -> Option<(String, Vec<String>)> {
    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::new();
        if let Ok(local_appdata) = env::var("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local_appdata)
                    .join("Programs")
                    .join("Microsoft VS Code")
                    .join("Code.exe"),
            );
        }
        if let Ok(program_files) = env::var("ProgramFiles") {
            candidates.push(
                PathBuf::from(program_files)
                    .join("Microsoft VS Code")
                    .join("Code.exe"),
            );
        }
        if let Ok(program_files_x86) = env::var("ProgramFiles(x86)") {
            candidates.push(
                PathBuf::from(program_files_x86)
                    .join("Microsoft VS Code")
                    .join("Code.exe"),
            );
        }
        for candidate in candidates {
            if candidate.exists() {
                return Some((
                    candidate.to_string_lossy().to_string(),
                    vec!["--wait".to_string()],
                ));
            }
        }
    }
    if let Some(candidate) = resolve_command_path("code") {
        return Some((
            candidate.to_string_lossy().to_string(),
            vec!["--wait".to_string()],
        ));
    }
    None
}

fn expand_env_vars(value: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        let mut result = String::new();
        let mut chars = value.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '%' {
                let mut name = String::new();
                while let Some(next) = chars.next() {
                    if next == '%' {
                        break;
                    }
                    name.push(next);
                }
                if name.is_empty() {
                    result.push('%');
                } else if let Ok(val) = env::var(&name) {
                    result.push_str(&val);
                } else {
                    result.push('%');
                    result.push_str(&name);
                    result.push('%');
                }
            } else {
                result.push(ch);
            }
        }
        return result;
    }
    #[cfg(not(target_os = "windows"))]
    {
        value.to_string()
    }
}

fn parse_command_line(input: &str) -> (String, Vec<String>) {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return (String::new(), Vec::new());
    }
    if trimmed.contains('"') || trimmed.contains('\'') {
        return split_quoted_command(trimmed);
    }
    let parts: Vec<String> = trimmed
        .split_whitespace()
        .map(|part| part.to_string())
        .collect();
    if parts.is_empty() {
        return (String::new(), Vec::new());
    }
    if parts.len() == 1 {
        // SAFETY: len() == 1 guarantees first() returns Some
        return (parts.first().cloned().unwrap_or_default(), Vec::new());
    }
    #[cfg(target_os = "windows")]
    {
        if let Some((command, args)) = infer_windows_path_command(&parts) {
            return (command, args);
        }
    }
    // SAFETY: len() > 1 guarantees first() returns Some
    let command = parts.first().cloned().unwrap_or_default();
    let args = parts.get(1..).map(|s| s.to_vec()).unwrap_or_default();
    (command, args)
}

fn split_quoted_command(input: &str) -> (String, Vec<String>) {
    let mut args: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\'' | '"' => {
                if let Some(active) = quote {
                    if active == ch {
                        quote = None;
                    } else {
                        current.push(ch);
                    }
                } else {
                    quote = Some(ch);
                }
            }
            '\\' if quote.is_some() => {
                if let Some(next) = chars.peek() {
                    if Some(*next) == quote {
                        current.push(*next);
                        chars.next();
                    } else {
                        current.push(ch);
                    }
                } else {
                    current.push(ch);
                }
            }
            _ if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    if args.is_empty() {
        return (String::new(), Vec::new());
    }
    let command = args.remove(0);
    (command, args)
}

#[cfg(target_os = "windows")]
fn infer_windows_path_command(parts: &[String]) -> Option<(String, Vec<String>)> {
    for end in (1..=parts.len()).rev() {
        let candidate = parts.get(..end)?.join(" ");
        if is_path_like(&candidate) && PathBuf::from(&candidate).is_file() {
            let args = parts.get(end..).map(|s| s.to_vec()).unwrap_or_default();
            return Some((candidate, args));
        }
    }
    None
}

fn is_path_like(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.contains(['\\', '/']) || lower.ends_with(".exe")
}

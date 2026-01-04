use anyhow::anyhow;
use std::path::PathBuf;
use std::process::Command;

use crate::profiles;

pub async fn open_profile_in_editor(
    editor_path: Option<String>,
    name: &str,
) -> anyhow::Result<()> {
    let profile = profiles::load_profile_info(name).await?;
    let (editor, args) = resolve_editor_command(editor_path.as_deref())?;
    let mut final_args = args;
    final_args.push(profile.path.clone());
    open_in_editor(&editor, &final_args)
}

fn open_in_editor(editor: &str, args: &[String]) -> anyhow::Result<()> {
    let mut command = Command::new(editor);
    command.args(args);
    command.spawn()?;
    Ok(())
}

fn resolve_editor_command(editor_path: Option<&str>) -> anyhow::Result<(String, Vec<String>)> {
    if let Some(path) = editor_path {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return default_editor_command();
        }
        let is_path_like = trimmed.contains(['\\', '/']);
        if is_path_like {
            let candidate = PathBuf::from(trimmed);
            if !candidate.exists() {
                return Err(anyhow!(
                    "未找到编辑器路径: {trimmed}"
                ));
            }
        } else if !is_command_available(trimmed) {
            return Err(anyhow!(
                "未找到编辑器命令: {trimmed}"
            ));
        }
        return Ok((trimmed.to_string(), Vec::new()));
    }
    default_editor_command()
}

fn default_editor_command() -> anyhow::Result<(String, Vec<String>)> {
    if is_vscode_available() {
        return Ok(("code".to_string(), vec!["-w".to_string()]));
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

fn is_command_available(command: &str) -> bool {
    #[cfg(target_os = "windows")]
    {
        return Command::new("where")
            .arg(command)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("which")
            .arg(command)
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn is_vscode_available() -> bool {
    is_command_available("code")
}

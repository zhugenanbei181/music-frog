use anyhow::anyhow;
use mihomo_rs::version::{Channel, VersionManager};
use std::path::{Path, PathBuf};

pub async fn resolve_binary(
    vm: &VersionManager,
    use_bundled: bool,
    bundled_candidates: &[PathBuf],
    data_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let installed = vm.list_installed().await.unwrap_or_default();
    if installed.is_empty() {
        if let Some(path) = copy_bundled_binary(bundled_candidates, data_dir).await? {
            return Ok(path);
        }
        return Err(anyhow!("未找到捆绑内核，且没有已下载版本，请检查安装包"));
    }

    if use_bundled {
        if let Some(path) = copy_bundled_binary(bundled_candidates, data_dir).await? {
            return Ok(path);
        }
        log::warn!("bundled core not found, fallback to installed versions");
    }

    if let Ok(default_version) = vm.get_default().await {
        if let Ok(path) = vm.get_binary_path(Some(&default_version)).await {
            return Ok(path);
        }
        log::warn!("default mihomo binary not found for {default_version}");
    }

    let mut versions: Vec<String> = installed.into_iter().map(|v| v.version).collect();
    sort_versions_desc(&mut versions);
    if let Some(latest) = versions.first() {
        if vm.set_default(latest).await.is_ok() {
            if let Ok(path) = vm.get_binary_path(Some(latest)).await {
                return Ok(path);
            }
        }
    }

    Err(anyhow!("未找到可用内核，请检查已下载版本或捆绑内核"))
}

pub async fn download_latest(vm: &VersionManager) -> anyhow::Result<String> {
    let version = vm
        .install_channel(Channel::Stable)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    vm.set_default(&version)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    Ok(version)
}

pub async fn copy_bundled_binary(
    bundled_candidates: &[PathBuf],
    data_dir: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    #[cfg(not(windows))]
    {
        let _ = (bundled_candidates, data_dir);
        Ok(None)
    }

    #[cfg(windows)]
    {
        let Some(source_path) = bundled_candidates
            .iter()
            .find(|p| p.exists())
            .cloned()
        else {
            log::warn!("bundled core not found in resources or project directory");
            return Ok(None);
        };
        log::info!("using bundled mihomo core: {}", source_path.display());

        let runtime_dir = data_dir.join("mihomo");
        tokio::fs::create_dir_all(&runtime_dir).await?;
        let target = runtime_dir.join("mihomo.exe");

        if !target.exists() {
            tokio::fs::copy(&source_path, &target).await?;
        }

        Ok(Some(target))
    }
}

pub fn sort_versions_desc(list: &mut [String]) {
    list.sort_by(|a, b| compare_versions_desc(a, b));
}

fn compare_versions_desc(a: &str, b: &str) -> std::cmp::Ordering {
    let va = parse_version(a);
    let vb = parse_version(b);
    match (va, vb) {
        (Some(va), Some(vb)) => vb.cmp(&va),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => b.cmp(a),
    }
}

fn parse_version(version: &str) -> Option<(u64, u64, u64)> {
    let trimmed = version.trim().trim_start_matches('v');
    let core = trimmed.split('-').next()?;
    let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    Some((major, minor, patch))
}

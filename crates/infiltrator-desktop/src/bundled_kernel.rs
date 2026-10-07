//! Locate packaged kernels and materialize immutable, content-addressed runtime copies.
use anyhow::{Context, bail};
use sha2::{Digest, Sha256};
use std::env::current_exe;
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(windows)]
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::id;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::fs;

static NEXT_COPY: AtomicU64 = AtomicU64::new(1);

pub fn kernel_file_name() -> &'static str {
    if cfg!(windows) {
        "mihomo.exe"
    } else {
        "mihomo"
    }
}

/// Packages use a sibling, macOS Resources, or an isolated libexec product directory.
pub fn candidate_paths(executable: &Path, development_vendor: Option<&Path>) -> Vec<PathBuf> {
    let Some(directory) = executable.parent() else {
        return Vec::new();
    };
    let file = kernel_file_name();
    let mut candidates = vec![
        directory.join(file),
        directory.join("../Resources").join(file),
    ];
    if let Some(product) = executable.file_stem() {
        candidates.push(
            directory
                .join("../libexec/musicfrog")
                .join(product)
                .join(file),
        );
    }
    if let Some(vendor) = development_vendor {
        let name = if cfg!(all(windows, target_arch = "aarch64")) {
            "mihomo-windows-arm64.exe"
        } else if cfg!(windows) {
            "mihomo.exe"
        } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            "mihomo-linux-amd64"
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            "mihomo-linux-arm64"
        } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
            "mihomo-darwin-amd64"
        } else {
            "mihomo-darwin-arm64"
        };
        candidates.push(vendor.join(name));
    }
    candidates
}

pub fn packaged_candidates() -> anyhow::Result<Vec<PathBuf>> {
    let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
    Ok(candidate_paths(
        &current_exe()?,
        cfg!(debug_assertions).then_some(vendor.as_path()),
    ))
}

pub async fn copy_bundled_binary(
    candidates: &[PathBuf],
    data_dir: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(source) = candidates.iter().find(|path| path.is_file()) else {
        return Ok(None);
    };
    let bytes = fs::read(source).await.context("read bundled kernel")?;
    if bytes.is_empty() {
        bail!("bundled kernel is empty: {}", source.display());
    }
    let digest: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let directory = data_dir.join("mihomo").join(&digest);
    fs::create_dir_all(&directory).await?;
    let target = directory.join(kernel_file_name());
    if let Ok(existing) = fs::read(&target).await
        && existing == bytes
    {
        #[cfg(unix)]
        fs::set_permissions(&target, Permissions::from_mode(0o755)).await?;
        return Ok(Some(target));
    }
    let temporary = directory.join(format!(
        ".copy-{}-{}",
        id(),
        NEXT_COPY.fetch_add(1, Ordering::Relaxed)
    ));
    let result = async {
        fs::write(&temporary, &bytes).await?;
        #[cfg(unix)]
        fs::set_permissions(&temporary, Permissions::from_mode(0o755)).await?;
        install_copy(&temporary, &target, &bytes).await?;
        Ok::<_, anyhow::Error>(())
    }
    .await;
    let _ = fs::remove_file(&temporary).await;
    result?;
    Ok(Some(target))
}

async fn install_copy(temporary: &Path, target: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    match fs::rename(temporary, target).await {
        Ok(()) => Ok(()),
        Err(_) if fs::read(target).await.ok().as_deref() == Some(bytes) => Ok(()),
        Err(error) => {
            #[cfg(windows)]
            if target.exists() {
                match fs::remove_file(target).await {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => return Err(error.into()),
                }
                if let Err(error) = fs::rename(temporary, target).await
                    && fs::read(target).await.ok().as_deref() != Some(bytes)
                {
                    return Err(error.into());
                }
                return Ok(());
            }
            Err(error.into())
        }
    }
}

#[cfg(test)]
#[path = "bundled_kernel_tests.rs"]
mod tests;

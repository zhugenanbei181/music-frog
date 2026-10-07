//! test-intent: behavior
use super::{candidate_paths, copy_bundled_binary, kernel_file_name};
#[cfg(unix)]
use std::fs::Permissions;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::slice::from_ref;
use tempfile::tempdir;
use tokio::fs;

#[tokio::test]
async fn runtime_copy_updates_by_content_and_repairs_corruption_without_replacing_old_versions() {
    let root = tempdir().unwrap();
    let source = root.path().join("package kernel");
    let data = root.path().join("runtime data");
    fs::write(&source, b"first verified kernel").await.unwrap();
    let first = copy_bundled_binary(from_ref(&source), &data)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fs::read(&first).await.unwrap(), b"first verified kernel");
    #[cfg(unix)]
    fs::set_permissions(&first, Permissions::from_mode(0o600))
        .await
        .unwrap();
    assert_eq!(
        copy_bundled_binary(from_ref(&source), &data).await.unwrap(),
        Some(first.clone())
    );
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&first).await.unwrap().permissions().mode() & 0o777,
        0o755
    );
    fs::write(&source, b"second verified kernel").await.unwrap();
    let second = copy_bundled_binary(from_ref(&source), &data)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(first, second);
    assert_eq!(fs::read(&first).await.unwrap(), b"first verified kernel");
    fs::write(&second, b"partial corrupt copy").await.unwrap();
    assert_eq!(
        copy_bundled_binary(&[source], &data).await.unwrap(),
        Some(second.clone())
    );
    assert_eq!(fs::read(&second).await.unwrap(), b"second verified kernel");
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&second).await.unwrap().permissions().mode() & 0o777,
        0o755
    );
    let mut entries = fs::read_dir(second.parent().unwrap()).await.unwrap();
    let only = entries.next_entry().await.unwrap().unwrap();
    assert_eq!(only.file_name(), kernel_file_name());
    assert!(entries.next_entry().await.unwrap().is_none());
}

#[tokio::test]
async fn concurrent_peer_staging_returns_one_complete_kernel_and_leaves_no_temporary_files() {
    let root = tempdir().unwrap();
    let source = root.path().join("package kernel");
    fs::write(&source, b"shared peer kernel").await.unwrap();
    let candidates = [source];
    let (first, second) = tokio::join!(
        copy_bundled_binary(&candidates, root.path()),
        copy_bundled_binary(&candidates, root.path()),
    );
    let first = first.unwrap().unwrap();
    assert_eq!(second.unwrap(), Some(first.clone()));
    assert_eq!(fs::read(&first).await.unwrap(), b"shared peer kernel");
    let mut entries = fs::read_dir(first.parent().unwrap()).await.unwrap();
    assert_eq!(
        entries.next_entry().await.unwrap().unwrap().file_name(),
        kernel_file_name()
    );
    assert!(entries.next_entry().await.unwrap().is_none());
}

#[tokio::test]
async fn missing_and_empty_package_kernels_never_produce_a_successful_runtime_path() {
    let root = tempdir().unwrap();
    let empty = root.path().join("empty");
    fs::write(&empty, b"").await.unwrap();
    assert!(
        copy_bundled_binary(&[root.path().to_owned(), empty], root.path())
            .await
            .is_err()
    );
    assert_eq!(
        copy_bundled_binary(&[root.path().join("missing")], root.path())
            .await
            .unwrap(),
        None
    );
}

#[test]
fn installed_product_paths_resolve_coinstalled_packages_and_macos_resources() {
    let executable = Path::new("/usr/bin/infiltrator-bevy-ui");
    let candidates = candidate_paths(executable, None);
    assert_eq!(
        candidates,
        vec![
            Path::new("/usr/bin").join(kernel_file_name()),
            Path::new("/usr/bin/../Resources").join(kernel_file_name()),
            Path::new("/usr/bin/../libexec/musicfrog/infiltrator-bevy-ui").join(kernel_file_name()),
        ]
    );
    let macos = candidate_paths(
        Path::new("/Applications/Product.app/Contents/MacOS/product"),
        None,
    );
    assert!(
        macos.contains(
            &Path::new("/Applications/Product.app/Contents/MacOS/../Resources")
                .join(kernel_file_name())
        )
    );
}

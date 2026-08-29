use futures_util::StreamExt;
use mihomo_api::error::{MihomoError, Result};
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::verify;

#[derive(Debug, Clone, Copy)]
pub struct DownloadProgress {
    pub downloaded: u64,
    pub total: Option<u64>,
}

pub struct Downloader {
    client: reqwest::Client,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Download the release archive for `version` and install the contained
    /// binary at `dest`. The archive bytes are verified against
    /// `expected_digest` *before* anything touches the filesystem; a `None`
    /// digest is refused (fail-closed, UP-001). The binary lands via a
    /// temp-file rename so an interrupted write never leaves a partial
    /// artifact at `dest`.
    pub async fn download_version_with_progress<F>(
        &self,
        version: &str,
        dest: &Path,
        expected_digest: Option<&str>,
        mut on_progress: F,
    ) -> Result<()>
    where
        F: FnMut(DownloadProgress),
    {
        let platform = Self::detect_platform();
        let os_name = Self::get_os_name();
        let extension = Self::get_file_extension();
        let filename = format!("mihomo-{}-{}-{}.{}", os_name, platform, version, extension);
        let url = format!(
            "https://github.com/MetaCubeX/mihomo/releases/download/{}/{}",
            version, filename
        );

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "mihomo-rs")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(MihomoError::Version(format!(
                "Failed to download version {}: HTTP {}",
                version,
                resp.status()
            )));
        }

        let total = resp.content_length();
        let mut downloaded: u64 = 0;
        let mut bytes = Vec::new();
        let mut stream = resp.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            downloaded += chunk.len() as u64;
            bytes.extend_from_slice(&chunk);
            on_progress(DownloadProgress { downloaded, total });
        }

        Self::install_archive(&bytes, expected_digest, &filename, dest).await
    }

    /// Verify, decompress, and atomically install an already-downloaded
    /// release archive. Exposed for tests: the whole pipeline up to (but
    /// excluding) the HTTP fetch is exercised without network access.
    pub async fn install_archive(
        archive: &[u8],
        expected_digest: Option<&str>,
        label: &str,
        dest: &Path,
    ) -> Result<()> {
        // Fail-closed gate: no bytes are decompressed or written before the
        // archive digest matches the release-API digest.
        verify::verify_bytes(archive, expected_digest, label)?;

        let extension = Self::get_file_extension();
        let decompressed = if extension == "zip" {
            Self::decompress_zip(archive)?
        } else {
            Self::decompress_gz(archive)?
        };

        // Atomic install: write a temp file next to `dest`, fsync, then
        // rename. A crash mid-write leaves the previous state untouched.
        let dir = dest.parent().ok_or_else(|| {
            MihomoError::Version(format!("install destination has no parent: {}", dest.display()))
        })?;
        let file_name = dest
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("mihomo");
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let tmp = dir.join(format!(".{file_name}.tmp-{}-{nanos}", std::process::id()));

        let write = async {
            let mut file = fs::File::create(&tmp).await?;
            file.write_all(&decompressed).await?;
            file.sync_all().await?;
            #[cfg(unix)]
            {
                // 0o755: owner-writable only; tamper-hardening for the
                // installed kernel (CORE-006). Applied before the rename so
                // the final artifact never exists with weak permissions.
                use std::os::unix::fs::PermissionsExt;
                let mut perms = file.metadata().await?.permissions();
                perms.set_mode(0o755);
                file.set_permissions(perms).await?;
            }
            Ok::<(), MihomoError>(())
        }
        .await;
        if let Err(err) = write {
            let _ = fs::remove_file(&tmp).await;
            return Err(err);
        }
        if let Err(err) = fs::rename(&tmp, dest).await {
            let _ = fs::remove_file(&tmp).await;
            return Err(MihomoError::Io(err));
        }
        Ok(())
    }

    pub(crate) fn get_os_name() -> &'static str {
        match std::env::consts::OS {
            "linux" => "linux",
            "macos" => "darwin",
            "windows" => "windows",
            _ => "linux",
        }
    }

    pub(crate) fn detect_platform() -> String {
        let arch = std::env::consts::ARCH;
        match arch {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            "arm" => "armv7",
            _ => "amd64",
        }
        .to_string()
    }

    pub(crate) fn get_file_extension() -> &'static str {
        match std::env::consts::OS {
            "windows" => "zip",
            _ => "gz",
        }
    }

    fn decompress_gz(bytes: &[u8]) -> Result<Vec<u8>> {
        use flate2::read::GzDecoder;
        use std::io::Read;

        let mut decoder = GzDecoder::new(bytes);
        let mut decompressed = Vec::new();
        decoder
            .read_to_end(&mut decompressed)
            .map_err(|e| MihomoError::Version(format!("Failed to decompress gz: {}", e)))?;
        Ok(decompressed)
    }

    fn decompress_zip(bytes: &[u8]) -> Result<Vec<u8>> {
        use std::io::{Cursor, Read};
        use zip::ZipArchive;

        let reader = Cursor::new(bytes);
        let mut archive = ZipArchive::new(reader)
            .map_err(|e| MihomoError::Version(format!("Failed to open zip archive: {}", e)))?;

        // mihomo zip archives should contain a single binary file
        if archive.len() != 1 {
            return Err(MihomoError::Version(format!(
                "Expected 1 file in zip archive, found {}",
                archive.len()
            )));
        }

        let mut file = archive
            .by_index(0)
            .map_err(|e| MihomoError::Version(format!("Failed to read zip entry: {}", e)))?;

        let mut decompressed = Vec::new();
        file.read_to_end(&mut decompressed)
            .map_err(|e| MihomoError::Version(format!("Failed to decompress zip: {}", e)))?;

        Ok(decompressed)
    }
}

impl Default for Downloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verify::sha256_hex;

    fn gz_archive(content: &[u8]) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(content).unwrap();
        encoder.finish().unwrap()
    }

    #[tokio::test]
    async fn install_archive_verifies_writes_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("mihomo");
        let content = b"fake kernel binary";
        let archive = gz_archive(content);
        let digest = format!("sha256:{}", sha256_hex(&archive));

        Downloader::install_archive(&archive, Some(&digest), "test.gz", &dest)
            .await
            .unwrap();

        assert_eq!(std::fs::read(&dest).unwrap(), content);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&dest).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp-"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");
    }

    #[tokio::test]
    async fn install_archive_rejects_digest_mismatch_without_touching_disk() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("mihomo");
        let archive = gz_archive(b"evil payload");
        let digest = format!("sha256:{}", sha256_hex(b"benign payload"));

        let err = Downloader::install_archive(&archive, Some(&digest), "test.gz", &dest)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("SHA-256 mismatch"), "{err}");
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn install_archive_fails_closed_without_digest() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("mihomo");
        let archive = gz_archive(b"anything");

        let err = Downloader::install_archive(&archive, None, "test.gz", &dest)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("fail-closed"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn test_get_os_name() {
        // Test that get_os_name returns one of the expected values
        let os_name = Downloader::get_os_name();
        assert!(
            os_name == "linux" || os_name == "darwin" || os_name == "windows",
            "OS name should be linux, darwin, or windows, got: {}",
            os_name
        );
    }

    #[test]
    fn test_detect_platform() {
        // Test that detect_platform returns a valid platform string
        let platform = Downloader::detect_platform();
        assert!(
            platform == "amd64" || platform == "arm64" || platform == "armv7",
            "Platform should be amd64, arm64, or armv7, got: {}",
            platform
        );
    }

    #[test]
    fn test_get_file_extension() {
        // Test that get_file_extension returns either zip or gz
        let extension = Downloader::get_file_extension();
        assert!(
            extension == "zip" || extension == "gz",
            "Extension should be zip or gz, got: {}",
            extension
        );
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_windows_uses_zip() {
        assert_eq!(Downloader::get_file_extension(), "zip");
        assert_eq!(Downloader::get_os_name(), "windows");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_uses_gz() {
        assert_eq!(Downloader::get_file_extension(), "gz");
        assert_eq!(Downloader::get_os_name(), "linux");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_uses_gz() {
        assert_eq!(Downloader::get_file_extension(), "gz");
        assert_eq!(Downloader::get_os_name(), "darwin");
    }

    #[test]
    fn test_decompress_gz() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        // Create test data
        let test_data = b"Hello, this is test data for gzip compression!";

        // Compress the data
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(test_data).unwrap();
        let compressed = encoder.finish().unwrap();

        // Test decompression
        let decompressed = Downloader::decompress_gz(&compressed).unwrap();
        assert_eq!(decompressed, test_data);
    }

    #[test]
    fn test_decompress_zip() {
        use std::io::{Cursor, Write};
        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;

        // Create test data
        let test_data = b"Hello, this is test data for zip compression!";

        // Create a zip file in memory with a single entry
        let mut zip_buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut zip_buffer);
            let options = SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated)
                .unix_permissions(0o755);

            zip.start_file("mihomo", options).unwrap();
            zip.write_all(test_data).unwrap();
            zip.finish().unwrap();
        }

        let compressed = zip_buffer.into_inner();

        // Test decompression
        let decompressed = Downloader::decompress_zip(&compressed).unwrap();
        assert_eq!(decompressed, test_data);
    }

    #[test]
    fn test_decompress_zip_with_multiple_files_fails() {
        use std::io::{Cursor, Write};
        use zip::ZipWriter;
        use zip::write::SimpleFileOptions;

        // Create a zip file with multiple entries
        let mut zip_buffer = Cursor::new(Vec::new());
        {
            let mut zip = ZipWriter::new(&mut zip_buffer);
            let options =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

            // Add first file
            zip.start_file("file1", options).unwrap();
            zip.write_all(b"File 1 content").unwrap();

            // Add second file
            zip.start_file("file2", options).unwrap();
            zip.write_all(b"File 2 content").unwrap();

            zip.finish().unwrap();
        }

        let compressed = zip_buffer.into_inner();

        // Test that decompression fails with multiple files
        let result = Downloader::decompress_zip(&compressed);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Expected 1 file in zip archive, found 2")
        );
    }

    #[test]
    fn test_decompress_gz_with_invalid_data() {
        let invalid_data = b"This is not gzip compressed data";
        let result = Downloader::decompress_gz(invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_zip_with_invalid_data() {
        let invalid_data = b"This is not zip compressed data";
        let result = Downloader::decompress_zip(invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_filename_format() {
        // Test that the filename format is correct for different platforms
        let version = "v1.19.17";
        let platform = Downloader::detect_platform();
        let os_name = Downloader::get_os_name();
        let extension = Downloader::get_file_extension();

        let filename = format!("mihomo-{}-{}-{}.{}", os_name, platform, version, extension);

        // Verify the filename matches expected pattern
        assert!(filename.starts_with("mihomo-"));
        assert!(filename.contains(version));
        assert!(filename.ends_with(".zip") || filename.ends_with(".gz"));
    }
}

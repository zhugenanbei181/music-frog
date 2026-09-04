use super::*;
use chrono::{TimeZone, Utc};
use infiltrator_domain::backup::prune_snapshots;
use infiltrator_domain::snapshots::SnapshotMeta;
use std::io::{Cursor, Write};
use std::path::PathBuf;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

fn sample_bundle() -> BackupBundle {
    let profiles = vec![
        ProfileBackupItem::new("default", "port: 7890\nmode: rule\n"),
        ProfileBackupItem::with_options(
            "subs",
            "port: 7892\nproxies: []\n",
            Some("filter: direct\n".to_string()),
        ),
    ];
    let settings = "language = \"zh\"\nnotify = true\n";
    let mixin = "mode: global\n";

    BackupBundle::new(profiles, settings.to_string(), mixin.to_string())
}

#[test]
fn test_bundle_creation_and_digest() {
    let bundle = sample_bundle();
    assert!(bundle.verify_digest());
    assert!(bundle.validate_checksum().is_ok());
    assert_eq!(bundle.manifest.version, "1.0.0");
    assert_eq!(bundle.profiles.len(), 2);
}

#[test]
fn test_bundle_digest_detects_tampering() {
    let mut bundle = sample_bundle();
    assert!(bundle.verify_digest());

    bundle.settings_toml.push_str("\n# tampered");
    assert!(!bundle.verify_digest());
    assert!(matches!(
        bundle.validate_checksum(),
        Err(BackupError::IntegrityMismatch { .. })
    ));

    bundle.update_digest();
    assert!(bundle.verify_digest());
}

#[test]
fn test_json_export_import_roundtrip() {
    let bundle = sample_bundle();
    let json = bundle.to_json().expect("export JSON");
    assert!(json.contains("default"));
    assert!(json.contains("subs"));
    assert!(json.contains("sha256_digest"));

    let imported = BackupBundle::from_json(&json).expect("import JSON");
    assert_eq!(bundle, imported);
}

#[test]
fn test_json_import_tampered_digest_fails() {
    let bundle = sample_bundle();
    let mut json = bundle.to_json().expect("export JSON");
    json = json.replace("port: 7890", "port: 9999");

    let err = BackupBundle::from_json(&json).unwrap_err();
    assert!(matches!(err, BackupError::IntegrityMismatch { .. }));
}

#[test]
fn test_encrypted_export_import_roundtrip() {
    let bundle = sample_bundle();
    let password = "CorrectHorseBatteryStaple123!";

    let encrypted_bytes = bundle
        .export_encrypted(password)
        .expect("export encrypted bundle");
    assert!(!encrypted_bytes.is_empty());
    assert!(encrypted_bytes.starts_with(b"IFTR_BACKUP_V1"));

    let imported = BackupBundle::import_encrypted(&encrypted_bytes, password)
        .expect("import encrypted bundle");
    assert_eq!(bundle, imported);
}

#[test]
fn test_encrypted_wrong_password_fails() {
    let bundle = sample_bundle();
    let password = "CorrectPassword123";
    let encrypted = bundle.export_encrypted(password).expect("export encrypted");

    let err = BackupBundle::import_encrypted(&encrypted, "WrongPassword").unwrap_err();
    assert!(matches!(err, BackupError::DecryptionFailed));
}

#[test]
fn test_encrypted_empty_password_fails() {
    let bundle = sample_bundle();
    assert!(matches!(
        bundle.export_encrypted(""),
        Err(BackupError::EmptyPassword)
    ));
    assert!(matches!(
        bundle.export_encrypted("   "),
        Err(BackupError::EmptyPassword)
    ));

    let valid_enc = bundle.export_encrypted("pass").unwrap();
    assert!(matches!(
        BackupBundle::import_encrypted(&valid_enc, ""),
        Err(BackupError::EmptyPassword)
    ));
}

#[test]
fn test_encrypted_corrupted_payload_fails() {
    let bundle = sample_bundle();
    let mut encrypted = bundle.export_encrypted("secure_pass").unwrap();

    // Corrupt magic
    encrypted[0] ^= 0xFF;
    let err = BackupBundle::import_encrypted(&encrypted, "secure_pass").unwrap_err();
    assert!(matches!(err, BackupError::InvalidFormat(_)));

    // Restore magic, corrupt ciphertext
    encrypted[0] ^= 0xFF;
    let last_idx = encrypted.len() - 1;
    encrypted[last_idx] ^= 0xAA;
    let err = BackupBundle::import_encrypted(&encrypted, "secure_pass").unwrap_err();
    assert!(matches!(err, BackupError::DecryptionFailed));
}

#[test]
fn test_encrypted_short_payload_fails() {
    let short_bytes = vec![0u8; 10];
    let err = BackupBundle::import_encrypted(&short_bytes, "pass").unwrap_err();
    assert!(matches!(err, BackupError::InvalidFormat(_)));
}

#[test]
fn test_zip_export_import_roundtrip() {
    let bundle = sample_bundle();
    let zip_bytes = bundle.to_zip().expect("export zip");
    assert!(!zip_bytes.is_empty());

    let imported = BackupBundle::from_zip(&zip_bytes).expect("import zip");
    assert_eq!(bundle, imported);
}

#[test]
fn test_zip_import_missing_manifest_fails() {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default();
        zip.start_file("settings.toml", options).unwrap();
        zip.write_all(b"test = 1").unwrap();
        zip.finish().unwrap();
    }
    let zip_bytes = buffer.into_inner();

    let err = BackupBundle::from_zip(&zip_bytes).unwrap_err();
    assert!(matches!(err, BackupError::InvalidFormat(_)));
}

#[test]
fn test_zip_import_tampered_profile_fails() {
    let bundle = sample_bundle();
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default();

        // Write original manifest
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(serde_json::to_string(&bundle.manifest).unwrap().as_bytes())
            .unwrap();

        zip.start_file("settings.toml", options).unwrap();
        zip.write_all(bundle.settings_toml.as_bytes()).unwrap();

        zip.start_file("mixin.yaml", options).unwrap();
        zip.write_all(bundle.mixin_yaml.as_bytes()).unwrap();

        // Write tampered profile content
        zip.start_file("profiles/default.yaml", options).unwrap();
        zip.write_all(b"port: 9999\n").unwrap();

        zip.finish().unwrap();
    }
    let zip_bytes = buffer.into_inner();

    let err = BackupBundle::from_zip(&zip_bytes).unwrap_err();
    assert!(matches!(err, BackupError::IntegrityMismatch { .. }));
}

#[test]
fn test_zip_slip_protection() {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options = SimpleFileOptions::default();
        zip.start_file("../../../evil.txt", options).unwrap();
        zip.write_all(b"malicious").unwrap();
        zip.finish().unwrap();
    }
    let zip_bytes = buffer.into_inner();

    let err = BackupBundle::from_zip(&zip_bytes).unwrap_err();
    assert!(matches!(err, BackupError::InvalidFormat(_)));
}

#[test]
fn test_prune_snapshots_empty() {
    let pruned = prune_snapshots(&[], 5);
    assert!(pruned.is_empty());
}

#[test]
fn test_prune_snapshots_retains_newest_and_deduplicates() {
    let t1 = Utc.timestamp_opt(1000, 0).unwrap();
    let t2 = Utc.timestamp_opt(2000, 0).unwrap();
    let t3 = Utc.timestamp_opt(3000, 0).unwrap();
    let t4 = Utc.timestamp_opt(4000, 0).unwrap();
    let t5 = Utc.timestamp_opt(5000, 0).unwrap();

    let snapshots = vec![
        SnapshotMeta {
            profile: "main".to_string(),
            timestamp: t1, // Oldest, hash A
            sha256: "hash_A".to_string(),
            path: PathBuf::from("/snapshots/main/1000-hash_A.yaml"),
        },
        SnapshotMeta {
            profile: "main".to_string(),
            timestamp: t2, // hash B
            sha256: "hash_B".to_string(),
            path: PathBuf::from("/snapshots/main/2000-hash_B.yaml"),
        },
        SnapshotMeta {
            profile: "main".to_string(),
            timestamp: t3, // Duplicate of hash A
            sha256: "hash_A".to_string(),
            path: PathBuf::from("/snapshots/main/3000-hash_A.yaml"),
        },
        SnapshotMeta {
            profile: "main".to_string(),
            timestamp: t4, // hash C
            sha256: "hash_C".to_string(),
            path: PathBuf::from("/snapshots/main/4000-hash_C.yaml"),
        },
        SnapshotMeta {
            profile: "main".to_string(),
            timestamp: t5, // Newest, duplicate of hash A
            sha256: "hash_A".to_string(),
            path: PathBuf::from("/snapshots/main/5000-hash_A.yaml"),
        },
    ];

    // max_retain = 2:
    // Sorted by time desc: t5 (hash_A), t4 (hash_C), t3 (hash_A dup), t2 (hash_B), t1 (hash_A dup)
    // Retains: t5 (unique hash_A, #1), t4 (unique hash_C, #2)
    // Pruned: t3 (dup hash_A), t2 (exceeds budget 2), t1 (dup & exceeds budget)
    let pruned = prune_snapshots(&snapshots, 2);
    assert_eq!(pruned.len(), 3);
    assert!(pruned.contains(&"/snapshots/main/3000-hash_A.yaml".to_string()));
    assert!(pruned.contains(&"/snapshots/main/2000-hash_B.yaml".to_string()));
    assert!(pruned.contains(&"/snapshots/main/1000-hash_A.yaml".to_string()));
}

#[test]
fn test_prune_snapshots_zero_budget_prunes_all() {
    let t1 = Utc.timestamp_opt(1000, 0).unwrap();
    let snapshots = vec![SnapshotMeta {
        profile: "main".to_string(),
        timestamp: t1,
        sha256: "hash_A".to_string(),
        path: PathBuf::from("/snapshots/main/1000-hash_A.yaml"),
    }];

    let pruned = prune_snapshots(&snapshots, 0);
    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0], "/snapshots/main/1000-hash_A.yaml");
}

#[test]
fn test_prune_snapshots_multiple_profiles() {
    let t1 = Utc.timestamp_opt(1000, 0).unwrap();
    let t2 = Utc.timestamp_opt(2000, 0).unwrap();
    let t3 = Utc.timestamp_opt(3000, 0).unwrap();

    let snapshots = vec![
        SnapshotMeta {
            profile: "p1".to_string(),
            timestamp: t1,
            sha256: "hash_1".to_string(),
            path: PathBuf::from("/p1/1.yaml"),
        },
        SnapshotMeta {
            profile: "p1".to_string(),
            timestamp: t2,
            sha256: "hash_2".to_string(),
            path: PathBuf::from("/p1/2.yaml"),
        },
        SnapshotMeta {
            profile: "p2".to_string(),
            timestamp: t3,
            sha256: "hash_3".to_string(),
            path: PathBuf::from("/p2/3.yaml"),
        },
    ];

    // max_retain = 1: p1 prunes /p1/1.yaml, p2 retains /p2/3.yaml
    let pruned = prune_snapshots(&snapshots, 1);
    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0], "/p1/1.yaml");
}

#[tokio::test]
async fn test_export_all_configs_bundle_backward_compatible() {
    let temp = tempdir().unwrap();
    let base = temp.path();
    let configs = base.join("configs");
    tokio::fs::create_dir_all(&configs).await.unwrap();
    tokio::fs::write(configs.join("test.yaml"), "port: 7890\n")
        .await
        .unwrap();

    let json = export_all_configs_bundle(base).await.unwrap();
    assert!(json.contains("test"));
    assert!(json.contains("port: 7890"));
}

#[tokio::test]
async fn test_filesystem_export_and_restore_roundtrip() {
    let temp_src = tempdir().unwrap();
    let src_path = temp_src.path();

    let configs_dir = src_path.join("configs");
    let options_dir = src_path.join("options");
    tokio::fs::create_dir_all(&configs_dir).await.unwrap();
    tokio::fs::create_dir_all(&options_dir).await.unwrap();

    tokio::fs::write(src_path.join("settings.toml"), "port = 9090\n")
        .await
        .unwrap();
    tokio::fs::write(src_path.join("mixin.yaml"), "mode: rule\n")
        .await
        .unwrap();
    tokio::fs::write(configs_dir.join("alpha.yaml"), "proxies: [a, b]\n")
        .await
        .unwrap();
    tokio::fs::write(options_dir.join("alpha.yaml"), "filter: test\n")
        .await
        .unwrap();

    let bundle = export_bundle_from_dir(src_path)
        .await
        .expect("export bundle");
    assert_eq!(bundle.profiles.len(), 1);
    assert_eq!(bundle.profiles[0].name, "alpha");
    assert_eq!(
        bundle.profiles[0].options_yaml.as_deref(),
        Some("filter: test\n")
    );

    let temp_dst = tempdir().unwrap();
    let dst_path = temp_dst.path();

    restore_bundle_to_dir(&bundle, dst_path, true)
        .await
        .expect("restore bundle");

    let restored_settings = tokio::fs::read_to_string(dst_path.join("settings.toml"))
        .await
        .unwrap();
    assert_eq!(restored_settings, "port = 9090\n");

    let restored_profile = tokio::fs::read_to_string(dst_path.join("configs/alpha.yaml"))
        .await
        .unwrap();
    assert_eq!(restored_profile, "proxies: [a, b]\n");

    let restored_options = tokio::fs::read_to_string(dst_path.join("options/alpha.yaml"))
        .await
        .unwrap();
    assert_eq!(restored_options, "filter: test\n");
}

#[test]
fn test_bundle_empty_profiles() {
    let bundle = BackupBundle::new(Vec::new(), "port = 7890\n".to_string(), "".to_string());
    assert!(bundle.verify_digest());
    let json = bundle.to_json().unwrap();
    let imported = BackupBundle::from_json(&json).unwrap();
    assert_eq!(bundle, imported);
}

#[test]
fn test_bundle_with_client_version() {
    let bundle = BackupBundle::with_client_version(
        vec![ProfileBackupItem::new("test", "port: 1234\n")],
        "port = 1234\n".to_string(),
        "mode: rule\n".to_string(),
        "custom-client-2.0.0",
    );
    assert_eq!(bundle.manifest.client_version, "custom-client-2.0.0");
    assert!(bundle.verify_digest());
}

#[test]
fn test_encrypted_multiple_passwords_isolation() {
    let bundle = sample_bundle();
    let enc1 = bundle.export_encrypted("pass_alpha").unwrap();
    let enc2 = bundle.export_encrypted("pass_beta").unwrap();

    assert!(BackupBundle::import_encrypted(&enc1, "pass_alpha").is_ok());
    assert!(BackupBundle::import_encrypted(&enc2, "pass_beta").is_ok());
    assert!(BackupBundle::import_encrypted(&enc1, "pass_beta").is_err());
    assert!(BackupBundle::import_encrypted(&enc2, "pass_alpha").is_err());
}

#[test]
fn test_prune_snapshots_identical_hashes_multiple() {
    let t1 = Utc.timestamp_opt(100, 0).unwrap();
    let t2 = Utc.timestamp_opt(200, 0).unwrap();
    let t3 = Utc.timestamp_opt(300, 0).unwrap();

    let snapshots = vec![
        SnapshotMeta {
            profile: "p".to_string(),
            timestamp: t1,
            sha256: "dup".to_string(),
            path: PathBuf::from("/p/1.yaml"),
        },
        SnapshotMeta {
            profile: "p".to_string(),
            timestamp: t2,
            sha256: "dup".to_string(),
            path: PathBuf::from("/p/2.yaml"),
        },
        SnapshotMeta {
            profile: "p".to_string(),
            timestamp: t3,
            sha256: "dup".to_string(),
            path: PathBuf::from("/p/3.yaml"),
        },
    ];

    // max_retain = 5: Still prunes 2 because hashes are identical!
    let pruned = prune_snapshots(&snapshots, 5);
    assert_eq!(pruned.len(), 2);
    assert!(pruned.contains(&"/p/1.yaml".to_string()));
    assert!(pruned.contains(&"/p/2.yaml".to_string()));
}

#[tokio::test]
async fn test_restore_bundle_no_overwrite() {
    let temp = tempdir().unwrap();
    let base = temp.path();

    tokio::fs::write(base.join("settings.toml"), "port = 1111\n")
        .await
        .unwrap();

    let bundle = sample_bundle();
    restore_bundle_to_dir(&bundle, base, false).await.unwrap();

    let settings = tokio::fs::read_to_string(base.join("settings.toml"))
        .await
        .unwrap();
    assert_eq!(
        settings, "port = 1111\n",
        "Should preserve existing when overwrite = false"
    );
}

use super::*;

#[test]
fn test_semver_parse_and_formatting() {
    let v1 = SemVer::parse("0.20.0").unwrap();
    assert_eq!(v1.major, 0);
    assert_eq!(v1.minor, 20);
    assert_eq!(v1.patch, 0);
    assert_eq!(v1.pre_release, None);
    assert_eq!(v1.to_string(), "0.20.0");

    let v2 = SemVer::parse("v1.2.3-beta.1+build.2026").unwrap();
    assert_eq!(v2.major, 1);
    assert_eq!(v2.minor, 2);
    assert_eq!(v2.patch, 3);
    assert_eq!(v2.pre_release.as_deref(), Some("beta.1"));
    assert_eq!(v2.build_metadata.as_deref(), Some("build.2026"));
    assert!(v2.is_prerelease());
    assert_eq!(v2.to_string(), "1.2.3-beta.1+build.2026");

    let v3 = SemVer::parse("2").unwrap();
    assert_eq!(v3.major, 2);
    assert_eq!(v3.minor, 0);
    assert_eq!(v3.patch, 0);

    assert_eq!(SemVer::parse(""), None);
    assert_eq!(SemVer::parse("invalid.version.str.extra"), None);
    assert_eq!(SemVer::parse("abc"), None);
}

#[test]
fn test_semver_precedence_and_ordering() {
    let v1 = SemVer::parse("1.0.0-alpha").unwrap();
    let v2 = SemVer::parse("1.0.0-alpha.1").unwrap();
    let v3 = SemVer::parse("1.0.0-alpha.beta").unwrap();
    let v4 = SemVer::parse("1.0.0-beta").unwrap();
    let v5 = SemVer::parse("1.0.0-beta.2").unwrap();
    let v6 = SemVer::parse("1.0.0-beta.11").unwrap();
    let v7 = SemVer::parse("1.0.0-rc.1").unwrap();
    let v8 = SemVer::parse("1.0.0").unwrap();
    let v9 = SemVer::parse("1.0.1").unwrap();
    let v10 = SemVer::parse("1.1.0").unwrap();
    let v11 = SemVer::parse("2.0.0").unwrap();

    assert!(v1 < v2);
    assert!(v2 < v3);
    assert!(v3 < v4);
    assert!(v4 < v5);
    assert!(v5 < v6);
    assert!(v6 < v7);
    assert!(v7 < v8);
    assert!(v8 < v9);
    assert!(v9 < v10);
    assert!(v10 < v11);

    // Build metadata does not affect precedence
    let v8_build = SemVer::parse("1.0.0+build123").unwrap();
    assert_eq!(v8.cmp(&v8_build), std::cmp::Ordering::Equal);
}

#[test]
fn test_parse_semver_compatibility() {
    assert_eq!(parse_semver("0.20.0"), Some((0, 20, 0)));
    assert_eq!(parse_semver("v1.2.3"), Some((1, 2, 3)));
    assert_eq!(parse_semver("v2.0.0-beta.1"), Some((2, 0, 0)));
    assert_eq!(parse_semver("invalid"), None);
}

#[test]
fn test_channel_parsing_and_display() {
    use std::str::FromStr;
    assert_eq!(UpdateChannel::from_str("stable").unwrap(), UpdateChannel::Stable);
    assert_eq!(UpdateChannel::from_str("Beta").unwrap(), UpdateChannel::Beta);
    assert_eq!(UpdateChannel::from_str("NIGHTLY").unwrap(), UpdateChannel::Nightly);
    assert!(UpdateChannel::from_str("unknown").is_err());

    assert_eq!(UpdateChannel::Stable.to_string(), "stable");
    assert_eq!(UpdateChannel::Beta.to_string(), "beta");
    assert_eq!(UpdateChannel::Nightly.to_string(), "nightly");
}

#[test]
fn test_manifest_artifact_and_delta_lookup() {
    let manifest = UpdateManifest {
        version: "0.21.0".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Release notes".to_string(),
        min_supported_version: Some("0.19.0".to_string()),
        critical_security_fix: false,
        rollout_percentage: 100,
        artifacts: vec![
            UpdateArtifactInfo {
                name: "infiltrator-x86_64-linux".to_string(),
                target_triple: "x86_64-unknown-linux-gnu".to_string(),
                download_url: "https://example.com/bin-linux".to_string(),
                sha256: "aabbcc".to_string(),
                size_bytes: 1024,
                signature: None,
            },
            UpdateArtifactInfo {
                name: "infiltrator-x86_64-windows".to_string(),
                target_triple: "x86_64-pc-windows-msvc".to_string(),
                download_url: "https://example.com/bin-win".to_string(),
                sha256: "ddeeff".to_string(),
                size_bytes: 2048,
                signature: Some("sig123".to_string()),
            },
        ],
        deltas: vec![DeltaPatchInfo {
            base_sha256: "112233".to_string(),
            target_sha256: "aabbcc".to_string(),
            patch_url: "https://example.com/patch".to_string(),
            patch_sha256: "334455".to_string(),
            patch_size_bytes: 512,
        }],
    };

    let art_linux = manifest.find_artifact_for_target("x86_64-unknown-linux-gnu");
    assert!(art_linux.is_some());
    assert_eq!(art_linux.unwrap().name, "infiltrator-x86_64-linux");

    let art_win = manifest.find_artifact_for_target("X86_64-PC-WINDOWS-MSVC");
    assert!(art_win.is_some());
    assert_eq!(art_win.unwrap().name, "infiltrator-x86_64-windows");

    let art_none = manifest.find_artifact_for_target("aarch64-apple-darwin");
    assert!(art_none.is_none());

    let delta = manifest.find_delta_patch("112233");
    assert!(delta.is_some());
    assert_eq!(delta.unwrap().patch_size_bytes, 512);

    let delta_none = manifest.find_delta_patch("999999");
    assert!(delta_none.is_none());
}

#[test]
fn test_downgrade_barrier_and_eligibility() {
    let manifest = UpdateManifest {
        version: "0.20.0".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Fixes".to_string(),
        min_supported_version: None,
        critical_security_fix: false,
        rollout_percentage: 100,
        artifacts: vec![],
        deltas: vec![],
    };

    // Newer version -> Eligible
    let el1 = ClientUpdater::check_eligibility("0.19.9", &manifest, "uuid-1", None, None);
    assert!(matches!(el1, UpdateEligibility::Eligible { .. }));

    // Same version -> UpToDate
    let el2 = ClientUpdater::check_eligibility("0.20.0", &manifest, "uuid-1", None, None);
    assert_eq!(
        el2,
        UpdateEligibility::UpToDate {
            current_version: "0.20.0".to_string()
        }
    );

    // Older version -> DowngradeBlocked
    let el3 = ClientUpdater::check_eligibility("0.21.0", &manifest, "uuid-1", None, None);
    assert_eq!(
        el3,
        UpdateEligibility::DowngradeBlocked {
            current_version: "0.21.0".to_string(),
            target_version: "0.20.0".to_string(),
        }
    );
}

#[test]
fn test_min_supported_version_barrier() {
    let manifest = UpdateManifest {
        version: "2.0.0".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Major V2".to_string(),
        min_supported_version: Some("1.5.0".to_string()),
        critical_security_fix: false,
        rollout_percentage: 100,
        artifacts: vec![],
        deltas: vec![],
    };

    // 1.0.0 < 1.5.0 -> BelowMinSupportedVersion
    let el1 = ClientUpdater::check_eligibility("1.0.0", &manifest, "uuid-1", None, None);
    assert_eq!(
        el1,
        UpdateEligibility::BelowMinSupportedVersion {
            current_version: "1.0.0".to_string(),
            min_supported_version: "1.5.0".to_string(),
        }
    );

    // 1.5.0 >= 1.5.0 -> Eligible
    let el2 = ClientUpdater::check_eligibility("1.5.0", &manifest, "uuid-1", None, None);
    assert!(matches!(el2, UpdateEligibility::Eligible { .. }));
}

#[test]
fn test_critical_security_fix_barrier_bypass() {
    let manifest = UpdateManifest {
        version: "2.0.0".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Critical zero-day hotfix".to_string(),
        min_supported_version: Some("1.5.0".to_string()),
        critical_security_fix: true,
        rollout_percentage: 1,
        artifacts: vec![],
        deltas: vec![],
    };

    // Critical security fix bypasses minimum version barrier and rollout restriction
    let el = ClientUpdater::check_eligibility("1.0.0", &manifest, "uuid-excluded", None, None);
    assert!(matches!(
        el,
        UpdateEligibility::Eligible { critical: true, .. }
    ));
    assert!(ClientUpdater::is_update_eligible(
        "1.0.0",
        &manifest,
        "uuid-excluded"
    ));
}

#[test]
fn test_channel_mismatch_check() {
    let manifest = UpdateManifest {
        version: "0.21.0".to_string(),
        channel: UpdateChannel::Beta,
        release_date: "2026-09-02".to_string(),
        release_notes: "Beta update".to_string(),
        min_supported_version: None,
        critical_security_fix: false,
        rollout_percentage: 100,
        artifacts: vec![],
        deltas: vec![],
    };

    let el = ClientUpdater::check_eligibility(
        "0.20.0",
        &manifest,
        "uuid-1",
        Some(UpdateChannel::Stable),
        None,
    );
    assert_eq!(
        el,
        UpdateEligibility::ChannelMismatch {
            current_channel: UpdateChannel::Stable,
            manifest_channel: UpdateChannel::Beta,
        }
    );
}

#[test]
fn test_grayscale_rollout_distribution_and_determinism() {
    let client_a = "device-mac-48-2c-6a-1e-59-3d";
    let bucket_a1 = ClientUpdater::compute_rollout_bucket(client_a);
    let bucket_a2 = ClientUpdater::compute_rollout_bucket(client_a);
    assert_eq!(bucket_a1, bucket_a2);
    assert!(bucket_a1 < 100);

    let manifest_100 = UpdateManifest {
        version: "1.0.1".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Rollout".to_string(),
        min_supported_version: None,
        critical_security_fix: false,
        rollout_percentage: 100,
        artifacts: vec![],
        deltas: vec![],
    };
    assert!(ClientUpdater::is_update_eligible(
        "1.0.0",
        &manifest_100,
        client_a
    ));

    let manifest_0 = UpdateManifest {
        version: "1.0.1".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Rollout".to_string(),
        min_supported_version: None,
        critical_security_fix: false,
        rollout_percentage: 0,
        artifacts: vec![],
        deltas: vec![],
    };
    assert!(!ClientUpdater::is_update_eligible(
        "1.0.0",
        &manifest_0,
        client_a
    ));
    assert!(!ClientUpdater::is_in_rollout(client_a, 0));
    assert!(ClientUpdater::is_in_rollout(client_a, 100));
}

#[test]
fn test_sha256_checksum_and_file_verification() {
    let temp_dir = tempfile::tempdir().unwrap();
    let payload = b"infiltrator silent updater cryptographic payload test";

    let hex = ClientUpdater::compute_sha256(payload);
    assert_eq!(hex.len(), 64);
    assert!(ClientUpdater::verify_sha256(payload, &hex));
    assert!(ClientUpdater::verify_sha256(payload, &hex.to_uppercase()));
    assert!(!ClientUpdater::verify_sha256(payload, "badchecksum1234"));

    let file_path = temp_dir.path().join("payload.bin");
    fs::write(&file_path, payload).unwrap();

    let file_hex = ClientUpdater::compute_file_sha256(&file_path).unwrap();
    assert_eq!(file_hex, hex);
    assert!(ClientUpdater::verify_file_sha256(&file_path, &hex).unwrap());
    assert!(!ClientUpdater::verify_file_sha256(&file_path, "invalidhex").unwrap());
}

#[test]
fn test_stage_payload_integrity_enforcement() {
    let temp_dir = tempfile::tempdir().unwrap();
    let staging_dir = temp_dir.path().join("staging");
    let payload = b"stage payload binary";
    let valid_sha = ClientUpdater::compute_sha256(payload);

    // Staging with valid SHA-256 succeeds
    let staged_path = ClientUpdater::stage_payload(
        payload,
        &staging_dir,
        "staged_binary",
        &valid_sha,
    )
    .unwrap();
    assert!(staged_path.exists());
    assert_eq!(fs::read(&staged_path).unwrap(), payload);

    // Staging with corrupt SHA-256 fails immediately
    let err = ClientUpdater::stage_payload(
        payload,
        &staging_dir,
        "corrupt_binary",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    assert!(err.is_err());
}

#[test]
fn test_atomic_update_and_rollback_flow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target_bin = temp_dir.path().join("infiltrator-client");
    let staging_dir = temp_dir.path().join("staging");

    // 1. Write initial binary
    let v1_data = b"infiltrator v0.20.0 binary content";
    fs::write(&target_bin, v1_data).unwrap();

    // 2. Stage updated binary
    let v2_data = b"infiltrator v0.21.0 new upgraded binary content";
    let v2_sha = ClientUpdater::compute_sha256(v2_data);
    let staged = ClientUpdater::stage_payload(
        v2_data,
        &staging_dir,
        "infiltrator-client.staged",
        &v2_sha,
    )
    .unwrap();

    // 3. Apply atomic update
    let backup = ClientUpdater::apply_atomic_update(&target_bin, &staged).unwrap();
    assert!(backup.exists());
    assert_eq!(fs::read(&target_bin).unwrap(), v2_data);
    assert_eq!(fs::read(&backup).unwrap(), v1_data);

    // 4. Test rollback
    ClientUpdater::rollback(&target_bin, &backup).unwrap();
    assert_eq!(fs::read(&target_bin).unwrap(), v1_data);

    // 5. Cleanup
    ClientUpdater::cleanup_old_artifacts(&target_bin).unwrap();
    assert!(!backup.exists());
    ClientUpdater::cleanup_staging(&staging_dir).unwrap();
    assert!(!staging_dir.exists());
}

#[test]
fn test_updater_instance_workflow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target_bin = temp_dir.path().join("infiltrator");
    let staging_dir = temp_dir.path().join("staging");

    fs::write(&target_bin, b"version 0.20.0 initial binary").unwrap();

    let config = ClientUpdaterConfig {
        current_version: "0.20.0".to_string(),
        client_uuid: "test-client-123".to_string(),
        channel: UpdateChannel::Stable,
        target_binary: target_bin.clone(),
        staging_dir: staging_dir.clone(),
        target_triple: Some("x86_64-unknown-linux-gnu".to_string()),
        allow_downgrade: false,
    };

    let updater = ClientUpdater::new(config);

    let manifest = UpdateManifest {
        version: "0.21.0".to_string(),
        channel: UpdateChannel::Stable,
        release_date: "2026-09-02".to_string(),
        release_notes: "Security & Performance updates".to_string(),
        min_supported_version: Some("0.19.0".to_string()),
        critical_security_fix: false,
        rollout_percentage: 100,
        artifacts: vec![UpdateArtifactInfo {
            name: "infiltrator-linux".to_string(),
            target_triple: "x86_64-unknown-linux-gnu".to_string(),
            download_url: "https://example.com/download".to_string(),
            sha256: "dummy".to_string(),
            size_bytes: 1024,
            signature: None,
        }],
        deltas: vec![],
    };

    let el = updater.evaluate_manifest(&manifest);
    assert!(matches!(el, UpdateEligibility::Eligible { .. }));

    let v2_data = b"version 0.21.0 upgraded binary payload";
    let v2_sha = ClientUpdater::compute_sha256(v2_data);

    let report = updater
        .apply_update_payload(v2_data, "0.21.0", &v2_sha)
        .unwrap();

    assert_eq!(report.target_version, "0.21.0");
    assert_eq!(fs::read(&target_bin).unwrap(), v2_data);
}

#[test]
fn test_atomic_update_missing_staged_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target_bin = temp_dir.path().join("infiltrator");
    let non_existent = temp_dir.path().join("does_not_exist");

    let res = ClientUpdater::apply_atomic_update(&target_bin, &non_existent);
    assert!(res.is_err());
}

#[test]
fn test_rollback_missing_backup_error() {
    let temp_dir = tempfile::tempdir().unwrap();
    let target_bin = temp_dir.path().join("infiltrator");
    let non_existent_bak = temp_dir.path().join("missing.bak");

    let res = ClientUpdater::rollback(&target_bin, &non_existent_bak);
    assert!(res.is_err());
}

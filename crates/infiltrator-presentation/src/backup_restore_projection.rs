//! Pure cross-surface projection for encrypted backup and restore (FLPARITY-013).

use infiltrator_contract::backup_restore::{
    BackupDeliveryTarget, BackupOperationReceipt, BackupRestoreScope, BackupRestoreSnapshot,
};

/// Projected view model for the encrypted backup & restore card/sheet across all surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupRestoreProjection {
    /// Localized label for the selected delivery target.
    pub target_label: String,
    /// Localized label for the selected restore scope.
    pub scope_label: String,
    /// Status badge label (`"就绪"`, `"导出成功"`, `"恢复成功"`, `"校验失败"`).
    pub status_badge_label: String,
    /// Semantic badge tone (`"neutral"`, `"success"`, `"danger"`).
    pub status_badge_tone: String,
    /// Primary receipt headline or guidance text.
    pub receipt_headline: String,
    /// Secondary detail text (e.g. SHA-256 digest prefix, profile count, or failure diagnostics).
    pub receipt_detail: String,
    /// Count of verified backup archives available in the catalog.
    pub available_bundle_count: usize,
}

/// Project a `BackupRestoreSnapshot` into a surface-ready `BackupRestoreProjection`.
pub fn project_backup_restore(snapshot: &BackupRestoreSnapshot) -> BackupRestoreProjection {
    let target_label = snapshot.selected_target.label_zh().to_string();
    let scope_label = snapshot.selected_scope.label_zh().to_string();
    let status_badge_tone = snapshot.last_receipt.badge_tone().to_string();

    let (status_badge_label, receipt_headline, receipt_detail) = match &snapshot.last_receipt {
        BackupOperationReceipt::Idle => (
            "就绪".to_string(),
            "AES-256-GCM 加密归档已就绪".to_string(),
            format!(
                "目标：{} · 恢复策略：{}",
                snapshot.selected_target.label_zh(),
                snapshot.selected_scope.label_zh()
            ),
        ),
        BackupOperationReceipt::ExportSucceeded {
            target,
            destination,
            bundle_version,
            sha256_digest,
            profile_count,
            ..
        } => {
            let short_sha: String = sha256_digest.chars().take(12).collect();
            (
                "导出成功".to_string(),
                format!("已导出至 {} ({destination})", target.label_zh()),
                format!(
                    "归档版本 v{bundle_version} · {profile_count} 个订阅配置档 · SHA-256 {short_sha}…"
                ),
            )
        }
        BackupOperationReceipt::RestoreSucceeded {
            target,
            scope,
            restored_profiles,
            ..
        } => (
            "恢复成功".to_string(),
            format!("已从 {} 完成恢复", target.label_zh()),
            format!(
                "策略：{} · 已恢复 {restored_profiles} 个订阅配置档",
                scope.label_zh()
            ),
        ),
        BackupOperationReceipt::Failed { failure, .. } => (
            "校验失败".to_string(),
            failure.summary_zh(),
            "现有本地配置与订阅档案保持未修改状态".to_string(),
        ),
    };

    BackupRestoreProjection {
        target_label,
        scope_label,
        status_badge_label,
        status_badge_tone,
        receipt_headline,
        receipt_detail,
        available_bundle_count: snapshot.available_bundles.len(),
    }
}

/// Helper to cycle between `LocalFile` and `WebDav` delivery targets.
pub const fn next_delivery_target(current: BackupDeliveryTarget) -> BackupDeliveryTarget {
    match current {
        BackupDeliveryTarget::LocalFile => BackupDeliveryTarget::WebDav,
        BackupDeliveryTarget::WebDav => BackupDeliveryTarget::LocalFile,
    }
}

/// Helper to cycle between `ConfigOnly` and `FullData` restore scopes.
pub const fn next_restore_scope(current: BackupRestoreScope) -> BackupRestoreScope {
    match current {
        BackupRestoreScope::ConfigOnly => BackupRestoreScope::FullData,
        BackupRestoreScope::FullData => BackupRestoreScope::ConfigOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::backup_restore::BackupVerificationFailure;

    #[test]
    fn projects_idle_export_restore_and_failure_states() {
        let mut snap = BackupRestoreSnapshot::default();
        let idle = project_backup_restore(&snap);
        assert_eq!(idle.status_badge_label, "就绪");
        assert_eq!(idle.status_badge_tone, "neutral");

        snap.last_receipt = BackupOperationReceipt::ExportSucceeded {
            target: BackupDeliveryTarget::LocalFile,
            destination: "backup-2026.encpkg".to_string(),
            bundle_version: "1.0.0".to_string(),
            sha256_digest: "abcdef0123456789abcdef".to_string(),
            profile_count: 3,
            exported_at_unix_ms: 1000,
        };
        let exported = project_backup_restore(&snap);
        assert_eq!(exported.status_badge_label, "导出成功");
        assert!(exported.receipt_detail.contains("abcdef012345"));

        snap.last_receipt = BackupOperationReceipt::Failed {
            failure: BackupVerificationFailure::WrongPassphraseOrAuthTag,
            attempted_at_unix_ms: 2000,
        };
        let failed = project_backup_restore(&snap);
        assert_eq!(failed.status_badge_label, "校验失败");
        assert_eq!(failed.status_badge_tone, "danger");
        assert!(failed.receipt_headline.contains("口令错误"));
    }
}

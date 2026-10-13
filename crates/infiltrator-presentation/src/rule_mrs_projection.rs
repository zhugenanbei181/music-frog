//! One fold of observed MRS facts for both native products.
use infiltrator_contract::mrs_acceleration::{MrsAccelerationSnapshot, MrsItemSnapshot};
use infiltrator_shared::locales::{Lang, Localizer};

/// DUAL-11-03: shared MRS acceleration status line rendered from the read model
/// published by the surface reader (never a UI-local fabricated list).
pub fn mrs_acceleration_status_line(lang: &Lang<'_>, mrs: &MrsAccelerationSnapshot) -> String {
    use infiltrator_contract::mrs_acceleration::MrsAccelerationStatus;
    match mrs.status {
        MrsAccelerationStatus::Ready => format!(
            "MRS {} · {} {} · {} {} · {} {} · {}",
            lang.tr("mrs_accel_ready"),
            mrs.total_providers,
            lang.tr("mrs_accel_providers"),
            mrs.total_accelerated_rules,
            lang.tr("mrs_accel_rules"),
            mrs.total_memory_saved_bytes,
            lang.tr("mrs_accel_memory_saved"),
            if mrs.mmap_acceleration_active {
                lang.tr("mrs_accel_mmap_on")
            } else {
                lang.tr("mrs_accel_mmap_off")
            },
        ),
        MrsAccelerationStatus::Empty => lang.tr("mrs_accel_empty").to_string(),
        MrsAccelerationStatus::Unsupported => format!(
            "MRS {}: {}",
            lang.tr("mrs_accel_unsupported"),
            mrs.failure
                .as_deref()
                .unwrap_or(lang.tr("shell_readout_unknown").as_ref())
        ),
        MrsAccelerationStatus::Failed => format!(
            "MRS {}: {}",
            lang.tr("mrs_accel_failed"),
            mrs.failure
                .as_deref()
                .unwrap_or(lang.tr("shell_readout_unknown").as_ref())
        ),
        MrsAccelerationStatus::Unknown => format!(
            "MRS {}: {}",
            lang.tr("mrs_accel_unavailable"),
            mrs.failure
                .as_deref()
                .unwrap_or(lang.tr("shell_readout_unknown").as_ref())
        ),
    }
}

/// One shared MRS item line: name, count, behavior, validity and digest prefix.
pub fn mrs_acceleration_item_label(lang: &Lang<'_>, item: &MrsItemSnapshot) -> String {
    let digest = item
        .sha256_digest
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(|value| format!("sha256 {}", value.chars().take(12).collect::<String>()))
        .unwrap_or_else(|| lang.tr("mrs_accel_no_digest").to_string());
    let validity = if item.is_valid {
        lang.tr("mrs_accel_valid")
    } else {
        lang.tr("mrs_accel_invalid")
    };
    format!(
        "{} · {} {} · {} · {} · {}",
        item.name,
        item.rule_count,
        lang.tr("mrs_accel_entries"),
        item.behavior.as_str(),
        validity,
        digest
    )
}

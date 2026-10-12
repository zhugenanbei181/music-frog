//! Durable storage for the product-wide active-profile reference record.

use crate::active_reference_owner::{
    ACTIVE_CORE_OWNER_STATE_FILE, ActiveCoreOwnerProofRecord, ActiveProfileReferenceRecord,
    parse_active_core_owner_proof_record,
};
use crate::document_store::{
    RECORD_DURABILITY_UNCERTAIN_PREFIX, read_optional, write_document_bytes_durable,
    write_document_bytes_durable_sync,
};
use infiltrator_contract::active_profile_reference::ActiveProfileReferenceState;
use infiltrator_platform::instance_lock::{
    ThreadedInstanceLockGuard, try_acquire_threaded_instance_lock,
};
use infiltrator_ports::error::PortError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::path::PathBuf;
use std::time::Duration;

pub const ACTIVE_PROFILE_REFERENCE_FILE: &str = "active-profile-reference.json";
const ACTIVE_PROFILE_REFERENCE_LOCK_FILE: &str = "active-profile-reference.lock";
const ACTIVE_CORE_OWNER_LOCK_FILE: &str = "active-core-owner.lock";
const ACTIVE_CORE_OWNER_STATE_LOCK_FILE: &str = "active-core-owner-state.lock";

/// Stateless product-root file owner. Revision locks and typed transitions
/// are acquired by the protocol coordinator before calling these operations.
pub struct ActiveProfileReferenceFile {
    config_root: PathBuf,
}

impl ActiveProfileReferenceFile {
    pub fn new(config_root: impl Into<PathBuf>) -> Self {
        Self {
            config_root: config_root.into(),
        }
    }

    pub fn path(&self) -> PathBuf {
        active_profile_reference_path(&self.config_root)
    }

    /// Acquire the product-root serialization lock used by all reference and
    /// owner transitions. Callers acquire owner-state and profile locks after
    /// this lock, preserving the root → owner-state → profile order.
    pub async fn acquire_root_lock(
        &self,
    ) -> Result<(PathBuf, ThreadedInstanceLockGuard), PortError> {
        tokio::fs::create_dir_all(&self.config_root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let config_root = tokio::fs::canonicalize(&self.config_root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let lock_path = config_root.join(ACTIVE_PROFILE_REFERENCE_LOCK_FILE);
        let guard = acquire_instance_lock_async(lock_path).await?;
        Ok((config_root, guard))
    }

    pub async fn acquire_owner_state_lock(&self) -> Result<ThreadedInstanceLockGuard, PortError> {
        acquire_instance_lock_async(self.config_root.join(ACTIVE_CORE_OWNER_STATE_LOCK_FILE)).await
    }

    pub fn try_acquire_owner_lock(&self) -> Result<Option<ThreadedInstanceLockGuard>, PortError> {
        try_acquire_threaded_instance_lock(&self.config_root.join(ACTIVE_CORE_OWNER_LOCK_FILE))
            .map_err(|error| PortError::Io(error.to_string()))
    }

    pub async fn read_active_profile_record(
        &self,
    ) -> Result<Option<ActiveProfileReferenceRecord>, PortError> {
        let record: Option<ActiveProfileReferenceRecord> = self.read().await?;
        record
            .map(|record| record.validate().map(|()| record))
            .transpose()
    }

    pub fn read_active_profile_record_sync(
        &self,
    ) -> Result<Option<ActiveProfileReferenceRecord>, PortError> {
        let record: Option<ActiveProfileReferenceRecord> = self.read_sync()?;
        record
            .map(|record| record.validate().map(|()| record))
            .transpose()
    }

    pub fn write_active_profile_record_sync(
        &self,
        record: &ActiveProfileReferenceRecord,
    ) -> Result<(), PortError> {
        record.validate()?;
        self.write_sync(record)
    }

    pub async fn publish_active_profile_record(
        &self,
        record: &ActiveProfileReferenceRecord,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        record.validate()?;
        let observed = self.publish(record).await?;
        observed.validate()?;
        Ok(observed.selection)
    }

    pub fn publish_active_profile_record_sync(
        &self,
        record: &ActiveProfileReferenceRecord,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        record.validate()?;
        let observed = self.publish_sync(record)?;
        observed.validate()?;
        Ok(observed.selection)
    }

    pub fn read_owner_proof_sync(&self) -> Result<ActiveCoreOwnerProofRecord, PortError> {
        let path = self.config_root.join(ACTIVE_CORE_OWNER_STATE_FILE);
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(ActiveCoreOwnerProofRecord::unowned());
            }
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        parse_active_core_owner_proof_record(&text)
    }

    pub fn write_owner_proof_sync(
        &self,
        record: &ActiveCoreOwnerProofRecord,
    ) -> Result<(), PortError> {
        record.validate()?;
        let text =
            serde_json::to_vec_pretty(record).map_err(|error| PortError::Io(error.to_string()))?;
        write_document_bytes_durable_sync(
            &self.config_root.join(ACTIVE_CORE_OWNER_STATE_FILE),
            &text,
        )
    }

    pub async fn read<T: DeserializeOwned>(&self) -> Result<Option<T>, PortError> {
        let Some(text) = read_optional(&self.path()).await? else {
            return Ok(None);
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| PortError::Io(format!("invalid active-profile reference: {error}")))
    }

    pub fn read_sync<T: DeserializeOwned>(&self) -> Result<Option<T>, PortError> {
        let text = match std::fs::read_to_string(self.path()) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|error| PortError::Io(format!("invalid active-profile reference: {error}")))
    }

    pub async fn write<T: Serialize>(&self, record: &T) -> Result<(), PortError> {
        let text = serde_json::to_string_pretty(record)
            .map_err(|error| PortError::Io(error.to_string()))?;
        write_document_bytes_durable(&self.path(), text.as_bytes()).await
    }

    pub fn write_sync<T: Serialize>(&self, record: &T) -> Result<(), PortError> {
        let bytes =
            serde_json::to_vec_pretty(record).map_err(|error| PortError::Io(error.to_string()))?;
        write_document_bytes_durable_sync(&self.path(), &bytes)
    }

    pub async fn publish<T>(&self, record: &T) -> Result<T, PortError>
    where
        T: Serialize + DeserializeOwned + PartialEq,
    {
        self.write(record).await?;
        let observed = self.read().await?.ok_or_else(|| {
            PortError::Failed("active profile reference disappeared after publication".into())
        })?;
        if observed != *record {
            return Err(PortError::Failed(
                "active profile reference readback differed from the committed record".into(),
            ));
        }
        Ok(observed)
    }

    pub fn publish_sync<T>(&self, record: &T) -> Result<T, PortError>
    where
        T: Serialize + DeserializeOwned + PartialEq,
    {
        self.write_sync(record)?;
        let observed = self.read_sync()?.ok_or_else(|| {
            PortError::Failed("active profile reference disappeared after publication".into())
        })?;
        if observed != *record {
            return Err(PortError::Failed(
                "active profile reference readback differed from the committed record".into(),
            ));
        }
        Ok(observed)
    }
}

pub fn is_durability_uncertain(error: &PortError) -> bool {
    matches!(error, PortError::Failed(message) if message.starts_with(RECORD_DURABILITY_UNCERTAIN_PREFIX))
}

fn active_profile_reference_path(config_root: &std::path::Path) -> PathBuf {
    config_root.join(ACTIVE_PROFILE_REFERENCE_FILE)
}

async fn acquire_instance_lock_async(
    lock_path: PathBuf,
) -> Result<ThreadedInstanceLockGuard, PortError> {
    loop {
        let attempt_path = lock_path.clone();
        let attempt = tokio::task::spawn_blocking(move || {
            try_acquire_threaded_instance_lock(&attempt_path)
                .map_err(|error| PortError::Io(error.to_string()))
        })
        .await
        .map_err(|error| PortError::Io(error.to_string()))??;
        if let Some(guard) = attempt {
            return Ok(guard);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(test)]
#[path = "active_reference_file_tests.rs"]
mod tests;

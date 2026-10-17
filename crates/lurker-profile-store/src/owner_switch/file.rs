//! Durable storage for one generic owner-switch record and its owner proof.

use crate::document_store::{
    RECORD_DURABILITY_UNCERTAIN_PREFIX, read_optional, write_document_bytes_durable,
    write_document_bytes_durable_sync,
};
use crate::instance_lock::{ThreadedInstanceLockGuard, try_acquire_threaded_instance_lock};
use crate::owner_switch::codec::OwnerSwitchCodec;
use crate::owner_switch::owner::{
    ActiveCoreOwnerProofRecord, parse_active_core_owner_proof_record,
};
use lurker_core_ports::error::PortError;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fs::read_to_string;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::{canonicalize, create_dir_all};
use tokio::task::spawn_blocking;
use tokio::time::sleep;

/// Stateless product-root file owner. Revision locks and typed transitions are
/// acquired by the protocol coordinator before calling these operations.
pub struct OwnerSwitchFile<C: OwnerSwitchCodec> {
    config_root: PathBuf,
    codec: PhantomData<C>,
}

impl<C: OwnerSwitchCodec> OwnerSwitchFile<C> {
    pub fn new(config_root: impl Into<PathBuf>) -> Self {
        Self {
            config_root: config_root.into(),
            codec: PhantomData,
        }
    }

    pub fn path(&self) -> PathBuf {
        self.config_root
            .join(C::layout().active_profile_reference_file_name.as_str())
    }

    /// Acquire the product-root serialization lock used by all reference and
    /// owner transitions. Callers acquire owner-state and profile locks after
    /// this lock, preserving the root -> owner-state -> profile order.
    pub async fn acquire_root_lock(
        &self,
    ) -> Result<(PathBuf, ThreadedInstanceLockGuard), PortError> {
        let config_root = self.config_root.clone();
        create_dir_all(&config_root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let config_root = canonicalize(&config_root)
            .await
            .map_err(|error| PortError::Io(error.to_string()))?;
        let lock_path =
            config_root.join(C::layout().active_profile_reference_lock_file_name.as_str());
        let guard = acquire_instance_lock_async(lock_path).await?;
        Ok((config_root, guard))
    }

    pub async fn acquire_owner_state_lock(&self) -> Result<ThreadedInstanceLockGuard, PortError> {
        acquire_instance_lock_async(
            self.config_root
                .join(C::layout().active_core_owner_state_lock_file_name.as_str()),
        )
        .await
    }

    pub fn try_acquire_owner_lock(&self) -> Result<Option<ThreadedInstanceLockGuard>, PortError> {
        try_acquire_threaded_instance_lock(
            &self
                .config_root
                .join(C::layout().active_core_owner_lock_file_name.as_str()),
        )
        .map_err(|error| PortError::Io(error.to_string()))
    }

    pub async fn read_active_profile_record(&self) -> Result<Option<C::Record>, PortError> {
        let Some(text) = read_optional(&self.path()).await? else {
            return Ok(None);
        };
        let record = C::decode(text.as_bytes())?;
        C::validate(&record)?;
        Ok(Some(record))
    }

    pub async fn read_active_profile_record_or_pending(&self) -> Result<C::Record, PortError> {
        Ok(self
            .read_active_profile_record()
            .await?
            .unwrap_or_else(C::pending))
    }

    pub fn read_active_profile_record_sync(&self) -> Result<Option<C::Record>, PortError> {
        let text = match read_to_string(self.path()) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(PortError::Io(error.to_string())),
        };
        let record = C::decode(text.as_bytes())?;
        C::validate(&record)?;
        Ok(Some(record))
    }

    pub fn read_active_profile_record_sync_or_pending(&self) -> Result<C::Record, PortError> {
        Ok(self
            .read_active_profile_record_sync()?
            .unwrap_or_else(C::pending))
    }

    pub fn write_active_profile_record_sync(&self, record: &C::Record) -> Result<(), PortError> {
        C::validate(record)?;
        let bytes = C::encode(record)?;
        write_document_bytes_durable_sync(&self.path(), &bytes)
    }

    pub async fn publish_active_profile_record(
        &self,
        record: &C::Record,
    ) -> Result<C::Selection, PortError> {
        C::validate(record)?;
        let bytes = C::encode(record)?;
        write_document_bytes_durable(&self.path(), &bytes).await?;
        let observed = self.read_active_profile_record().await?.ok_or_else(|| {
            PortError::Failed("active profile reference disappeared after publication".into())
        })?;
        if &observed != record {
            return Err(PortError::Failed(
                "active profile reference readback differed from the committed record".into(),
            ));
        }
        Ok(C::selection(&observed).clone())
    }

    pub fn publish_active_profile_record_sync(
        &self,
        record: &C::Record,
    ) -> Result<C::Selection, PortError> {
        C::validate(record)?;
        self.write_active_profile_record_sync(record)?;
        let observed = self.read_active_profile_record_sync()?.ok_or_else(|| {
            PortError::Failed("active profile reference disappeared after publication".into())
        })?;
        if &observed != record {
            return Err(PortError::Failed(
                "active profile reference readback differed from the committed record".into(),
            ));
        }
        Ok(C::selection(&observed).clone())
    }

    pub fn read_owner_proof_sync(&self) -> Result<ActiveCoreOwnerProofRecord, PortError> {
        let path = self
            .config_root
            .join(C::layout().active_core_owner_state_file_name.as_str());
        let text = match read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => {
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
            &self
                .config_root
                .join(C::layout().active_core_owner_state_file_name.as_str()),
            &text,
        )
    }

    pub fn publish_active_profile_record_sync_reconciled(
        &self,
        record: &C::Record,
    ) -> Result<(), PortError> {
        let write_result = self.write_active_profile_record_sync(record);
        reconcile_record_publication("active profile reference", record, write_result, || {
            self.read_active_profile_record_sync()?.ok_or_else(|| {
                PortError::Failed("active profile reference disappeared after write".into())
            })
        })
    }

    pub fn publish_owner_proof_sync_reconciled(
        &self,
        record: &ActiveCoreOwnerProofRecord,
    ) -> Result<(), PortError> {
        let write_result = self.write_owner_proof_sync(record);
        reconcile_record_publication("active Core owner", record, write_result, || {
            self.read_owner_proof_sync()
        })
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
        let text = match read_to_string(self.path()) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
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

    pub fn config_root(&self) -> &Path {
        &self.config_root
    }
}

pub fn is_durability_uncertain(error: &PortError) -> bool {
    matches!(error, PortError::Failed(message) if message.starts_with(RECORD_DURABILITY_UNCERTAIN_PREFIX))
}

async fn acquire_instance_lock_async(
    lock_path: PathBuf,
) -> Result<ThreadedInstanceLockGuard, PortError> {
    loop {
        let attempt_path = lock_path.clone();
        let attempt = spawn_blocking(move || {
            try_acquire_threaded_instance_lock(&attempt_path)
                .map_err(|error| PortError::Io(error.to_string()))
        })
        .await
        .map_err(|error| PortError::Io(error.to_string()))??;
        if let Some(guard) = attempt {
            return Ok(guard);
        }
        sleep(Duration::from_millis(10)).await;
    }
}

pub fn reconcile_record_publication<T: PartialEq>(
    record_name: &str,
    expected: &T,
    write_result: Result<(), PortError>,
    readback: impl FnOnce() -> Result<T, PortError>,
) -> Result<(), PortError> {
    match write_result {
        Ok(()) => {
            let observed = readback()?;
            if observed == *expected {
                Ok(())
            } else {
                Err(PortError::Failed(format!(
                    "{record_name} readback differed from the committed record"
                )))
            }
        }
        Err(write_error) if is_durability_uncertain(&write_error) => {
            let observed = readback().map_err(|read_error| {
                PortError::Failed(format!(
                    "{record_name} replacement is visible but durability is uncertain; readback also failed: {read_error}"
                ))
            })?;
            if observed != *expected {
                return Err(PortError::Failed(format!(
                    "{record_name} replacement is visible but durability is uncertain; readback differed from the intended record ({write_error})"
                )));
            }
            log::warn!(
                "{record_name} readback matches after a directory durability error; continuing with the visible committed state: {write_error}"
            );
            Ok(())
        }
        Err(write_error) => Err(write_error),
    }
}

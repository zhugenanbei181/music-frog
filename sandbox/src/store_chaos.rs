//! The store scenarios of the headless sandbox.
//!
//! The scenarios observe the durable facts of the profile store: the
//! cross-process write fence and the corrupted profile document. Both
//! observations are read back from the store itself, never simulated.

use lurker_core_contract::profile_document::DocumentValidation;
use lurker_core_ports::profile_document_codec::ProfileDocumentCodecPort;
use lurker_profile_store::document_store::{
    read_optional_bytes, write_document_bytes_durable_sync,
};
use lurker_profile_store::instance_lock::try_acquire_instance_lock;
use std::path::Path;

/// The observation of one write fence contention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FenceContention {
    /// The first owner acquired the fence.
    pub acquired: bool,
    /// The second acquisition was refused while the first owner held it.
    pub contended: bool,
    /// The fence was acquirable again after the holder released it.
    pub reclaimed: bool,
}

/// Observes the cross-process write fence under contention.
///
/// The second acquisition never waits: the kernel lock answers immediately
/// with "busy", so the observation is deterministic.
pub fn observe_fence_contention(lock_path: &Path) -> FenceContention {
    let first = try_acquire_instance_lock(lock_path).expect("the lock path is usable");
    let acquired = first.is_some();
    let contended = try_acquire_instance_lock(lock_path)
        .expect("the lock path is usable")
        .is_none();
    drop(first);
    let reclaimed = try_acquire_instance_lock(lock_path)
        .expect("the lock path is usable")
        .is_some();
    FenceContention {
        acquired,
        contended,
        reclaimed,
    }
}

/// The observation of one corrupted profile document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorruptedDocument {
    /// The codec accepted the intact document.
    pub intact_accepted: bool,
    /// The codec rejected the corrupted document with typed diagnostics.
    pub corrupted_rejected: bool,
    /// The number of diagnostics the codec reported.
    pub diagnostics: usize,
    /// The durable write was read back exactly.
    pub recovered_exactly: bool,
}

/// Observes how the store and the codec answer a corrupted profile document.
///
/// The corrupted bytes are written to the store, read back and handed to the
/// codec, so the rejection is a typed observation of the real bytes. The
/// recovery writes the intact document again and reads it back.
pub async fn observe_corrupted_document(
    codec: &dyn ProfileDocumentCodecPort,
    document_path: &Path,
) -> CorruptedDocument {
    let intact = b"proxies: []\n";
    write_document_bytes_durable_sync(document_path, intact).expect("the durable write succeeds");

    let stored = read_optional_bytes(document_path)
        .await
        .expect("the document read succeeds");
    let intact_accepted = matches!(
        codec
            .inspect(stored.as_deref().unwrap_or_default())
            .expect("the codec inspects the intact document"),
        DocumentValidation::Valid
    );

    let corrupted = b"proxies: [ this is not: valid yaml\n";
    std::fs::write(document_path, corrupted).expect("the corrupted document is written");
    let stored = read_optional_bytes(document_path)
        .await
        .expect("the document read succeeds");
    let validation = codec
        .inspect(stored.as_deref().unwrap_or_default())
        .expect("the codec inspects the corrupted document");
    let (corrupted_rejected, diagnostics) = match validation {
        DocumentValidation::Valid => (false, 0),
        DocumentValidation::Invalid { diagnostics } => (true, diagnostics.len()),
    };

    write_document_bytes_durable_sync(document_path, intact).expect("the recovery write succeeds");
    let recovered = read_optional_bytes(document_path)
        .await
        .expect("the document read succeeds");
    let recovered_exactly = recovered.as_deref() == Some(intact.as_slice());

    CorruptedDocument {
        intact_accepted,
        corrupted_rejected,
        diagnostics,
        recovered_exactly,
    }
}

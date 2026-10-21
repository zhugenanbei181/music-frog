//! Isolated rule-provider runtime and cache port fixtures.
//!
//! The runtime fixture publishes a real provider inventory, refuses a refresh
//! whose expected revision is stale, and fails the refresh when the capture
//! asks for a failing provider. The cache fixture publishes the real bytes of
//! the provider files it holds, reports honest purge counts, and answers
//! `Ok(None)` for a provider it never downloaded.

use async_trait::async_trait;
use infiltrator_contract::provider_cache::{
    ProviderCachePurge, ProviderContentOrigin, ProviderFileFingerprint, RuleProviderCacheSnapshot,
};
use infiltrator_contract::rule_provider_runtime::{
    RuleProviderContractError, RuleProviderFact, RuleProviderId, RuleProviderInventorySnapshot,
    RuleProviderPayload, RuleProviderPayloadSnapshot, RuleProviderRefreshReceipt,
    RuleProviderRevision,
};
use infiltrator_domain::rules::provider_store::RuleProviderDeclaration;
use infiltrator_domain::snapshots::content_hash;
use infiltrator_ports::rule_provider_cache::{
    ProviderCacheEntry, ProviderFileFact, RuleProviderCachePort,
};
use infiltrator_ports::rule_provider_runtime::RuleProviderPort;
use lurker_core_contract::error::{ErrorCode, Failure};
use lurker_core_contract::identity::{CoreScope, EngineId};
use lurker_core_ports::error::PortError;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// The engine the capture providers belong to.
pub const CAPTURE_ENGINE: &str = "mihomo";
/// The provider the capture publishes first.
pub const CAPTURE_PROVIDER: &str = "ads";
/// The provider the capture publishes second.
pub const CAPTURE_SECOND_PROVIDER: &str = "geo";
/// The rules the capture provider publishes.
pub const CAPTURE_PROVIDER_RULES: [&str; 2] = [
    "DOMAIN-SUFFIX,ads.example.com",
    "DOMAIN,tracker.example.com",
];

/// An isolated rule-provider runtime that publishes a real inventory.
pub struct RuleProviderCapturePort {
    providers: Mutex<Vec<RuleProviderFact>>,
    revision: Mutex<RuleProviderRevision>,
    refresh_failure: Mutex<Option<String>>,
    payload: Mutex<BTreeMap<String, Vec<String>>>,
}

impl Default for RuleProviderCapturePort {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleProviderCapturePort {
    /// A capture with two providers and a published payload.
    pub fn new() -> Self {
        let engine_id = EngineId::new(CAPTURE_ENGINE).expect("valid capture engine ID");
        let providers = vec![
            RuleProviderFact {
                id: RuleProviderId::new(engine_id.clone(), CAPTURE_PROVIDER)
                    .expect("valid capture provider ID"),
                provider_type: "http".to_owned(),
                behavior: "domain".to_owned(),
                vehicle_type: "HTTP".to_owned(),
                updated_at: "2026-10-20T00:00:00Z".to_owned(),
                rule_count: CAPTURE_PROVIDER_RULES.len() as u32,
            },
            RuleProviderFact {
                id: RuleProviderId::new(engine_id, CAPTURE_SECOND_PROVIDER)
                    .expect("valid capture provider ID"),
                provider_type: "http".to_owned(),
                behavior: "geoip".to_owned(),
                vehicle_type: "HTTP".to_owned(),
                updated_at: "2026-10-20T00:00:00Z".to_owned(),
                rule_count: 1,
            },
        ];
        let mut payload = BTreeMap::new();
        payload.insert(
            CAPTURE_PROVIDER.to_owned(),
            CAPTURE_PROVIDER_RULES
                .iter()
                .map(|rule| (*rule).to_owned())
                .collect(),
        );
        Self {
            providers: Mutex::new(providers),
            revision: Mutex::new(RuleProviderRevision::initial()),
            refresh_failure: Mutex::new(None),
            payload: Mutex::new(payload),
        }
    }

    /// Makes the next refresh fail with the given reason.
    pub fn fail_refresh(&self, reason: impl Into<String>) {
        *self
            .refresh_failure
            .lock()
            .expect("rule provider capture failure") = Some(reason.into());
    }

    /// The inventory revision the port currently publishes.
    pub fn current_revision(&self) -> RuleProviderRevision {
        *self
            .revision
            .lock()
            .expect("rule provider capture revision")
    }

    fn build_inventory(
        &self,
        scope: &CoreScope,
    ) -> Result<RuleProviderInventorySnapshot, RuleProviderContractError> {
        RuleProviderInventorySnapshot::new(
            scope.clone(),
            self.current_revision(),
            self.providers
                .lock()
                .expect("rule provider capture providers")
                .clone(),
        )
    }
}

fn contract_failure(error: RuleProviderContractError) -> PortError {
    PortError::Rejected(Failure::new(
        ErrorCode::InvalidState,
        error.to_string(),
        false,
    ))
}

#[async_trait]
impl RuleProviderPort for RuleProviderCapturePort {
    async fn inventory(
        &self,
        scope: CoreScope,
    ) -> Result<RuleProviderInventorySnapshot, PortError> {
        self.build_inventory(&scope).map_err(contract_failure)
    }

    async fn refresh(
        &self,
        scope: CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderRefreshReceipt, PortError> {
        if expected_revision != self.current_revision() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                format!(
                    "stale rule provider revision: expected {}, observed {}",
                    expected_revision.value(),
                    self.current_revision().value()
                ),
                true,
            )));
        }
        let inventory = self.build_inventory(&scope).map_err(contract_failure)?;
        if inventory.provider(&provider_id).is_none() {
            return Err(PortError::NotFound(format!(
                "rule provider {} is not part of the observed inventory",
                provider_id.name()
            )));
        }
        if let Some(reason) = self
            .refresh_failure
            .lock()
            .expect("rule provider capture failure")
            .clone()
        {
            return Err(PortError::Network(reason));
        }
        {
            let mut revision = self
                .revision
                .lock()
                .expect("rule provider capture revision");
            let next = revision.next().map_err(contract_failure)?;
            *revision = next;
        }
        let refreshed = self.build_inventory(&scope).map_err(contract_failure)?;
        RuleProviderRefreshReceipt::accepted(scope, expected_revision, refreshed, provider_id)
            .map_err(contract_failure)
    }

    async fn payload(
        &self,
        scope: CoreScope,
        expected_revision: RuleProviderRevision,
        provider_id: RuleProviderId,
    ) -> Result<RuleProviderPayloadSnapshot, PortError> {
        if expected_revision != self.current_revision() {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                format!(
                    "stale rule provider revision: expected {}, observed {}",
                    expected_revision.value(),
                    self.current_revision().value()
                ),
                true,
            )));
        }
        let inventory = self.build_inventory(&scope).map_err(contract_failure)?;
        if inventory.provider(&provider_id).is_none() {
            return Err(PortError::NotFound(format!(
                "rule provider {} is not part of the observed inventory",
                provider_id.name()
            )));
        }
        let rules = self
            .payload
            .lock()
            .expect("rule provider capture payload")
            .get(provider_id.name())
            .cloned();
        let payload = match rules {
            Some(rules) => RuleProviderPayload::Published(rules),
            None => RuleProviderPayload::NotPublished,
        };
        RuleProviderPayloadSnapshot::new(scope, expected_revision, provider_id, payload)
            .map_err(contract_failure)
    }
}

/// The cache directory the capture fixture publishes.
pub const CAPTURE_CACHE_DIRECTORY: &str = "/isolated-profile-store/rules";

/// An isolated rule-provider cache that publishes the real cached bytes.
pub struct RuleProviderCacheCapture {
    directory: String,
    files: Mutex<BTreeMap<String, ProviderCacheEntry>>,
}

impl Default for RuleProviderCacheCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl RuleProviderCacheCapture {
    /// A cache that holds one downloaded provider file.
    pub fn new() -> Self {
        let mut files = BTreeMap::new();
        files.insert(
            CAPTURE_PROVIDER.to_owned(),
            ProviderCacheEntry {
                origin: ProviderContentOrigin::KernelCacheFile,
                path: Some(PathBuf::from(format!(
                    "{CAPTURE_CACHE_DIRECTORY}/{CAPTURE_PROVIDER}.yaml"
                ))),
                bytes: CAPTURE_PROVIDER_RULES
                    .iter()
                    .map(|rule| format!("{rule}\n"))
                    .collect::<Vec<_>>()
                    .concat()
                    .into_bytes(),
            },
        );
        Self {
            directory: CAPTURE_CACHE_DIRECTORY.to_owned(),
            files: Mutex::new(files),
        }
    }

    /// A cache with no provider file at all.
    pub fn empty() -> Self {
        Self {
            directory: CAPTURE_CACHE_DIRECTORY.to_owned(),
            files: Mutex::new(BTreeMap::new()),
        }
    }

    fn entry(&self, declaration: &RuleProviderDeclaration) -> Option<ProviderCacheEntry> {
        self.files
            .lock()
            .expect("rule provider cache files")
            .get(&declaration.name)
            .cloned()
    }
}

#[async_trait]
impl RuleProviderCachePort for RuleProviderCacheCapture {
    async fn read_provider(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderCacheEntry>, PortError> {
        Ok(self.entry(declaration))
    }

    async fn fingerprint(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Result<Option<ProviderFileFact>, PortError> {
        let Some(entry) = self.entry(declaration) else {
            return Ok(None);
        };
        let Some(path) = entry.path.clone() else {
            return Ok(None);
        };
        Ok(Some(ProviderFileFact {
            path,
            fingerprint: ProviderFileFingerprint {
                size_bytes: entry.bytes.len() as u64,
                sha256: content_hash(&entry.bytes),
                modified_unix_secs: None,
            },
        }))
    }

    async fn purge(&self) -> Result<ProviderCachePurge, PortError> {
        let mut files = self.files.lock().expect("rule provider cache files");
        let files_removed = files.len();
        let bytes_freed = files
            .values()
            .map(|entry| entry.bytes.len() as u64)
            .sum::<u64>();
        files.clear();
        Ok(ProviderCachePurge {
            directory: Some(self.directory.clone()),
            files_removed,
            bytes_freed,
        })
    }

    async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError> {
        let files = self.files.lock().expect("rule provider cache files");
        let total_bytes = files
            .values()
            .map(|entry| entry.bytes.len() as u64)
            .sum::<u64>();
        Ok(RuleProviderCacheSnapshot::ready(
            self.directory.clone(),
            files.len(),
            total_bytes,
        ))
    }
}

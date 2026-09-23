//! Rule-provider unpack and cache maintenance use-cases (DUAL-11-06/07).
//!
//! This is the one place that turns a profile's `rule-providers` declaration
//! into real custom rules and the one place that purges the host's kernel
//! cache. Both surfaces call into this module, so neither can fabricate a
//! provider payload or claim a cleanup that never happened.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::provider_cache::{
    ProviderCacheFingerprint, ProviderCachePurge, ProviderContentOrigin, ProviderFileFingerprint,
};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::provider_store::{
    RuleProviderDeclaration, deconstruct_provider_payload,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Real rules read out of one provider, plus the source they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderUnpackPlan {
    pub provider_name: String,
    pub origin: ProviderContentOrigin,
    pub entries: Vec<RuleEntry>,
    pub considered: usize,
    pub skipped: usize,
}

impl ProviderUnpackPlan {
    pub fn imported(&self) -> usize {
        self.entries.len()
    }
}

/// Shared unpack + cache-maintenance service.
#[derive(Clone, Default)]
pub struct RuleProviderApplication {
    cache: Option<Arc<dyn RuleProviderCachePort>>,
    /// DUAL-11-05: the last local cache fingerprint this client observed per
    /// provider. The comparison happens here, between two local observations,
    /// because the kernel's HTTP validator exchange is invisible to the client.
    observed_fingerprints: Arc<Mutex<HashMap<String, ProviderFileFingerprint>>>,
}

impl RuleProviderApplication {
    pub fn new(cache: Option<Arc<dyn RuleProviderCachePort>>) -> Self {
        Self {
            cache,
            observed_fingerprints: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Whether the host exposes a rule-provider cache location at all.
    pub fn configured(&self) -> bool {
        self.cache.is_some()
    }

    /// Resolve one provider's real rules.
    ///
    /// Source order, all of them observable facts:
    /// 1. the profile's `payload:` (inline providers keep their rules there),
    /// 2. the declaration's local file / the kernel's downloaded cache file,
    /// 3. the running controller's published payload.
    ///
    /// When none of the three produces rules the failure is typed and names
    /// every source that was tried — no surface gets a fabricated sample.
    pub async fn deconstruct(
        &self,
        declaration: &RuleProviderDeclaration,
        target: &str,
        gateway: Option<&dyn RuntimeGateway>,
    ) -> Result<ProviderUnpackPlan, Failure> {
        let name = declaration.name.as_str();
        if !declaration.payload.is_empty() {
            return deconstruct_lines(
                declaration,
                &declaration.payload,
                target,
                ProviderContentOrigin::InlinePayload,
            );
        }
        let mut local_failure: Option<String> = None;
        if let Some(cache) = self.cache.as_ref() {
            match cache.read_provider(declaration).await {
                Ok(Some(entry)) => {
                    let origin = entry.origin;
                    let deconstructed =
                        deconstruct_provider_payload(&entry.bytes, declaration, target).map_err(
                            |error| {
                                Failure::new(
                                    ErrorCode::InvalidInput,
                                    format!("rule provider {name} could not be read: {error}"),
                                    false,
                                )
                            },
                        )?;
                    return Ok(ProviderUnpackPlan {
                        provider_name: name.to_owned(),
                        origin,
                        entries: deconstructed.entries,
                        considered: deconstructed.considered,
                        skipped: deconstructed.skipped,
                    });
                }
                Ok(None) => {}
                Err(error) => local_failure = Some(error.to_string()),
            }
        }
        if let Some(gateway) = gateway {
            match gateway.rule_provider_payload(name).await {
                Ok(Some(payload)) if !payload.is_empty() => {
                    return deconstruct_lines(
                        declaration,
                        &payload,
                        target,
                        ProviderContentOrigin::ControllerPayload,
                    );
                }
                Ok(_) => {}
                Err(PortError::Unsupported { reason, .. }) => {
                    local_failure.get_or_insert(reason);
                }
                Err(error) => return Err(Failure::from(error)),
            }
        }
        let mut tried = vec!["profile inline payload"];
        if self.cache.is_some() {
            tried.push("declared file / kernel cache file");
        }
        tried.push("controller payload");
        let detail = local_failure
            .map(|failure| format!(" ({failure})"))
            .unwrap_or_default();
        Err(Failure::new(
            ErrorCode::Unsupported,
            format!(
                "rule provider {name} exposes no readable rule list: tried {}{detail}",
                tried.join(", ")
            ),
            false,
        ))
    }

    /// Delete the host's cached rule-provider files and report real counts.
    pub async fn purge(&self) -> Result<ProviderCachePurge, Failure> {
        let Some(cache) = self.cache.as_ref() else {
            return Err(unsupported_cache());
        };
        cache.purge().await.map_err(Failure::from)
    }

    /// Observed cache location fact for the shared read model.
    pub async fn snapshot(
        &self,
    ) -> infiltrator_contract::provider_cache::RuleProviderCacheSnapshot {
        let Some(cache) = self.cache.as_ref() else {
            return infiltrator_contract::provider_cache::RuleProviderCacheSnapshot::unsupported(
                "this host does not expose a rule-provider cache location",
            );
        };
        match cache.snapshot().await {
            Ok(snapshot) => snapshot,
            Err(error) => infiltrator_contract::provider_cache::RuleProviderCacheSnapshot::failed(
                error.to_string(),
            ),
        }
    }

    /// DUAL-11-05: observe a provider's local cache file and compare it with
    /// the previous observation *this client* made.
    ///
    /// `None` means the host had no file to fingerprint (no cache port, a
    /// non-remote provider, or no downloaded file yet). The returned
    /// observation carries only local facts — size, SHA-256 and last-modified —
    /// and must never be rendered as an HTTP `ETag`/`304` result.
    pub async fn observe_fingerprint(
        &self,
        declaration: &RuleProviderDeclaration,
    ) -> Option<ProviderCacheFingerprint> {
        let cache = self.cache.as_ref()?;
        let fact = match cache.fingerprint(declaration).await {
            Ok(Some(fact)) => fact,
            Ok(None) | Err(_) => return None,
        };
        let provider = declaration.name.clone();
        let (previous, change) = {
            let mut log = self
                .observed_fingerprints
                .lock()
                .expect("provider fingerprint log lock");
            let previous = log.get(&provider).cloned();
            let change = ProviderCacheFingerprint::compare(previous.as_ref(), &fact.fingerprint);
            log.insert(provider.clone(), fact.fingerprint.clone());
            (previous, change)
        };
        Some(ProviderCacheFingerprint {
            provider,
            path: fact.path.display().to_string(),
            change,
            current: fact.fingerprint,
            previous,
        })
    }
}

fn unsupported_cache() -> Failure {
    Failure::new(
        ErrorCode::Unsupported,
        "this host does not expose a rule-provider cache location",
        false,
    )
}

fn deconstruct_lines(
    declaration: &RuleProviderDeclaration,
    lines: &[String],
    target: &str,
    origin: ProviderContentOrigin,
) -> Result<ProviderUnpackPlan, Failure> {
    let (entries, skipped) =
        infiltrator_domain::rules::provider_store::unpack_provider_rules_with_behavior(
            lines,
            declaration.behavior,
            target,
        );
    if entries.is_empty() {
        return Err(Failure::new(
            ErrorCode::InvalidInput,
            format!(
                "rule provider {} produced no usable rules from {} payload",
                declaration.name,
                origin.as_str()
            ),
            false,
        ));
    }
    Ok(ProviderUnpackPlan {
        provider_name: declaration.name.clone(),
        origin,
        considered: lines.len(),
        entries,
        skipped,
    })
}

/// Map an unsupported port failure onto the shared failure contract.
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use infiltrator_contract::provider_cache::RuleProviderCacheSnapshot;
    use infiltrator_ports::rule_provider_cache::ProviderCacheEntry;
    use infiltrator_ports::rule_provider_cache::ProviderFileFact;
    use serde_json::json;
    use std::path::PathBuf;

    struct FakeCache {
        entry: Option<ProviderCacheEntry>,
        purge: ProviderCachePurge,
        /// Scripted fingerprint answers, popped per call; `None` means "no
        /// local file to fingerpring" (an honest absence, not an error).
        facts: Arc<Mutex<Vec<Option<ProviderFileFact>>>>,
    }

    impl FakeCache {
        fn new(entry: Option<ProviderCacheEntry>, purge: ProviderCachePurge) -> Self {
            Self {
                entry,
                purge,
                facts: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_facts(mut self, facts: Vec<Option<ProviderFileFact>>) -> Self {
            self.facts = Arc::new(Mutex::new(facts));
            self
        }
    }

    fn file_fact(
        sha256: &str,
        size_bytes: u64,
        modified_unix_secs: Option<i64>,
    ) -> ProviderFileFact {
        ProviderFileFact {
            path: PathBuf::from(format!("/kernel/rules/{sha256}")),
            fingerprint: ProviderFileFingerprint {
                size_bytes,
                sha256: sha256.to_owned(),
                modified_unix_secs,
            },
        }
    }

    #[async_trait]
    impl RuleProviderCachePort for FakeCache {
        async fn read_provider(
            &self,
            _declaration: &RuleProviderDeclaration,
        ) -> Result<Option<ProviderCacheEntry>, PortError> {
            Ok(self.entry.clone())
        }

        async fn fingerprint(
            &self,
            _declaration: &RuleProviderDeclaration,
        ) -> Result<Option<ProviderFileFact>, PortError> {
            let mut facts = self.facts.lock().expect("fake facts");
            if facts.is_empty() {
                return Ok(None);
            }
            Ok(facts.remove(0))
        }

        async fn purge(&self) -> Result<ProviderCachePurge, PortError> {
            Ok(self.purge.clone())
        }

        async fn snapshot(&self) -> Result<RuleProviderCacheSnapshot, PortError> {
            Ok(RuleProviderCacheSnapshot::ready("/cache/rules", 2, 512))
        }
    }

    fn declaration(value: serde_json::Value) -> RuleProviderDeclaration {
        RuleProviderDeclaration::from_value("ads", &value)
    }

    #[tokio::test]
    async fn inline_payload_unpacks_without_touching_the_host() {
        let application = RuleProviderApplication::default();
        let decl = declaration(json!({
            "type": "inline",
            "behavior": "domain",
            "payload": ["ads.com", "tracker.net"]
        }));
        let plan = application
            .deconstruct(&decl, "REJECT", None)
            .await
            .expect("plan");
        assert_eq!(plan.origin, ProviderContentOrigin::InlinePayload);
        assert_eq!(plan.imported(), 2);
        assert_eq!(plan.skipped, 0);
        assert_eq!(plan.entries[0].rule, "DOMAIN-SUFFIX,ads.com,REJECT");
    }

    #[tokio::test]
    async fn local_cache_file_is_read_before_the_controller() {
        let cache = FakeCache::new(
            Some(ProviderCacheEntry {
                origin: ProviderContentOrigin::KernelCacheFile,
                path: Some(PathBuf::from("/kernel/rules/abc")),
                bytes: b"cached.cn\n".to_vec(),
            }),
            ProviderCachePurge::default(),
        );
        let application = RuleProviderApplication::new(Some(Arc::new(cache)));
        let decl = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": "https://example.com/ads.txt"
        }));
        let plan = application
            .deconstruct(&decl, "PROXY", None)
            .await
            .expect("plan");
        assert_eq!(plan.origin, ProviderContentOrigin::KernelCacheFile);
        assert_eq!(plan.entries[0].rule, "DOMAIN-SUFFIX,cached.cn,PROXY");
    }

    #[tokio::test]
    async fn missing_everywhere_is_a_typed_unsupported_failure() {
        let cache = FakeCache::new(None, ProviderCachePurge::default());
        let application = RuleProviderApplication::new(Some(Arc::new(cache)));
        let decl = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": "https://example.com/ads.txt"
        }));
        let failure = application
            .deconstruct(&decl, "PROXY", None)
            .await
            .expect_err("no source");
        assert_eq!(failure.code, ErrorCode::Unsupported);
        assert!(failure.message.contains("ads"), "{}", failure.message);
        assert!(failure.message.contains("controller payload"));
    }

    #[tokio::test]
    async fn purge_requires_a_host_cache_and_reports_port_counts() {
        let hostless = RuleProviderApplication::default();
        let failure = hostless.purge().await.expect_err("no cache port");
        assert_eq!(failure.code, ErrorCode::Unsupported);

        let cache = FakeCache::new(
            None,
            ProviderCachePurge {
                directory: Some("/cache/rules".to_owned()),
                files_removed: 4,
                bytes_freed: 2048,
            },
        );
        let application = RuleProviderApplication::new(Some(Arc::new(cache)));
        let purge = application.purge().await.expect("purge");
        assert_eq!(purge.files_removed, 4);
        assert_eq!(purge.bytes_freed, 2048);
        let snapshot = application.snapshot().await;
        assert_eq!(snapshot.file_count, 2);
        assert!(snapshot.is_available());
    }

    #[tokio::test]
    async fn fingerprint_observation_compares_against_the_previous_local_read() {
        let cache = FakeCache::new(None, ProviderCachePurge::default()).with_facts(vec![
            Some(file_fact("aa", 10, Some(100))),
            Some(file_fact("aa", 10, Some(200))),
            Some(file_fact("bb", 10, Some(200))),
            None,
        ]);
        let application = RuleProviderApplication::new(Some(Arc::new(cache)));
        let decl = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": "https://example.com/ads.txt"
        }));

        let first = application
            .observe_fingerprint(&decl)
            .await
            .expect("first observation");
        assert_eq!(first.change_token(), "first-seen");
        assert_eq!(first.previous, None);
        assert_eq!(first.current.sha256, "aa");
        assert_eq!(first.path, "/kernel/rules/aa");

        // Same content, later timestamp: still the same local content.
        let second = application
            .observe_fingerprint(&decl)
            .await
            .expect("second observation");
        assert_eq!(second.change_token(), "unchanged");
        assert_eq!(
            second.previous.as_ref().map(|f| f.sha256.as_str()),
            Some("aa")
        );

        // A real rewrite is reported as changed and carries the old value.
        let third = application
            .observe_fingerprint(&decl)
            .await
            .expect("third observation");
        assert_eq!(third.change_token(), "changed");
        assert_eq!(third.current.sha256, "bb");
        assert_eq!(
            third.previous.as_ref().map(|f| f.sha256.as_str()),
            Some("aa")
        );

        // A missing file is an honest absence: no observation is fabricated,
        // and the previous local read is retained for the next comparison.
        assert!(application.observe_fingerprint(&decl).await.is_none());
        assert!(application.observe_fingerprint(&decl).await.is_none());
    }

    #[tokio::test]
    async fn fingerprint_is_absent_without_a_cache_port() {
        let application = RuleProviderApplication::default();
        let decl = declaration(json!({
            "type": "http",
            "behavior": "domain",
            "format": "text",
            "url": "https://example.com/ads.txt"
        }));
        assert!(application.observe_fingerprint(&decl).await.is_none());
    }
}

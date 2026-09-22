//! Rule-provider unpack and cache maintenance use-cases (DUAL-11-06/07).
//!
//! This is the one place that turns a profile's `rule-providers` declaration
//! into real custom rules and the one place that purges the host's kernel
//! cache. Both surfaces call into this module, so neither can fabricate a
//! provider payload or claim a cleanup that never happened.

use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::provider_cache::{ProviderCachePurge, ProviderContentOrigin};
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::provider_store::{
    RuleProviderDeclaration, deconstruct_provider_payload,
};
use infiltrator_ports::error::PortError;
use infiltrator_ports::rule_provider_cache::RuleProviderCachePort;
use infiltrator_ports::runtime_gateway::RuntimeGateway;
use std::sync::Arc;

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
}

impl RuleProviderApplication {
    pub fn new(cache: Option<Arc<dyn RuleProviderCachePort>>) -> Self {
        Self { cache }
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
    use serde_json::json;
    use std::path::PathBuf;

    struct FakeCache {
        entry: Option<ProviderCacheEntry>,
        purge: ProviderCachePurge,
    }

    #[async_trait]
    impl RuleProviderCachePort for FakeCache {
        async fn read_provider(
            &self,
            _declaration: &RuleProviderDeclaration,
        ) -> Result<Option<ProviderCacheEntry>, PortError> {
            Ok(self.entry.clone())
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
        let cache = FakeCache {
            entry: Some(ProviderCacheEntry {
                origin: ProviderContentOrigin::KernelCacheFile,
                path: Some(PathBuf::from("/kernel/rules/abc")),
                bytes: b"cached.cn\n".to_vec(),
            }),
            purge: ProviderCachePurge::default(),
        };
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
        let cache = FakeCache {
            entry: None,
            purge: ProviderCachePurge::default(),
        };
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

        let cache = FakeCache {
            entry: None,
            purge: ProviderCachePurge {
                directory: Some("/cache/rules".to_owned()),
                files_removed: 4,
                bytes_freed: 2048,
            },
        };
        let application = RuleProviderApplication::new(Some(Arc::new(cache)));
        let purge = application.purge().await.expect("purge");
        assert_eq!(purge.files_removed, 4);
        assert_eq!(purge.bytes_freed, 2048);
        let snapshot = application.snapshot().await;
        assert_eq!(snapshot.file_count, 2);
        assert!(snapshot.is_available());
    }
}

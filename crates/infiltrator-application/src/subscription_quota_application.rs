//! Application seam for the active subscription quota dashboard.

use infiltrator_contract::error::Failure;
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::subscription_quota::SubscriptionQuotaSnapshot;
use infiltrator_domain::profiles::ProfileInfo;

#[derive(Clone, Copy, Debug, Default)]
pub struct SubscriptionQuotaApplication;

impl SubscriptionQuotaApplication {
    pub fn project(
        &self,
        core: &CoreSnapshot,
        profiles: Option<&Result<Vec<ProfileInfo>, Failure>>,
    ) -> SubscriptionQuotaSnapshot {
        let revision = core.revision.max(1);
        let Some(profiles) = profiles else {
            return SubscriptionQuotaSnapshot::unsupported(
                core.generation,
                revision,
                "profile application is not composed",
            );
        };
        let profiles = match profiles {
            Ok(profiles) => profiles,
            Err(error) => {
                return SubscriptionQuotaSnapshot::failed(
                    core.generation,
                    revision,
                    format!("profile quota input failed: {}", error.message),
                );
            }
        };
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|value| i64::try_from(value.as_secs()).ok())
            .unwrap_or_default();
        infiltrator_domain::subscription_quota::derive(
            core.generation,
            revision,
            profiles,
            now_unix,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::snapshot::CoreLifecycle;
    use infiltrator_contract::subscription_quota::SubscriptionQuotaStatus;

    #[test]
    fn missing_profile_application_is_typed_unsupported() {
        let snapshot = SubscriptionQuotaApplication.project(
            &CoreSnapshot {
                lifecycle: CoreLifecycle::Stopped,
                generation: 1,
                session_token: None,
                revision: 1,
                proxy_mode: None,
                core_version: None,
                sampled_at_epoch_ms: None,
                failure: None,
                upload_bps: 0.0,
                download_bps: 0.0,
                active_connections: 0,
                memory_bytes: None,
                watchdog: Default::default(),
            },
            None,
        );
        assert_eq!(snapshot.status, SubscriptionQuotaStatus::Unsupported);
    }
}

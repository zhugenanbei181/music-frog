//! One application-owned fold for shell facts, including retained but stale observations.
use infiltrator_contract::error::Failure;
use infiltrator_contract::mini_hud::MiniHudWaveformStrip;
use infiltrator_contract::shell_readout::{ShellObservation, ShellProfile, ShellReadoutSnapshot};
use infiltrator_contract::snapshot::{CoreLifecycle, CoreSnapshot};
use infiltrator_contract::subscription_quota::SubscriptionQuotaStatus;
use infiltrator_contract::surface_snapshot::{PageData, PageStatus, SurfaceSnapshot};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct ShellReadoutApplication {
    state: Arc<Mutex<ShellReadoutSnapshot>>,
}
impl ShellReadoutApplication {
    /// Overview-only adapters use the same rate fold without inventing page facts.
    pub fn project_core_rates(
        &self,
        core: &CoreSnapshot,
        failure: Option<&Failure>,
    ) -> ShellReadoutSnapshot {
        let mut state = self.state.lock().expect("shell readout owner");
        fold_rates(&mut state, core, failure);
        state.clone()
    }
    pub fn project(&self, snapshot: &SurfaceSnapshot) -> ShellReadoutSnapshot {
        let mut state = self.state.lock().expect("shell readout owner");
        if (snapshot.generation, snapshot.revision) < (state.generation, state.revision) {
            return state.clone();
        }
        observe(&mut state.proxies, &snapshot.pages.proxies, |page| {
            page.groups.len()
        });
        observe(&mut state.rules, &snapshot.pages.rules, |page| {
            page.total_rules
        });
        observe(
            &mut state.connections,
            &snapshot.pages.connections,
            |page| page.total_connections,
        );
        observe(&mut state.dns, &snapshot.pages.dns, |page| {
            page.servers.len()
        });
        observe_profile(&mut state.profile, snapshot);
        fold_rates(
            &mut state,
            &snapshot.core,
            snapshot.shell_readout.rate_failure.as_ref(),
        );
        state.revision = snapshot.revision;
        state.waveform = MiniHudWaveformStrip::from_snapshot(&snapshot.traffic_waveform);
        state.clone()
    }
}

fn fold_rates(state: &mut ShellReadoutSnapshot, core: &CoreSnapshot, failure: Option<&Failure>) {
    if state.generation != core.generation
        || core
            .session_token
            .is_some_and(|token| state.session_token != Some(token))
    {
        state.upload_bps = ShellObservation::default();
        state.download_bps = ShellObservation::default();
        state.session_token = core.session_token;
    }
    let sampled = core.sampled_at_epoch_ms.is_some()
        && core.failure.is_none()
        && failure.is_none()
        && matches!(
            core.lifecycle,
            CoreLifecycle::Running | CoreLifecycle::Ready
        );
    observe_rate(&mut state.upload_bps, sampled, core.upload_bps);
    observe_rate(&mut state.download_bps, sampled, core.download_bps);
    state.generation = core.generation;
    if core.session_token.is_some() {
        state.session_token = core.session_token;
    }
    state.rate_failure = failure.cloned().or_else(|| core.failure.clone());
}

fn observe_rate(observation: &mut ShellObservation<f64>, sampled: bool, rate: f64) {
    observation.current = sampled && rate.is_finite() && rate >= 0.0;
    if observation.current {
        observation.value = Some(rate);
    }
}

fn observe<T, U>(output: &mut ShellObservation<U>, page: &PageData<T>, fold: impl FnOnce(&T) -> U) {
    output.current =
        matches!(page.status, PageStatus::Ready | PageStatus::Empty) && page.data.is_some();
    if output.current {
        output.value = page.data.as_ref().map(fold);
    }
}

fn observe_profile(output: &mut ShellObservation<ShellProfile>, snapshot: &SurfaceSnapshot) {
    let page = &snapshot.pages.profiles;
    output.current =
        matches!(page.status, PageStatus::Ready | PageStatus::Empty) && page.data.is_some();
    if !output.current {
        return;
    }
    let Some(active) = page
        .data
        .as_ref()
        .and_then(|page| page.profiles.iter().find(|profile| profile.is_active))
    else {
        output.value = None;
        return;
    };
    let quota = &snapshot.subscription_quota;
    let quota_current = quota.profile_name.as_deref() == Some(&active.name)
        && !matches!(
            quota.status,
            SubscriptionQuotaStatus::Failed
                | SubscriptionQuotaStatus::Unsupported
                | SubscriptionQuotaStatus::Unknown
        );
    let old = output
        .value
        .as_ref()
        .filter(|profile| profile.id == active.id);
    let retained = !quota_current && old.is_some();
    let fraction = quota
        .usage_percent
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| (value / 100.0).clamp(0.0, 1.0) as f32);
    output.value = Some(ShellProfile {
        id: active.id.clone(),
        name: active.name.clone(),
        subscription: !active.url.is_empty(),
        used_bytes: if quota_current {
            quota.used_bytes
        } else {
            old.and_then(|profile| profile.used_bytes)
        },
        total_bytes: if quota_current {
            quota.total_bytes
        } else {
            old.and_then(|profile| profile.total_bytes)
        },
        usage_fraction: if quota_current {
            fraction
        } else {
            old.and_then(|profile| profile.usage_fraction)
        },
    });
    output.current = !retained;
}

#[cfg(test)]
#[path = "shell_readout_application_test.rs"]
mod tests;

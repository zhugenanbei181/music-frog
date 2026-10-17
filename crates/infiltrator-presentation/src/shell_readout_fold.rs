//! One shared fold for shell facts, including retained but stale observations.
use infiltrator_contract::mini_hud::MiniHudWaveformStrip;
use infiltrator_contract::shell_readout::{ShellObservation, ShellProfile, ShellReadoutSnapshot};
use infiltrator_contract::snapshot::CoreSnapshot;
use infiltrator_contract::subscription_quota::SubscriptionQuotaStatus;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::Failure;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::outbound::inventory::{
    OutboundInventorySnapshot, OutboundInventoryState,
};
use lurker_core_contract::page::{PageData, PageStatus};
use std::mem;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct ShellReadoutProjection {
    state: Arc<Mutex<ShellReadoutSnapshot>>,
}
impl ShellReadoutProjection {
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
        observe_outbound_count(&mut state.proxies, &snapshot.outbound_inventory);
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

fn observe_outbound_count(
    output: &mut ShellObservation<usize>,
    page: &PageData<OutboundInventorySnapshot>,
) {
    *output = match (&page.status, page.data.as_ref()) {
        (PageStatus::Ready | PageStatus::Empty, Some(snapshot)) => match snapshot.inventory() {
            OutboundInventoryState::Ready { collections, .. } => {
                ShellObservation::Current(collections.len())
            }
            OutboundInventoryState::Empty => ShellObservation::Current(0),
            OutboundInventoryState::Unknown
            | OutboundInventoryState::Unsupported
            | OutboundInventoryState::Failed { .. } => mem::take(output).into_stale(),
        },
        _ => mem::take(output).into_stale(),
    };
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
    observe_rate(
        &mut state.upload_bps,
        rate_observation(core, failure, core.upload_bps),
    );
    observe_rate(
        &mut state.download_bps,
        rate_observation(core, failure, core.download_bps),
    );
    state.generation = core.generation;
    if core.session_token.is_some() {
        state.session_token = core.session_token;
    }
    state.rate_failure = failure.cloned().or_else(|| core.failure.clone());
}

#[derive(Clone, Copy)]
enum RateObservation {
    Current(f64),
    Unavailable,
}

fn rate_observation(core: &CoreSnapshot, failure: Option<&Failure>, rate: f64) -> RateObservation {
    match (
        &core.lifecycle,
        core.sampled_at_epoch_ms,
        &core.failure,
        failure,
        rate.is_finite(),
        rate >= 0.0,
    ) {
        (CoreLifecycle::Running | CoreLifecycle::Ready, Some(_), None, None, true, true) => {
            RateObservation::Current(rate)
        }
        _ => RateObservation::Unavailable,
    }
}

fn observe_rate(observation: &mut ShellObservation<f64>, sample: RateObservation) {
    *observation = match sample {
        RateObservation::Current(rate) => ShellObservation::Current(rate),
        RateObservation::Unavailable => mem::take(observation).into_stale(),
    };
}

fn observe<T, U>(output: &mut ShellObservation<U>, page: &PageData<T>, fold: impl FnOnce(&T) -> U) {
    *output = match (&page.status, page.data.as_ref()) {
        (PageStatus::Ready | PageStatus::Empty, Some(data)) => {
            ShellObservation::Current(fold(data))
        }
        _ => mem::take(output).into_stale(),
    };
}

fn observe_profile(output: &mut ShellObservation<ShellProfile>, snapshot: &SurfaceSnapshot) {
    let page = &snapshot.pages.profiles;
    let Some(page_data) = (match (&page.status, page.data.as_ref()) {
        (PageStatus::Ready | PageStatus::Empty, Some(page_data)) => Some(page_data),
        _ => None,
    }) else {
        *output = mem::take(output).into_stale();
        return;
    };
    let Some(active) = page_data.profiles.iter().find(|profile| profile.is_active) else {
        *output = ShellObservation::CurrentEmpty;
        return;
    };
    let quota = &snapshot.subscription_quota;
    let quota_facts = match (
        quota.profile_name.as_deref() == Some(&active.name),
        quota.status,
    ) {
        (
            true,
            SubscriptionQuotaStatus::Failed
            | SubscriptionQuotaStatus::Unsupported
            | SubscriptionQuotaStatus::Unknown,
        )
        | (false, _) => ProfileQuotaFacts::Unavailable,
        (true, _) => ProfileQuotaFacts::Current {
            used_bytes: quota.used_bytes,
            total_bytes: quota.total_bytes,
            usage_fraction: quota
                .usage_percent
                .filter(|value| value.is_finite() && *value >= 0.0)
                .map(|value| (value / 100.0).clamp(0.0, 1.0) as f32),
        },
    };
    let old = output
        .value()
        .filter(|profile| profile.id == active.id)
        .cloned();
    *output = match (quota_facts, old) {
        (
            ProfileQuotaFacts::Current {
                used_bytes,
                total_bytes,
                usage_fraction,
            },
            _,
        ) => ShellObservation::Current(ShellProfile {
            id: active.id.clone(),
            name: active.name.clone(),
            subscription: !active.url.is_empty(),
            used_bytes,
            total_bytes,
            usage_fraction,
        }),
        (ProfileQuotaFacts::Unavailable, Some(old)) => ShellObservation::Stale(ShellProfile {
            id: active.id.clone(),
            name: active.name.clone(),
            subscription: !active.url.is_empty(),
            used_bytes: old.used_bytes,
            total_bytes: old.total_bytes,
            usage_fraction: old.usage_fraction,
        }),
        (ProfileQuotaFacts::Unavailable, None) => ShellObservation::Current(ShellProfile {
            id: active.id.clone(),
            name: active.name.clone(),
            subscription: !active.url.is_empty(),
            used_bytes: None,
            total_bytes: None,
            usage_fraction: None,
        }),
    };
}

enum ProfileQuotaFacts {
    Current {
        used_bytes: Option<u64>,
        total_bytes: Option<u64>,
        usage_fraction: Option<f32>,
    },
    Unavailable,
}

#[cfg(test)]
#[path = "shell_readout_fold_tests.rs"]
mod tests;

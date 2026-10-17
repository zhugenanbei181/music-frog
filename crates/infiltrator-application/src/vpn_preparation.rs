//! One-shot VPN configuration handoff between Android Builder and Rust worker.

use infiltrator_contract::vpn::{
    PreparedVpnConfiguration, VpnConfigurationActivationLease, VpnGeneration, VpnPreparationId,
    VpnStartRequest,
};
use lurker_core_contract::identity::CoreScope;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum VpnPreparationState {
    #[default]
    Empty,
    Preparing(PreparedVpnConfiguration),
    Prepared(PreparedVpnConfiguration),
    Activating {
        prepared: PreparedVpnConfiguration,
        lease: VpnConfigurationActivationLease,
    },
    ConsumedActivating(VpnConfigurationActivationLease),
    Consumed {
        preparation_id: VpnPreparationId,
        vpn_generation: VpnGeneration,
    },
}

/// Serialized owner of the single prepared VPN configuration transaction.
#[derive(Clone, Debug, Default)]
pub struct VpnPreparationStore {
    state: Arc<Mutex<VpnPreparationState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnPreparationInstallOutcome {
    Installed,
    ReplacedOlder(VpnPreparationId),
    RejectedCurrent(VpnPreparationId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnPreparationReadyOutcome {
    Ready,
    Missing,
    Stale,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VpnConfigurationActivationBeginOutcome {
    Acquired(VpnConfigurationActivationLease),
    Busy,
    Missing,
    NotReady,
    TransactionMismatch,
    StaleGeneration,
    PreparedRevisionMismatch {
        prepared_revision: u64,
        requested_revision: u64,
    },
    StaleOwnerData {
        expected_revision: u64,
        observed_revision: u64,
    },
    InvalidLeaseId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VpnConfigurationActivationConsumeOutcome {
    Consumed(VpnStartRequest),
    LeaseMissing,
    LeaseMismatch,
    StaleOwnerData {
        expected_revision: u64,
        observed_revision: u64,
    },
    CoreScopeChanged,
    InvalidTunnelDescriptor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnConfigurationActivationReleaseOutcome {
    ReleasedBeforeConsume,
    ReleasedAfterConsume,
    LeaseMissing,
    LeaseMismatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnConfigurationActivationStatus {
    Available,
    BuilderAdmitted,
    DataPlaneStarting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VpnPreparationClearOutcome {
    Cleared,
    NotPresent,
    PreservedNewer,
    DifferentTransaction,
}

impl VpnPreparationStore {
    pub fn install(&self, prepared: PreparedVpnConfiguration) -> VpnPreparationInstallOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        match &*state {
            VpnPreparationState::Activating { prepared, .. } => {
                return VpnPreparationInstallOutcome::RejectedCurrent(prepared.preparation_id);
            }
            VpnPreparationState::ConsumedActivating(lease) => {
                return VpnPreparationInstallOutcome::RejectedCurrent(lease.preparation_id);
            }
            VpnPreparationState::Empty
            | VpnPreparationState::Preparing(_)
            | VpnPreparationState::Prepared(_)
            | VpnPreparationState::Consumed { .. } => {}
        }
        let prior = match &*state {
            VpnPreparationState::Empty => {
                *state = VpnPreparationState::Preparing(prepared);
                return VpnPreparationInstallOutcome::Installed;
            }
            VpnPreparationState::Preparing(current) | VpnPreparationState::Prepared(current) => {
                (current.preparation_id, current.vpn_generation)
            }
            VpnPreparationState::Activating { prepared, .. } => {
                (prepared.preparation_id, prepared.vpn_generation)
            }
            VpnPreparationState::ConsumedActivating(lease) => {
                (lease.preparation_id, lease.vpn_generation)
            }
            VpnPreparationState::Consumed {
                preparation_id,
                vpn_generation,
            } => (*preparation_id, *vpn_generation),
        };
        if prior.1 < prepared.vpn_generation {
            *state = VpnPreparationState::Preparing(prepared);
            VpnPreparationInstallOutcome::ReplacedOlder(prior.0)
        } else {
            VpnPreparationInstallOutcome::RejectedCurrent(prior.0)
        }
    }

    pub fn mark_prepared(&self, preparation_id: VpnPreparationId) -> VpnPreparationReadyOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        match &*state {
            VpnPreparationState::Preparing(prepared)
                if prepared.preparation_id == preparation_id =>
            {
                let prepared = prepared.clone();
                *state = VpnPreparationState::Prepared(prepared);
                VpnPreparationReadyOutcome::Ready
            }
            VpnPreparationState::Empty
            | VpnPreparationState::ConsumedActivating(_)
            | VpnPreparationState::Consumed { .. } => VpnPreparationReadyOutcome::Missing,
            VpnPreparationState::Preparing(_)
            | VpnPreparationState::Prepared(_)
            | VpnPreparationState::Activating { .. } => VpnPreparationReadyOutcome::Stale,
        }
    }

    pub fn begin_activation(
        &self,
        preparation_id: VpnPreparationId,
        vpn_generation: VpnGeneration,
        requested_owner_data_revision: u64,
        observed_owner_data_revision: u64,
        lease_id: u64,
    ) -> VpnConfigurationActivationBeginOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        match &*state {
            VpnPreparationState::Empty => VpnConfigurationActivationBeginOutcome::Missing,
            VpnPreparationState::Preparing(prepared) => {
                if prepared.preparation_id == preparation_id
                    && prepared.vpn_generation == vpn_generation
                {
                    VpnConfigurationActivationBeginOutcome::NotReady
                } else if vpn_generation != prepared.vpn_generation {
                    VpnConfigurationActivationBeginOutcome::StaleGeneration
                } else {
                    VpnConfigurationActivationBeginOutcome::TransactionMismatch
                }
            }
            VpnPreparationState::Prepared(prepared) => {
                if prepared.preparation_id != preparation_id {
                    return VpnConfigurationActivationBeginOutcome::TransactionMismatch;
                }
                if prepared.vpn_generation != vpn_generation {
                    return VpnConfigurationActivationBeginOutcome::StaleGeneration;
                }
                if lease_id == 0 {
                    return VpnConfigurationActivationBeginOutcome::InvalidLeaseId;
                }
                if prepared.owner_data_revision != requested_owner_data_revision {
                    return VpnConfigurationActivationBeginOutcome::PreparedRevisionMismatch {
                        prepared_revision: prepared.owner_data_revision,
                        requested_revision: requested_owner_data_revision,
                    };
                }
                if prepared.owner_data_revision != observed_owner_data_revision {
                    return VpnConfigurationActivationBeginOutcome::StaleOwnerData {
                        expected_revision: prepared.owner_data_revision,
                        observed_revision: observed_owner_data_revision,
                    };
                }
                let lease = VpnConfigurationActivationLease {
                    lease_id,
                    preparation_id,
                    vpn_generation,
                    owner_data_revision: prepared.owner_data_revision,
                };
                let frozen = prepared.clone();
                *state = VpnPreparationState::Activating {
                    prepared: frozen,
                    lease,
                };
                VpnConfigurationActivationBeginOutcome::Acquired(lease)
            }
            VpnPreparationState::Activating { .. } | VpnPreparationState::ConsumedActivating(_) => {
                VpnConfigurationActivationBeginOutcome::Busy
            }
            VpnPreparationState::Consumed {
                preparation_id: consumed_id,
                vpn_generation: consumed_generation,
            } if *consumed_id == preparation_id && *consumed_generation == vpn_generation => {
                VpnConfigurationActivationBeginOutcome::Busy
            }
            VpnPreparationState::Consumed { .. } => VpnConfigurationActivationBeginOutcome::Missing,
        }
    }

    pub fn consume_activation(
        &self,
        lease_id: u64,
        preparation_id: VpnPreparationId,
        vpn_generation: VpnGeneration,
        current_core_scope: &CoreScope,
        observed_owner_data_revision: u64,
        tun_fd: i32,
    ) -> VpnConfigurationActivationConsumeOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        let (prepared, lease) = match &*state {
            VpnPreparationState::Activating { prepared, lease }
                if lease.lease_id == lease_id
                    && lease.preparation_id == preparation_id
                    && lease.vpn_generation == vpn_generation =>
            {
                (prepared.clone(), *lease)
            }
            VpnPreparationState::Activating { .. } | VpnPreparationState::ConsumedActivating(_) => {
                return VpnConfigurationActivationConsumeOutcome::LeaseMismatch;
            }
            VpnPreparationState::Empty
            | VpnPreparationState::Preparing(_)
            | VpnPreparationState::Prepared(_)
            | VpnPreparationState::Consumed { .. } => {
                return VpnConfigurationActivationConsumeOutcome::LeaseMissing;
            }
        };
        if prepared.owner_data_revision != observed_owner_data_revision
            || lease.owner_data_revision != observed_owner_data_revision
        {
            return VpnConfigurationActivationConsumeOutcome::StaleOwnerData {
                expected_revision: lease.owner_data_revision,
                observed_revision: observed_owner_data_revision,
            };
        }
        if prepared.core_scope != *current_core_scope {
            return VpnConfigurationActivationConsumeOutcome::CoreScopeChanged;
        }
        if tun_fd <= 0 {
            return VpnConfigurationActivationConsumeOutcome::InvalidTunnelDescriptor;
        }
        *state = VpnPreparationState::ConsumedActivating(lease);
        VpnConfigurationActivationConsumeOutcome::Consumed(VpnStartRequest { tun_fd, prepared })
    }

    pub fn finish_activation(&self, lease_id: u64) -> VpnConfigurationActivationReleaseOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        match &*state {
            VpnPreparationState::Activating { prepared, lease } if lease.lease_id == lease_id => {
                let prepared = prepared.clone();
                *state = VpnPreparationState::Prepared(prepared);
                VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume
            }
            VpnPreparationState::ConsumedActivating(lease) if lease.lease_id == lease_id => {
                let preparation_id = lease.preparation_id;
                let vpn_generation = lease.vpn_generation;
                *state = VpnPreparationState::Consumed {
                    preparation_id,
                    vpn_generation,
                };
                VpnConfigurationActivationReleaseOutcome::ReleasedAfterConsume
            }
            VpnPreparationState::Activating { .. }
            | VpnPreparationState::ConsumedActivating(_)
            | VpnPreparationState::Empty
            | VpnPreparationState::Preparing(_)
            | VpnPreparationState::Prepared(_)
            | VpnPreparationState::Consumed { .. } => {
                VpnConfigurationActivationReleaseOutcome::LeaseMismatch
            }
        }
    }

    pub fn activation_status(&self) -> VpnConfigurationActivationStatus {
        let state = self.state.lock().expect("VPN preparation state lock");
        match &*state {
            VpnPreparationState::Activating { .. } => {
                VpnConfigurationActivationStatus::BuilderAdmitted
            }
            VpnPreparationState::ConsumedActivating(_) => {
                VpnConfigurationActivationStatus::DataPlaneStarting
            }
            VpnPreparationState::Empty
            | VpnPreparationState::Preparing(_)
            | VpnPreparationState::Prepared(_)
            | VpnPreparationState::Consumed { .. } => VpnConfigurationActivationStatus::Available,
        }
    }

    pub fn clear_through(&self, vpn_generation: VpnGeneration) -> VpnPreparationClearOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        let current_generation = match &*state {
            VpnPreparationState::Empty => return VpnPreparationClearOutcome::NotPresent,
            VpnPreparationState::Preparing(prepared) | VpnPreparationState::Prepared(prepared) => {
                prepared.vpn_generation
            }
            VpnPreparationState::Activating { prepared, .. } => prepared.vpn_generation,
            VpnPreparationState::ConsumedActivating(lease) => lease.vpn_generation,
            VpnPreparationState::Consumed { vpn_generation, .. } => *vpn_generation,
        };
        if current_generation <= vpn_generation {
            *state = VpnPreparationState::Empty;
            VpnPreparationClearOutcome::Cleared
        } else {
            VpnPreparationClearOutcome::PreservedNewer
        }
    }

    pub fn clear_transaction(
        &self,
        preparation_id: VpnPreparationId,
    ) -> VpnPreparationClearOutcome {
        let mut state = self.state.lock().expect("VPN preparation state lock");
        let current_id = match &*state {
            VpnPreparationState::Empty => return VpnPreparationClearOutcome::NotPresent,
            VpnPreparationState::Preparing(prepared) | VpnPreparationState::Prepared(prepared) => {
                prepared.preparation_id
            }
            VpnPreparationState::Activating { prepared, .. } => prepared.preparation_id,
            VpnPreparationState::ConsumedActivating(lease) => lease.preparation_id,
            VpnPreparationState::Consumed { preparation_id, .. } => *preparation_id,
        };
        if current_id == preparation_id {
            *state = VpnPreparationState::Empty;
            VpnPreparationClearOutcome::Cleared
        } else {
            VpnPreparationClearOutcome::DifferentTransaction
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::vpn::{VpnConfiguration, VpnRoute};
    use lurker_core_contract::identity::{
        CoreGeneration, CoreIdentity, CoreInstanceId, CoreScope, CoreVersion, EngineId,
    };
    use lurker_core_contract::session::SessionToken;

    fn core_scope(session: u128) -> CoreScope {
        CoreScope {
            identity: CoreIdentity::new(
                EngineId::new("mihomo").expect("engine ID"),
                CoreInstanceId::new("primary").expect("instance ID"),
                Some(CoreVersion::new("1.19.12").expect("core version")),
            ),
            generation: CoreGeneration::new(9),
            session_token: Some(SessionToken::new(session)),
        }
    }

    fn prepared(id: u64, generation: u64, scope: CoreScope) -> PreparedVpnConfiguration {
        PreparedVpnConfiguration {
            preparation_id: VpnPreparationId::new(id),
            vpn_generation: VpnGeneration::new(generation),
            owner_data_revision: 44,
            core_scope: scope,
            configuration: VpnConfiguration {
                proxy_endpoint: "socks5://127.0.0.1:7891".to_owned(),
                mtu: 1420,
                routes: vec![VpnRoute {
                    address: "0.0.0.0".to_owned(),
                    prefix: 0,
                    exclude: false,
                }],
                dns_servers: vec!["1.1.1.1".to_owned()],
                ipv6: false,
                foreground_requested: true,
            },
        }
    }

    fn install_ready(store: &VpnPreparationStore, prepared: PreparedVpnConfiguration) {
        let id = prepared.preparation_id;
        assert_eq!(
            store.install(prepared),
            VpnPreparationInstallOutcome::Installed,
        );
        assert_eq!(store.mark_prepared(id), VpnPreparationReadyOutcome::Ready);
    }

    fn begin(
        store: &VpnPreparationStore,
        id: u64,
        generation: u64,
        lease_id: u64,
    ) -> VpnConfigurationActivationLease {
        match store.begin_activation(
            VpnPreparationId::new(id),
            VpnGeneration::new(generation),
            44,
            44,
            lease_id,
        ) {
            VpnConfigurationActivationBeginOutcome::Acquired(lease) => lease,
            outcome => panic!("expected activation admission, got {outcome:?}"),
        }
    }

    #[test]
    fn begin_admits_once_and_duplicate_acquire_is_busy_until_terminal_release() {
        let store = VpnPreparationStore::default();
        let scope = core_scope(101);
        install_ready(&store, prepared(3, 7, scope.clone()));
        assert_eq!(
            store.begin_activation(VpnPreparationId::new(3), VpnGeneration::new(7), 44, 44, 31,),
            VpnConfigurationActivationBeginOutcome::Acquired(VpnConfigurationActivationLease {
                lease_id: 31,
                preparation_id: VpnPreparationId::new(3),
                vpn_generation: VpnGeneration::new(7),
                owner_data_revision: 44,
            }),
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::BuilderAdmitted
        );
        assert_eq!(
            store.begin_activation(VpnPreparationId::new(3), VpnGeneration::new(7), 44, 44, 32,),
            VpnConfigurationActivationBeginOutcome::Busy,
        );
        assert_eq!(
            store.finish_activation(31),
            VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::Available
        );
    }

    #[test]
    fn stale_owner_revision_is_rejected_before_activation_lease_is_acquired() {
        let store = VpnPreparationStore::default();
        install_ready(&store, prepared(4, 8, core_scope(101)));

        match store.begin_activation(VpnPreparationId::new(4), VpnGeneration::new(8), 44, 45, 41) {
            VpnConfigurationActivationBeginOutcome::StaleOwnerData {
                expected_revision: 44,
                observed_revision: 45,
            } => {}
            outcome => panic!("expected stale owner data rejection, got {outcome:?}"),
        }

        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::Available
        );
    }

    #[test]
    fn caller_revision_must_match_the_revision_frozen_in_prepared_configuration() {
        let store = VpnPreparationStore::default();
        install_ready(&store, prepared(5, 9, core_scope(101)));

        assert_eq!(
            store.begin_activation(VpnPreparationId::new(5), VpnGeneration::new(9), 43, 44, 51,),
            VpnConfigurationActivationBeginOutcome::PreparedRevisionMismatch {
                prepared_revision: 44,
                requested_revision: 43,
            },
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::Available
        );
    }

    #[test]
    fn admitted_revision_is_rechecked_atomically_before_preparation_consume() {
        let store = VpnPreparationStore::default();
        let scope = core_scope(101);
        install_ready(&store, prepared(6, 12, scope));
        let lease = begin(&store, 6, 12, 52);
        assert_eq!(
            store.consume_activation(
                lease.lease_id,
                lease.preparation_id,
                lease.vpn_generation,
                &core_scope(101),
                45,
                34,
            ),
            VpnConfigurationActivationConsumeOutcome::StaleOwnerData {
                expected_revision: 44,
                observed_revision: 45,
            },
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::BuilderAdmitted
        );
        assert_eq!(
            store.finish_activation(lease.lease_id),
            VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume,
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::Available
        );
    }

    #[test]
    fn generation_and_core_session_fences_preserve_the_newer_preparation() {
        let store = VpnPreparationStore::default();
        let scope = core_scope(101);
        install_ready(&store, prepared(6, 12, scope.clone()));
        assert_eq!(
            store.begin_activation(VpnPreparationId::new(6), VpnGeneration::new(11), 44, 44, 61),
            VpnConfigurationActivationBeginOutcome::StaleGeneration,
        );
        assert_eq!(
            store.clear_through(VpnGeneration::new(11)),
            VpnPreparationClearOutcome::PreservedNewer,
        );
        let lease = begin(&store, 6, 12, 62);
        assert_eq!(
            store.consume_activation(
                lease.lease_id,
                lease.preparation_id,
                lease.vpn_generation,
                &core_scope(102),
                44,
                34
            ),
            VpnConfigurationActivationConsumeOutcome::CoreScopeChanged,
        );
        assert_eq!(
            store.finish_activation(lease.lease_id),
            VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume
        );
    }

    #[test]
    fn builder_failure_or_cancellation_releases_and_successful_consume_releases() {
        let store = VpnPreparationStore::default();
        let scope = core_scope(101);
        install_ready(&store, prepared(7, 13, scope.clone()));
        let failed_builder_lease = begin(&store, 7, 13, 70);
        assert_eq!(
            store.finish_activation(failed_builder_lease.lease_id),
            VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume,
        );
        let cancelled_lease = begin(&store, 7, 13, 71);
        assert_eq!(
            store.finish_activation(cancelled_lease.lease_id),
            VpnConfigurationActivationReleaseOutcome::ReleasedBeforeConsume,
        );

        let successful_lease = begin(&store, 7, 13, 72);
        let VpnConfigurationActivationConsumeOutcome::Consumed(request) = store.consume_activation(
            successful_lease.lease_id,
            successful_lease.preparation_id,
            successful_lease.vpn_generation,
            &scope,
            44,
            35,
        ) else {
            panic!("matching activation lease should consume the frozen configuration");
        };
        assert_eq!(request.tun_fd, 35);
        assert_eq!(request.prepared.owner_data_revision, 44);
        assert_eq!(request.prepared.configuration.mtu, 1420);
        assert_eq!(request.prepared.configuration.dns_servers, vec!["1.1.1.1"]);
        assert_eq!(request.prepared.core_scope, scope);
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::DataPlaneStarting
        );
        assert_eq!(
            store.finish_activation(successful_lease.lease_id),
            VpnConfigurationActivationReleaseOutcome::ReleasedAfterConsume,
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::Available
        );
        assert_eq!(
            store.consume_activation(
                successful_lease.lease_id,
                successful_lease.preparation_id,
                successful_lease.vpn_generation,
                &core_scope(101),
                44,
                35
            ),
            VpnConfigurationActivationConsumeOutcome::LeaseMissing,
        );
    }

    #[test]
    fn active_activation_is_a_typed_mutation_fence_and_generation_cleanup_releases_it() {
        let store = VpnPreparationStore::default();
        install_ready(&store, prepared(8, 14, core_scope(101)));
        let lease = begin(&store, 8, 14, 80);
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::BuilderAdmitted,
        );
        assert_eq!(
            store.install(prepared(9, 15, core_scope(101))),
            VpnPreparationInstallOutcome::RejectedCurrent(VpnPreparationId::new(8)),
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::BuilderAdmitted,
        );
        assert_eq!(
            store.clear_through(VpnGeneration::new(14)),
            VpnPreparationClearOutcome::Cleared,
        );
        assert_eq!(
            store.activation_status(),
            VpnConfigurationActivationStatus::Available
        );
        assert_eq!(
            store.finish_activation(lease.lease_id),
            VpnConfigurationActivationReleaseOutcome::LeaseMismatch
        );

        install_ready(&store, prepared(9, 15, core_scope(101)));
        assert_eq!(
            store.install(prepared(10, 16, core_scope(101))),
            VpnPreparationInstallOutcome::ReplacedOlder(VpnPreparationId::new(9))
        );
        assert_eq!(
            store.mark_prepared(VpnPreparationId::new(9)),
            VpnPreparationReadyOutcome::Stale
        );
        assert_eq!(
            store.mark_prepared(VpnPreparationId::new(10)),
            VpnPreparationReadyOutcome::Ready,
        );
        assert_eq!(
            store.clear_transaction(VpnPreparationId::new(9)),
            VpnPreparationClearOutcome::DifferentTransaction,
        );
    }
}

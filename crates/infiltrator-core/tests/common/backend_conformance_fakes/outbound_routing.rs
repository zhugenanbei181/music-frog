use super::*;

pub(super) struct FakeOutbound {
    pub(super) engine: EngineId,
    pub(super) expected_scope: CoreScope,
    pub(super) collection_id: OutboundId,
    pub(super) members: Vec<OutboundId>,
    pub(super) selected: Mutex<OutboundId>,
    pub(super) revision: Mutex<u64>,
    pub(super) probe_id: OutboundId,
    pub(super) details_behavior: DetailsBehavior,
    pub(super) probe_behavior: ProbeBehavior,
}

#[async_trait]
impl OutboundPort for FakeOutbound {
    async fn inventory(&self, scope: CoreScope) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        let selected = self
            .selected
            .lock()
            .expect("fake selected member lock")
            .clone();
        let revision = *self.revision.lock().expect("fake inventory revision lock");
        OutboundInventorySnapshot::new(
            scope,
            SnapshotRevision::new(revision),
            OutboundInventoryState::Ready {
                entries: self
                    .members
                    .iter()
                    .map(
                        |id| OutboundSnapshot {
                            id: id.clone(),
                            display_name: Some(id.value().to_owned()),
                        },
                    )
                    .collect(),
                collections: vec![OutboundSelectionSet {
                    id: self.collection_id.clone(),
                    member_ids: self.members.clone(),
                    availability: SelectionAvailability::Selectable,
                    selection: OutboundSelectionState::Ready { selected },
                }],
            },
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn details(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
    ) -> Result<OutboundDetailsSnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        let state = match self.details_behavior {
            DetailsBehavior::Ready => OutboundDetailsState::Ready {
                details: vec![fake_detail(self.probe_id.clone())],
            },
            DetailsBehavior::Unsupported => OutboundDetailsState::Unsupported,
        };
        OutboundDetailsSnapshot::new(scope, SnapshotRevision::new(1), state)
            .map_err(|error| PortError::Failed(error.to_string()))
    }

    async fn select(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        collection_id: OutboundId,
        outbound_id: OutboundId,
    ) -> Result<OutboundInventorySnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        if collection_id != self.collection_id || !self.members.contains(&outbound_id) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake selector request was not in its current inventory",
                false,
            )));
        }
        *self.selected.lock().expect("fake selected member lock") = outbound_id;
        {
            let mut revision = self.revision.lock().expect("fake inventory revision lock");
            *revision = revision
                .checked_add(1)
                .ok_or_else(|| PortError::Failed("fake revision exhausted".to_owned()))?;
        }
        self.inventory(scope).await
    }

    async fn probe(
        &self,
        scope: CoreScope,
        expected_revision: SnapshotRevision,
        outbound_id: OutboundId,
        _request: OutboundProbeRequest,
    ) -> Result<OutboundProbeSnapshot, PortError> {
        self.verify_scope(&scope)?;
        self.verify_revision(expected_revision)?;
        if outbound_id != self.probe_id {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake probe target was not registered",
                false,
            )));
        }
        let state = match self.probe_behavior {
            ProbeBehavior::Measured => OutboundProbeState::Measured { delay_ms: 1 },
            ProbeBehavior::Unsupported => OutboundProbeState::Unsupported,
        };
        OutboundProbeSnapshot::new(
            scope,
            expected_revision,
            SnapshotRevision::new(1),
            outbound_id,
            state,
        )
        .map_err(|error| PortError::Failed(error.to_string()))
    }
}

impl FakeOutbound {
    fn verify_scope(&self, scope: &CoreScope) -> Result<(), PortError> {
        if scope == &self.expected_scope && scope.identity.engine == self.engine {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake adapter received a foreign or stale scope",
                false,
            )))
        }
    }

    fn verify_revision(&self, expected: SnapshotRevision) -> Result<(), PortError> {
        let actual = *self.revision.lock().expect("fake revision lock");
        if expected == SnapshotRevision::new(actual) {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake adapter rejected a stale inventory revision",
                false,
            )))
        }
    }
}

pub(super) struct FakeRouting {
    pub(super) expected_scope: CoreScope,
    pub(super) behavior: RoutingBehavior,
    pub(super) current: Mutex<RoutingIntent>,
}

#[async_trait]
impl RoutingIntentPort for FakeRouting {
    async fn observe(&self, scope: &CoreScope) -> Result<RoutingIntent, PortError> {
        self.verify_scope(scope)?;
        Ok(self
            .current
            .lock()
            .expect("fake routing state lock")
            .clone())
    }

    async fn apply(
        &self,
        scope: &CoreScope,
        intent: RoutingIntent,
    ) -> Result<RoutingIntent, PortError> {
        self.verify_scope(scope)?;
        match &self.behavior {
            RoutingBehavior::Writable { initial, requested }
                if &intent == initial || &intent == requested =>
            {
                *self.current.lock().expect("fake routing state lock") = intent.clone();
                Ok(intent)
            }
            RoutingBehavior::ReadOnlyUnsupported { .. } => Err(PortError::unsupported(
                Capability::RoutingIntent,
                "fake sing-box-like backend preserves EngineDefined mode only",
            )),
            RoutingBehavior::Writable { .. } => Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake routing adapter received an unregistered intent",
                false,
            ))),
        }
    }
}

impl FakeRouting {
    fn verify_scope(&self, scope: &CoreScope) -> Result<(), PortError> {
        if scope == &self.expected_scope {
            Ok(())
        } else {
            Err(PortError::Rejected(Failure::new(
                ErrorCode::InvalidState,
                "fake routing adapter received a foreign scope",
                false,
            )))
        }
    }
}


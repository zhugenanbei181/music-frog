use super::*;

impl ProfileApplication {
    pub async fn save_profile_content<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: String,
        content: String,
        strategy: ApplyStrategy,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let profile = valid_name(&profile)?;
        let expected = self.active_profile_reference().await?;
        self.save_profile_content_at_reference(runtime, profile, content, strategy, expected)
            .await
    }

    /// Save profile content against the complete observed active-reference
    /// state. Active owner writes apply through a native transaction bound
    /// to that exact reference, while offline/stopped writes use the exact
    /// selected-workspace persistence transaction.
    pub async fn save_profile_content_at_reference<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: String,
        content: String,
        strategy: ApplyStrategy,
        expected: ActiveProfileReferenceState,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let profile = valid_name(&profile)?;
        self.save_profile_content_for_reference(runtime, profile, &content, strategy, &expected)
            .await
    }

    /// DUAL-09-12: direct-edit protection derived from the stored subscription
    /// URL. An empty URL means the profile is local (imported by hand/file).
    pub async fn write_protection(&self, name: &str) -> Result<ProfileWriteProtection, Failure> {
        let info = self.load_profile_info(name).await?;
        Ok(ProfileWriteProtection::from_subscription_url(
            info.subscription_url.as_deref().unwrap_or_default(),
        ))
    }

    pub async fn save_current_profile_content<F, E, R>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        strategy: ApplyStrategy,
        transform: F,
    ) -> Result<ProfileContentSaveOutcome, Failure>
    where
        F: FnOnce(&str) -> Result<String, E> + Send + 'static,
        E: Display + Send + 'static,
        R: ActiveProfileReferenceApplier + ?Sized,
    {
        let expected = self.active_profile_reference().await?;
        let ActiveProfileReferenceState::Selected(reference) = &expected else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "cannot edit current profile while active profile is unselected",
                false,
            ));
        };
        if reference.engine_id != self.store.engine_id() {
            return Err(Failure::unsupported(
                "active profile belongs to another engine namespace",
            ));
        }
        let workspace = self
            .store
            .load_workspace(&reference.profile_id)
            .await
            .map_err(Failure::from)?;
        if workspace.document.identity.engine_id != reference.engine_id
            || workspace.document.identity.profile_id != reference.profile_id
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "selected reference does not match the current profile workspace",
                true,
            ));
        }
        let content = workspace
            .document
            .utf8_text()
            .map_err(|error| Failure::new(ErrorCode::Storage, error.to_string(), false))?;
        let updated = transform(&content)
            .map_err(|error| Failure::new(ErrorCode::Configuration, error.to_string(), false))?;
        self.save_selected_profile_content(runtime, &expected, &workspace, &updated, strategy)
            .await
    }

    async fn save_profile_content_for_reference<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: String,
        content: &str,
        strategy: ApplyStrategy,
        expected: &ActiveProfileReferenceState,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let observed = self.active_profile_reference().await?;
        if &observed != expected {
            return Err(stale_reference_failure(expected, &observed));
        }
        match expected {
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.store.engine_id()
                    && reference.profile_id.as_str() == profile =>
            {
                let workspace = self
                    .store
                    .load_workspace(&reference.profile_id)
                    .await
                    .map_err(Failure::from)?;
                if workspace.identity().document.engine_id != reference.engine_id
                    || workspace.identity().document.profile_id != reference.profile_id
                {
                    return Err(Failure::new(
                        ErrorCode::NotReady,
                        "selected reference does not match the current profile workspace",
                        true,
                    ));
                }
                self.save_selected_profile_content(runtime, expected, &workspace, content, strategy)
                    .await
            }
            _ => self
                .persist_profile_content_if_reference_current(&profile, content, expected)
                .await
                .map(|()| ProfileContentSaveOutcome::StoredInactive),
        }
    }

    async fn save_selected_profile_content<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        expected_reference: &ActiveProfileReferenceState,
        previous: &ProfileWorkspace,
        content: &str,
        strategy: ApplyStrategy,
    ) -> Result<ProfileContentSaveOutcome, Failure> {
        let ActiveProfileReferenceState::Selected(reference) = expected_reference else {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "cannot save selected-profile content while the active reference is unselected",
                false,
            ));
        };
        let expected_workspace = previous.identity();
        if expected_workspace.document.engine_id != reference.engine_id
            || expected_workspace.document.profile_id != reference.profile_id
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "active reference does not identify the submitted profile workspace",
                false,
            ));
        }
        let update = ProfileWorkspaceUpdate {
            purpose: ProfileWorkspacePurpose::Derived,
            document_bytes: content.as_bytes().to_vec(),
            sidecar_bytes: previous.sidecar_bytes.clone(),
        };
        let outcome = match runtime.as_ref() {
            ProfileRuntimeAccessRef::Offline => {
                self.persist_active_workspace(
                    reference,
                    &expected_workspace,
                    &update,
                    ActiveProfileReferenceWriteAuthorization::Unowned,
                )
                .await?;
                ProfileContentSaveOutcome::StoredOffline
            }
            ProfileRuntimeAccessRef::CoreOwner(runtime) => {
                if runtime.engine_id().as_ref() != Some(&reference.engine_id) {
                    return Err(Failure::unsupported(
                        "profile content runtime does not own the active profile engine",
                    ));
                }
                let authorization = runtime.write_authorization()?;
                ensure_bound_owner_authorization(authorization.clone(), &reference.engine_id)?;
                match runtime.core_lifecycle() {
                    CoreLifecycle::Ready | CoreLifecycle::Running => {
                        runtime
                            .apply_active_profile_workspace(
                                reference,
                                &expected_workspace,
                                &update,
                                authorization,
                                strategy,
                            )
                            .await
                            .map_err(Failure::from)?;
                        ProfileContentSaveOutcome::Applied
                    }
                    CoreLifecycle::Stopped => {
                        self.persist_active_workspace(
                            reference,
                            &expected_workspace,
                            &update,
                            authorization,
                        )
                        .await?;
                        ProfileContentSaveOutcome::StoredWhileInactive(
                            CoreInactiveLifecycle::Stopped,
                        )
                    }
                    CoreLifecycle::Failed => {
                        self.persist_active_workspace(
                            reference,
                            &expected_workspace,
                            &update,
                            authorization,
                        )
                        .await?;
                        ProfileContentSaveOutcome::StoredWhileInactive(
                            CoreInactiveLifecycle::Failed,
                        )
                    }
                    CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                        return Err(Failure::new(
                            ErrorCode::InvalidState,
                            "Core lifecycle is transitioning; profile content was not changed",
                            true,
                        ));
                    }
                }
            }
        };
        Ok(outcome)
    }

    async fn persist_profile_content_if_reference_current(
        &self,
        profile: &str,
        content: &str,
        expected: &ActiveProfileReferenceState,
    ) -> Result<(), Failure> {
        let observed = self.active_profile_reference().await?;
        if &observed != expected {
            return Err(stale_reference_failure(expected, &observed));
        }
        self.store
            .save_inactive_profile(profile, content)
            .await
            .map_err(Failure::from)?;
        self.store
            .clear_backup(profile)
            .await
            .map_err(Failure::from)
    }

    /// Switch the revisioned active-profile reference and apply that exact
    /// committed profile. A failed apply restores the previous reference only
    /// while this activation's revision is still current.
    pub async fn activate_profile<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccess<R>,
        profile: &str,
    ) -> Result<ProfileActivationOutcome, Failure> {
        self.activate_profile_using(runtime.as_ref(), profile).await
    }

    /// Activation entry for owners that can apply an exact reference but do
    /// not implement the full runtime controller surface.
    pub async fn activate_profile_with_applier(
        &self,
        applier: Arc<dyn ActiveProfileReferenceApplier>,
        profile: &str,
    ) -> Result<ProfileActivationOutcome, Failure> {
        self.activate_profile_using(
            ProfileRuntimeAccessRef::CoreOwner(applier.as_ref()),
            profile,
        )
        .await
    }

    async fn activate_profile_using<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: ProfileRuntimeAccessRef<'_, R>,
        profile: &str,
    ) -> Result<ProfileActivationOutcome, Failure> {
        let profile = valid_name(profile)?;
        let engine_id = self.store.engine_id();
        let profile_id = ProfileId::new(profile.clone())
            .map_err(|error| Failure::new(ErrorCode::InvalidInput, error.to_string(), false))?;
        let mode = match runtime {
            ProfileRuntimeAccessRef::Offline => ActivationMode::Offline,
            ProfileRuntimeAccessRef::CoreOwner(runtime) => {
                if runtime.engine_id().as_ref() != Some(&engine_id) {
                    return Err(Failure::unsupported(
                        "profile activation runtime does not own the profile engine",
                    ));
                }
                let authorization = runtime.write_authorization().map_err(Failure::from)?;
                ensure_bound_owner_authorization(authorization.clone(), &engine_id)?;
                match runtime.core_lifecycle() {
                    CoreLifecycle::Ready | CoreLifecycle::Running => ActivationMode::Running {
                        runtime,
                        authorization,
                    },
                    CoreLifecycle::Stopped => ActivationMode::Inactive {
                        authorization,
                        lifecycle: CoreInactiveLifecycle::Stopped,
                    },
                    CoreLifecycle::Failed => ActivationMode::Inactive {
                        authorization,
                        lifecycle: CoreInactiveLifecycle::Failed,
                    },
                    CoreLifecycle::Starting | CoreLifecycle::Stopping => {
                        return Err(Failure::new(
                            ErrorCode::InvalidState,
                            "Core lifecycle is transitioning; profile selection was not changed",
                            true,
                        ));
                    }
                }
            }
        };
        let authorization = mode.authorization();
        let target = ActiveProfileReferenceTarget::selected(engine_id, profile_id);
        let previous = self
            .store
            .read_active_profile_reference()
            .await
            .map_err(Failure::from)?;
        if matches!(&mode, ActivationMode::Running { .. })
            && matches!(&previous, ActiveProfileReferenceState::Unselected { .. })
        {
            return Err(Failure::new(
                ErrorCode::InvalidState,
                "a runtime profile activation requires an already selected profile for conditional recovery",
                false,
            ));
        }
        if selection_matches_target(&previous, &target) {
            return Ok(match mode {
                ActivationMode::Offline => ProfileActivationOutcome::AlreadySelectedWithoutRuntime,
                ActivationMode::Running { .. } => {
                    ProfileActivationOutcome::AlreadySelectedWithRuntime
                }
                ActivationMode::Inactive { lifecycle, .. } => {
                    ProfileActivationOutcome::AlreadySelectedWhileInactive(lifecycle)
                }
            });
        }

        let committed = match self
            .store
            .compare_and_set_active_profile_reference(
                previous.clone(),
                target.clone(),
                authorization.clone(),
            )
            .await
            .map_err(Failure::from)?
        {
            ActiveProfileReferenceCasOutcome::Committed {
                previous: observed_previous,
                current,
            } => {
                validate_committed_selection(&previous, &observed_previous, &current, &target)?;
                current
            }
            ActiveProfileReferenceCasOutcome::Stale { expected, observed } => {
                return Err(stale_reference_failure(&expected, &observed));
            }
        };

        match mode {
            ActivationMode::Offline => {
                return Ok(ProfileActivationOutcome::SelectedWithoutRuntime);
            }
            ActivationMode::Inactive { lifecycle, .. } => {
                return Ok(ProfileActivationOutcome::SelectedWhileInactive(lifecycle));
            }
            ActivationMode::Running {
                runtime,
                authorization,
            } => {
                let ActiveProfileReferenceState::Selected(committed_reference) = &committed else {
                    return Err(Failure::new(
                        ErrorCode::Internal,
                        "profile activation compare-and-set committed an unselected reference",
                        false,
                    ));
                };
                if let Err(apply_error) = runtime
                    .apply_active_profile_reference(
                        committed_reference,
                        ApplyStrategy::AlwaysRestart,
                    )
                    .await
                {
                    return Err(self
                        .recover_failed_activation(
                            runtime,
                            &previous,
                            &committed,
                            &authorization,
                            apply_error,
                        )
                        .await);
                }
                Ok(ProfileActivationOutcome::SelectedAndApplied)
            }
        }
    }

    async fn recover_failed_activation<R: ActiveProfileReferenceApplier + ?Sized>(
        &self,
        runtime: &R,
        previous: &ActiveProfileReferenceState,
        committed: &ActiveProfileReferenceState,
        authorization: &ActiveProfileReferenceWriteAuthorization,
        apply_error: PortError,
    ) -> Failure {
        let expected_revision = committed.revision();
        let apply_failure = core_apply_failure_kind(&apply_error);
        let recovery = self
            .store
            .compare_and_set_active_profile_reference(
                committed.clone(),
                target_from_reference_state(previous),
                authorization.clone(),
            )
            .await;
        match recovery {
            Ok(ActiveProfileReferenceCasOutcome::Committed {
                previous: observed_previous,
                current: restored,
            }) => {
                if observed_previous != *committed
                    || !restored_matches_previous(previous, &restored, expected_revision)
                {
                    return Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile apply failed: {apply_error}; conditional recovery returned an invalid selection receipt"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::CompareFailed {
                            expected_revision: expected_revision.value(),
                            message: "compare-and-set returned an unexpected recovery receipt"
                                .to_owned(),
                            apply_failure,
                        },
                    ));
                }
                match (previous, &restored) {
                    (
                        ActiveProfileReferenceState::Unselected { .. },
                        ActiveProfileReferenceState::Unselected { .. },
                    ) => Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile apply failed from an unselected state; runtime recovery is not authorized: {apply_error}"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::CompareFailed {
                            expected_revision: expected_revision.value(),
                            message: "runtime activation must not recover to Unselected".to_owned(),
                            apply_failure,
                        },
                    )),
                    (
                        ActiveProfileReferenceState::Selected(previous_reference),
                        ActiveProfileReferenceState::Selected(restored_reference),
                    ) if runtime.engine_id().as_ref() == Some(&previous_reference.engine_id) => {
                        match runtime.core_lifecycle() {
                            CoreLifecycle::Stopped => {
                                Failure::new(
                                    ErrorCode::Internal,
                                    format!(
                                        "profile switch failed; the previous reference was restored while Core remains stopped: {apply_error}"
                                    ),
                                    false,
                                )
                                .with_reason(FailureReason::ActiveProfileRecovery(
                                    ActiveProfileRecoveryOutcome::RestoredButRuntimeUnavailable {
                                        restored_revision: restored.revision().value(),
                                        engine_id: previous_reference.engine_id.clone(),
                                        lifecycle: CoreInactiveLifecycle::Stopped,
                                        apply_failure,
                                    },
                                ))
                            }
                            CoreLifecycle::Failed => {
                                Failure::new(
                                    ErrorCode::Internal,
                                    format!(
                                        "profile switch failed; the previous reference was restored while Core remains failed: {apply_error}"
                                    ),
                                    false,
                                )
                                .with_reason(FailureReason::ActiveProfileRecovery(
                                    ActiveProfileRecoveryOutcome::RestoredButRuntimeUnavailable {
                                        restored_revision: restored.revision().value(),
                                        engine_id: previous_reference.engine_id.clone(),
                                        lifecycle: CoreInactiveLifecycle::Failed,
                                        apply_failure,
                                    },
                                ))
                            }
                            CoreLifecycle::Starting | CoreLifecycle::Stopping => Failure::new(
                                ErrorCode::Internal,
                                format!(
                                    "profile switch failed; the previous reference was restored but Core is transitioning: {apply_error}"
                                ),
                                false,
                            )
                            .with_reason(FailureReason::ActiveProfileRecovery(
                                ActiveProfileRecoveryOutcome::ReapplyFailed {
                                    restored_revision: restored.revision().value(),
                                    message: "Core lifecycle is transitioning; reapply was not dispatched".to_owned(),
                                    apply_failure,
                                },
                            )),
                            CoreLifecycle::Ready | CoreLifecycle::Running => match runtime
                                .apply_active_profile_reference(
                                    restored_reference,
                                    ApplyStrategy::AlwaysRestart,
                                )
                                .await
                            {
                                Ok(_) => Failure::new(
                                ErrorCode::Internal,
                                format!(
                                    "profile switch failed; the previous profile was restored and reapplied: {apply_error}"
                                ),
                                false,
                            )
                            .with_reason(FailureReason::ActiveProfileRecovery(
                                ActiveProfileRecoveryOutcome::RestoredAndReapplied {
                                    restored_revision: restored.revision().value(),
                                    apply_failure,
                                },
                            )),
                                Err(recovery_error) => Failure::new(
                                ErrorCode::Internal,
                                format!(
                                    "profile switch failed and the restored profile could not be reapplied: {apply_error}; {recovery_error}"
                                ),
                                false,
                            )
                            .with_reason(FailureReason::ActiveProfileRecovery(
                                ActiveProfileRecoveryOutcome::ReapplyFailed {
                                    restored_revision: restored.revision().value(),
                                    message: recovery_error.to_string(),
                                    apply_failure,
                                },
                            )),
                            },
                        }
                    }
                    (
                        ActiveProfileReferenceState::Selected(previous_reference),
                        ActiveProfileReferenceState::Selected(_),
                    ) => Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile switch failed; the previous engine reference was restored but this runtime cannot reapply it: {apply_error}"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::RestoredButRuntimeMismatch {
                            restored_revision: restored.revision().value(),
                            engine_id: previous_reference.engine_id.clone(),
                            apply_failure,
                        },
                    )),
                    _ => Failure::new(
                        ErrorCode::Internal,
                        format!(
                            "profile apply failed: {apply_error}; conditional recovery returned a state inconsistent with the previous selection"
                        ),
                        false,
                    )
                    .with_reason(FailureReason::ActiveProfileRecovery(
                        ActiveProfileRecoveryOutcome::CompareFailed {
                            expected_revision: expected_revision.value(),
                            message: "recovery state did not match the previous selection"
                                .to_owned(),
                            apply_failure,
                        },
                    )),
                }
            }
            Ok(ActiveProfileReferenceCasOutcome::Stale { observed, .. }) => Failure::new(
                ErrorCode::NotReady,
                format!(
                    "profile apply failed: {apply_error}; recovery skipped because selection revision {} superseded committed revision {}",
                    observed.revision().value(),
                    expected_revision.value()
                ),
                true,
            )
            .with_reason(FailureReason::ActiveProfileRecovery(
                ActiveProfileRecoveryOutcome::Superseded {
                    expected_revision: expected_revision.value(),
                    observed_revision: observed.revision().value(),
                    apply_failure,
                },
            )),
            Err(recovery_error) => Failure::new(
                ErrorCode::Internal,
                format!(
                    "profile apply failed: {apply_error}; conditional recovery failed without changing the active reference: {recovery_error}"
                ),
                false,
            )
            .with_reason(FailureReason::ActiveProfileRecovery(
                ActiveProfileRecoveryOutcome::CompareFailed {
                    expected_revision: expected_revision.value(),
                    message: recovery_error.to_string(),
                    apply_failure,
                },
            )),
        }
    }
}

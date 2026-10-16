use super::*;
use std::mem::take;

#[async_trait]
impl ProfileStore for FakeStore {
    fn engine_id(&self) -> EngineId {
        EngineId::new("mihomo").expect("test engine ID")
    }

    async fn load_workspace(&self, profile: &ProfileId) -> Result<ProfileWorkspace, PortError> {
        self.workspace_reads.fetch_add(1, Ordering::SeqCst);
        let profile = profile.as_str();
        if let Some(error) = self.options_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let result = {
            let profiles = self.profiles.lock().unwrap();
            let sidecars = self.sidecars.lock().unwrap();
            let (content, metadata) = profiles
                .get(profile)
                .ok_or_else(|| PortError::NotFound(profile.into()))?;
            let mut result = workspace_with_raw_sidecar(
                profile,
                content.as_bytes(),
                sidecars.get(profile).cloned(),
                metadata.subscription_url.as_deref(),
            )?;
            result.write_protection = ProfileWriteProtection::from_subscription_url(
                metadata.subscription_url.as_deref().unwrap_or_default(),
            );
            result
        };
        if self
            .stale_after_next_workspace_read
            .swap(false, Ordering::SeqCst)
        {
            let mut profiles = self.profiles.lock().unwrap();
            let (content, _) = profiles
                .get_mut(profile)
                .ok_or_else(|| PortError::NotFound(profile.into()))?;
            content.push_str("# concurrent edit\n");
        }
        Ok(result)
    }

    async fn load_active_workspace(&self) -> Result<ProfileWorkspace, PortError> {
        let profile = match self.active_reference() {
            ActiveProfileReferenceState::Selected(reference) => {
                reference.profile_id.as_str().to_owned()
            }
            ActiveProfileReferenceState::Unselected { .. } => {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "fake store has no selected profile",
                ));
            }
        };
        let profile = ProfileId::new(profile).map_err(|error| PortError::Io(error.to_string()))?;
        self.load_workspace(&profile).await
    }

    async fn compare_and_save_inactive_workspace(
        &self,
        expected: &ProfileWorkspaceIdentity,
        update: &ProfileWorkspaceUpdate,
    ) -> Result<ProfileWorkspace, PortError> {
        self.publish_workspace_locked(expected, update)
    }

    async fn acquire_active_workspace_write_lease(
        &self,
        expected_reference: ActiveProfileReference,
        expected: ProfileWorkspaceIdentity,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<Box<dyn ActiveProfileWorkspaceWriteLease + '_>, PortError> {
        let guard = Arc::clone(&self.workspace_lease_gate).lock_owned().await;
        let bound_owner = self.bound_owner.lock().expect("test active owner").clone();
        let authorized = match (&authorization, &bound_owner) {
            (ActiveProfileReferenceWriteAuthorization::Unowned, None) => true,
            (
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity,
                    generation,
                },
                Some((bound_identity, bound_generation)),
            ) => identity == bound_identity && generation == bound_generation,
            _ => false,
        };
        if !authorized {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace owner authorization is stale",
                true,
            )));
        }
        let reference_matches = {
            let reference_state = self.active_reference.lock().expect("active reference");
            *reference_state == ActiveProfileReferenceState::Selected(expected_reference.clone())
        };
        if !reference_matches
            || expected.document.engine_id != expected_reference.engine_id
            || expected.document.profile_id != expected_reference.profile_id
        {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active profile reference changed",
                true,
            )));
        }
        let observed = ProfileStore::load_workspace(self, &expected.document.profile_id).await?;
        if observed.identity() != expected {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "active workspace identity is stale",
                true,
            )));
        }
        Ok(Box::new(FakeActiveWorkspaceWriteLease {
            store: self,
            expected_reference,
            observed,
            _guard: guard,
        }))
    }
    fn config_dir(&self) -> PathBuf {
        PathBuf::from("/fake/configs")
    }

    async fn list_profiles(&self) -> Result<Vec<ProfileInfo>, PortError> {
        if let Some(error) = self.list_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        let current = match self.active_reference() {
            ActiveProfileReferenceState::Selected(reference) => {
                Some(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => None,
        };
        let mut profiles = self
            .profiles
            .lock()
            .expect("profiles lock")
            .iter()
            .map(|(name, (_content, metadata))| ProfileInfo {
                name: name.clone(),
                active: current.as_deref() == Some(name.as_str()),
                path: format!("/fake/configs/{name}.yaml"),
                subscription_url: metadata.subscription_url.clone(),
                auto_update_enabled: metadata.auto_update_enabled,
                update_interval_hours: metadata.update_interval_hours,
                last_updated: metadata.last_updated,
                next_update: metadata.next_update,
                traffic_upload: metadata.traffic_upload,
                traffic_download: metadata.traffic_download,
                traffic_total: metadata.traffic_total,
                expire_at: metadata.expire_at,
                controller_url: None,
                controller_changed: None,
                user_agent: metadata.user_agent.clone(),
                etag: metadata.etag.clone(),
                last_modified: metadata.last_modified.clone(),
                cron_expression: metadata.cron_expression.clone(),
                insecure_skip_verify: metadata.insecure_skip_verify,
                auto_reload_core: metadata.auto_reload_core,
                has_backup: self
                    .restorable_backups
                    .lock()
                    .expect("backup lock")
                    .contains(name),
            })
            .collect::<Vec<_>>();
        catalog_order::sort_fake_profiles(&self.catalog_order, &mut profiles);
        Ok(profiles)
    }

    async fn set_profile_catalog_order(
        &self,
        request: ProfileCatalogOrderRequest,
    ) -> Result<ProfileCatalogOrderMutation, PortError> {
        catalog_order::set_fake_store_order(self, request).await
    }

    async fn get_current(&self) -> Result<String, PortError> {
        match self.active_reference() {
            ActiveProfileReferenceState::Selected(reference) => {
                Ok(reference.profile_id.as_str().to_owned())
            }
            ActiveProfileReferenceState::Unselected { .. } => Err(PortError::unsupported(
                Capability::Profiles,
                "fake store has no selected profile",
            )),
        }
    }

    async fn read_active_profile_reference(
        &self,
    ) -> Result<ActiveProfileReferenceState, PortError> {
        let observed = self.active_reference();
        let barrier = {
            let guard = self
                .reference_read_barrier
                .lock()
                .expect("reference read barrier");
            guard.clone()
        };
        if let Some(barrier) = barrier {
            barrier.wait().await;
        }
        Ok(observed)
    }

    async fn compare_and_set_active_profile_reference(
        &self,
        expected: ActiveProfileReferenceState,
        target: ActiveProfileReferenceTarget,
        authorization: ActiveProfileReferenceWriteAuthorization,
    ) -> Result<ActiveProfileReferenceCasOutcome, PortError> {
        let bound_owner = self.bound_owner.lock().expect("test active owner").clone();
        match (&authorization, &bound_owner) {
            (ActiveProfileReferenceWriteAuthorization::Unowned, None) => {}
            (
                ActiveProfileReferenceWriteAuthorization::BoundOwner {
                    identity,
                    generation,
                },
                Some((bound_identity, bound_generation)),
            ) if identity == bound_identity && generation == bound_generation => {}
            (ActiveProfileReferenceWriteAuthorization::Unowned, Some(_)) => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "offline selection is blocked while a Core owner is bound",
                    true,
                )));
            }
            _ => {
                return Err(PortError::Rejected(Failure::new(
                    ErrorCode::NotReady,
                    "active Core owner authorization does not match the bound identity and generation",
                    true,
                )));
            }
        }
        let concurrent_target = self
            .reference_change_before_compare
            .lock()
            .expect("reference compare hook")
            .take();
        if let Some(concurrent_target) = concurrent_target {
            self.update_reference_target(concurrent_target)?;
        }
        let mut state = self.active_reference.lock().expect("active reference");
        let observed = state.clone();
        if observed.revision() != expected.revision() {
            return Ok(ActiveProfileReferenceCasOutcome::Stale { expected, observed });
        }
        if observed != expected {
            return Err(PortError::Failed(
                "active reference state changed without advancing its revision".to_owned(),
            ));
        }
        if let ActiveProfileReferenceTarget::Selected {
            engine_id,
            profile_id,
        } = &target
        {
            if engine_id != &self.engine_id() {
                return Err(PortError::unsupported(
                    Capability::Profiles,
                    "fake store cannot select a foreign engine profile",
                ));
            }
            if !self
                .profiles
                .lock()
                .expect("profiles")
                .contains_key(profile_id.as_str())
            {
                return Err(PortError::NotFound(profile_id.as_str().to_owned()));
            }
        }
        let revision = expected
            .revision()
            .next()
            .ok_or_else(|| PortError::Failed("active reference revision exhausted".to_owned()))?;
        let current = match target {
            ActiveProfileReferenceTarget::Unselected => {
                ActiveProfileReferenceState::Unselected { revision }
            }
            ActiveProfileReferenceTarget::Selected {
                engine_id,
                profile_id,
            } => ActiveProfileReferenceState::Selected(ActiveProfileReference::new(
                engine_id, profile_id, revision,
            )),
        };
        *state = current.clone();
        self.reference_write_count.fetch_add(1, Ordering::SeqCst);
        Ok(ActiveProfileReferenceCasOutcome::Committed {
            previous: expected,
            current,
        })
    }

    async fn load(&self, profile: &str) -> Result<String, PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(content, _)| content.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn save_inactive_profile(&self, profile: &str, content: &str) -> Result<(), PortError> {
        if matches!(
            self.active_reference(),
            ActiveProfileReferenceState::Selected(reference)
                if reference.engine_id == self.engine_id()
                    && reference.profile_id.as_str() == profile
        ) {
            return Err(PortError::Rejected(Failure::new(
                ErrorCode::NotReady,
                "fake inactive save cannot write the selected profile",
                true,
            )));
        }
        self.profiles
            .lock()
            .expect("profiles lock")
            .entry(profile.to_string())
            .or_insert_with(|| (String::new(), ProfileMetadata::default()))
            .0 = content.to_string();
        self.profile_save_count.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    async fn delete_profile(&self, profile: &str) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .remove(profile)
            .map(|_| ())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))?;
        self.deleted_options
            .lock()
            .expect("options lock")
            .push(profile.to_owned());
        self.sidecars.lock().expect("sidecars lock").remove(profile);
        Ok(())
    }

    async fn get_profile_metadata(&self, profile: &str) -> Result<ProfileMetadata, PortError> {
        if let Some(error) = self.metadata_read_error.lock().unwrap().clone() {
            return Err(error);
        }
        self.profiles
            .lock()
            .expect("profiles lock")
            .get(profile)
            .map(|(_, metadata)| metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn update_profile_metadata(
        &self,
        profile: &str,
        metadata: &ProfileMetadata,
    ) -> Result<(), PortError> {
        self.profiles
            .lock()
            .expect("profiles lock")
            .get_mut(profile)
            .map(|(_, current)| *current = metadata.clone())
            .ok_or_else(|| PortError::NotFound(profile.to_string()))
    }

    async fn profile_display_name(
        &self,
        profile: &ProfileId,
    ) -> Result<ProfileDisplayNameObservation, PortError> {
        display_name::observe_fake_store(self, profile)
    }

    async fn set_profile_display_name(
        &self,
        profile: &ProfileId,
        target: ProfileDisplayNameTarget,
    ) -> Result<ProfileDisplayNameMutation, PortError> {
        display_name::set_fake_store(self, profile, target)
    }

    async fn profile_age_credential(
        &self,
        request: ProfileAgeCredentialReadRequest,
    ) -> Result<ProfileAgeCredentialReadOutcome, PortError> {
        Ok(ProfileAgeCredentialReadOutcome::Observed {
            current: ProfileAgeCredentialObservation::new(
                self.engine_id(),
                request.profile_id,
                ProfileAgeCredentialRevision::new(4),
                ProfileAgeCredentialState::NotConfigured,
            ),
            selection: request.expected_selection,
        })
    }

    async fn decrypt_age_subscription_content(
        &self,
        _profile: &ProfileId,
        content: ProfileAgeEncryptedContent,
    ) -> Result<ProfileAgeContentDecryption, PortError> {
        self.age_decryption_calls.fetch_add(1, Ordering::SeqCst);
        if !content
            .as_str()
            .trim_start()
            .starts_with("-----BEGIN AGE ENCRYPTED FILE-----")
        {
            return Err(PortError::Failed(
                "Age decrypt fake received non-armored content".to_owned(),
            ));
        }
        let outcome = take(&mut *self.age_decryption.lock().expect("Age decryption"));
        match outcome {
            FakeAgeDecryption::Unsupported => Err(PortError::unsupported(
                Capability::Profiles,
                "Age decryption fake is not configured",
            )),
            FakeAgeDecryption::Decrypted { content, revision } => {
                Ok(ProfileAgeContentDecryption::Decrypted {
                    content: ProfileAgeDecryptedContent::new(content),
                    credential_revision: ProfileAgeCredentialRevision::new(revision),
                })
            }
            FakeAgeDecryption::CredentialNotConfigured => {
                Ok(ProfileAgeContentDecryption::CredentialNotConfigured)
            }
            FakeAgeDecryption::InvalidCredentialOrContent => {
                Ok(ProfileAgeContentDecryption::InvalidCredentialOrContent)
            }
        }
    }

    async fn delete_subscription_credential(&self, _profile: &str) -> Result<(), PortError> {
        Ok(())
    }

    async fn clear_backup(&self, profile: &str) -> Result<(), PortError> {
        self.cleared_backups
            .lock()
            .expect("backup lock")
            .push(profile.to_string());
        Ok(())
    }

    async fn restore_inactive_profile_backup(&self, profile: &str) -> Result<bool, PortError> {
        let mut restorable = self.restorable_backups.lock().expect("backup lock");
        if let Some(index) = restorable.iter().position(|name| name == profile) {
            restorable.remove(index);
            self.restored_backups
                .lock()
                .expect("backup lock")
                .push(profile.to_string());
            Ok(true)
        } else {
            Ok(false)
        }
    }
}


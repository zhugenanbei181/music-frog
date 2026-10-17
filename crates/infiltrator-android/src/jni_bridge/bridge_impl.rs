use super::*;

#[async_trait::async_trait]
impl AndroidBridge for JniBridge {
    fn process_role(&self) -> AndroidHostProcessRole {
        self.call_int("processRole", SIG_NOARGS_INT, &[])
            .ok()
            .and_then(AndroidHostProcessRole::from_wire_code)
            .unwrap_or(AndroidHostProcessRole::Unsupported)
    }

    async fn core_start(
        &self,
        launch_spec: &CoreLaunchSpec,
        context: &CoreProcessStartContext,
    ) -> AndroidBridgeResult<CoreProcessStartReceipt> {
        #[derive(serde::Serialize)]
        struct StartRequest<'a> {
            launch_spec: &'a CoreLaunchSpec,
            start_context: &'a CoreProcessStartContext,
        }
        let start_request_json = serde_json::to_string(&StartRequest {
            launch_spec,
            start_context: context,
        })
        .map_err(|error| {
            PortError::Failed(format!(
                "Android Core start request could not be serialized: {error}"
            ))
        })?;
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreStart { start_request_json })
            .await?
        {
            AndroidBridgeCallReply::CoreStart { wire_code } => {
                CoreProcessStartReceipt::from_wire_code(wire_code).ok_or_else(|| {
                    PortError::Failed(format!(
                        "Android host returned unknown Core start receipt code {wire_code}"
                    ))
                })
            }
            _ => Err(unexpected_bridge_reply("CoreStart")),
        }
    }

    async fn core_stop(&self) -> AndroidBridgeResult<CoreProcessStopReceipt> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreStop)
            .await?
        {
            AndroidBridgeCallReply::CoreStop { wire_code } => {
                CoreProcessStopReceipt::from_wire_code(wire_code).ok_or_else(|| {
                    PortError::Failed(format!(
                        "Android host returned unknown Core stop receipt code {wire_code}"
                    ))
                })
            }
            _ => Err(unexpected_bridge_reply("CoreStop")),
        }
    }

    async fn core_process_observation(&self) -> AndroidBridgeResult<AndroidCoreProcessObservation> {
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreProcessObservation)
            .await?
        {
            AndroidBridgeCallReply::CoreProcessObservation(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("CoreProcessObservation")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!(
                "Android bridge returned an invalid Core process observation: {error}"
            ))
        })
    }

    async fn core_process_recovery_completed(
        &self,
        context: &CoreProcessStartContext,
    ) -> AndroidBridgeResult<AndroidCoreProcessRecoveryReceipt> {
        let context_json = serde_json::to_string(context).map_err(|error| {
            PortError::Failed(format!(
                "Android Core recovery context serialization failed: {error}"
            ))
        })?;
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreProcessRecoveryCompleted {
                context_json,
            })
            .await?
        {
            AndroidBridgeCallReply::CoreProcessRecoveryCompleted { wire_code } => {
                AndroidCoreProcessRecoveryReceipt::from_wire_code(wire_code).ok_or_else(|| {
                    PortError::Failed(format!(
                        "Android host returned unknown Core recovery receipt code {wire_code}"
                    ))
                })
            }
            _ => Err(unexpected_bridge_reply("CoreProcessRecoveryCompleted")),
        }
    }

    async fn core_validate_configuration(
        &self,
        request: &AndroidCoreConfigurationValidationRequest,
    ) -> AndroidBridgeResult<AndroidCoreConfigurationValidationOutcome> {
        let request_json = serde_json::to_string(request).map_err(|error| {
            PortError::Failed(format!(
                "Android Core validation request serialization failed: {error}"
            ))
        })?;
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreValidateConfiguration {
                request_json,
            })
            .await?
        {
            AndroidBridgeCallReply::CoreValidateConfiguration(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("CoreValidateConfiguration")),
        };
        match serde_json::from_str::<AndroidCoreConfigurationValidationReply>(&encoded).map_err(
            |error| {
                PortError::Failed(format!(
                    "Android Core configuration validation returned an invalid receipt: {error}"
                ))
            },
        )? {
            AndroidCoreConfigurationValidationReply::Receipt {
                request: receipt_request,
                outcome,
            } if &receipt_request == request => Ok(outcome),
            AndroidCoreConfigurationValidationReply::Receipt { .. } => Err(PortError::Failed(
                "Android Core configuration validation receipt did not match its request"
                    .to_owned(),
            )),
            AndroidCoreConfigurationValidationReply::InvalidRequest { reason } => {
                Err(PortError::Failed(format!(
                    "Android Core configuration validation rejected its request: {reason}"
                )))
            }
        }
    }

    async fn core_lifecycle_snapshot(&self) -> AndroidBridgeResult<AndroidCoreLifecycleSnapshot> {
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::CoreLifecycleSnapshot)
            .await?
        {
            AndroidBridgeCallReply::CoreLifecycleSnapshot(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("CoreLifecycleSnapshot")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!("Android lifecycle snapshot is invalid: {error}"))
        })
    }

    async fn execute_product_command_json(
        &self,
        command_json: &str,
    ) -> AndroidBridgeResult<String> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::ExecuteProductCommand {
                command_json: command_json.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::ExecuteProductCommand(result) => Ok(result),
            _ => Err(unexpected_bridge_reply("ExecuteProductCommand")),
        }
    }

    async fn surface_snapshot_json(&self) -> AndroidBridgeResult<String> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::SurfaceSnapshot)
            .await?
        {
            AndroidBridgeCallReply::SurfaceSnapshot(snapshot) => Ok(snapshot),
            _ => Err(unexpected_bridge_reply("SurfaceSnapshot")),
        }
    }

    async fn execute_owner_operation_json(
        &self,
        operation_json: &str,
    ) -> AndroidBridgeResult<String> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::ExecuteOwnerOperation {
                operation_json: operation_json.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::ExecuteOwnerOperation(result) => Ok(result),
            _ => Err(unexpected_bridge_reply("ExecuteOwnerOperation")),
        }
    }

    async fn execute_profile_age_credential_owner_operation(
        &self,
        submission: ProfileAgeCredentialOwnerSubmission,
    ) -> AndroidBridgeResult<ProfileAgeCredentialOwnerReadback> {
        let metadata = submission.metadata;
        let expected_metadata = metadata.clone();
        let request_json = serde_json::to_string(&ProfileAgeCredentialOwnerWireRequest {
            protocol: PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2,
            metadata,
        })
        .map_err(|error| {
            PortError::Failed(format!(
                "Age credential metadata serialization failed: {error}"
            ))
        })?;
        let reply_json = self
            .invoke_profile_age_credential_owner_operation(
                &request_json,
                submission.secret.as_bytes(),
            )
            .await?;
        let reply = serde_json::from_str::<ProfileAgeCredentialOwnerWireReply>(&reply_json)
            .map_err(|error| {
                PortError::Failed(format!(
                    "Android Age credential owner reply is invalid: {error}"
                ))
            })?;
        if reply.protocol != PROFILE_AGE_CREDENTIAL_OWNER_PROTOCOL_V2
            || reply.metadata != expected_metadata
        {
            return Err(PortError::Failed(
                "Android Age credential owner reply did not match its request".to_owned(),
            ));
        }
        match reply.outcome {
            ProfileAgeCredentialOwnerWireOutcome::Completed { readback } => Ok(readback),
            ProfileAgeCredentialOwnerWireOutcome::Rejected { status } => {
                Err(PortError::Failed(status.message.unwrap_or_else(|| {
                    format!(
                        "Android Age credential owner rejected the request: {:?}",
                        status.code
                    )
                })))
            }
        }
    }

    async fn credential_get(
        &self,
        service: &str,
        key: &str,
    ) -> AndroidBridgeResult<Option<String>> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CredentialGet {
                service: service.to_owned(),
                key: key.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::CredentialGet(value) => Ok(value),
            _ => Err(unexpected_bridge_reply("CredentialGet")),
        }
    }

    async fn credential_set(
        &self,
        service: &str,
        key: &str,
        value: &str,
    ) -> AndroidBridgeResult<()> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CredentialSet {
                service: service.to_owned(),
                key: key.to_owned(),
                value: value.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::CredentialSet {
                outcome: AndroidCredentialMutationOutcome::Applied,
            } => Ok(()),
            AndroidBridgeCallReply::CredentialSet {
                outcome: AndroidCredentialMutationOutcome::Rejected,
            } => Err(PortError::Failed(
                "Android credential store rejected the write".to_owned(),
            )),
            _ => Err(unexpected_bridge_reply("CredentialSet")),
        }
    }

    async fn credential_delete(&self, service: &str, key: &str) -> AndroidBridgeResult<()> {
        match self
            .invoke_android_bridge_call(AndroidBridgeCall::CredentialDelete {
                service: service.to_owned(),
                key: key.to_owned(),
            })
            .await?
        {
            AndroidBridgeCallReply::CredentialDelete {
                outcome: AndroidCredentialMutationOutcome::Applied,
            } => Ok(()),
            AndroidBridgeCallReply::CredentialDelete {
                outcome: AndroidCredentialMutationOutcome::Rejected,
            } => Err(PortError::Failed(
                "Android credential store rejected the delete".to_owned(),
            )),
            _ => Err(unexpected_bridge_reply("CredentialDelete")),
        }
    }

    fn data_dir(&self) -> Option<PathBuf> {
        self.call_string("dataDir", SIG_NOARGS_STRING, &[])
            .ok()
            .flatten()
            .map(PathBuf::from)
    }

    fn cache_dir(&self) -> Option<PathBuf> {
        self.call_string("cacheDir", SIG_NOARGS_STRING, &[])
            .ok()
            .flatten()
            .map(PathBuf::from)
    }

    async fn vpn_start(&self) -> AndroidBridgeResult<VpnStartOutcome> {
        let wire_code = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnStart)
            .await?
        {
            AndroidBridgeCallReply::VpnStart { wire_code } => wire_code,
            _ => return Err(unexpected_bridge_reply("VpnStart")),
        };
        VpnStartOutcome::from_wire_code(wire_code).ok_or_else(|| {
            PortError::Failed(format!(
                "Android host returned unknown VPN start outcome {wire_code}"
            ))
        })
    }

    async fn vpn_authorization_state(&self) -> AndroidBridgeResult<VpnAuthorizationState> {
        let wire_code = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnAuthorizationState)
            .await?
        {
            AndroidBridgeCallReply::VpnAuthorizationState { wire_code } => wire_code,
            _ => return Err(unexpected_bridge_reply("VpnAuthorizationState")),
        };
        match wire_code {
            0 => Ok(VpnAuthorizationState::Granted),
            1 => Ok(VpnAuthorizationState::Required),
            code => Err(PortError::Failed(format!(
                "Android host returned unknown VPN authorization state {code}"
            ))),
        }
    }

    async fn vpn_apply_configuration(
        &self,
        prepared: &PreparedVpnConfiguration,
    ) -> AndroidBridgeResult<VpnConfigurationApplyOutcome> {
        let session_token = prepared
            .core_scope
            .session_token
            .filter(|token| token.is_valid())
            .ok_or_else(|| {
                PortError::Failed("prepared VPN Core session is unavailable".to_owned())
            })?;
        let wire = PreparedVpnConfigurationWire {
            preparation_id: prepared.preparation_id.value(),
            vpn_generation: prepared.vpn_generation.value(),
            owner_data_revision: prepared.owner_data_revision,
            core_scope: PreparedVpnCoreScopeWire {
                engine_id: prepared.core_scope.identity.engine.as_str(),
                instance_id: prepared.core_scope.identity.instance.as_str(),
                version: prepared
                    .core_scope
                    .identity
                    .version
                    .as_ref()
                    .map(|version| version.raw()),
                generation: prepared.core_scope.generation.value(),
                session_token: session_token.value().to_string(),
            },
            configuration: &prepared.configuration,
        };
        let prepared_json = serde_json::to_string(&wire).map_err(|error| {
            PortError::Failed(format!("serialize prepared VPN configuration: {error}"))
        })?;
        let wire_code = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnApplyConfiguration { prepared_json })
            .await?
        {
            AndroidBridgeCallReply::VpnConfigurationApply { wire_code } => wire_code,
            _ => return Err(unexpected_bridge_reply("VpnApplyConfiguration")),
        };
        VpnConfigurationApplyOutcome::from_wire_code(wire_code).ok_or_else(|| {
            PortError::Failed(format!(
                "Android host returned unknown VPN configuration outcome {wire_code}"
            ))
        })
    }

    async fn vpn_stop(&self, target: VpnStopTarget) -> AndroidBridgeResult<VpnOwnerStopReceipt> {
        if matches!(target, VpnStopTarget::Generation(_)) && target.generation_wire_value() == 0 {
            return Err(PortError::Failed(
                "VPN stop generation must be non-zero".to_owned(),
            ));
        }
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnStop { target })
            .await?
        {
            AndroidBridgeCallReply::VpnStop(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("VpnStop")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!("Android VPN stop receipt is invalid: {error}"))
        })
    }

    async fn vpn_owner_snapshot(&self) -> AndroidBridgeResult<VpnOwnerSnapshot> {
        let encoded = match self
            .invoke_android_bridge_call(AndroidBridgeCall::VpnOwnerSnapshot)
            .await?
        {
            AndroidBridgeCallReply::VpnOwnerSnapshot(encoded) => encoded,
            _ => return Err(unexpected_bridge_reply("VpnOwnerSnapshot")),
        };
        serde_json::from_str(&encoded).map_err(|error| {
            PortError::Failed(format!("Android VPN owner snapshot is invalid: {error}"))
        })
    }
}

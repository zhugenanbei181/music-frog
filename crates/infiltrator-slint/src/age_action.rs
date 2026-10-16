use super::*;

pub fn execute_slint_age_action(
    host: &dyn SlintProductHost,
    snapshot: Option<&SurfaceSnapshot>,
    action: SlintAction,
) -> Result<(), Failure> {
    let snapshot = snapshot.ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "The shared owner has not published a surface snapshot",
            true,
        )
    })?;
    let scope = snapshot.core.scope().ok_or_else(|| {
        Failure::new(
            ErrorCode::NotReady,
            "The shared owner has not published an active Core identity",
            true,
        )
    })?;

    match action {
        SlintAction::SetProfileAgeCredential {
            profile_id,
            private_key,
        } => mutate_age_credential(
            host,
            &scope,
            &profile_id,
            ProfileAgeCredentialTarget::Set(private_key),
        ),
        SlintAction::ClearProfileAgeCredential { profile_id } => {
            mutate_age_credential(host, &scope, &profile_id, ProfileAgeCredentialTarget::Clear)
        }
        SlintAction::SaveProfileSettings {
            profile_id,
            name,
            url,
            private_key,
        } => {
            let profile = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .and_then(|page| page.profiles.iter().find(|p| p.id == profile_id))
                .ok_or_else(|| {
                    Failure::new(ErrorCode::NotReady, "Profile was not observed", true)
                })?;
            if let Some(key) = private_key {
                mutate_age_credential(host, &scope, &profile_id, ProfileAgeCredentialTarget::Set(key))?;
            }
            if let Some(new_name) = name {
                if new_name != profile.name {
                    host.execute(
                        Some(snapshot),
                        SlintAction::Command(CommandIntent::RenameProfile {
                            profile_id: profile_id.clone(),
                            name: new_name,
                        }),
                    )?;
                }
            }
            if !url.is_empty() && url != profile.url {
                let draft = SubscriptionScheduleDraft {
                    url,
                    auto_update_enabled: false,
                    update_interval_hours: String::new(),
                    cron_expression: None,
                };
                host.execute(
                    Some(snapshot),
                    SlintAction::Command(CommandIntent::UpdateSubscriptionSchedule {
                        profile_id: profile_id.clone(),
                        draft,
                    }),
                )?;
            }
            Ok(())
        }
        SlintAction::ImportEncryptedSubscription {
            name,
            url,
            private_key,
        } => {
            if let Some(key) = private_key {
                mutate_age_credential(host, &scope, &name, ProfileAgeCredentialTarget::Set(key))?;
            }
            host.execute(
                Some(snapshot),
                SlintAction::Command(CommandIntent::ImportSubscription {
                    profile_id: name,
                    channel: SubscriptionImportChannel::Url,
                    source: url,
                }),
            )
        }
        SlintAction::ImportProfileFile {
            name,
            path,
            private_key,
        } => {
            if let Some(key) = private_key {
                mutate_age_credential(host, &scope, &name, ProfileAgeCredentialTarget::Set(key))?;
            }
            host.execute(
                Some(snapshot),
                SlintAction::Command(CommandIntent::ImportSubscription {
                    profile_id: name,
                    channel: SubscriptionImportChannel::LocalFile,
                    source: path,
                }),
            )
        }
        SlintAction::ImportOverrideFile { path } => {
            let content = fs::read_to_string(&path)
                .map_err(|err| Failure::new(ErrorCode::Storage, err.to_string(), true))?;
            if content.trim().is_empty() {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Override file is empty",
                    false,
                ));
            }
            let name = Path::new(&path)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("override")
                .to_string();
            host.execute(
                Some(snapshot),
                SlintAction::Command(CommandIntent::CreateNamedOverride {
                    name,
                    content,
                }),
            )
        }
        SlintAction::ExportProfile {
            profile_id,
            destination,
        } => {
            let profile = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .and_then(|page| page.profiles.iter().find(|p| p.id == profile_id))
                .ok_or_else(|| Failure::new(ErrorCode::NotReady, "Profile was not observed", true))?;
            let content = if let Some(doc) = &snapshot.profile_editor.document {
                if snapshot.profile_editor.read.profile.as_deref() == Some(&profile_id) {
                    doc.content.clone()
                } else {
                    format!("name: {}\nurl: {}\n", profile.name, profile.url)
                }
            } else {
                format!("name: {}\nurl: {}\n", profile.name, profile.url)
            };
            fs::write(&destination, content)
                .map_err(|err| Failure::new(ErrorCode::Storage, err.to_string(), true))?;
            Ok(())
        }
        SlintAction::ExportOverride {
            override_id,
            destination,
        } => {
            let page = snapshot
                .pages
                .profiles
                .data
                .as_ref()
                .ok_or_else(|| Failure::new(ErrorCode::NotReady, "Profiles page was not observed", true))?;
            let oid = NamedOverrideId::new(override_id)
                .map_err(|err| Failure::new(ErrorCode::InvalidInput, err.to_string(), false))?;
            let item = page
                .named_overrides
                .get_override(&oid)
                .ok_or_else(|| Failure::new(ErrorCode::InvalidInput, "Override not found", false))?;
            fs::write(&destination, &item.content)
                .map_err(|err| Failure::new(ErrorCode::Storage, err.to_string(), true))?;
            Ok(())
        }
        SlintAction::ExportLogs { destination } => {
            let page = snapshot
                .pages
                .logs
                .data
                .as_ref()
                .ok_or_else(|| Failure::new(ErrorCode::NotReady, "Logs page was not observed", true))?;
            let mut formatted = String::new();
            for entry in &page.entries {
                formatted.push_str(&format!(
                    "[{}] [{}] {}\n",
                    entry.timestamp, entry.level, entry.message
                ));
            }
            fs::write(&destination, formatted)
                .map_err(|err| Failure::new(ErrorCode::Storage, err.to_string(), true))?;
            Ok(())
        }
        SlintAction::StartRuntime => {
            let outcome = host.request_vpn_permission()?;
            match outcome {
                VpnPermissionOutcome::Granted => {
                    host.execute(Some(snapshot), SlintAction::Command(CommandIntent::StartCore))
                }
                VpnPermissionOutcome::Denied => {
                    Err(Failure::new(ErrorCode::Permission, "VPN permission denied", false))
                }
                VpnPermissionOutcome::Canceled => {
                    Err(Failure::new(ErrorCode::Canceled, "VPN permission canceled", false))
                }
            }
        }
        SlintAction::ScanQrCode => {
            let url = host.scan_qr_code()?;
            if !url.starts_with("http://") && !url.starts_with("https://") {
                return Err(Failure::new(
                    ErrorCode::InvalidInput,
                    "Scanned QR code is not a valid HTTP/HTTPS URL",
                    false,
                ));
            }
            host.execute(
                Some(snapshot),
                SlintAction::Command(CommandIntent::ImportSubscription {
                    profile_id: "qr-subscription".into(),
                    channel: SubscriptionImportChannel::Url,
                    source: url,
                }),
            )
        }
        SlintAction::OpenAutostartSettings => {
            host.open_autostart_settings()
        }
        SlintAction::RequestBatteryOptimizationExemption => {
            let outcome = host.request_battery_optimization_exemption()?;
            match outcome {
                BatteryOptimizationOutcome::Supported { enabled: true } => Ok(()),
                BatteryOptimizationOutcome::Supported { enabled: false } => {
                    Err(Failure::new(ErrorCode::Permission, "Battery optimization not exempt", false))
                }
                BatteryOptimizationOutcome::Unsupported => {
                    Err(Failure::unsupported("Battery optimization exemption unsupported on platform"))
                }
                BatteryOptimizationOutcome::Canceled => {
                    Err(Failure::new(ErrorCode::Canceled, "Battery optimization request canceled", false))
                }
                BatteryOptimizationOutcome::PermissionDenied => {
                    Err(Failure::new(ErrorCode::Permission, "Battery optimization permission denied", false))
                }
            }
        }
        SlintAction::SetHideBackgroundCard(enabled) => {
            host.set_hide_background_card(enabled)
        }
        SlintAction::SetReleaseUiWhenBackground(enabled) => {
            host.set_release_ui_when_background(enabled)
        }
        SlintAction::SetShowTrafficNotification(enabled) => {
            host.set_show_traffic_notification(enabled)
        }
        SlintAction::CacheBackPreview => {
            host.cache_back_preview()
        }
        SlintAction::CommitBackPreview => {
            host.commit_back_preview()
        }
        SlintAction::Command(_) | SlintAction::Sequence(_) | SlintAction::SetAutostart(_) => {
            Err(Failure::unsupported("Unhandled action in execute_slint_age_action"))
        }
    }
}

fn mutate_age_credential(
    host: &dyn SlintProductHost,
    scope: &CoreScope,
    profile_id: &str,
    target: ProfileAgeCredentialTarget,
) -> Result<(), Failure> {
    let profile_id = ProfileId::new(profile_id)
        .map_err(|err| Failure::new(ErrorCode::InvalidInput, err.to_string(), false))?;

    let read_request = ProfileAgeCredentialOwnerRequest::Read {
        scope: scope.clone(),
        profile_id: profile_id.clone(),
    };
    let readback = host.execute_age_credential_request(read_request)?;

    let (current, selection) = match readback {
        ProfileAgeCredentialOwnerReadback::Read(outcome) => match outcome {
            ProfileAgeCredentialReadOutcome::Observed { current, selection } => (current, selection),
            ProfileAgeCredentialReadOutcome::StaleSelection { .. } => {
                return Err(Failure::new(
                    ErrorCode::NotReady,
                    "active profile changed before Age credential read completed",
                    true,
                ));
            }
        },
        ProfileAgeCredentialOwnerReadback::Mutation(_) => {
            return Err(Failure::new(
                ErrorCode::Internal,
                "unexpected mutation readback from Age credential read request",
                true,
            ));
        }
    };

    let mutation_request = match target {
        ProfileAgeCredentialTarget::Set(private_key) => ProfileAgeCredentialOwnerRequest::Set {
            scope: scope.clone(),
            expected: current,
            expected_selection: selection,
            private_key,
        },
        ProfileAgeCredentialTarget::Clear => ProfileAgeCredentialOwnerRequest::Clear {
            scope: scope.clone(),
            expected: current,
            expected_selection: selection,
        },
        ProfileAgeCredentialTarget::Keep => return Ok(()),
    };

    let mutation_readback = host.execute_age_credential_request(mutation_request)?;
    match mutation_readback {
        ProfileAgeCredentialOwnerReadback::Mutation(mutation) => match mutation {
            ProfileAgeCredentialMutation::Updated { .. }
            | ProfileAgeCredentialMutation::Unchanged { .. }
            | ProfileAgeCredentialMutation::Cleared { .. } => Ok(()),
            ProfileAgeCredentialMutation::StaleCredential { .. } => Err(Failure::new(
                ErrorCode::NotReady,
                "Age credential changed concurrently",
                true,
            )),
            ProfileAgeCredentialMutation::StaleSelection { .. } => Err(Failure::new(
                ErrorCode::NotReady,
                "active profile changed concurrently",
                true,
            )),
        },
        ProfileAgeCredentialOwnerReadback::Read(_) => Err(Failure::new(
            ErrorCode::Internal,
            "unexpected read readback from Age credential mutation request",
            true,
        )),
    }
}

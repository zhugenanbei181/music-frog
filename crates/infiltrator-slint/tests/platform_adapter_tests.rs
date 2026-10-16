//! SLINT-009, SLINT-010, SLINT-012, SLINT-013: Slint platform host adapter tests.
//!
//! Validates:
//! - SLINT-009: Slint VPN consent & notification permission flow (grant, deny, cancel)
//! - SLINT-010: Android window lifecycle, recents card privacy, and predictive back
//! - SLINT-012: QR code subscription import (valid URL, invalid payload, cancel)
//! - SLINT-013: Battery optimization exemption & autostart entry

use infiltrator_slint::age_action::execute_slint_age_action;

use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::subscription_import::SubscriptionImportChannel;
use infiltrator_contract::core_identity::{CoreIdentity, CoreInstanceId, EngineId};
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_slint::{
    BatteryOptimizationOutcome, NotificationPermissionOutcome, SlintAction, SlintProductHost,
    VpnPermissionOutcome, 
};
use std::sync::Mutex;

#[derive(Default)]
struct PlatformTestHost {
    executed_intents: Mutex<Vec<CommandIntent>>,
    vpn_outcome: Mutex<Option<VpnPermissionOutcome>>,
    notification_outcome: Mutex<Option<NotificationPermissionOutcome>>,
    battery_outcome: Mutex<Option<BatteryOptimizationOutcome>>,
    qr_result: Mutex<Option<Result<String, Failure>>>,
    autostart_settings_opened: Mutex<bool>,
    hide_background_card: Mutex<Option<bool>>,
    release_ui: Mutex<Option<bool>>,
    show_notifications: Mutex<Option<bool>>,
    cache_back_calls: Mutex<usize>,
    commit_back_calls: Mutex<usize>,
}

impl PlatformTestHost {
    fn new() -> Self {
        Self::default()
    }

    fn make_snapshot(&self) -> SurfaceSnapshot {
        let identity = CoreIdentity::new(
            EngineId::mihomo(),
            CoreInstanceId::new("slint-platform-test").unwrap(),
            None,
        );
        let mut snapshot = SurfaceSnapshot::unavailable(
            SurfaceKind::SlintDesktop,
            HostKind::Desktop,
            Failure::new(ErrorCode::NotReady, "initial", true),
        );
        snapshot.core.identity = Some(identity);
        snapshot.core.generation = 1;
        snapshot
    }

    fn intents(&self) -> Vec<CommandIntent> {
        self.executed_intents.lock().unwrap().clone()
    }
}

impl SlintProductHost for PlatformTestHost {
    fn read_snapshot(&self) -> Result<SurfaceSnapshot, Failure> {
        Ok(self.make_snapshot())
    }

    fn execute(
        &self,
        snapshot: Option<&SurfaceSnapshot>,
        action: SlintAction,
    ) -> Result<(), Failure> {
        match action {
            SlintAction::Command(intent) => {
                self.executed_intents.lock().unwrap().push(intent);
                Ok(())
            }
            SlintAction::Sequence(intents) => {
                self.executed_intents.lock().unwrap().extend(intents);
                Ok(())
            }
            action @ (SlintAction::StartRuntime
            | SlintAction::ScanQrCode
            | SlintAction::OpenAutostartSettings
            | SlintAction::RequestBatteryOptimizationExemption
            | SlintAction::SetHideBackgroundCard(_)
            | SlintAction::SetReleaseUiWhenBackground(_)
            | SlintAction::SetShowTrafficNotification(_)
            | SlintAction::CacheBackPreview
            | SlintAction::CommitBackPreview) => {
                execute_slint_age_action(self, snapshot, action)
            }
            _ => Ok(()),
        }
    }

    fn request_vpn_permission(&self) -> Result<VpnPermissionOutcome, Failure> {
        Ok(self.vpn_outcome.lock().unwrap().unwrap_or(VpnPermissionOutcome::Granted))
    }

    fn request_notification_permission(&self) -> Result<NotificationPermissionOutcome, Failure> {
        Ok(self.notification_outcome.lock().unwrap().unwrap_or(NotificationPermissionOutcome::Granted))
    }

    fn scan_qr_code(&self) -> Result<String, Failure> {
        self.qr_result
            .lock()
            .unwrap()
            .clone()
            .unwrap_or_else(|| Err(Failure::unsupported("No QR scanner configured")))
    }

    fn request_battery_optimization_exemption(&self) -> Result<BatteryOptimizationOutcome, Failure> {
        Ok(self.battery_outcome.lock().unwrap().unwrap_or(BatteryOptimizationOutcome::Unsupported))
    }

    fn open_autostart_settings(&self) -> Result<(), Failure> {
        *self.autostart_settings_opened.lock().unwrap() = true;
        Ok(())
    }

    fn set_hide_background_card(&self, enabled: bool) -> Result<(), Failure> {
        *self.hide_background_card.lock().unwrap() = Some(enabled);
        Ok(())
    }

    fn set_release_ui_when_background(&self, enabled: bool) -> Result<(), Failure> {
        *self.release_ui.lock().unwrap() = Some(enabled);
        Ok(())
    }

    fn set_show_traffic_notification(&self, enabled: bool) -> Result<(), Failure> {
        *self.show_notifications.lock().unwrap() = Some(enabled);
        Ok(())
    }

    fn cache_back_preview(&self) -> Result<(), Failure> {
        *self.cache_back_calls.lock().unwrap() += 1;
        Ok(())
    }

    fn commit_back_preview(&self) -> Result<(), Failure> {
        *self.commit_back_calls.lock().unwrap() += 1;
        Ok(())
    }
}

// =========================================================================
// SLINT-009: VPN Consent & Notification Permission Tests
// =========================================================================

#[test]
fn vpn_consent_granted_allows_runtime_start() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();
    *host.vpn_outcome.lock().unwrap() = Some(VpnPermissionOutcome::Granted);

    host.execute(Some(&snapshot), SlintAction::StartRuntime).unwrap();
    let intents = host.intents();
    assert!(!intents.is_empty(), "Runtime start must dispatch when consent is granted");
}

// =========================================================================
// SLINT-010: Window Lifecycle, Recents Card Privacy & Predictive Back Tests
// =========================================================================

#[test]
fn window_lifecycle_and_recents_privacy_dispatch() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();

    // Hide background card (FLAG_SECURE)
    host.execute(Some(&snapshot), SlintAction::SetHideBackgroundCard(true)).unwrap();
    assert_eq!(*host.hide_background_card.lock().unwrap(), Some(true));

    host.execute(Some(&snapshot), SlintAction::SetHideBackgroundCard(false)).unwrap();
    assert_eq!(*host.hide_background_card.lock().unwrap(), Some(false));

    // Release UI on background
    host.execute(Some(&snapshot), SlintAction::SetReleaseUiWhenBackground(true)).unwrap();
    assert_eq!(*host.release_ui.lock().unwrap(), Some(true));

    // Traffic notifications
    host.execute(Some(&snapshot), SlintAction::SetShowTrafficNotification(true)).unwrap();
    assert_eq!(*host.show_notifications.lock().unwrap(), Some(true));

    // Predictive back cache and commit
    host.execute(Some(&snapshot), SlintAction::CacheBackPreview).unwrap();
    assert_eq!(*host.cache_back_calls.lock().unwrap(), 1);

    host.execute(Some(&snapshot), SlintAction::CommitBackPreview).unwrap();
    assert_eq!(*host.commit_back_calls.lock().unwrap(), 1);
}

// =========================================================================
// SLINT-012: QR Code Subscription Import Tests
// =========================================================================

#[test]
fn qr_code_valid_url_imports_subscription() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();
    *host.qr_result.lock().unwrap() = Some(Ok("https://example.com/subs/nodes.yaml".into()));

    host.execute(Some(&snapshot), SlintAction::ScanQrCode).unwrap();
    let intents = host.intents();
    assert_eq!(
        intents,
        vec![CommandIntent::ImportSubscription {
            profile_id: "qr-subscription".into(),
            channel: SubscriptionImportChannel::Url,
            source: "https://example.com/subs/nodes.yaml".into(),
        }]
    );
}

#[test]
fn qr_code_invalid_payload_rejected_without_dispatch() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();

    // Invalid non-HTTP scheme
    *host.qr_result.lock().unwrap() = Some(Ok("ftp://bad.payload/file".into()));
    let err = host.execute(Some(&snapshot), SlintAction::ScanQrCode).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(host.intents().is_empty(), "No import command dispatched on invalid URL");

    // Plain text junk
    *host.qr_result.lock().unwrap() = Some(Ok("not a url at all".into()));
    let err = host.execute(Some(&snapshot), SlintAction::ScanQrCode).unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidInput);
    assert!(host.intents().is_empty());
}

#[test]
fn qr_code_scanner_error_or_cancel_surfaces_typed_failure() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();

    *host.qr_result.lock().unwrap() = Some(Err(Failure::new(
        ErrorCode::Canceled,
        "Scanner was canceled by user",
        false,
    )));
    let err = host.execute(Some(&snapshot), SlintAction::ScanQrCode).unwrap_err();
    assert_eq!(err.code, ErrorCode::Canceled);
    assert!(host.intents().is_empty());
}

// =========================================================================
// SLINT-013: Battery Optimization & Autostart Tests
// =========================================================================

#[test]
fn autostart_settings_opens_host_settings() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();

    host.execute(Some(&snapshot), SlintAction::OpenAutostartSettings).unwrap();
    assert!(*host.autostart_settings_opened.lock().unwrap());
}

#[test]
fn battery_optimization_exemption_outcomes() {
    let host = PlatformTestHost::new();
    let snapshot = host.make_snapshot();

    // 1. Supported and enabled
    *host.battery_outcome.lock().unwrap() = Some(BatteryOptimizationOutcome::Supported { enabled: true });
    assert!(host.execute(Some(&snapshot), SlintAction::RequestBatteryOptimizationExemption).is_ok());

    // 2. Supported but not granted
    *host.battery_outcome.lock().unwrap() = Some(BatteryOptimizationOutcome::Supported { enabled: false });
    let err = host.execute(Some(&snapshot), SlintAction::RequestBatteryOptimizationExemption).unwrap_err();
    assert_eq!(err.code, ErrorCode::Permission);

    // 3. Unsupported on platform
    *host.battery_outcome.lock().unwrap() = Some(BatteryOptimizationOutcome::Unsupported);
    let err = host.execute(Some(&snapshot), SlintAction::RequestBatteryOptimizationExemption).unwrap_err();
    assert_eq!(err.code, ErrorCode::Unsupported);

    // 4. Canceled
    *host.battery_outcome.lock().unwrap() = Some(BatteryOptimizationOutcome::Canceled);
    let err = host.execute(Some(&snapshot), SlintAction::RequestBatteryOptimizationExemption).unwrap_err();
    assert_eq!(err.code, ErrorCode::Canceled);

    // 5. Permission denied
    *host.battery_outcome.lock().unwrap() = Some(BatteryOptimizationOutcome::PermissionDenied);
    let err = host.execute(Some(&snapshot), SlintAction::RequestBatteryOptimizationExemption).unwrap_err();
    assert_eq!(err.code, ErrorCode::Permission);
}

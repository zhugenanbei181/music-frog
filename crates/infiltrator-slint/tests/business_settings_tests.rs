//! The business panels of the settings workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{
    assert_accepted, assert_dismissed, assert_keeps_review, assert_locale, assert_refreshed,
    assert_refused, assert_retried, assert_unsupported, business_surface, cell,
};
use infiltrator_slint::surface::business::BusinessId;

/// The page that carries the settings businesses.
const PAGE: &str = "SettingsWorkbench";

#[test]
fn the_core_version_rollback_confirm_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::CoreVersionRollbackConfirm);
}

#[test]
fn the_core_version_rollback_confirm_cancel_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::CoreVersionRollbackConfirm);
}

#[test]
fn the_core_version_rollback_confirm_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::CoreVersionRollbackConfirm);
}

#[test]
fn the_settings_lan_security_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SettingsLanSecurity);
}

#[test]
fn the_settings_lan_security_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::SettingsLanSecurity);
}

#[test]
fn the_settings_language_choice_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SettingsLanguageChoice);
}

#[test]
fn the_settings_language_choice_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::SettingsLanguageChoice);
}

#[test]
fn the_settings_language_choice_retry_path() {
    let surface = business_surface(PAGE);
    assert_retried(&surface, BusinessId::SettingsLanguageChoice);
}

#[test]
fn the_settings_language_choice_refresh_path() {
    let surface = business_surface(PAGE);
    assert_refreshed(&surface, BusinessId::SettingsLanguageChoice);
}

#[test]
fn the_settings_language_choice_focus_path() {
    let surface = business_surface(PAGE);
    assert_keeps_review(&surface, PAGE, BusinessId::SettingsLanguageChoice);
}

#[test]
fn the_settings_language_choice_locale_path() {
    let surface = business_surface(PAGE);
    assert_locale(&surface, BusinessId::SettingsLanguageChoice);
}

#[test]
fn the_settings_permission_panel_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SettingsPermissionPanel);
}

#[test]
fn the_settings_permission_panel_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::SettingsPermissionPanel);
}

#[test]
fn the_settings_permission_panel_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::SettingsPermissionPanel);
}

#[test]
fn the_settings_port_conflict_repair_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SettingsPortConflictRepair);
}

#[test]
fn the_settings_port_conflict_repair_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::SettingsPortConflictRepair);
}

#[test]
fn the_settings_tun_permission_success_path() {
    let surface = business_surface(PAGE);
    assert_accepted(&surface, BusinessId::SettingsTunPermission);
}

#[test]
fn the_settings_tun_permission_cancel_path() {
    let surface = business_surface(PAGE);
    assert_dismissed(&surface, BusinessId::SettingsTunPermission);
}

#[test]
fn the_settings_tun_permission_error_path() {
    let surface = business_surface(PAGE);
    assert_refused(&surface, BusinessId::SettingsTunPermission);
}

#[test]
fn the_settings_tun_permission_unsupported_path() {
    let surface = business_surface(PAGE);
    assert_unsupported(&surface, PAGE, BusinessId::SettingsTunPermission);
}

#[test]
fn the_settings_panels_show_the_shared_settings_read_model() {
    let surface = business_surface(PAGE);
    for business in [
        BusinessId::CoreVersionRollbackConfirm,
        BusinessId::SettingsLanSecurity,
        BusinessId::SettingsLanguageChoice,
        BusinessId::SettingsPermissionPanel,
        BusinessId::SettingsPortConflictRepair,
        BusinessId::SettingsTunPermission,
    ] {
        assert!(
            cell(&surface, &format!("{}/summary", business.name())).is_some(),
            "{} must show the shared settings read model",
            business.name()
        );
        assert!(
            !cell(&surface, &format!("{}/confirm", business.name())).is_some(),
            "{} must not open its review before the user asks for it",
            business.name()
        );
    }
}

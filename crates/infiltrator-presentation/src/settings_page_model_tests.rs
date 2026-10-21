//! test-intent: behavior
use crate::settings_page_model::{LanSharingObservation, SettingsPageModel, observe_settings_page};
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::lan::{LanSecuritySnapshot, LanSharingSnapshot};
use infiltrator_contract::pac::{PacServiceState, PacSnapshot};
use infiltrator_contract::surface::{HostKind, SurfaceKind};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use lurker_core_contract::error::Failure;

fn lan_sharing() -> LanSharingSnapshot {
    LanSharingSnapshot::new(4, false, 7890, "*")
}

fn lan_security() -> LanSecuritySnapshot {
    LanSecuritySnapshot::new(
        7,
        vec!["192.168.0.0/16".to_owned()],
        Vec::new(),
        vec!["127.0.0.0/8".to_owned()],
        false,
        0,
        None,
    )
}

fn pac() -> PacSnapshot {
    PacSnapshot {
        state: PacServiceState::Disabled,
        script_bytes: 0,
        bypass_domains: vec!["localhost".to_owned()],
        bypass_lan: false,
        minify: false,
        revision: 2,
    }
}

#[test]
fn a_fresh_page_shows_the_owner_values_and_nothing_is_modified() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_sharing(Some(&lan_sharing()));
    model.observe_lan_security(&lan_security());
    model.observe_pac(&pac());

    assert!(!model.lan_sharing_modified());
    assert!(!model.lan_security_modified());
    assert!(!model.pac_modified());
    assert_eq!(model.lan_sharing().mixed_port, 7890);
    assert_eq!(model.lan_sharing().bind_address, "*");
    assert_eq!(
        model.lan_security().allowed_ips,
        vec!["192.168.0.0/16".to_owned()]
    );
    assert_eq!(model.pac().bypass_domains, vec!["localhost".to_owned()]);
}

#[test]
fn an_unobserved_lan_sharing_is_not_an_off_lan_sharing() {
    let mut model = SettingsPageModel::default();

    // Nothing was observed: the page reports "unobserved", never "off".
    assert!(!model.lan_sharing_observed());
    assert_eq!(
        model.lan_sharing_observation(),
        &LanSharingObservation::Unobserved
    );
    assert_eq!(model.lan_sharing_revision(), None);

    // An observed sharing that is off is a different fact.
    model.observe_lan_sharing(Some(&lan_sharing()));
    assert!(model.lan_sharing_observed());
    assert_eq!(model.lan_sharing_revision(), Some(4));
    assert_eq!(
        model.lan_sharing_observation(),
        &LanSharingObservation::Observed(lan_sharing())
    );
    assert!(!model.lan_sharing().enabled);

    // A readback that disappears leaves the last draft visible but marks the
    // observation as gone.
    model.observe_lan_sharing(None);
    assert!(!model.lan_sharing_observed());
    assert_eq!(model.lan_sharing_revision(), None);
    assert_eq!(model.lan_sharing().mixed_port, 7890);
}

#[test]
fn an_unobserved_lan_sharing_expresses_no_write_intent() {
    let mut model = SettingsPageModel::default();
    model.edit_lan_sharing(|draft| draft.mixed_port = 7891);

    // There is no owner baseline to write against.
    assert!(model.lan_sharing_modified());
    assert!(model.lan_sharing_intent().is_none());

    // Once the owner published a readback, the same edit expresses the write.
    model.observe_lan_sharing(Some(&lan_sharing()));
    assert_eq!(
        model.lan_sharing_intent(),
        Some(CommandIntent::SetLanSharing {
            enabled: false,
            mixed_port: 7891,
            bind_address: "*".to_owned(),
        })
    );
}

#[test]
fn an_edit_is_a_modification_and_an_owner_readback_is_not_one() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_sharing(Some(&lan_sharing()));

    model.edit_lan_sharing(|draft| draft.mixed_port = 7891);
    assert!(model.lan_sharing_modified());
    assert_eq!(model.lan_sharing().mixed_port, 7891);

    // A readback that still carries the old value keeps the edit visible.
    model.observe_lan_sharing(Some(&lan_sharing()));
    assert!(model.lan_sharing_modified());

    // A readback that carries the new value clears the modification.
    model.observe_lan_sharing(Some(&LanSharingSnapshot::new(5, false, 7891, "*")));
    assert!(!model.lan_sharing_modified());
    assert_eq!(model.lan_sharing().mixed_port, 7891);
}

#[test]
fn a_draft_that_returns_to_the_owner_value_is_not_a_modification() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_sharing(Some(&lan_sharing()));

    model.edit_lan_sharing(|draft| draft.mixed_port = 7891);
    assert!(model.lan_sharing_modified());

    model.edit_lan_sharing(|draft| draft.mixed_port = 7890);
    assert!(!model.lan_sharing_modified());
}

#[test]
fn an_unchanged_draft_has_no_owner_intent() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_sharing(Some(&lan_sharing()));
    model.observe_lan_security(&lan_security());
    model.observe_pac(&pac());

    assert!(model.lan_sharing_intent().is_none());
    assert!(model.lan_security_intent().is_none());
    assert!(model.pac_intent().is_none());
}

#[test]
fn the_owner_intents_carry_the_drafted_values() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_sharing(Some(&lan_sharing()));
    model.observe_lan_security(&lan_security());
    model.observe_pac(&pac());

    model.edit_lan_sharing(|draft| {
        draft.enabled = true;
        draft.mixed_port = 7891;
        draft.bind_address = "0.0.0.0".to_owned();
    });
    assert_eq!(
        model.lan_sharing_intent(),
        Some(CommandIntent::SetLanSharing {
            enabled: true,
            mixed_port: 7891,
            bind_address: "0.0.0.0".to_owned(),
        })
    );

    model.edit_lan_security(|draft| {
        draft.authentication_enabled = true;
        draft.auth_username = "musicfrog".to_owned();
        draft.auth_password = "secret".to_owned();
    });
    let intent = model
        .lan_security_intent()
        .expect("edited draft has an intent");
    match intent {
        CommandIntent::SetLanSecurity {
            allowed_ips,
            authentication_enabled,
            credentials,
            ..
        } => {
            assert_eq!(allowed_ips, vec!["192.168.0.0/16".to_owned()]);
            assert!(authentication_enabled);
            let credentials = credentials.expect("enabled authentication carries credentials");
            assert_eq!(credentials.username, "musicfrog");
            assert_eq!(credentials.password, "secret");
        }
        other => panic!("unexpected intent: {other:?}"),
    }

    model.edit_pac(|draft| {
        draft.enabled = true;
        draft.bypass_domains = vec!["localhost".to_owned(), "example.com".to_owned()];
    });
    assert_eq!(
        model.pac_intent(),
        Some(CommandIntent::ApplyPac {
            enabled: true,
            bypass_domains: vec!["localhost".to_owned(), "example.com".to_owned()],
            bypass_lan: false,
            minify: false,
        })
    );
}

#[test]
fn the_pac_draft_reads_the_owner_request_facts_instead_of_the_defaults() {
    let mut model = SettingsPageModel::default();
    let mut snapshot = pac();
    snapshot.bypass_lan = true;
    snapshot.minify = true;
    model.observe_pac(&snapshot);

    // The readback carries the two facts, so the draft is the owner's real
    // request and not the default one.
    assert!(model.pac().bypass_lan);
    assert!(model.pac().minify);
    assert!(!model.pac_modified());
    assert_eq!(
        model.pac_intent(),
        None,
        "the draft equals the owner request, so it expresses no write"
    );

    model.edit_pac(|draft| draft.bypass_lan = false);
    assert_eq!(
        model.pac_intent(),
        Some(CommandIntent::ApplyPac {
            enabled: false,
            bypass_domains: vec!["localhost".to_owned()],
            bypass_lan: false,
            minify: true,
        })
    );
}

#[test]
fn accepting_a_write_drops_the_in_memory_password() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_security(&lan_security());

    model.edit_lan_security(|draft| {
        draft.authentication_enabled = true;
        draft.auth_username = "musicfrog".to_owned();
        draft.auth_password = "secret".to_owned();
    });
    assert!(model.lan_security_modified());

    model.accept_lan_security();
    assert!(!model.lan_security_modified());
    assert_eq!(model.lan_security().auth_password, "");
    assert_eq!(model.lan_security().auth_username, "musicfrog");
}

#[test]
fn discarding_a_write_goes_back_to_the_owner_value() {
    let mut model = SettingsPageModel::default();
    model.observe_lan_sharing(Some(&lan_sharing()));
    model.observe_pac(&pac());

    model.edit_lan_sharing(|draft| draft.mixed_port = 7891);
    model.edit_pac(|draft| draft.enabled = true);
    assert!(model.lan_sharing_modified());
    assert!(model.pac_modified());

    model.discard_lan_sharing();
    model.discard_pac();
    assert!(!model.lan_sharing_modified());
    assert!(!model.pac_modified());
    assert_eq!(model.lan_sharing().mixed_port, 7890);
    assert!(!model.pac().enabled);
}

#[test]
fn the_pac_service_state_comes_from_the_owner_snapshot() {
    let mut model = SettingsPageModel::default();
    let mut snapshot = pac();
    snapshot.state = PacServiceState::Running {
        url: "http://127.0.0.1:8080/proxy.pac".to_owned(),
    };
    model.observe_pac(&snapshot);

    assert_eq!(
        model.pac_service(),
        Some(&PacServiceState::Running {
            url: "http://127.0.0.1:8080/proxy.pac".to_owned(),
        })
    );
    // The PAC service running means the draft is enabled.
    assert!(model.pac().enabled);
    assert!(!model.pac_modified());
}

#[test]
fn the_shared_fold_reads_the_lan_sharing_readback_of_the_surface_snapshot() {
    let mut snapshot = SurfaceSnapshot::unavailable(
        SurfaceKind::BevyDesktop,
        HostKind::Desktop,
        Failure::unsupported("not composed"),
    );
    let mut model = SettingsPageModel::default();

    // Nothing was observed: the model stays unobserved.
    observe_settings_page(&mut model, &snapshot);
    assert!(!model.lan_sharing_observed());
    assert_eq!(model.lan_sharing_revision(), None);

    // The runtime control readback is the owner fact.
    snapshot.runtime_control.lan_sharing = Some(LanSharingSnapshot::new(12, true, 7891, "*"));
    observe_settings_page(&mut model, &snapshot);
    assert!(model.lan_sharing_observed());
    assert_eq!(model.lan_sharing_revision(), Some(12));
    assert!(model.lan_sharing().enabled);
    assert_eq!(model.lan_sharing().mixed_port, 7891);
    assert_eq!(model.lan_sharing().bind_address, "*");
    assert!(!model.lan_sharing_modified());
}

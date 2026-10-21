//! The one typed page model of the settings page, shared by every surface.
//!
//! The settings page keeps three owner facts editable: the LAN sharing
//! configuration, the LAN ACL and authentication configuration, and the PAC
//! request. Iced kept a private mutable copy of each of them plus two
//! `*_dirty` booleans, so the page could say "modified" about a value it had
//! no committed baseline for.
//!
//! This module owns those drafts. The contract snapshots stay the truth, the
//! draft carries its own baseline, and "modified" is a derived fact of the
//! two. The typed owner intents are built here too, so no surface has to
//! re-translate a draft into a command.
//!
//! The LAN sharing fact is one typed observation: the owner publishes the
//! whole [`LanSharingSnapshot`] or nothing at all, so the page can tell
//! "sharing is off" from "sharing was never observed" instead of guessing.

use crate::editable_draft::EditableDraft;
use crate::operation_phase::OperationPhase;
use infiltrator_contract::command::CommandIntent;
use infiltrator_contract::lan::{LanCredentials, LanSecuritySnapshot, LanSharingSnapshot};
use infiltrator_contract::pac::{PacRequest, PacServiceState, PacSnapshot};
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;

/// The LAN sharing observation the owner published.
///
/// The two states are exclusive and explicit: a page that has no observation
/// reports "unobserved", never "off".
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum LanSharingObservation {
    /// The owner has not published a LAN sharing readback yet.
    #[default]
    Unobserved,
    /// The owner published a confirmed configuration.
    Observed(LanSharingSnapshot),
}

impl LanSharingObservation {
    /// The confirmed readback, when the owner published one.
    pub fn snapshot(&self) -> Option<&LanSharingSnapshot> {
        match self {
            Self::Unobserved => None,
            Self::Observed(snapshot) => Some(snapshot),
        }
    }

    /// Whether the owner published a confirmed readback.
    pub fn is_observed(&self) -> bool {
        matches!(self, Self::Observed(_))
    }

    /// The configuration revision the readback was taken at.
    pub fn revision(&self) -> Option<u64> {
        self.snapshot().map(|snapshot| snapshot.revision)
    }
}

/// The editable LAN sharing configuration.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanSharingDraft {
    /// Whether the host redirects traffic into the proxy.
    pub enabled: bool,
    /// The port the proxy listens on.
    pub mixed_port: u16,
    /// The address the proxy binds to.
    pub bind_address: String,
}

impl LanSharingDraft {
    /// The draft the owner's LAN sharing snapshot expresses.
    pub fn from_snapshot(snapshot: &LanSharingSnapshot) -> Self {
        Self {
            enabled: snapshot.enabled,
            mixed_port: snapshot.mixed_port,
            bind_address: snapshot.bind_address.clone(),
        }
    }
}

/// The editable LAN ACL and authentication configuration.
///
/// The password never enters a shared snapshot: it stays in this in-memory
/// draft and is only handed to the owner as a command input.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LanSecurityDraft {
    /// The networks the proxy accepts.
    pub allowed_ips: Vec<String>,
    /// The networks the proxy refuses.
    pub disallowed_ips: Vec<String>,
    /// The prefixes that never require authentication.
    pub skip_auth_prefixes: Vec<String>,
    /// Whether the proxy asks for credentials.
    pub authentication_enabled: bool,
    /// The user name the proxy asks for.
    pub auth_username: String,
    /// The password the proxy asks for.
    pub auth_password: String,
}

impl LanSecurityDraft {
    /// The draft the owner's LAN security snapshot expresses.
    ///
    /// The owner never publishes a password, so the draft starts without one.
    pub fn from_snapshot(snapshot: &LanSecuritySnapshot) -> Self {
        Self {
            allowed_ips: snapshot.allowed_ips.clone(),
            disallowed_ips: snapshot.disallowed_ips.clone(),
            skip_auth_prefixes: snapshot.skip_auth_prefixes.clone(),
            authentication_enabled: snapshot.authentication_enabled,
            auth_username: snapshot.authentication_username.clone().unwrap_or_default(),
            auth_password: String::new(),
        }
    }
}

/// The editable PAC request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PacDraft {
    /// Whether the PAC service runs.
    pub enabled: bool,
    /// The domains that always bypass the proxy.
    pub bypass_domains: Vec<String>,
    /// Whether the LAN bypasses the proxy.
    pub bypass_lan: bool,
    /// Whether the generated script is minified.
    pub minify: bool,
}

impl PacDraft {
    /// The draft the owner's PAC request expresses.
    pub fn from_request(request: &PacRequest) -> Self {
        Self {
            enabled: request.enabled,
            bypass_domains: request.bypass_domains.clone(),
            bypass_lan: request.bypass_lan,
            minify: request.minify,
        }
    }

    /// The owner request the draft expresses.
    pub fn to_request(&self) -> PacRequest {
        PacRequest {
            enabled: self.enabled,
            bypass_domains: self.bypass_domains.clone(),
            bypass_lan: self.bypass_lan,
            minify: self.minify,
        }
    }
}

/// The one page model of the settings page.
///
/// A surface renders the settings page from this model alone: the LAN
/// sharing draft, the LAN security draft and the PAC draft come from here,
/// and the surface only dispatches the typed intents the model builds.
#[derive(Clone, Debug, Default)]
pub struct SettingsPageModel {
    lan_sharing: EditableDraft<LanSharingDraft>,
    lan_sharing_observation: LanSharingObservation,
    lan_security: EditableDraft<LanSecurityDraft>,
    pac: EditableDraft<PacDraft>,
    pac_service: Option<PacServiceState>,
    lan_security_readback: LanSecuritySnapshot,
    lan_sharing_write: OperationPhase,
    lan_security_write: OperationPhase,
    pac_write: OperationPhase,
}

impl SettingsPageModel {
    /// Fold the owner's LAN sharing readback into the page model.
    ///
    /// The owner value becomes the new baseline; an edit the owner has not
    /// accepted yet stays visible as a modification. A readback the owner
    /// never published leaves the draft alone and reports "unobserved":
    /// the page must not claim the sharing is off.
    pub fn observe_lan_sharing(&mut self, sharing: Option<&LanSharingSnapshot>) {
        match sharing {
            Some(snapshot) => {
                self.lan_sharing_observation = LanSharingObservation::Observed(snapshot.clone());
                let owner = LanSharingDraft::from_snapshot(snapshot);
                // The owner's readback becomes the baseline. The fields the
                // user did not change follow the owner: a write must not
                // clear a field the user never touched.
                let previous = self.lan_sharing.baseline().cloned();
                let current = self.lan_sharing.value().clone();
                self.lan_sharing.adopt(owner.clone());
                let keep_enabled = previous
                    .as_ref()
                    .is_some_and(|previous| current.enabled != previous.enabled);
                let keep_port = previous
                    .as_ref()
                    .is_some_and(|previous| current.mixed_port != previous.mixed_port);
                let keep_bind = previous
                    .as_ref()
                    .is_some_and(|previous| current.bind_address != previous.bind_address);
                let owner_bind = owner.bind_address.clone();
                self.lan_sharing.edit_with(|draft: &mut LanSharingDraft| {
                    if keep_enabled {
                        draft.enabled = current.enabled;
                    }
                    if keep_port {
                        draft.mixed_port = current.mixed_port;
                    }
                    if keep_bind {
                        draft.bind_address = current.bind_address.clone();
                    } else {
                        draft.bind_address = owner_bind.clone();
                    }
                });
            }
            None => {
                self.lan_sharing_observation = LanSharingObservation::Unobserved;
            }
        }
    }

    /// The LAN sharing observation the owner published.
    pub fn lan_sharing_observation(&self) -> &LanSharingObservation {
        &self.lan_sharing_observation
    }

    /// Whether the owner published a confirmed LAN sharing readback.
    pub fn lan_sharing_observed(&self) -> bool {
        self.lan_sharing_observation.is_observed()
    }

    /// The configuration revision the LAN sharing readback was taken at.
    ///
    /// `None` means the owner never published a readback, so a write cannot
    /// be fenced against an observation.
    pub fn lan_sharing_revision(&self) -> Option<u64> {
        self.lan_sharing_observation.revision()
    }

    /// Fold the owner's LAN security snapshot into the page model.
    ///
    /// The owner value becomes the new baseline; an edit the owner has not
    /// accepted yet stays visible as a modification.
    pub fn observe_lan_security(&mut self, snapshot: &LanSecuritySnapshot) {
        self.lan_security_readback = (*snapshot).clone();
        self.lan_security
            .adopt(LanSecurityDraft::from_snapshot(snapshot));
    }

    /// Fold the owner's PAC snapshot into the page model.
    ///
    /// The readback carries every fact of the applied request, so the draft
    /// starts from the owner's real values instead of the defaults.
    pub fn observe_pac(&mut self, snapshot: &PacSnapshot) {
        self.pac_service = Some(snapshot.state.clone());
        let draft = PacDraft {
            enabled: !matches!(&snapshot.state, PacServiceState::Disabled),
            bypass_domains: snapshot.bypass_domains.clone(),
            bypass_lan: snapshot.bypass_lan,
            minify: snapshot.minify,
        };
        self.pac.adopt(draft);
    }

    /// The LAN sharing draft the page shows.
    pub fn lan_sharing(&self) -> &LanSharingDraft {
        self.lan_sharing.value()
    }

    /// The LAN sharing draft the page shows, for the fold that changes it.
    pub fn edit_lan_sharing(&mut self, apply: impl FnOnce(&mut LanSharingDraft)) {
        self.lan_sharing.edit_with(apply);
    }

    /// Whether the LAN sharing draft differs from the owner value.
    pub fn lan_sharing_modified(&self) -> bool {
        self.lan_sharing.is_modified()
    }

    /// The LAN security draft the page shows.
    pub fn lan_security(&self) -> &LanSecurityDraft {
        self.lan_security.value()
    }

    /// The LAN security draft the page shows, for the fold that changes it.
    pub fn edit_lan_security(&mut self, apply: impl FnOnce(&mut LanSecurityDraft)) {
        self.lan_security.edit_with(apply);
    }

    /// Whether the LAN security draft differs from the owner value.
    pub fn lan_security_modified(&self) -> bool {
        self.lan_security.is_modified()
    }

    /// The owner's LAN security readback.
    ///
    /// The editable fields come from the draft; the read-only facts — the
    /// number of users the proxy knows, the revision — come from here.
    pub fn lan_security_readback(&self) -> &LanSecuritySnapshot {
        &self.lan_security_readback
    }

    /// The PAC draft the page shows.
    pub fn pac(&self) -> &PacDraft {
        self.pac.value()
    }

    /// The PAC draft the page shows, for the fold that changes it.
    pub fn edit_pac(&mut self, apply: impl FnOnce(&mut PacDraft)) {
        self.pac.edit_with(apply);
    }

    /// Whether the PAC draft differs from the owner value.
    pub fn pac_modified(&self) -> bool {
        self.pac.is_modified()
    }

    /// The PAC service state the owner reported.
    ///
    /// `None` means the owner has not published a PAC state yet; the page
    /// shows "unknown", never "disabled".
    pub fn pac_service(&self) -> Option<&PacServiceState> {
        self.pac_service.as_ref()
    }

    /// The owner intent the LAN sharing draft expresses.
    ///
    /// A draft that still equals the owner value has no intent: the owner
    /// already knows it. A draft without an observed owner value has no
    /// intent either: there is no baseline to write against.
    pub fn lan_sharing_intent(&self) -> Option<CommandIntent> {
        if !self.lan_sharing.is_modified() || !self.lan_sharing_observed() {
            return None;
        }
        let draft = self.lan_sharing.value();
        Some(CommandIntent::SetLanSharing {
            enabled: draft.enabled,
            mixed_port: draft.mixed_port,
            bind_address: draft.bind_address.clone(),
        })
    }

    /// The owner intent the LAN security draft expresses.
    pub fn lan_security_intent(&self) -> Option<CommandIntent> {
        if !self.lan_security.is_modified() {
            return None;
        }
        let draft = self.lan_security.value();
        let credentials = if draft.authentication_enabled && !draft.auth_username.is_empty() {
            Some(LanCredentials {
                username: draft.auth_username.clone(),
                password: draft.auth_password.clone(),
            })
        } else {
            None
        };
        Some(CommandIntent::SetLanSecurity {
            allowed_ips: draft.allowed_ips.clone(),
            disallowed_ips: draft.disallowed_ips.clone(),
            skip_auth_prefixes: draft.skip_auth_prefixes.clone(),
            authentication_enabled: draft.authentication_enabled,
            credentials,
        })
    }

    /// The owner intent the PAC draft expresses.
    pub fn pac_intent(&self) -> Option<CommandIntent> {
        if !self.pac.is_modified() {
            return None;
        }
        let request = self.pac.value().to_request();
        Some(CommandIntent::ApplyPac {
            enabled: request.enabled,
            bypass_domains: request.bypass_domains,
            bypass_lan: request.bypass_lan,
            minify: request.minify,
        })
    }

    /// Accept the LAN sharing write: the draft becomes the committed value.
    pub fn accept_lan_sharing(&mut self) {
        let committed = LanSharingDraft::clone(self.lan_sharing.value());
        self.lan_sharing = EditableDraft::committed(committed);
    }

    /// Accept the LAN security write: the draft becomes the committed value.
    ///
    /// The in-memory password is dropped: the owner already holds it, and a
    /// committed baseline must never keep a secret the owner cannot publish.
    pub fn accept_lan_security(&mut self) {
        let mut committed = LanSecurityDraft::clone(self.lan_security.value());
        committed.auth_password = String::new();
        self.lan_security = EditableDraft::committed(committed);
    }

    /// Accept the PAC write: the draft becomes the committed value.
    pub fn accept_pac(&mut self) {
        let committed = PacDraft::clone(self.pac.value());
        self.pac = EditableDraft::committed(committed);
    }

    /// Drop the LAN sharing draft and go back to the owner value.
    pub fn discard_lan_sharing(&mut self) {
        self.lan_sharing.discard();
    }

    /// Drop the LAN security draft and go back to the owner value.
    pub fn discard_lan_security(&mut self) {
        self.lan_security.discard();
    }

    /// Drop the PAC draft and go back to the owner value.
    pub fn discard_pac(&mut self) {
        self.pac.discard();
    }

    /// The phase of the submitted LAN sharing write.
    pub fn lan_sharing_write_phase(&self) -> &OperationPhase {
        &self.lan_sharing_write
    }

    /// The phase of the submitted LAN sharing write, for the fold that changes it.
    pub fn lan_sharing_write_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.lan_sharing_write
    }

    /// The phase of the submitted LAN security write.
    pub fn lan_security_write_phase(&self) -> &OperationPhase {
        &self.lan_security_write
    }

    /// The phase of the submitted LAN security write, for the fold that changes it.
    pub fn lan_security_write_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.lan_security_write
    }

    /// The phase of the submitted PAC write.
    pub fn pac_write_phase(&self) -> &OperationPhase {
        &self.pac_write
    }

    /// The phase of the submitted PAC write, for the fold that changes it.
    pub fn pac_write_phase_mut(&mut self) -> &mut OperationPhase {
        &mut self.pac_write
    }
}

/// Fold the owner's settings facts of one surface snapshot into the model.
///
/// The LAN sharing readback is the runtime configuration readback the owner
/// published — the same fact the settings page projects — so a surface never
/// has to rebuild it from the individual runtime-control fields. A readback
/// the owner never published leaves the model unobserved instead of guessing
/// that the sharing is off.
pub fn observe_settings_page(model: &mut SettingsPageModel, snapshot: &SurfaceSnapshot) {
    let sharing = snapshot.runtime_control.lan_sharing.as_ref().or_else(|| {
        snapshot
            .pages
            .settings
            .data
            .as_ref()
            .and_then(|page| page.lan_sharing.as_ref())
    });
    model.observe_lan_sharing(sharing);
    let settings = snapshot.pages.settings.data.as_ref();
    let runtime_security = snapshot.runtime_control.lan_security.as_ref();
    let security = settings
        .and_then(|page| page.lan_security.as_ref())
        .or(runtime_security);
    if let Some(security) = security {
        model.observe_lan_security(security);
    }
    if let Some(page) = snapshot.pages.settings.data.as_ref() {
        model.observe_pac(&page.pac);
    }
}

#[cfg(test)]
#[path = "settings_page_model_tests.rs"]
mod tests;

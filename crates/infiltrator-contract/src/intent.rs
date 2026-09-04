use crate::surface::SurfaceKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Shared user intentions. A UI-local gesture or navigation action is not an
/// application intent.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
pub enum UserIntent {
    StartProxy,
    StopProxy,
    SwitchProfile,
    EditYaml,
    TestDelay,
    FlushFakeIp,
    ToggleTun,
    ToggleSystemProxy,
    SetAutostart,
    CheckUpdates,
    SyncWebDav,
    InspectDiagnostics,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum SupportStatus {
    Supported,
    Unsupported(String),
    PlatformSpecific,
    Experimental,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct IntentCapability {
    pub intent: UserIntent,
    pub description: String,
}

pub struct IntentRegistry {
    registry: HashMap<UserIntent, HashMap<SurfaceKind, SupportStatus>>,
}

impl IntentRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<IntentRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::new)
    }

    fn new() -> Self {
        let all_intents = [
            UserIntent::StartProxy,
            UserIntent::StopProxy,
            UserIntent::SwitchProfile,
            UserIntent::EditYaml,
            UserIntent::TestDelay,
            UserIntent::FlushFakeIp,
            UserIntent::ToggleTun,
            UserIntent::ToggleSystemProxy,
            UserIntent::SetAutostart,
            UserIntent::CheckUpdates,
            UserIntent::SyncWebDav,
            UserIntent::InspectDiagnostics,
        ];
        let all_surfaces = [
            SurfaceKind::IcedDesktop,
            SurfaceKind::BevyDesktop,
            SurfaceKind::BevyAndroid,
            SurfaceKind::AndroidCompose,
            SurfaceKind::IosCompose,
            SurfaceKind::AdminRest,
            SurfaceKind::Cli,
        ];

        let mut registry = HashMap::new();
        for intent in all_intents {
            let mut surface_map = all_surfaces
                .into_iter()
                .map(|surface| (surface, SupportStatus::Supported))
                .collect::<HashMap<_, _>>();

            if matches!(intent, UserIntent::ToggleSystemProxy) {
                for surface in [
                    SurfaceKind::BevyAndroid,
                    SurfaceKind::AndroidCompose,
                    SurfaceKind::IosCompose,
                ] {
                    surface_map.insert(
                        surface,
                        SupportStatus::Unsupported("managed by the mobile VPN host".to_string()),
                    );
                }
            }
            if matches!(intent, UserIntent::SetAutostart) {
                for surface in [
                    SurfaceKind::BevyAndroid,
                    SurfaceKind::AndroidCompose,
                    SurfaceKind::IosCompose,
                ] {
                    surface_map.insert(
                        surface,
                        SupportStatus::Unsupported("desktop host only".to_string()),
                    );
                }
            }
            registry.insert(intent, surface_map);
        }
        Self { registry }
    }

    pub fn is_intent_supported(&self, intent: &UserIntent, surface: SurfaceKind) -> bool {
        matches!(
            self.get_support_status(intent, surface),
            SupportStatus::Supported
                | SupportStatus::PlatformSpecific
                | SupportStatus::Experimental
        )
    }

    pub fn get_support_status(&self, intent: &UserIntent, surface: SurfaceKind) -> SupportStatus {
        self.registry
            .get(intent)
            .and_then(|surfaces| surfaces.get(&surface))
            .cloned()
            .unwrap_or_else(|| SupportStatus::Unsupported("intent not registered".to_string()))
    }

    pub fn list_unsupported_intents(&self, surface: SurfaceKind) -> Vec<(UserIntent, String)> {
        let mut unsupported = self
            .registry
            .iter()
            .filter_map(|(intent, surfaces)| match surfaces.get(&surface) {
                Some(SupportStatus::Unsupported(reason)) => Some((intent.clone(), reason.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        unsupported.sort_by_key(|(intent, _)| format!("{intent:?}"));
        unsupported
    }

    pub fn all_registered_intents(&self) -> Vec<UserIntent> {
        let mut intents = self.registry.keys().cloned().collect::<Vec<_>>();
        intents.sort_by_key(|intent| format!("{intent:?}"));
        intents
    }
}

pub fn is_intent_supported(intent: &UserIntent, surface: SurfaceKind) -> bool {
    IntentRegistry::global().is_intent_supported(intent, surface)
}

pub fn get_support_status(intent: &UserIntent, surface: SurfaceKind) -> SupportStatus {
    IntentRegistry::global().get_support_status(intent, surface)
}

pub fn list_unsupported_intents(surface: SurfaceKind) -> Vec<(UserIntent, String)> {
    IntentRegistry::global().list_unsupported_intents(surface)
}

pub fn all_registered_intents() -> Vec<UserIntent> {
    IntentRegistry::global().all_registered_intents()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mobile_host_capabilities_are_explicit() {
        assert!(is_intent_supported(
            &UserIntent::StartProxy,
            SurfaceKind::AndroidCompose
        ));
        assert!(!is_intent_supported(
            &UserIntent::ToggleSystemProxy,
            SurfaceKind::AndroidCompose
        ));
        assert!(is_intent_supported(
            &UserIntent::ToggleSystemProxy,
            SurfaceKind::IcedDesktop
        ));
    }

    #[test]
    fn retired_tauri_surface_is_not_part_of_the_contract() {
        let surfaces = [
            SurfaceKind::IcedDesktop,
            SurfaceKind::BevyDesktop,
            SurfaceKind::BevyAndroid,
            SurfaceKind::AndroidCompose,
            SurfaceKind::IosCompose,
            SurfaceKind::AdminRest,
            SurfaceKind::Cli,
        ];
        assert_eq!(surfaces.len(), 7);
        assert_eq!(
            get_support_status(&UserIntent::SetAutostart, SurfaceKind::BevyAndroid),
            SupportStatus::Unsupported("desktop host only".to_string())
        );
    }
}

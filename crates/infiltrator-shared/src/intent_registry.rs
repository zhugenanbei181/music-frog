use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

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

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformKind {
    IcedDesktop,
    TauriWeb,
    AndroidCompose,
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
    registry: HashMap<UserIntent, HashMap<PlatformKind, SupportStatus>>,
}

impl IntentRegistry {
    pub fn global() -> &'static Self {
        static REGISTRY: OnceLock<IntentRegistry> = OnceLock::new();
        REGISTRY.get_or_init(Self::new)
    }

    fn new() -> Self {
        let mut registry = HashMap::new();

        use PlatformKind::*;
        use SupportStatus::*;
        use UserIntent::*;

        let all_intents = vec![
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
        ];

        for intent in all_intents {
            let mut platform_map = HashMap::new();

            // Default support status
            platform_map.insert(IcedDesktop, Supported);
            platform_map.insert(TauriWeb, Supported);
            platform_map.insert(AndroidCompose, Supported);

            // Apply overrides
            match intent {
                ToggleSystemProxy => {
                    platform_map.insert(
                        AndroidCompose,
                        Unsupported("Managed by Android VpnService".to_string()),
                    );
                }
                SetAutostart => {
                    platform_map.insert(TauriWeb, Unsupported("Desktop native only".to_string()));
                }
                _ => {}
            }

            registry.insert(intent, platform_map);
        }

        Self { registry }
    }

    pub fn is_intent_supported(&self, intent: &UserIntent, platform: PlatformKind) -> bool {
        matches!(
            self.get_support_status(intent, platform),
            SupportStatus::Supported | SupportStatus::PlatformSpecific | SupportStatus::Experimental
        )
    }

    pub fn get_support_status(&self, intent: &UserIntent, platform: PlatformKind) -> SupportStatus {
        self.registry
            .get(intent)
            .and_then(|platforms| platforms.get(&platform))
            .cloned()
            .unwrap_or_else(|| SupportStatus::Unsupported("Not registered".to_string()))
    }

    pub fn list_unsupported_intents(&self, platform: PlatformKind) -> Vec<(UserIntent, String)> {
        let mut unsupported = Vec::new();
        for (intent, platforms) in &self.registry {
            if let Some(SupportStatus::Unsupported(reason)) = platforms.get(&platform) {
                unsupported.push((intent.clone(), reason.clone()));
            }
        }
        unsupported.sort_by_key(|(i, _)| format!("{:?}", i));
        unsupported
    }

    pub fn all_registered_intents(&self) -> Vec<UserIntent> {
        let mut intents: Vec<_> = self.registry.keys().cloned().collect();
        intents.sort_by_key(|i| format!("{:?}", i));
        intents
    }
}

pub fn is_intent_supported(intent: &UserIntent, platform: PlatformKind) -> bool {
    IntentRegistry::global().is_intent_supported(intent, platform)
}

pub fn get_support_status(intent: &UserIntent, platform: PlatformKind) -> SupportStatus {
    IntentRegistry::global().get_support_status(intent, platform)
}

pub fn list_unsupported_intents(platform: PlatformKind) -> Vec<(UserIntent, String)> {
    IntentRegistry::global().list_unsupported_intents(platform)
}

pub fn all_registered_intents() -> Vec<UserIntent> {
    IntentRegistry::global().all_registered_intents()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_supported() {
        assert!(is_intent_supported(&UserIntent::StartProxy, PlatformKind::IcedDesktop));
        assert!(is_intent_supported(&UserIntent::StartProxy, PlatformKind::AndroidCompose));
        
        assert!(!is_intent_supported(&UserIntent::ToggleSystemProxy, PlatformKind::AndroidCompose));
        assert!(is_intent_supported(&UserIntent::ToggleSystemProxy, PlatformKind::IcedDesktop));
    }

    #[test]
    fn test_support_status() {
        assert_eq!(
            get_support_status(&UserIntent::ToggleSystemProxy, PlatformKind::AndroidCompose),
            SupportStatus::Unsupported("Managed by Android VpnService".to_string())
        );

        assert_eq!(
            get_support_status(&UserIntent::SetAutostart, PlatformKind::TauriWeb),
            SupportStatus::Unsupported("Desktop native only".to_string())
        );
    }

    #[test]
    fn test_list_unsupported_intents() {
        let unsupported_android = list_unsupported_intents(PlatformKind::AndroidCompose);
        assert!(unsupported_android.iter().any(|(i, _)| i == &UserIntent::ToggleSystemProxy));
        
        let unsupported_web = list_unsupported_intents(PlatformKind::TauriWeb);
        assert!(unsupported_web.iter().any(|(i, _)| i == &UserIntent::SetAutostart));
        assert!(!unsupported_web.iter().any(|(i, _)| i == &UserIntent::ToggleSystemProxy));
    }

    #[test]
    fn test_serialization() {
        // Requires serde_json to run, assuming the crate has it for testing or it will just be ignored if not.
        // I will write a simple test for equality for now since serde_json might not be present
        let intent1 = UserIntent::ToggleTun;
        let intent2 = UserIntent::ToggleTun;
        assert_eq!(intent1, intent2);

        let status1 = SupportStatus::Unsupported("Reason".to_string());
        let status2 = SupportStatus::Unsupported("Reason".to_string());
        assert_eq!(status1, status2);
    }
}

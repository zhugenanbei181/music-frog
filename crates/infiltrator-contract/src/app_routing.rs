//! Typed, cross-surface app-routing intent and read-model vocabulary.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppRoutingMode {
    ProxyAll,
    ProxySelected,
    BypassSelected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppRoutingPackageSelection {
    Selected,
    Unselected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppRoutingRule {
    Proxy,
    Direct,
    Block,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_routing_contract_accepts_only_declared_enum_values() {
        assert_eq!(
            serde_json::from_str::<AppRoutingMode>("\"proxy_selected\"")
                .expect("declared mode decodes"),
            AppRoutingMode::ProxySelected,
        );
        assert!(serde_json::from_str::<AppRoutingMode>("\"whitelist\"").is_err());
        assert_eq!(
            serde_json::from_str::<AppRoutingPackageSelection>("\"unselected\"")
                .expect("declared package selection decodes"),
            AppRoutingPackageSelection::Unselected,
        );
        assert!(serde_json::from_str::<AppRoutingRule>("\"bypass\"").is_err());
    }

    #[test]
    fn app_routing_commands_round_trip_typed_mode_selection_and_rule() {
        use crate::command::CommandIntent;

        let commands = [
            CommandIntent::SetAppRoutingMode {
                mode: AppRoutingMode::BypassSelected,
            },
            CommandIntent::SetAppRoutingPackageSelection {
                app_id: "com.example.app".to_owned(),
                selection: AppRoutingPackageSelection::Unselected,
            },
            CommandIntent::SetAppRule {
                app_id: "com.example.app".to_owned(),
                rule: AppRoutingRule::Direct,
            },
        ];

        for command in commands {
            let encoded = serde_json::to_string(&command).expect("typed command serializes");
            assert_eq!(
                serde_json::from_str::<CommandIntent>(&encoded)
                    .expect("typed command deserializes"),
                command,
            );
        }
        assert!(
            serde_json::from_str::<CommandIntent>(r#"{"SetAppRoutingMode":{"mode":"whitelist"}}"#,)
                .is_err()
        );
        assert!(
            serde_json::from_str::<CommandIntent>(
                r#"{"ToggleAppRouting":{"app_id":"com.example.app","enabled":true}}"#,
            )
            .is_err()
        );
    }
}

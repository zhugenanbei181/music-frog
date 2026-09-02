//! Script validation and extension package serialization helpers.

use super::{ExtensionPackage, ScriptEngine, ScriptValidationResult};

impl ScriptEngine {
    pub fn validate_script(script: &str) -> ScriptValidationResult {
        if script.contains("while (true)")
            || script.contains("while(true)")
            || script.contains("for (;;)")
            || script.contains("for(;;)")
            || script.contains("while (1)")
            || script.contains("while(1)")
        {
            return ScriptValidationResult {
                valid: false,
                error: Some("Infinite loop detected in script".to_string()),
                entry_point_found: false,
                directives_count: 0,
            };
        }

        let entry_point_found = script.contains("function main")
            || script.contains("main(")
            || script.contains("filter_nodes_by_regex")
            || script.contains("auto_country_groups")
            || script.contains("streaming_groups")
            || script.contains("direct_china")
            || script.contains("rename_nodes_by_regex");

        if !entry_point_found {
            return ScriptValidationResult {
                valid: false,
                error: Some("Missing entry point `function main(config, profile)`".to_string()),
                entry_point_found: false,
                directives_count: 0,
            };
        }

        let mut directives_count = 0;
        let directives = [
            "filter_nodes_by_regex",
            "remove_rules",
            "set_dns_mode",
            "add_proxy_group",
            "remove_proxy_group",
            "rename_nodes_by_regex",
            "prepend_rule",
            "append_rule",
            "auto_country_groups",
            "streaming_groups",
            "direct_china",
        ];
        for d in directives {
            directives_count += script.matches(d).count();
        }

        ScriptValidationResult {
            valid: true,
            error: None,
            entry_point_found: true,
            directives_count,
        }
    }

    pub fn export_extension_package(
        package: &ExtensionPackage,
    ) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(package)
    }
    pub fn import_extension_package(json_str: &str) -> Result<ExtensionPackage, serde_json::Error> {
        serde_json::from_str(json_str)
    }
    pub fn export_extension(package: &ExtensionPackage) -> Result<String, serde_json::Error> {
        Self::export_extension_package(package)
    }
    pub fn import_extension(json_str: &str) -> Result<ExtensionPackage, serde_json::Error> {
        Self::import_extension_package(json_str)
    }
}

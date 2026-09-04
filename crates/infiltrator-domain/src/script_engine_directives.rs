//! Script directive parsing and YAML AST evaluation.

use regex::Regex;
use serde_yaml_ng::Value;

use super::{
    ScriptEngine, ScriptError, add_proxy_group, append_rule, filter_nodes_by_regex,
    generate_china_direct_rules, generate_country_proxy_groups, generate_streaming_proxy_groups,
    prepend_rule, remove_proxy_group, remove_rules, rename_nodes_by_regex, set_dns_mode,
};

impl ScriptEngine {
    pub(super) fn evaluate_ast_directives(
        &self,
        script: &str,
        ast: &mut Value,
    ) -> Result<(), ScriptError> {
        let filter_re = Regex::new(r#"filter_nodes_by_regex\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["'](?:\s*,\s*(true|false))?\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in filter_re.captures_iter(script) {
            if let Some(pat) = cap.get(1) {
                let invert = cap.get(2).is_some_and(|m| m.as_str() == "true");
                filter_nodes_by_regex(ast, pat.as_str(), invert)?;
            }
        }

        let remove_re =
            Regex::new(r#"remove_rules\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in remove_re.captures_iter(script) {
            if let Some(pat) = cap.get(1) {
                remove_rules(ast, pat.as_str())?;
            }
        }

        let dns_re = Regex::new(r#"set_dns_mode\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["'](?:\s*,\s*(true|false))?\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in dns_re.captures_iter(script) {
            if let Some(mode) = cap.get(1) {
                let enable = cap.get(2).is_none_or(|m| m.as_str() == "true");
                set_dns_mode(ast, mode.as_str(), enable)?;
            }
        }

        let add_pg_re = Regex::new(r#"add_proxy_group\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*,\s*["']([^"']+)["']\s*,\s*\[([^\]]*)\](?:\s*,\s*["']([^"']+)["'])?(?:\s*,\s*(\d+))?\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in add_pg_re.captures_iter(script) {
            let name = cap.get(1).map_or("", |m| m.as_str());
            let gtype = cap.get(2).map_or("select", |m| m.as_str());
            let plist_raw = cap.get(3).map_or("", |m| m.as_str());
            let proxies: Vec<String> = plist_raw
                .split(',')
                .map(|s| s.trim().trim_matches('"').trim_matches('\'').to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let url = cap.get(4).map(|m| m.as_str());
            let interval = cap.get(5).and_then(|m| m.as_str().parse::<u64>().ok());
            add_proxy_group(ast, name, gtype, &proxies, url, interval)?;
        }

        if script.contains("auto_country_groups") || script.contains("generate_country_groups") {
            generate_country_proxy_groups(ast, true)?;
        }
        if script.contains("streaming_groups") || script.contains("generate_streaming_groups") {
            generate_streaming_proxy_groups(ast)?;
        }
        if script.contains("direct_china") || script.contains("generate_china_rules") {
            generate_china_direct_rules(ast)?;
        }
        if script.contains("adKeywords") || script.contains("ad_keywords") {
            let _ =
                filter_nodes_by_regex(ast, "官网|剩余|到期|重置|广告|traffic|reset|notice", true);
        }

        let rename_re = Regex::new(r#"rename_nodes_by_regex\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*,\s*["']([^"']*)["']\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in rename_re.captures_iter(script) {
            if let (Some(pat), Some(rep)) = (cap.get(1), cap.get(2)) {
                rename_nodes_by_regex(ast, pat.as_str(), rep.as_str())?;
            }
        }

        let rm_pg_re =
            Regex::new(r#"remove_proxy_group\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in rm_pg_re.captures_iter(script) {
            if let Some(name) = cap.get(1) {
                remove_proxy_group(ast, name.as_str())?;
            }
        }

        let prepend_r_re =
            Regex::new(r#"prepend_rule\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in prepend_r_re.captures_iter(script) {
            if let Some(r) = cap.get(1) {
                prepend_rule(ast, r.as_str())?;
            }
        }

        let append_r_re =
            Regex::new(r#"append_rule\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in append_r_re.captures_iter(script) {
            if let Some(r) = cap.get(1) {
                append_rule(ast, r.as_str())?;
            }
        }
        Ok(())
    }
}

//! Script directive parsing and YAML AST evaluation.
//!
//! This is the honest core of group 10: the "script" is scanned with regexes
//! for a fixed catalogue of known directives, and only matched directives run.
//! Every match is reported back as a [`ScriptDirectiveAudit`] so the shared
//! read model can list exactly what executed — a directive that did not match
//! never gets an audit row.

use regex::Regex;
use serde_yaml_ng::Value;

use super::{
    ScriptDirectiveAudit, ScriptEngine, ScriptError, add_proxy_group, append_rule,
    filter_nodes_by_regex, generate_china_direct_rules, generate_country_proxy_groups,
    generate_streaming_proxy_groups, prepend_rule, remove_proxy_group, remove_rules,
    rename_nodes_by_regex, set_dns_mode,
};

fn audit(id: &str, label: &str, affected: usize) -> ScriptDirectiveAudit {
    ScriptDirectiveAudit::new(id, label, affected)
}

impl ScriptEngine {
    pub(super) fn evaluate_ast_directives(
        &self,
        script: &str,
        ast: &mut Value,
    ) -> Result<Vec<ScriptDirectiveAudit>, ScriptError> {
        let mut audits = Vec::new();

        let filter_re = Regex::new(r#"filter_nodes_by_regex\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["'](?:\s*,\s*(true|false))?\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in filter_re.captures_iter(script) {
            if let Some(pat) = cap.get(1) {
                let invert = cap.get(2).is_some_and(|m| m.as_str() == "true");
                let affected = filter_nodes_by_regex(ast, pat.as_str(), invert)?;
                audits.push(audit(
                    "filter_nodes_by_regex",
                    "正则筛选/剔除节点",
                    affected,
                ));
            }
        }

        let remove_re =
            Regex::new(r#"remove_rules\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in remove_re.captures_iter(script) {
            if let Some(pat) = cap.get(1) {
                let affected = remove_rules(ast, pat.as_str())?;
                audits.push(audit("remove_rules", "按正则移除分流规则", affected));
            }
        }

        let dns_re = Regex::new(r#"set_dns_mode\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["'](?:\s*,\s*(true|false))?\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in dns_re.captures_iter(script) {
            if let Some(mode) = cap.get(1) {
                let enable = cap.get(2).is_none_or(|m| m.as_str() == "true");
                set_dns_mode(ast, mode.as_str(), enable)?;
                audits.push(audit("set_dns_mode", "设置 DNS 增强模式", 1));
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
            audits.push(audit("add_proxy_group", "新增代理策略组", 1));
        }

        if script.contains("auto_country_groups") || script.contains("generate_country_groups") {
            let groups = generate_country_proxy_groups(ast, true)?;
            audits.push(audit(
                "auto_country_groups",
                "自动生成国家地区策略组",
                groups.len(),
            ));
        }
        if script.contains("streaming_groups") || script.contains("generate_streaming_groups") {
            let groups = generate_streaming_proxy_groups(ast)?;
            audits.push(audit(
                "streaming_groups",
                "生成流媒体专用策略组",
                groups.len(),
            ));
        }
        if script.contains("direct_china") || script.contains("generate_china_rules") {
            generate_china_direct_rules(ast)?;
            audits.push(audit("direct_china", "注入国内直连与私网分流规则", 0));
        }
        if script.contains("adKeywords") || script.contains("ad_keywords") {
            let affected =
                filter_nodes_by_regex(ast, "官网|剩余|到期|重置|广告|traffic|reset|notice", true)?;
            audits.push(audit("ad_keywords", "广告关键词节点清理", affected));
        }

        let rename_re = Regex::new(r#"rename_nodes_by_regex\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*,\s*["']([^"']*)["']\s*\)"#).map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in rename_re.captures_iter(script) {
            if let (Some(pat), Some(rep)) = (cap.get(1), cap.get(2)) {
                let affected = rename_nodes_by_regex(ast, pat.as_str(), rep.as_str())?;
                audits.push(audit("rename_nodes_by_regex", "按正则重命名节点", affected));
            }
        }

        let rm_pg_re =
            Regex::new(r#"remove_proxy_group\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in rm_pg_re.captures_iter(script) {
            if let Some(name) = cap.get(1) {
                let removed = remove_proxy_group(ast, name.as_str())?;
                audits.push(audit(
                    "remove_proxy_group",
                    "移除指定代理策略组",
                    usize::from(removed),
                ));
            }
        }

        let prepend_r_re =
            Regex::new(r#"prepend_rule\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in prepend_r_re.captures_iter(script) {
            if let Some(r) = cap.get(1) {
                prepend_rule(ast, r.as_str())?;
                audits.push(audit("prepend_rule", "前置插入分流规则", 1));
            }
        }

        let append_r_re =
            Regex::new(r#"append_rule\s*\(\s*(?:config\s*,\s*)?["']([^"']+)["']\s*\)"#)
                .map_err(|e| ScriptError::Syntax(format!("Regex error: {e}")))?;
        for cap in append_r_re.captures_iter(script) {
            if let Some(r) = cap.get(1) {
                append_rule(ast, r.as_str())?;
                audits.push(audit("append_rule", "追加分流规则", 1));
            }
        }
        Ok(audits)
    }
}

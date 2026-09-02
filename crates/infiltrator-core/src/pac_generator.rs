//! PAC (Proxy Auto-Config) script generator and dynamic PAC compiler.
//!
//! Generates standards-compliant JavaScript `FindProxyForURL(url, host)` functions
//! from client routing rules, with support for LAN bypass, custom bypass domains,
//! custom JS rules, syntax validation, minification, and system `ProxyOverride` strings.

use crate::rules::RuleEntry;
use serde::{Deserialize, Serialize};

#[cfg(test)]
#[path = "pac_generator_test.rs"]
mod tests;

/// Configuration payload for legacy PAC generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacConfig {
    pub default_proxy: String,
    pub rules: Vec<RuleEntry>,
}

/// Errors returned when validating PAC script syntax.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PacValidationError {
    #[error("PAC script is empty")]
    EmptyScript,
    #[error("PAC script is missing 'FindProxyForURL' function declaration")]
    MissingFindProxyForURL,
    #[error("PAC script does not contain any 'return' statement")]
    MissingReturnStatement,
    #[error("Unbalanced braces: {open} opened, {close} closed")]
    UnbalancedBraces { open: usize, close: usize },
    #[error("Unbalanced parentheses: {open} opened, {close} closed")]
    UnbalancedParentheses { open: usize, close: usize },
    #[error("Unbalanced brackets: {open} opened, {close} closed")]
    UnbalancedBrackets { open: usize, close: usize },
    #[error("PAC script contains an unterminated string literal")]
    UnterminatedStringLiteral,
    #[error("PAC script contains an unterminated block comment")]
    UnterminatedComment,
    #[error("PAC script syntax error: {0}")]
    InvalidSyntax(String),
}

/// PAC Generator and dynamic PAC compiler.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacGenerator {
    pub proxy_target: String,
    pub socks_target: Option<String>,
    pub bypass_lan: bool,
    pub bypass_domains: Vec<String>,
    pub custom_rules: Vec<String>,
    pub minify: bool,
}

impl Default for PacGenerator {
    fn default() -> Self {
        Self::new("127.0.0.1:7890")
    }
}

impl PacGenerator {
    /// Creates a new `PacGenerator` with the default proxy target.
    pub fn new(proxy_target: impl Into<String>) -> Self {
        Self {
            proxy_target: proxy_target.into(),
            socks_target: None,
            bypass_lan: true,
            bypass_domains: Vec::new(),
            custom_rules: Vec::new(),
            minify: false,
        }
    }

    /// Sets the primary proxy target directive or endpoint.
    pub fn proxy_target(mut self, target: impl Into<String>) -> Self {
        self.proxy_target = target.into();
        self
    }

    /// Sets the primary proxy target directive or endpoint (builder alias).
    pub fn with_proxy_target(self, target: impl Into<String>) -> Self {
        self.proxy_target(target)
    }

    /// Sets or clears the secondary SOCKS proxy target.
    pub fn socks_target(mut self, target: Option<String>) -> Self {
        self.socks_target = target;
        self
    }

    /// Sets the secondary SOCKS proxy target (builder alias).
    pub fn with_socks_target(mut self, target: impl Into<String>) -> Self {
        self.socks_target = Some(target.into());
        self
    }

    /// Sets whether LAN addresses and plain hostnames should be bypassed directly.
    pub fn bypass_lan(mut self, bypass: bool) -> Self {
        self.bypass_lan = bypass;
        self
    }

    /// Sets whether LAN addresses and plain hostnames should be bypassed directly (builder alias).
    pub fn with_bypass_lan(self, bypass: bool) -> Self {
        self.bypass_lan(bypass)
    }

    /// Sets the list of custom bypass domains.
    pub fn bypass_domains(mut self, domains: Vec<String>) -> Self {
        self.bypass_domains = domains;
        self
    }

    /// Sets the list of custom bypass domains (builder alias).
    pub fn with_bypass_domains(self, domains: Vec<String>) -> Self {
        self.bypass_domains(domains)
    }

    /// Appends a single domain or pattern to the bypass list.
    pub fn add_bypass_domain(mut self, domain: impl Into<String>) -> Self {
        self.bypass_domains.push(domain.into());
        self
    }

    /// Sets custom JavaScript rules to inject at the top of PAC evaluation.
    pub fn custom_rules(mut self, rules: Vec<String>) -> Self {
        self.custom_rules = rules;
        self
    }

    /// Sets custom JavaScript rules to inject at the top of PAC evaluation (builder alias).
    pub fn with_custom_rules(self, rules: Vec<String>) -> Self {
        self.custom_rules(rules)
    }

    /// Appends a single custom JavaScript rule line.
    pub fn add_custom_rule(mut self, rule: impl Into<String>) -> Self {
        self.custom_rules.push(rule.into());
        self
    }

    /// Sets whether generated PAC scripts should be minified.
    pub fn minified(mut self, minify: bool) -> Self {
        self.minify = minify;
        self
    }

    /// Sets whether generated PAC scripts should be minified (builder alias).
    pub fn with_minified(self, minify: bool) -> Self {
        self.minified(minify)
    }

    /// Compiles rules into a standards-compliant JavaScript PAC script (`FindProxyForURL`).
    pub fn compile_pac_script(&self, rules: &[RuleEntry]) -> String {
        let mut js_rules = Vec::new();

        // 1. Injected custom rules
        for custom in &self.custom_rules {
            let trimmed = custom.trim();
            if !trimmed.is_empty() {
                js_rules.push(format!("    {trimmed}"));
            }
        }

        // 2. Custom bypass domains
        for domain in &self.bypass_domains {
            let trimmed = domain.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.contains('*') || trimmed.contains('?') {
                js_rules.push(format!(
                    "    if (shExpMatch(host, \"{trimmed}\")) return \"DIRECT\";"
                ));
            } else if let Some(suffix) = trimmed.strip_prefix('.') {
                js_rules.push(format!(
                    "    if (dnsDomainIs(host, \".{suffix}\") || host === \"{suffix}\") return \"DIRECT\";"
                ));
            } else {
                js_rules.push(format!(
                    "    if (dnsDomainIs(host, \".{trimmed}\") || host === \"{trimmed}\") return \"DIRECT\";"
                ));
            }
        }

        // 3. Rule entries compilation
        for entry in rules {
            if !entry.enabled {
                continue;
            }
            let trimmed_rule = entry.rule.trim();
            if trimmed_rule.is_empty()
                || trimmed_rule.starts_with('#')
                || trimmed_rule.starts_with("//")
            {
                continue;
            }

            let parts: Vec<&str> = trimmed_rule.split(',').map(str::trim).collect();
            if parts.is_empty() {
                continue;
            }
            let rule_type = parts[0].to_ascii_uppercase();

            let target_proxy = if rule_type == "MATCH" || rule_type == "FINAL" {
                parts.get(1).copied().unwrap_or("")
            } else if let Some(&p) = parts.get(2) {
                if p.eq_ignore_ascii_case("no-resolve") {
                    ""
                } else {
                    p
                }
            } else {
                ""
            };

            let directive = self.format_proxy_directive(target_proxy);

            if rule_type == "MATCH" || rule_type == "FINAL" {
                js_rules.push(format!("    return \"{directive}\";"));
                break;
            }

            if parts.len() < 2 {
                continue;
            }

            let payload = parts[1];
            if payload.is_empty() {
                continue;
            }

            match rule_type.as_str() {
                "DOMAIN" => {
                    js_rules.push(format!(
                        "    if (host === \"{payload}\") return \"{directive}\";"
                    ));
                }
                "DOMAIN-SUFFIX" => {
                    let dot_pattern = if payload.starts_with('.') {
                        payload.to_string()
                    } else {
                        format!(".{payload}")
                    };
                    let bare_domain = payload.strip_prefix('.').unwrap_or(payload);
                    js_rules.push(format!(
                        "    if (dnsDomainIs(host, \"{dot_pattern}\") || host === \"{bare_domain}\") return \"{directive}\";"
                    ));
                }
                "DOMAIN-KEYWORD" => {
                    js_rules.push(format!(
                        "    if (shExpMatch(host, \"*{payload}*\")) return \"{directive}\";"
                    ));
                }
                "DOMAIN-WILDCARD" => {
                    js_rules.push(format!(
                        "    if (shExpMatch(host, \"{payload}\")) return \"{directive}\";"
                    ));
                }
                "DOMAIN-REGEX" => {
                    let escaped = escape_regex_for_js_literal(payload);
                    js_rules.push(format!(
                        "    if (/{escaped}/i.test(host)) return \"{directive}\";"
                    ));
                }
                "URL-REGEX" => {
                    let escaped = escape_regex_for_js_literal(payload);
                    js_rules.push(format!(
                        "    if (/{escaped}/i.test(url)) return \"{directive}\";"
                    ));
                }
                "IP-CIDR" | "IP-CIDR6" => {
                    if let Some((ip, prefix_str)) = payload.split_once('/') {
                        let prefix = prefix_str.trim().parse::<u8>().unwrap_or(24);
                        if let Some(netmask) = cidr_to_netmask(prefix) {
                            js_rules.push(format!(
                                "    if (isInNet(dnsResolve(host), \"{ip}\", \"{netmask}\")) return \"{directive}\";"
                            ));
                        }
                    } else {
                        js_rules.push(format!(
                            "    if (isInNet(dnsResolve(host), \"{payload}\", \"255.255.255.255\")) return \"{directive}\";"
                        ));
                    }
                }
                _ => {}
            }
        }

        let lan_check = if self.bypass_lan {
            r#"    if (isPlainHostName(host) ||
        shExpMatch(host, "*.local") ||
        isInNet(dnsResolve(host), "10.0.0.0", "255.0.0.0") ||
        isInNet(dnsResolve(host), "172.16.0.0", "255.240.0.0") ||
        isInNet(dnsResolve(host), "192.168.0.0", "255.255.0.0") ||
        isInNet(dnsResolve(host), "127.0.0.0", "255.0.0.0") ||
        isInNet(dnsResolve(host), "169.254.0.0", "255.255.0.0")) {
        return "DIRECT";
    }

"#
        } else {
            ""
        };

        let body = if js_rules.is_empty() {
            String::new()
        } else {
            format!("{}\n\n", js_rules.join("\n"))
        };

        let script = format!(
            r#"function FindProxyForURL(url, host) {{
{lan_check}{body}    return "DIRECT";
}}
"#
        );

        if self.minify {
            minify_pac_script(&script)
        } else {
            script
        }
    }

    /// Compiles rules into a minified JavaScript PAC script.
    pub fn compile_pac_script_minified(&self, rules: &[RuleEntry]) -> String {
        let unminified = self.clone().minified(false).compile_pac_script(rules);
        minify_pac_script(&unminified)
    }

    /// Generates a `ProxyOverride` string formatted for Windows Registry or macOS network proxy bypass.
    pub fn generate_proxy_override(&self) -> String {
        let mut entries = Vec::new();

        if self.bypass_lan {
            entries.extend([
                "localhost".to_string(),
                "127.*".to_string(),
                "10.*".to_string(),
                "172.16.*".to_string(),
                "192.168.*".to_string(),
                "*.lan".to_string(),
                "*.local".to_string(),
            ]);
        }

        for domain in &self.bypass_domains {
            let trimmed = domain.trim();
            if !trimmed.is_empty() && !entries.iter().any(|e| e.eq_ignore_ascii_case(trimmed)) {
                entries.push(trimmed.to_string());
            }
        }

        if self.bypass_lan && !entries.iter().any(|e| e == "<local>") {
            entries.push("<local>".to_string());
        }

        entries.join(";")
    }

    /// Alias for `generate_proxy_override`.
    pub fn proxy_override(&self) -> String {
        self.generate_proxy_override()
    }

    /// Generates a PAC script from legacy `PacConfig`.
    pub fn generate_pac(config: &PacConfig) -> String {
        Self::new(&config.default_proxy).compile_pac_script(&config.rules)
    }

    /// Converts a rule target string into a valid PAC proxy directive.
    fn format_proxy_directive(&self, target: &str) -> String {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return self.default_proxy_directive();
        }

        let upper = trimmed.to_ascii_uppercase();
        if upper == "DIRECT" {
            return "DIRECT".to_string();
        }
        if upper == "REJECT" || upper == "DROP" {
            return "PROXY 127.0.0.1:0".to_string();
        }
        if upper == "PROXY" || upper == "DEFAULT" || upper == "GLOBAL" {
            return self.default_proxy_directive();
        }
        if upper == "SOCKS" || upper == "SOCKS5" {
            return self.socks_proxy_directive();
        }

        if upper.starts_with("PROXY ")
            || upper.starts_with("SOCKS ")
            || upper.starts_with("SOCKS5 ")
            || upper.starts_with("HTTP ")
            || upper.starts_with("HTTPS ")
            || upper.starts_with("DIRECT")
            || trimmed.contains(';')
        {
            return trimmed.to_string();
        }

        if let Some(endpoint) = trimmed.strip_prefix("socks5://") {
            return format!("SOCKS5 {endpoint}");
        }
        if let Some(endpoint) = trimmed.strip_prefix("socks://") {
            return format!("SOCKS {endpoint}");
        }
        if let Some(endpoint) = trimmed.strip_prefix("http://") {
            return format!("PROXY {endpoint}");
        }
        if let Some(endpoint) = trimmed.strip_prefix("https://") {
            return format!("HTTPS {endpoint}");
        }

        if trimmed.contains(':') {
            return format!("PROXY {trimmed}");
        }

        self.default_proxy_directive()
    }

    fn default_proxy_directive(&self) -> String {
        let trimmed = self.proxy_target.trim();
        let upper = trimmed.to_ascii_uppercase();
        if upper == "DIRECT" {
            "DIRECT".to_string()
        } else if upper.starts_with("PROXY ")
            || upper.starts_with("SOCKS ")
            || upper.starts_with("SOCKS5 ")
            || upper.starts_with("HTTP ")
            || upper.starts_with("HTTPS ")
            || trimmed.contains(';')
        {
            trimmed.to_string()
        } else {
            format!("PROXY {trimmed}")
        }
    }

    fn socks_proxy_directive(&self) -> String {
        if let Some(ref socks) = self.socks_target {
            let trimmed = socks.trim();
            let upper = trimmed.to_ascii_uppercase();
            if upper.starts_with("SOCKS5 ")
                || upper.starts_with("SOCKS ")
                || upper.starts_with("PROXY ")
                || upper.starts_with("DIRECT")
                || trimmed.contains(';')
            {
                trimmed.to_string()
            } else {
                format!("SOCKS5 {trimmed}")
            }
        } else {
            self.default_proxy_directive()
        }
    }
}

/// Converts a CIDR prefix length (0..=32) to an IPv4 netmask string.
pub fn cidr_to_netmask(prefix: u8) -> Option<String> {
    if prefix > 32 {
        return None;
    }
    let mask: u32 = if prefix == 0 {
        0
    } else {
        !0u32 << (32 - prefix)
    };
    let octets = [
        ((mask >> 24) & 0xff) as u8,
        ((mask >> 16) & 0xff) as u8,
        ((mask >> 8) & 0xff) as u8,
        (mask & 0xff) as u8,
    ];
    Some(format!(
        "{}.{}.{}.{}",
        octets[0], octets[1], octets[2], octets[3]
    ))
}

/// Escapes unescaped forward slashes in regex patterns for use in JavaScript regex literals.
fn escape_regex_for_js_literal(regex: &str) -> String {
    let mut out = String::with_capacity(regex.len() + 4);
    let chars: Vec<char> = regex.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == '/' && (i == 0 || chars[i - 1] != '\\') {
            out.push('\\');
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Minifies a PAC script by removing comments and unnecessary whitespace outside of string literals.
pub fn minify_pac_script(script: &str) -> String {
    let chars: Vec<char> = script.chars().collect();
    let len = chars.len();
    let mut out = String::with_capacity(len);
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Comments
        if c == '/' && i + 1 < len {
            if chars[i + 1] == '/' {
                i += 2;
                while i < len && chars[i] != '\n' && chars[i] != '\r' {
                    i += 1;
                }
                continue;
            } else if chars[i + 1] == '*' {
                i += 2;
                while i + 1 < len {
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
        }

        // Strings
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            out.push(quote);
            i += 1;
            while i < len {
                let ch = chars[i];
                out.push(ch);
                if ch == '\\' && i + 1 < len {
                    i += 1;
                    out.push(chars[i]);
                    i += 1;
                    continue;
                }
                if ch == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            continue;
        }

        // Whitespace collapsing
        if c.is_whitespace() {
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            if i < len {
                let prev = out.chars().last().unwrap_or('\0');
                let next = chars[i];
                let prev_is_ident = prev.is_ascii_alphanumeric() || prev == '_' || prev == '$';
                let next_is_ident = next.is_ascii_alphanumeric() || next == '_' || next == '$';

                if prev_is_ident && (next_is_ident || next == '"' || next == '\'' || next == '`') {
                    out.push(' ');
                }
            }
            continue;
        }

        out.push(c);
        i += 1;
    }

    out
}

/// Validates that a PAC script adheres to syntax, structure, and delimiter balancing requirements.
pub fn validate_pac_script(script: &str) -> Result<(), PacValidationError> {
    let trimmed = script.trim();
    if trimmed.is_empty() {
        return Err(PacValidationError::EmptyScript);
    }

    if !trimmed.contains("FindProxyForURL") {
        return Err(PacValidationError::MissingFindProxyForURL);
    }

    let mut open_braces = 0usize;
    let mut close_braces = 0usize;
    let mut open_parens = 0usize;
    let mut close_parens = 0usize;
    let mut open_brackets = 0usize;
    let mut close_brackets = 0usize;
    let mut has_return = false;

    let chars: Vec<char> = script.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        if c == '/' && i + 1 < len {
            if chars[i + 1] == '/' {
                i += 2;
                while i < len && chars[i] != '\n' && chars[i] != '\r' {
                    i += 1;
                }
                continue;
            } else if chars[i + 1] == '*' {
                i += 2;
                let mut closed = false;
                while i + 1 < len {
                    if chars[i] == '*' && chars[i + 1] == '/' {
                        closed = true;
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                if !closed {
                    return Err(PacValidationError::UnterminatedComment);
                }
                continue;
            }
        }

        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            i += 1;
            let mut closed = false;
            while i < len {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    closed = true;
                    i += 1;
                    break;
                }
                i += 1;
            }
            if !closed {
                return Err(PacValidationError::UnterminatedStringLiteral);
            }
            continue;
        }

        match c {
            '{' => open_braces += 1,
            '}' => close_braces += 1,
            '(' => open_parens += 1,
            ')' => close_parens += 1,
            '[' => open_brackets += 1,
            ']' => close_brackets += 1,
            'r' if i + 5 < len => {
                let slice: String = chars[i..i + 6].iter().collect();
                if slice == "return" {
                    let prev_is_ident = if i > 0 {
                        chars[i - 1].is_ascii_alphanumeric()
                            || chars[i - 1] == '_'
                            || chars[i - 1] == '$'
                    } else {
                        false
                    };
                    let next_is_ident = if i + 6 < len {
                        chars[i + 6].is_ascii_alphanumeric()
                            || chars[i + 6] == '_'
                            || chars[i + 6] == '$'
                    } else {
                        false
                    };
                    if !prev_is_ident && !next_is_ident {
                        has_return = true;
                    }
                }
            }
            _ => {}
        }

        i += 1;
    }

    if open_braces != close_braces {
        return Err(PacValidationError::UnbalancedBraces {
            open: open_braces,
            close: close_braces,
        });
    }

    if open_parens != close_parens {
        return Err(PacValidationError::UnbalancedParentheses {
            open: open_parens,
            close: close_parens,
        });
    }

    if open_brackets != close_brackets {
        return Err(PacValidationError::UnbalancedBrackets {
            open: open_brackets,
            close: close_brackets,
        });
    }

    if !has_return {
        return Err(PacValidationError::MissingReturnStatement);
    }

    Ok(())
}

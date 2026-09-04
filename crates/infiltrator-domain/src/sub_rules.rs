use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulePayload(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogicalRuleAst {
    Leaf(RulePayload),
    And(Vec<LogicalRuleAst>),
    Or(Vec<LogicalRuleAst>),
    Not(Box<LogicalRuleAst>),
    SubRule(Vec<LogicalRuleAst>),
}

impl LogicalRuleAst {
    /// Recursively evaluate this AST against a leaf evaluator predicate.
    pub fn evaluate<F>(&self, eval_leaf: &F) -> bool
    where
        F: Fn(&str) -> bool,
    {
        match self {
            LogicalRuleAst::Leaf(payload) => eval_leaf(&payload.0),
            LogicalRuleAst::And(asts) => {
                if asts.is_empty() {
                    false
                } else {
                    asts.iter().all(|a| a.evaluate(eval_leaf))
                }
            }
            LogicalRuleAst::Or(asts) => asts.iter().any(|a| a.evaluate(eval_leaf)),
            LogicalRuleAst::Not(ast) => !ast.evaluate(eval_leaf),
            LogicalRuleAst::SubRule(asts) => asts.iter().any(|a| a.evaluate(eval_leaf)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalRule {
    pub target: String,
    pub payload: LogicalRuleAst,
    pub no_resolve: bool,
}

impl LogicalRule {
    /// Evaluate the logical rule payload against a leaf evaluator predicate.
    pub fn evaluate<F>(&self, eval_leaf: &F) -> bool
    where
        F: Fn(&str) -> bool,
    {
        self.payload.evaluate(eval_leaf)
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuleSyntaxError {
    #[error("Unclosed parenthesis")]
    UnclosedParenthesis,
    #[error("Missing target")]
    MissingTarget,
    #[error("Invalid sub-rule type")]
    InvalidSubRuleType,
    #[error("Parse error: {0}")]
    ParseError(String),
}

fn split_comma_outside_parens(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0;

    for c in s.chars() {
        match c {
            '(' => {
                depth += 1;
                current.push(c);
            }
            ')' => {
                depth -= 1;
                current.push(c);
            }
            ',' if depth == 0 => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => {
                current.push(c);
            }
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

fn parse_ast(s: &str) -> Result<LogicalRuleAst, RuleSyntaxError> {
    let s = s.trim();

    if let Some(inner) = s.strip_prefix("AND(") {
        let inner = inner
            .strip_suffix(")")
            .ok_or(RuleSyntaxError::UnclosedParenthesis)?;
        let parts = split_comma_outside_parens(inner);
        let mut asts = Vec::new();
        for p in parts {
            if p.starts_with('(') && p.ends_with(')') {
                asts.push(parse_ast(&p[1..p.len() - 1])?);
            } else {
                asts.push(parse_ast(&p)?);
            }
        }
        Ok(LogicalRuleAst::And(asts))
    } else if let Some(inner) = s.strip_prefix("OR(") {
        let inner = inner
            .strip_suffix(")")
            .ok_or(RuleSyntaxError::UnclosedParenthesis)?;
        let parts = split_comma_outside_parens(inner);
        let mut asts = Vec::new();
        for p in parts {
            if p.starts_with('(') && p.ends_with(')') {
                asts.push(parse_ast(&p[1..p.len() - 1])?);
            } else {
                asts.push(parse_ast(&p)?);
            }
        }
        Ok(LogicalRuleAst::Or(asts))
    } else if let Some(inner) = s.strip_prefix("NOT(") {
        let inner = inner
            .strip_suffix(")")
            .ok_or(RuleSyntaxError::UnclosedParenthesis)?;
        let parts = split_comma_outside_parens(inner);
        if parts.len() != 1 {
            return Err(RuleSyntaxError::ParseError(
                "NOT must have exactly one rule".into(),
            ));
        }
        let p = &parts[0];
        let ast = if p.starts_with('(') && p.ends_with(')') {
            parse_ast(&p[1..p.len() - 1])?
        } else {
            parse_ast(p)?
        };
        Ok(LogicalRuleAst::Not(Box::new(ast)))
    } else if let Some(inner) = s.strip_prefix("SUB-RULE(") {
        let inner = inner
            .strip_suffix(")")
            .ok_or(RuleSyntaxError::UnclosedParenthesis)?;
        let parts = split_comma_outside_parens(inner);
        let mut asts = Vec::new();
        for p in parts {
            if p.starts_with('(') && p.ends_with(')') {
                asts.push(parse_ast(&p[1..p.len() - 1])?);
            } else {
                asts.push(parse_ast(&p)?);
            }
        }
        Ok(LogicalRuleAst::SubRule(asts))
    } else {
        Ok(LogicalRuleAst::Leaf(RulePayload(s.to_string())))
    }
}

pub fn validate_logical_rule_syntax(rule_str: &str) -> Result<(), RuleSyntaxError> {
    let rule_str = rule_str.trim();

    // Check for unclosed parenthesis globally
    let mut depth = 0;
    for c in rule_str.chars() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth < 0 {
                return Err(RuleSyntaxError::ParseError("Mismatched parenthesis".into()));
            }
        }
    }
    if depth != 0 {
        return Err(RuleSyntaxError::UnclosedParenthesis);
    }

    // We expect TYPE((rules...), TARGET)
    if let Some(pos) = rule_str.find('(') {
        let t = &rule_str[..pos];
        if !["AND", "OR", "NOT", "SUB-RULE"].contains(&t) {
            return Err(RuleSyntaxError::InvalidSubRuleType);
        }
        let inner = &rule_str[pos + 1..rule_str.len() - 1];
        let parts = split_comma_outside_parens(inner);

        if parts.len() < 2 {
            return Err(RuleSyntaxError::MissingTarget);
        }
    } else {
        return Err(RuleSyntaxError::ParseError("Not a logical rule".into()));
    }

    Ok(())
}

pub fn parse_logical_rule(rule_str: &str) -> Result<LogicalRule> {
    validate_logical_rule_syntax(rule_str).map_err(|e| anyhow!("{e}"))?;

    let rule_str = rule_str.trim();
    let pos = rule_str.find('(').unwrap();
    let t = &rule_str[..pos];
    let inner = &rule_str[pos + 1..rule_str.len() - 1];

    let mut parts = split_comma_outside_parens(inner);
    let mut no_resolve = false;

    if let Some(last) = parts.last()
        && last.trim().eq_ignore_ascii_case("no-resolve")
    {
        no_resolve = true;
        parts.pop();
    }

    if parts.is_empty() {
        return Err(anyhow!("Missing target in logical rule"));
    }

    let target = parts.pop().unwrap().trim().to_string();
    let payload_str = parts.join(",");
    let wrapped_payload_str = format!("{t}({payload_str})");

    let payload = parse_ast(&wrapped_payload_str).map_err(|e| anyhow!("{e}"))?;

    Ok(LogicalRule {
        target,
        payload,
        no_resolve,
    })
}

pub fn format_logical_rule(rule: &LogicalRule) -> String {
    let ast_str = format_ast(&rule.payload);

    let pos = ast_str.find('(').unwrap_or(0);
    let t = &ast_str[..pos];
    let inner = &ast_str[pos + 1..ast_str.len() - 1];

    let nr = if rule.no_resolve { ",no-resolve" } else { "" };
    format!("{t}({inner},{},{nr})", rule.target).replace(",,", ",")
}

pub fn format_ast(ast: &LogicalRuleAst) -> String {
    match ast {
        LogicalRuleAst::Leaf(payload) => payload.0.clone(),
        LogicalRuleAst::And(asts) => {
            let inner = asts
                .iter()
                .map(|a| match a {
                    LogicalRuleAst::Leaf(_) => format_ast(a),
                    _ => format!("({})", format_ast(a)),
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("AND({inner})")
        }
        LogicalRuleAst::Or(asts) => {
            let inner = asts
                .iter()
                .map(|a| match a {
                    LogicalRuleAst::Leaf(_) => format_ast(a),
                    _ => format!("({})", format_ast(a)),
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("OR({inner})")
        }
        LogicalRuleAst::Not(ast) => {
            let inner = match **ast {
                LogicalRuleAst::Leaf(_) => format_ast(ast),
                _ => format!("({})", format_ast(ast)),
            };
            format!("NOT({inner})")
        }
        LogicalRuleAst::SubRule(asts) => {
            let inner = asts
                .iter()
                .map(|a| match a {
                    LogicalRuleAst::Leaf(_) => format_ast(a),
                    _ => format!("({})", format_ast(a)),
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("SUB-RULE({inner})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_and() {
        let rule = "AND((DOMAIN,example.com),(IP-CIDR,1.2.3.4/24), Proxy)";
        let parsed = parse_logical_rule(rule).unwrap();
        assert_eq!(parsed.target, "Proxy");
        assert!(!parsed.no_resolve);

        let expected_ast = LogicalRuleAst::And(vec![
            LogicalRuleAst::Leaf(RulePayload("DOMAIN,example.com".into())),
            LogicalRuleAst::Leaf(RulePayload("IP-CIDR,1.2.3.4/24".into())),
        ]);
        assert_eq!(parsed.payload, expected_ast);
    }

    #[test]
    fn test_parse_nested() {
        let rule = "OR((AND((DOMAIN,example.com),(IP-CIDR,1.2.3.4/24))),(DOMAIN-SUFFIX,google.com), Direct,no-resolve)";
        let parsed = parse_logical_rule(rule).unwrap();
        assert_eq!(parsed.target, "Direct");
        assert!(parsed.no_resolve);

        let formatted = format_logical_rule(&parsed);
        assert_eq!(
            formatted,
            "OR((AND(DOMAIN,example.com,IP-CIDR,1.2.3.4/24)),DOMAIN-SUFFIX,google.com,Direct,no-resolve)"
        );
    }

    #[test]
    fn test_evaluate_ast() {
        let ast = LogicalRuleAst::And(vec![
            LogicalRuleAst::Leaf(RulePayload("DOMAIN,google.com".into())),
            LogicalRuleAst::Leaf(RulePayload("DST-PORT,443".into())),
        ]);
        assert!(ast.evaluate(&|leaf| leaf == "DOMAIN,google.com" || leaf == "DST-PORT,443"));
        assert!(!ast.evaluate(&|leaf| leaf == "DOMAIN,google.com"));

        let not_ast = LogicalRuleAst::Not(Box::new(LogicalRuleAst::Leaf(RulePayload(
            "DOMAIN,google.com".into(),
        ))));
        assert!(!not_ast.evaluate(&|leaf| leaf == "DOMAIN,google.com"));
        assert!(not_ast.evaluate(&|leaf| leaf == "DOMAIN,bing.com"));
    }

    #[test]
    fn test_syntax_errors() {
        assert_eq!(
            validate_logical_rule_syntax("AND((DOMAIN,example.com), Proxy"),
            Err(RuleSyntaxError::UnclosedParenthesis)
        );
        assert_eq!(
            validate_logical_rule_syntax("AND((DOMAIN,example.com))"),
            Err(RuleSyntaxError::MissingTarget)
        );
        assert_eq!(
            validate_logical_rule_syntax("XYZ((DOMAIN,example.com), Proxy)"),
            Err(RuleSyntaxError::InvalidSubRuleType)
        );
    }
}

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

fn balanced(source: &str) -> Result<(), RuleSyntaxError> {
    let mut depth = 0usize;
    for character in source.chars() {
        match character {
            '(' => {
                depth += 1;
                if depth > 64 {
                    return Err(RuleSyntaxError::ParseError(
                        "Logical nesting exceeds 64 levels".into(),
                    ));
                }
            }
            ')' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or_else(|| RuleSyntaxError::ParseError("Mismatched parenthesis".into()))?;
            }
            _ => {}
        }
    }
    if depth == 0 {
        Ok(())
    } else {
        Err(RuleSyntaxError::UnclosedParenthesis)
    }
}
fn unwrap_condition(source: &str) -> Result<&str, RuleSyntaxError> {
    source
        .trim()
        .strip_prefix('(')
        .and_then(|source| source.strip_suffix(')'))
        .ok_or_else(|| {
            RuleSyntaxError::ParseError("Each logical condition must be parenthesized".into())
        })
}
fn parse_ast(source: &str) -> Result<LogicalRuleAst, RuleSyntaxError> {
    let parts = split_comma_outside_parens(source);
    let Some(kind) = parts.first() else {
        return Err(RuleSyntaxError::ParseError("Empty condition".into()));
    };
    match kind.as_str() {
        "AND" | "OR" | "NOT" => {
            if parts.len() != 2 {
                return Err(RuleSyntaxError::ParseError(
                    "Logical condition has unexpected parameters".into(),
                ));
            }
            let inner = unwrap_condition(&parts[1])?;
            let children = split_comma_outside_parens(inner)
                .iter()
                .map(|child| parse_ast(unwrap_condition(child)?))
                .collect::<Result<Vec<_>, _>>()?;
            if children.is_empty() {
                return Err(RuleSyntaxError::ParseError(
                    "Empty logical expression".into(),
                ));
            }
            Ok(match kind.as_str() {
                "AND" => LogicalRuleAst::And(children),
                "OR" => LogicalRuleAst::Or(children),
                _ => {
                    if children.len() != 1 {
                        return Err(RuleSyntaxError::ParseError(
                            "NOT must contain one condition".into(),
                        ));
                    }
                    LogicalRuleAst::Not(Box::new(children.into_iter().next().expect("one child")))
                }
            })
        }
        "SUB-RULE" | "MATCH" => Err(RuleSyntaxError::InvalidSubRuleType),
        _ if parts.len() >= 2 && !kind.contains('(') => {
            Ok(LogicalRuleAst::Leaf(RulePayload(source.trim().into())))
        }
        _ => Err(RuleSyntaxError::ParseError(
            "Malformed leaf condition".into(),
        )),
    }
}
fn parse_native(source: &str) -> Result<LogicalRule, RuleSyntaxError> {
    balanced(source)?;
    let parts = split_comma_outside_parens(source.trim());
    if parts.len() < 3 {
        return Err(RuleSyntaxError::MissingTarget);
    }
    if parts.len() > 3 {
        return Err(RuleSyntaxError::ParseError(
            "Unexpected logical parameters".into(),
        ));
    }
    let kind = &parts[0];
    if !["AND", "OR", "NOT", "SUB-RULE"].contains(&kind.as_str()) {
        return Err(RuleSyntaxError::InvalidSubRuleType);
    }
    let target = parts[2].trim().to_owned();
    if target.is_empty() {
        return Err(RuleSyntaxError::MissingTarget);
    }
    let payload = if kind == "SUB-RULE" {
        LogicalRuleAst::SubRule(vec![parse_ast(unwrap_condition(&parts[1])?)?])
    } else {
        parse_ast(&format!("{kind},{}", parts[1]))?
    };
    Ok(LogicalRule { target, payload })
}
pub fn validate_logical_rule_syntax(source: &str) -> Result<(), RuleSyntaxError> {
    parse_native(source).map(|_| ())
}
pub fn parse_logical_rule(source: &str) -> Result<LogicalRule> {
    parse_native(source).map_err(|error| anyhow!("{error}"))
}
pub fn format_logical_rule(rule: &LogicalRule) -> String {
    format!("{},{}", format_ast(&rule.payload), rule.target)
}
pub fn format_ast(ast: &LogicalRuleAst) -> String {
    match ast {
        LogicalRuleAst::Leaf(payload) => payload.0.clone(),
        LogicalRuleAst::And(children) | LogicalRuleAst::Or(children) => {
            let kind = if matches!(ast, LogicalRuleAst::And(_)) {
                "AND"
            } else {
                "OR"
            };
            format!(
                "{kind},({})",
                children
                    .iter()
                    .map(|child| format!("({})", format_ast(child)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        }
        LogicalRuleAst::Not(child) => format!("NOT,(({}))", format_ast(child)),
        LogicalRuleAst::SubRule(children) => format!(
            "SUB-RULE,({})",
            children.first().map(format_ast).unwrap_or_default()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_and() {
        let rule = "AND,((DOMAIN,example.com),(IP-CIDR,1.2.3.4/24)),Proxy";
        let parsed = parse_logical_rule(rule).unwrap();
        assert_eq!(parsed.target, "Proxy");

        let expected_ast = LogicalRuleAst::And(vec![
            LogicalRuleAst::Leaf(RulePayload("DOMAIN,example.com".into())),
            LogicalRuleAst::Leaf(RulePayload("IP-CIDR,1.2.3.4/24".into())),
        ]);
        assert_eq!(parsed.payload, expected_ast);
    }

    #[test]
    fn test_parse_nested() {
        let rule = "OR,((AND,((DOMAIN,example.com),(IP-CIDR,1.2.3.4/24,no-resolve))),(DOMAIN-SUFFIX,google.com)),Direct";
        let parsed = parse_logical_rule(rule).unwrap();
        assert_eq!(parsed.target, "Direct");

        let formatted = format_logical_rule(&parsed);
        assert_eq!(
            formatted,
            "OR,((AND,((DOMAIN,example.com),(IP-CIDR,1.2.3.4/24,no-resolve))),(DOMAIN-SUFFIX,google.com)),Direct"
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
            validate_logical_rule_syntax("AND,((DOMAIN,example.com))"),
            Err(RuleSyntaxError::MissingTarget)
        );
        assert_eq!(
            validate_logical_rule_syntax("XYZ,((DOMAIN,example.com)),Proxy"),
            Err(RuleSyntaxError::InvalidSubRuleType)
        );
    }
    #[test]
    fn legacy_function_syntax_is_rejected_instead_of_persisted() {
        for source in [
            "AND((DOMAIN,example.com),(DST-PORT,443),DIRECT)",
            "NOT((NETWORK,udp),DIRECT)",
            "SUB-RULE((DOMAIN,example.com),DIRECT)",
        ] {
            assert!(parse_logical_rule(source).is_err(), "{source}");
        }
    }
}

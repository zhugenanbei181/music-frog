use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use yaml_rust2::YamlLoader;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyntaxDiagnostic {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

pub fn validate_yaml(content: &str) -> anyhow::Result<()> {
    if content.trim().is_empty() {
        return Err(anyhow!("配置内容不能为空"));
    }
    YamlLoader::load_from_str(content)
        .map(|_| ())
        .map_err(|e| anyhow!("配置内容不是有效的 YAML: {e}"))
}

/// Real-time YAML syntax preflight checker returning precise line and column diagnostics.
pub fn preflight_yaml_syntax(content: &str) -> Result<(), SyntaxDiagnostic> {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    match YamlLoader::load_from_str(content) {
        Ok(_) => Ok(()),
        Err(err) => {
            let marker = err.marker();
            Err(SyntaxDiagnostic {
                line: marker.line(),
                column: marker.col(),
                message: err.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_yaml_valid() {
        let valid_yaml = "port: 7890\nmode: rule";
        assert!(validate_yaml(valid_yaml).is_ok());

        let nested_yaml = "dns:\n  enable: true\n  nameserver:\n    - 8.8.8.8";
        assert!(validate_yaml(nested_yaml).is_ok());

        let with_comments = "# comment\nport: 7890 # another comment";
        assert!(validate_yaml(with_comments).is_ok());
    }

    #[test]
    fn test_validate_yaml_invalid() {
        let invalid_yaml = "port: 7890\nmode: : rule";
        assert!(validate_yaml(invalid_yaml).is_err());

        let tab_indent = "port: 7890\n\tmode: rule";
        assert!(validate_yaml(tab_indent).is_err());
    }

    #[test]
    fn test_validate_yaml_empty() {
        assert!(validate_yaml("").is_err());
        assert!(validate_yaml("   ").is_err());
        assert!(validate_yaml("\n\n").is_err());
    }

    #[test]
    fn test_preflight_yaml_syntax_valid() {
        let valid = "port: 7890\nmode: rule";
        assert_eq!(preflight_yaml_syntax(valid), Ok(()));
        assert_eq!(preflight_yaml_syntax(""), Ok(()));
    }

    #[test]
    fn test_preflight_yaml_syntax_invalid_diagnostic() {
        let invalid = "port: 7890\nmode: [\nfoo: bar";
        let diag = preflight_yaml_syntax(invalid).unwrap_err();
        assert!(diag.line >= 2);
        assert!(!diag.message.is_empty());
    }
}

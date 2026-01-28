use anyhow::anyhow;
use yaml_rust2::YamlLoader;

pub fn validate_yaml(content: &str) -> anyhow::Result<()> {
    if content.trim().is_empty() {
        return Err(anyhow!("配置内容不能为空"));
    }
    YamlLoader::load_from_str(content)
        .map(|_| ())
        .map_err(|_| anyhow!("配置内容不是有效的 YAML"))
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
}

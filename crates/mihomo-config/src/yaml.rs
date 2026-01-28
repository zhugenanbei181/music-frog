use mihomo_api::{MihomoError, Result};
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

pub(crate) fn validate(content: &str) -> Result<()> {
    let _ = load_yaml(content)?;
    Ok(())
}

pub(crate) fn load_yaml(content: &str) -> Result<Yaml> {
    let docs = YamlLoader::load_from_str(content)?;
    Ok(docs.into_iter().next().unwrap_or(Yaml::Null))
}

pub(crate) fn get_str(doc: &Yaml, key: &str) -> Option<String> {
    doc.as_hash()?
        .get(&Yaml::String(key.to_string()))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub(crate) fn get_u16(doc: &Yaml, key: &str) -> Option<u16> {
    let value = doc.as_hash()?.get(&Yaml::String(key.to_string()))?;
    match value {
        Yaml::Integer(num) => {
            if *num >= 0 && *num <= u16::MAX as i64 {
                Some(*num as u16)
            } else {
                None
            }
        }
        Yaml::Real(raw) => raw.parse::<f64>().ok().map(|f| f as u16),
        Yaml::String(raw) => raw.parse::<u16>().ok(),
        _ => None,
    }
}

pub(crate) fn set_str(doc: &mut Yaml, key: &str, value: &str) -> Result<()> {
    if let Yaml::Hash(hash) = doc {
        hash.insert(
            Yaml::String(key.to_string()),
            Yaml::String(value.to_string()),
        );
        Ok(())
    } else {
        Err(MihomoError::Config(
            "Invalid YAML mapping".to_string(),
        ))
    }
}

pub(crate) fn set_u16(doc: &mut Yaml, key: &str, value: u16) -> Result<()> {
    if let Yaml::Hash(hash) = doc {
        hash.insert(
            Yaml::String(key.to_string()),
            Yaml::Integer(value as i64),
        );
        Ok(())
    } else {
        Err(MihomoError::Config(
            "Invalid YAML mapping".to_string(),
        ))
    }
}

pub(crate) fn to_string(doc: &Yaml) -> Result<String> {
    let mut out = String::new();
    let mut emitter = YamlEmitter::new(&mut out);
    emitter
        .dump(doc)
        .map_err(|_| MihomoError::YamlEmit("Failed to serialize YAML".to_string()))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_yaml() {
        let yaml = r#"
port: 7890
socks-port: 7891
mode: rule
"#;
        let result = validate(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_invalid_yaml() {
        let yaml = "invalid: yaml: content: [";
        let result = validate(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_yaml() {
        let yaml = "";
        let result = validate(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_load_yaml_success() {
        let yaml = r#"
port: 7890
socks-port: 7891
"#;
        let result = load_yaml(yaml);
        assert!(result.is_ok());
        let doc = result.unwrap();
        assert!(doc.is_hash());
    }

    #[test]
    fn test_load_yaml_empty() {
        let yaml = "";
        let result = load_yaml(yaml);
        assert!(result.is_ok());
        assert!(result.unwrap().is_null());
    }

    #[test]
    fn test_get_str_success() {
        let yaml = "port: 7890\nname: test";
        let doc = load_yaml(yaml).unwrap();
        let result = get_str(&doc, "name");
        assert_eq!(result, Some("test".to_string()));
    }

    #[test]
    fn test_get_str_not_found() {
        let yaml = "port: 7890";
        let doc = load_yaml(yaml).unwrap();
        let result = get_str(&doc, "name");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_str_invalid_type() {
        let yaml = "port: 7890";
        let doc = load_yaml(yaml).unwrap();
        let result = get_str(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_u16_integer() {
        let yaml = "port: 7890";
        let doc = load_yaml(yaml).unwrap();
        let result = get_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_u16_real() {
        let yaml = "port: 7890.0";
        let doc = load_yaml(yaml).unwrap();
        let result = get_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_u16_string() {
        let yaml = "port: \"7890\"";
        let doc = load_yaml(yaml).unwrap();
        let result = get_u16(&doc, "port");
        assert_eq!(result, Some(7890));
    }

    #[test]
    fn test_get_u16_out_of_range_positive() {
        let yaml = "port: 100000";
        let doc = load_yaml(yaml).unwrap();
        let result = get_u16(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_u16_negative() {
        let yaml = "port: -1";
        let doc = load_yaml(yaml).unwrap();
        let result = get_u16(&doc, "port");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_u16_max() {
        let yaml = "port: 65535";
        let doc = load_yaml(yaml).unwrap();
        let result = get_u16(&doc, "port");
        assert_eq!(result, Some(65535));
    }

    #[test]
    fn test_set_str_success() {
        let yaml = "port: 7890";
        let mut doc = load_yaml(yaml).unwrap();
        let result = set_str(&mut doc, "name", "test");
        assert!(result.is_ok());
        assert_eq!(get_str(&doc, "name"), Some("test".to_string()));
    }

    #[test]
    fn test_set_str_invalid_mapping() {
        let mut doc = Yaml::Integer(42);
        let result = set_str(&mut doc, "name", "test");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid YAML mapping"));
    }

    #[test]
    fn test_set_u16_success() {
        let yaml = "name: test";
        let mut doc = load_yaml(yaml).unwrap();
        let result = set_u16(&mut doc, "port", 9090);
        assert!(result.is_ok());
        assert_eq!(get_u16(&doc, "port"), Some(9090));
    }

    #[test]
    fn test_set_u16_invalid_mapping() {
        let mut doc = Yaml::Integer(42);
        let result = set_u16(&mut doc, "port", 9090);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid YAML mapping"));
    }

    #[test]
    fn test_to_string_success() {
        let yaml = "port: 7890\nname: test";
        let doc = load_yaml(yaml).unwrap();
        let result = to_string(&doc);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("port"));
        assert!(output.contains("7890"));
    }

    #[test]
    fn test_to_string_preserves_structure() {
        let yaml = r#"
port: 7890
socks-port: 7891
mode: rule
"#;
        let doc = load_yaml(yaml).unwrap();
        let result = to_string(&doc);
        assert!(result.is_ok());
    }
}

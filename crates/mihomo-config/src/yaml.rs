use mihomo_api::{MihomoError, Result};
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

pub(crate) fn validate(content: &str) -> Result<()> {
    let _ = load_yaml(content)?;
    Ok(())
}

pub(crate) fn load_yaml(content: &str) -> Result<Yaml> {
    let docs = YamlLoader::load_from_str(content)?;
    docs.into_iter().next().ok_or_else(|| {
        MihomoError::Config("Invalid YAML content".to_string())
    })
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
        Yaml::Real(raw) => raw.parse::<u16>().ok(),
        Yaml::String(raw) => raw.parse::<u16>().ok(),
        _ => None,
    }
}

pub(crate) fn set_str(doc: &mut Yaml, key: &str, value: &str) -> Result<()> {
    if let Yaml::Hash(ref mut hash) = doc {
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
    if let Yaml::Hash(ref mut hash) = doc {
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

//! Replace one qualified rule scalar without rewriting unrelated document bytes.
use super::{SourceDoc, YamlEditError};
use crate::rules::parse_rule_entry;
use infiltrator_contract::rule_location::RuleLocation;
use serde_yaml_ng::Value;

pub fn replace_rule_at_location(
    content: &str,
    location: &RuleLocation,
    expected: &str,
    replacement: &str,
) -> Result<String, YamlEditError> {
    let original: Value = serde_yaml_ng::from_str(content.trim_start_matches('\u{feff}'))
        .map_err(|error| YamlEditError::Unsupported(format!("Source parse: {error}")))?;
    let rules = selected(&original, location)?;
    let entry = rules
        .get(location.index)
        .and_then(Value::as_str)
        .ok_or_else(|| YamlEditError::RuleNotFound(expected.into()))?;
    if parse_rule_entry(entry).rule != expected || !parse_rule_entry(entry).enabled {
        return Err(YamlEditError::RuleNotFound(expected.into()));
    }
    let mut doc = SourceDoc::parse(content)?;
    let root = if location.table.is_some() {
        "sub-rules"
    } else {
        "rules"
    };
    let mut header = doc
        .find_top_level_key(root)
        .ok_or_else(|| YamlEditError::KeyNotFound(root.into()))?;
    if let Some(table) = &location.table {
        header = ((header + 1)..doc.lines.len())
            .take_while(|index| {
                let line = &doc.lines[*index].text;
                line.trim().is_empty()
                    || line.trim_start().starts_with('#')
                    || line.starts_with(' ')
            })
            .find(|index| key_of(&doc.lines[*index].text).as_deref() == Some(table))
            .ok_or_else(|| YamlEditError::KeyNotFound(table.clone()))?;
    }
    let header_text = &doc.lines[header].text;
    let header_indent = indentation(header_text);
    let parsed_header: Value = serde_yaml_ng::from_str(header_text.trim())
        .map_err(|error| YamlEditError::Unsupported(error.to_string()))?;
    if parsed_header
        .as_mapping()
        .and_then(|mapping| mapping.values().next())
        .is_none_or(|value| !value.is_null())
    {
        return Err(YamlEditError::Unsupported(
            "Rule replacement requires a block sequence, not a flow sequence or alias".into(),
        ));
    }
    let mut slots = Vec::new();
    for index in (header + 1)..doc.lines.len() {
        let line = &doc.lines[index].text;
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let indent = indentation(line);
        let sequence = line.trim_start().starts_with("- ");
        if indent < header_indent || (indent == header_indent && !sequence) {
            break;
        }
        if sequence {
            let value: Value = serde_yaml_ng::from_str(line.trim())
                .map_err(|error| YamlEditError::Unsupported(error.to_string()))?;
            if value
                .as_sequence()
                .is_none_or(|items| items.len() != 1 || !items[0].is_string())
            {
                return Err(YamlEditError::Unsupported(
                    "Multiline and nested rule scalars require an explicit document edit".into(),
                ));
            }
            slots.push(index);
        } else {
            return Err(YamlEditError::Unsupported(
                "Unexpected content in the selected rule sequence".into(),
            ));
        }
    }
    if slots.len() != rules.len() {
        return Err(YamlEditError::Unsupported(
            "Physical rule sequence differs from the parsed source".into(),
        ));
    }
    let index = slots[location.index];
    let line = &doc.lines[index].text;
    let dash = line
        .find("- ")
        .ok_or_else(|| YamlEditError::RuleNotFound(expected.into()))?
        + 2;
    let scalar = &line[dash..];
    let (value_end, comment) = scalar_end(scalar);
    let value = scalar[..value_end].trim_end();
    let whitespace = &scalar[value.len()..value_end];
    let leading = &value[..value.len() - value.trim_start().len()];
    let rendered = match value.trim_start().chars().next() {
        Some('\'') => format!("'{}'", replacement.replace('\'', "''")),
        Some('"') => serde_json::to_string(replacement)
            .map_err(|error| YamlEditError::Unsupported(error.to_string()))?,
        _ => serde_yaml_ng::to_string(replacement)
            .map_err(|error| YamlEditError::Unsupported(error.to_string()))?
            .trim_end_matches(['\r', '\n'])
            .to_owned(),
    };
    doc.lines[index].text = format!(
        "{}{}{}{}{}",
        &line[..dash],
        leading,
        rendered,
        whitespace,
        comment
    );
    let updated = doc.render();
    let actual: Value = serde_yaml_ng::from_str(updated.trim_start_matches('\u{feff}'))
        .map_err(|error| YamlEditError::Unsupported(format!("Edited parse: {error}")))?;
    let mut wanted = original;
    let entries = selected_mut(&mut wanted, location)?;
    entries[location.index] = Value::String(replacement.into());
    if actual != wanted {
        return Err(YamlEditError::Unsupported(
            "Rule edit changed data outside the confirmed scalar".into(),
        ));
    }
    Ok(updated)
}
fn selected<'a>(doc: &'a Value, location: &RuleLocation) -> Result<&'a Vec<Value>, YamlEditError> {
    let value = match &location.table {
        Some(table) => doc.get("sub-rules").and_then(|tables| tables.get(table)),
        None => doc.get("rules"),
    };
    value
        .and_then(Value::as_sequence)
        .ok_or(YamlEditError::RulesBlockMissing)
}
fn selected_mut<'a>(
    doc: &'a mut Value,
    location: &RuleLocation,
) -> Result<&'a mut Vec<Value>, YamlEditError> {
    let value = match &location.table {
        Some(table) => doc
            .get_mut("sub-rules")
            .and_then(|tables| tables.get_mut(table)),
        None => doc.get_mut("rules"),
    };
    value
        .and_then(Value::as_sequence_mut)
        .ok_or(YamlEditError::RulesBlockMissing)
}
fn indentation(line: &str) -> usize {
    line.len() - line.trim_start().len()
}
fn key_of(line: &str) -> Option<String> {
    let value: Value = serde_yaml_ng::from_str(line.trim()).ok()?;
    value
        .as_mapping()?
        .keys()
        .next()?
        .as_str()
        .map(str::to_owned)
}
fn scalar_end(scalar: &str) -> (usize, &str) {
    let mut quote = None;
    let mut escaped = false;
    for (index, character) in scalar.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if quote == Some('"') && character == '\\' {
            escaped = true;
            continue;
        }
        if matches!(character, '\'' | '"') {
            if quote == Some(character) {
                quote = None;
            } else if quote.is_none() {
                quote = Some(character);
            }
        }
        if quote.is_none()
            && character == '#'
            && (index == 0 || scalar[..index].ends_with(char::is_whitespace))
        {
            return (index, &scalar[index..]);
        }
    }
    (scalar.len(), "")
}

#[cfg(test)]
#[path = "rule_location_test.rs"]
mod tests;

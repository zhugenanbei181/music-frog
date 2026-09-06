//! Pure validation for user-provided PAC bypass patterns.

use std::collections::HashSet;

pub fn normalize_bypass_domains(values: &[String]) -> Result<Vec<String>, String> {
    if values.len() > 256 {
        return Err("PAC bypass list exceeds 256 entries".to_owned());
    }
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();
    for value in values {
        let domain = value.trim();
        if domain.is_empty() {
            continue;
        }
        if domain.len() > 253
            || domain.chars().any(|character| {
                character.is_control() || character.is_whitespace() || matches!(character, '"' | '\\')
            })
        {
            return Err(format!("invalid PAC bypass pattern: {domain}"));
        }
        let domain = domain.to_ascii_lowercase();
        if seen.insert(domain.clone()) {
            normalized.push(domain);
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::normalize_bypass_domains;

    #[test]
    fn pac_bypass_patterns_are_normalized_and_deduplicated() {
        let values = vec![" Example.COM ".to_owned(), "example.com".to_owned(), "*.lan".to_owned()];
        assert_eq!(
            normalize_bypass_domains(&values).expect("valid PAC patterns"),
            vec!["example.com", "*.lan"]
        );
    }

    #[test]
    fn pac_bypass_patterns_reject_script_injection() {
        assert!(normalize_bypass_domains(&["ok.com\"; alert(1)//".to_owned()]).is_err());
        assert!(normalize_bypass_domains(&["bad domain".to_owned()]).is_err());
    }
}

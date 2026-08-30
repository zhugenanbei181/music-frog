use anyhow::Result;

/// Update a top-level field in a YAML string while preserving formatting.
pub fn update_top_level_field(yaml_str: &str, key: &str, new_value: &str) -> Result<String> {
    update_nested_field(yaml_str, &[key], new_value)
}

/// Update a nested field in a YAML string given its path.
pub fn update_nested_field(yaml_str: &str, path: &[&str], new_value: &str) -> Result<String> {
    apply_patches(yaml_str, &[(path, new_value)])
}

/// Apply multiple patches to a YAML string.
pub fn apply_patches(yaml_str: &str, patches: &[(&[&str], &str)]) -> Result<String> {
    let mut current = yaml_str.to_string();
    for &(path, new_value) in patches {
        current = apply_patch(&current, path, new_value)?;
    }
    Ok(current)
}

fn apply_patch(yaml: &str, path: &[&str], new_value: &str) -> Result<String> {
    if path.is_empty() {
        return Ok(yaml.to_string());
    }

    let mut lines: Vec<String> = yaml.split('\n').map(|s| s.to_string()).collect();
    
    let mut current_path = Vec::new();
    let mut target_line_idx = None;
    
    let mut deepest_match_idx = None;
    let mut deepest_match_len = 0;
    let mut deepest_match_indent = 0;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = line.len() - trimmed.len();

        while let Some(&(last_indent, _)) = current_path.last() {
            if last_indent >= indent {
                current_path.pop();
            } else {
                break;
            }
        }

        if trimmed.starts_with('-') && trimmed[1..].starts_with(' ') {
            // Simplified handling: ignore list items for path matching
            continue;
        }

        if let Some(colon_idx) = trimmed.find(':') {
            let key_part = trimmed[..colon_idx].trim_end();
            let key = key_part.trim_matches(|c| c == '"' || c == '\'');
            
            current_path.push((indent, key.to_string()));

            let current_path_strs: Vec<&str> = current_path.iter().map(|(_, k)| k.as_str()).collect();
            
            if current_path_strs == path {
                target_line_idx = Some(i);
                break;
            }

            if path.starts_with(&current_path_strs) {
                if current_path_strs.len() > deepest_match_len {
                    deepest_match_len = current_path_strs.len();
                    deepest_match_idx = Some(i);
                    deepest_match_indent = indent;
                }
            }
        }
    }

    if let Some(idx) = target_line_idx {
        let line = &lines[idx];
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        let colon_idx = trimmed.find(':').unwrap();
        let key_part = trimmed[..colon_idx].trim_end();
        
        let after_colon = &trimmed[colon_idx + 1..];
        
        let mut hash_pos = None;
        let mut in_single = false;
        let mut in_double = false;
        for (pos, c) in after_colon.char_indices() {
            if c == '\'' && !in_double { in_single = !in_single; }
            if c == '"' && !in_single { in_double = !in_double; }
            if c == '#' && !in_single && !in_double {
                hash_pos = Some(pos);
                break;
            }
        }

        let mut before_comment = after_colon;
        let mut comment = "";
        if let Some(pos) = hash_pos {
            comment = after_colon[pos..].trim_start();
            before_comment = &after_colon[..pos];
        }

        let before_comment_trimmed = before_comment.trim();
        let mut anchor = "";
        if before_comment_trimmed.starts_with('&') {
            if let Some(space_idx) = before_comment_trimmed.find(' ') {
                anchor = &before_comment_trimmed[..space_idx];
            } else {
                anchor = before_comment_trimmed;
            }
        } else if before_comment_trimmed.starts_with('*') {
            if let Some(space_idx) = before_comment_trimmed.find(' ') {
                anchor = &before_comment_trimmed[..space_idx];
            } else {
                anchor = before_comment_trimmed;
            }
        }

        let mut new_line = format!("{:indent$}{}:", "", key_part, indent = indent);
        if !anchor.is_empty() {
            new_line.push_str(" ");
            new_line.push_str(anchor);
        }
        new_line.push_str(" ");
        new_line.push_str(new_value);
        if !comment.is_empty() {
            new_line.push_str(" ");
            new_line.push_str(comment);
        }
        lines[idx] = new_line;

        let mut has_child_block = false;
        for i in idx + 1 .. lines.len() {
            let ln = &lines[i];
            let t = ln.trim_start();
            if t.is_empty() || t.starts_with('#') { continue; }
            let child_indent = ln.len() - t.len();
            if child_indent > indent {
                has_child_block = true;
            }
            break;
        }

        if has_child_block {
            let mut end_idx = idx + 1;
            for i in idx + 1 .. lines.len() {
                let ln = &lines[i];
                let t = ln.trim_start();
                if t.is_empty() || t.starts_with('#') {
                    end_idx = i + 1;
                    continue;
                }
                let child_indent = ln.len() - t.len();
                if child_indent <= indent {
                    end_idx = i;
                    break;
                } else {
                    end_idx = i + 1;
                }
            }
            lines.drain(idx + 1 .. end_idx);
        }

    } else {
        // Insert new field
        let mut insert_idx = lines.len();
        if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
            insert_idx = insert_idx.saturating_sub(1);
        }
        
        let mut base_indent = 0;
        
        if let Some(parent_idx) = deepest_match_idx {
            base_indent = deepest_match_indent + 2; 
            insert_idx = lines.len();
            if lines.last().map(|s| s.is_empty()).unwrap_or(false) {
                insert_idx = insert_idx.saturating_sub(1);
            }
            for i in parent_idx + 1 .. lines.len() {
                let line = &lines[i];
                let trimmed = line.trim_start();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    continue;
                }
                let indent = line.len() - trimmed.len();
                if indent <= deepest_match_indent {
                    insert_idx = i;
                    break;
                }
            }
        }
        
        let mut to_insert = Vec::new();
        let mut current_indent = base_indent;
        for i in deepest_match_len .. path.len() {
            if i == path.len() - 1 {
                to_insert.push(format!("{:indent$}{}: {}", "", path[i], new_value, indent = current_indent));
            } else {
                to_insert.push(format!("{:indent$}{}:", "", path[i], indent = current_indent));
                current_indent += 2;
            }
        }
        
        lines.splice(insert_idx..insert_idx, to_insert);
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_top_level_field() {
        let yaml = "\
# Custom rules
mode: rule # inline comment
ipv6: false
";
        let expected = "\
# Custom rules
mode: global # inline comment
ipv6: false
";
        let result = update_top_level_field(yaml, "mode", "global").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_update_nested_field() {
        let yaml = "\
dns:
  enable: true
  listen: 127.0.0.1:53
  ipv6: false
";
        let expected = "\
dns:
  enable: true
  listen: 127.0.0.1:5353
  ipv6: false
";
        let result = update_nested_field(yaml, &["dns", "listen"], "127.0.0.1:5353").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_update_boolean() {
        let yaml = "ipv6: false\n";
        let expected = "ipv6: true\n";
        let result = update_top_level_field(yaml, "ipv6", "true").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_apply_multiple_patches() {
        let yaml = "\
mode: rule
dns:
  enable: false
ipv6: false
";
        let patches: Vec<(&[&str], &str)> = vec![
            (&["mode"], "global"),
            (&["dns", "enable"], "true"),
            (&["ipv6"], "true"),
        ];
        
        let expected = "\
mode: global
dns:
  enable: true
ipv6: true
";
        let result = apply_patches(yaml, &patches).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_preserve_anchors() {
        let yaml = "\
listen: &anchor 127.0.0.1:53
";
        let expected = "\
listen: &anchor 127.0.0.1:5353
";
        let result = update_top_level_field(yaml, "listen", "127.0.0.1:5353").unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_insert_missing_fields() {
        let yaml = "\
dns:
  enable: true
";
        let expected = "\
dns:
  enable: true
  listen: 0.0.0.0:53
";
        let result = update_nested_field(yaml, &["dns", "listen"], "0.0.0.0:53").unwrap();
        assert_eq!(result, expected);

        let expected_top = "\
dns:
  enable: true
  listen: 0.0.0.0:53
mode: rule
";
        let result = update_top_level_field(&result, "mode", "rule").unwrap();
        assert_eq!(result, expected_top);
    }
}

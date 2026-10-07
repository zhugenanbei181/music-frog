//! Lightweight multi-line code editor state machine with YAML/Rule syntax tokenization.

use bevy::ecs::component::Component;

/// Syntax token classification for code highlighting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyntaxTokenKind {
    Keyword,
    StringLiteral,
    NumberLiteral,
    Comment,
    Punctuation,
    Plain,
}

/// Tokenized segment on a single line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyntaxToken {
    pub text: String,
    pub kind: SyntaxTokenKind,
}

pub mod state;
use state::CodeEditorState;

/// Tokenize a line of YAML / Rule configuration.
pub fn tokenize_yaml_line(line: &str) -> Vec<SyntaxToken> {
    let trimmed = line.trim_start();
    if trimmed.starts_with('#') {
        return vec![SyntaxToken {
            text: line.to_string(),
            kind: SyntaxTokenKind::Comment,
        }];
    }

    let mut tokens = Vec::new();
    let parts: Vec<&str> = line.split(':').collect();
    if parts.len() > 1 {
        tokens.push(SyntaxToken {
            text: parts[0].to_string(),
            kind: SyntaxTokenKind::Keyword,
        });
        tokens.push(SyntaxToken {
            text: ":".to_string(),
            kind: SyntaxTokenKind::Punctuation,
        });
        tokens.push(SyntaxToken {
            text: parts[1..].join(":"),
            kind: SyntaxTokenKind::Plain,
        });
    } else {
        tokens.push(SyntaxToken {
            text: line.to_string(),
            kind: SyntaxTokenKind::Plain,
        });
    }

    tokens
}

use crate::palette::UiPalette;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, Overflow,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;

/// Marker component for the code editor container root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEditorRoot;

/// Marker component for the line number gutter column.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEditorGutter;

/// Marker component for the code content body area.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEditorBody;

/// Construct a full syntax-highlighted multi-line code editor scene.
pub fn code_editor_scene(state: &CodeEditorState, palette: &UiPalette) -> Box<dyn Scene> {
    let total_lines = state.line_count();
    let edge = palette.border;

    let gutter_lines: Vec<Box<dyn Scene>> = (1..=total_lines)
        .map(|num| {
            Box::new(bsn! {
                            Node {
                                height: px(palette.control_height_px * 0.6),
                                justify_content: JustifyContent::FlexEnd,
                                padding: UiRect::horizontal(Val::Px(space::S4)),
                            }
                            Children [
                                Text({ format!("{}", num) })
                                TextRole(Role::Mono)
                            ]
            }) as Box<dyn Scene>
        })
        .collect();

    let code_lines: Vec<Box<dyn Scene>> = state
        .lines
        .iter()
        .map(|line| {
            let tokens = tokenize_yaml_line(line);
            let token_nodes: Vec<Box<dyn Scene>> = tokens
                .into_iter()
                .map(|tok| {
                    Box::new(bsn! {
                                            Text({ tok.text })
                                            TextRole(Role::Mono)
                    }) as Box<dyn Scene>
                })
                .collect();

            Box::new(bsn! {
                            Node {
                                height: px(palette.control_height_px * 0.6),
                                flex_direction: FlexDirection::Row,
                                align_items: AlignItems::Center,
                            }
                            Children [
                                { token_nodes }
                            ]
            }) as Box<dyn Scene>
        })
        .collect();

    Box::new(bsn! {
            Node {
                width: percent(100),
                height: percent(100),
                flex_direction: FlexDirection::Row,
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
                overflow: Overflow::clip(),
            }
            BackgroundColor({ palette.surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            CodeEditorRoot
            Children [
                Node {
                    width: px(48.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::vertical(Val::Px(space::S8)),
                }
                BackgroundColor({ palette.surface_elevated })
                CodeEditorGutter
                Children [
                    { gutter_lines }
                ]
                --
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(Val::Px(space::S8)),
                }
                CodeEditorBody
                Children [
                    { code_lines }
                ]
            ]
    })
}

/// Severity level of an inline editor diagnostic finding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
}

/// An inline syntax diagnostic message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorDiagnostic {
    pub line: usize,
    pub column: usize,
    pub severity: DiagnosticSeverity,
    pub message: String,
}

/// Known Mihomo / Clash rule prefixes for rule validation.
pub const KNOWN_RULE_TYPES: &[&str] = &[
    "DOMAIN-SUFFIX",
    "DOMAIN-KEYWORD",
    "DOMAIN",
    "IP-CIDR",
    "IP-CIDR6",
    "SRC-IP-CIDR",
    "GEOIP",
    "GEOSITE",
    "PROCESS-NAME",
    "PROCESS-PATH",
    "MATCH",
];

/// Known routing targets.
pub const KNOWN_TARGETS: &[&str] = &["DIRECT", "REJECT", "GLOBAL", "PROXY"];

/// Perform lightweight validation on YAML configuration or Clash rule lines.
pub fn validate_yaml_rule_syntax(lines: &[String]) -> Vec<EditorDiagnostic> {
    let mut diagnostics = Vec::new();

    for (line_idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            continue;
        }

        // Check for unclosed quotes
        let single_quotes = trimmed.chars().filter(|&c| c == '\'').count();
        let double_quotes = trimmed.chars().filter(|&c| c == '"').count();
        if single_quotes % 2 != 0 || double_quotes % 2 != 0 {
            diagnostics.push(EditorDiagnostic {
                line: line_idx + 1,
                column: trimmed.len(),
                severity: DiagnosticSeverity::Error,
                message: "字符串引号未闭合 (Unclosed quotation mark)".to_string(),
            });
            continue;
        }

        // Check rule line format (e.g. "DOMAIN-SUFFIX,google.com,PROXY")
        let rule_candidate = trimmed.strip_prefix("- ").unwrap_or(trimmed);
        if rule_candidate.contains(',') {
            let segments: Vec<&str> = rule_candidate.split(',').map(|s| s.trim()).collect();
            if segments.len() >= 2 {
                let rule_type = segments[0].to_uppercase();
                if KNOWN_RULE_TYPES.contains(&rule_type.as_str()) {
                    let target = segments[segments.len() - 1].to_uppercase();
                    if segments.len() == 2 && rule_type != "MATCH" {
                        diagnostics.push(EditorDiagnostic {
                            line: line_idx + 1,
                            column: segments[0].len() + 1,
                            severity: DiagnosticSeverity::Warning,
                            message: format!("分流规则缺少目标策略组: {}", line),
                        });
                    }
                    if KNOWN_TARGETS.contains(&target.as_str()) {
                        // Valid target
                    }
                }
            }
        }
    }

    diagnostics
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_undo_redo_lifecycle() {
        let mut editor = CodeEditorState::new("line1\nline2");
        assert_eq!(editor.line_count(), 2);

        editor.insert_char('a');
        assert_eq!(editor.lines[0], "aline1");

        assert!(editor.undo());
        assert_eq!(editor.lines[0], "line1");

        assert!(editor.redo());
        assert_eq!(editor.lines[0], "aline1");
    }

    #[test]
    fn test_yaml_rule_syntax_validation() {
        let lines = vec![
            "# This is a comment".to_string(),
            "- DOMAIN-SUFFIX,google.com,PROXY".to_string(),
            "- DOMAIN-SUFFIX,unclosed-quote\"".to_string(),
            "- IP-CIDR,192.168.1.0/24".to_string(),
        ];

        let diagnostics = validate_yaml_rule_syntax(&lines);
        assert_eq!(diagnostics.len(), 2);

        // Line 3 has unclosed quote
        assert_eq!(diagnostics[0].line, 3);
        assert_eq!(diagnostics[0].severity, DiagnosticSeverity::Error);

        // Line 4 is missing routing target
        assert_eq!(diagnostics[1].line, 4);
        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Warning);
    }
}

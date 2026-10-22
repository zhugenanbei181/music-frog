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
        let value = parts[1..].join(":");
        tokens.extend(tokenize_yaml_value(&value));
    } else {
        tokens.push(SyntaxToken {
            text: line.to_string(),
            kind: SyntaxTokenKind::Plain,
        });
    }

    tokens
}

/// The value half of a YAML line: its own kind plus a trailing comment.
///
/// The value keeps the line's own text (the leading space, the value and the
/// comment concatenate back to the original), so the tokenizer only decides
/// the *kind* of each piece — the same vocabulary the JSON view uses.
fn tokenize_yaml_value(value: &str) -> Vec<SyntaxToken> {
    let trimmed = value.trim_start();
    let leading = value.len() - trimmed.len();
    let mut tokens = Vec::new();
    if leading > 0 {
        tokens.push(SyntaxToken {
            text: value[..leading].to_string(),
            kind: SyntaxTokenKind::Plain,
        });
    }
    if trimmed.is_empty() {
        return tokens;
    }
    // A quoted value runs to its own closing quote, so a "#" inside the
    // quotes is part of the string and not the start of a comment.
    if let Some(end) = quoted_value_end(trimmed) {
        tokens.push(SyntaxToken {
            text: trimmed[..end].to_string(),
            kind: SyntaxTokenKind::StringLiteral,
        });
        tokens.extend(tokenize_yaml_tail(&trimmed[end..]));
        return tokens;
    }
    // Otherwise the value ends where a trailing comment starts.
    let (body, comment) = match trimmed.find(" #") {
        Some(offset) => (&trimmed[..offset], Some(&trimmed[offset..])),
        None => (trimmed, None),
    };
    if !body.is_empty() {
        tokens.push(SyntaxToken {
            text: body.to_string(),
            kind: classify_yaml_value(body),
        });
    }
    if let Some(comment) = comment {
        tokens.push(SyntaxToken {
            text: comment.to_string(),
            kind: SyntaxTokenKind::Comment,
        });
    }
    tokens
}

/// The text that follows a value: a trailing comment or plain text.
fn tokenize_yaml_tail(tail: &str) -> Vec<SyntaxToken> {
    if tail.is_empty() {
        return Vec::new();
    }
    match tail.find(" #") {
        Some(offset) => {
            let mut tokens = Vec::new();
            if !tail[..offset].is_empty() {
                tokens.push(SyntaxToken {
                    text: tail[..offset].to_string(),
                    kind: SyntaxTokenKind::Plain,
                });
            }
            tokens.push(SyntaxToken {
                text: tail[offset..].to_string(),
                kind: SyntaxTokenKind::Comment,
            });
            tokens
        }
        None => vec![SyntaxToken {
            text: tail.to_string(),
            kind: SyntaxTokenKind::Plain,
        }],
    }
}

/// The end of a quoted value: the byte offset just past its closing quote.
fn quoted_value_end(value: &str) -> Option<usize> {
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    for (index, character) in value.char_indices().skip(1) {
        if character == quote {
            return Some(index + character.len_utf8());
        }
    }
    None
}

/// The kind of one YAML value: a quoted string, a number or plain text.
fn classify_yaml_value(value: &str) -> SyntaxTokenKind {
    let trimmed = value.trim();
    let quoted = trimmed.len() >= 2
        && ((trimmed.starts_with('"') && trimmed.ends_with('"'))
            || (trimmed.starts_with('\'') && trimmed.ends_with('\'')));
    if quoted {
        return SyntaxTokenKind::StringLiteral;
    }
    if trimmed.parse::<f64>().is_ok() {
        return SyntaxTokenKind::NumberLiteral;
    }
    SyntaxTokenKind::Plain
}

use crate::feathers_theme::Token;
use crate::palette::UiPalette;
use crate::palette::rem;
use crate::text::{Role, TextRole};
use crate::theme::space;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderColor;
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, Overflow, UiRect, Val, percent,
    px,
};
use bevy::ui::widget::Text;

/// One-off layout value (not a ladder rung): the width of the line-number
/// gutter column beside the code body.
const CODE_EDITOR_GUTTER_WIDTH_PX: f32 = 48.0;

/// Marker component for the code editor container root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEditorRoot;

/// Marker component for the line number gutter column.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEditorGutter;

/// Marker component for the code content body area.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CodeEditorBody;

/// The semantic ink of one syntax token.
///
/// The token vocabulary is the shared editor's; the hue is the shared reading
/// of it, so the YAML editor and the rules JSON view read alike: a key is the
/// accent, a string is the success ink, a number is the warning ink, a
/// comment and a punctuation mark are the dim ink, and plain text is the
/// reading ink.
pub fn syntax_token_ink(kind: SyntaxTokenKind) -> Token {
    match kind {
        SyntaxTokenKind::Keyword => Token::Accent,
        SyntaxTokenKind::StringLiteral => Token::Success,
        SyntaxTokenKind::NumberLiteral => Token::Warning,
        SyntaxTokenKind::Comment => Token::InkDim,
        SyntaxTokenKind::Punctuation => Token::InkDim,
        SyntaxTokenKind::Plain => Token::Ink,
    }
}

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
                                padding: UiRect::horizontal(rem(space::S4)),
                            }
                            Children [
                                Text({ format!("{}", num) })
                                TextRole(Role::Mono)
                                ThemeTextColor({ Token::InkDim })
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
                    let ink = syntax_token_ink(tok.kind);
                    Box::new(bsn! {
                                            Text({ tok.text })
                                            TextRole(Role::Mono)
                                            ThemeTextColor({ ink })
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
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
                overflow: Overflow::clip(),
            }
            ThemeBackgroundColor({ Token::Surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            CodeEditorRoot
            Children [
                Node {
                    width: rem(CODE_EDITOR_GUTTER_WIDTH_PX),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::vertical(rem(space::S8)),
                }
                ThemeBackgroundColor({ Token::SurfaceElevated })
                CodeEditorGutter
                Children [
                    { gutter_lines }
                ]
                --
                Node {
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(rem(space::S8)),
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
    /// The locale key of this diagnostic's copy.
    ///
    /// The widget layer names the fact; the surface renders the words. The
    /// copy itself lives in the shared locale table, so the kit carries no
    /// language of its own.
    pub message_key: &'static str,
    /// The parameter the copy interpolates, when it takes one.
    pub message_arg: Option<String>,
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
                message_key: "editor_diagnostic_unclosed_quote",
                message_arg: None,
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
                            message_key: "editor_diagnostic_rule_without_target",
                            message_arg: Some(line.to_string()),
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

    /// The YAML tokenizer keeps the line's own text and only decides the
    /// kind of each piece, so the editor can paint a key, a number, a quoted
    /// string and a trailing comment apart.
    #[test]
    fn test_yaml_tokenizer_classifies_the_value_kinds() {
        let number = tokenize_yaml_line("port: 7890");
        assert_eq!(number[0].kind, SyntaxTokenKind::Keyword);
        assert_eq!(number[1].kind, SyntaxTokenKind::Punctuation);
        assert!(
            number
                .iter()
                .any(|token| token.kind == SyntaxTokenKind::NumberLiteral)
        );

        let quoted = tokenize_yaml_line("mode: \"rule\"");
        assert!(
            quoted
                .iter()
                .any(|token| token.kind == SyntaxTokenKind::StringLiteral)
        );

        let commented = tokenize_yaml_line("port: 7890 # socks5");
        assert!(
            commented
                .iter()
                .any(|token| token.kind == SyntaxTokenKind::Comment)
        );

        for line in ["port: 7890", "mode: \"rule\"", "port: 7890 # socks5"] {
            let tokens = tokenize_yaml_line(line);
            let rebuilt: String = tokens.iter().map(|token| token.text.as_str()).collect();
            assert_eq!(rebuilt, line, "the tokenizer must keep the line's text");
        }
    }

    /// The shared token vocabulary paints the same ink on every surface: a
    /// key is the accent, a string the success ink, a number the warning ink.
    #[test]
    fn test_syntax_token_ink_matches_the_shared_reading() {
        assert_eq!(syntax_token_ink(SyntaxTokenKind::Keyword), Token::Accent);
        assert_eq!(
            syntax_token_ink(SyntaxTokenKind::StringLiteral),
            Token::Success
        );
        assert_eq!(
            syntax_token_ink(SyntaxTokenKind::NumberLiteral),
            Token::Warning
        );
        assert_eq!(syntax_token_ink(SyntaxTokenKind::Comment), Token::InkDim);
        assert_eq!(
            syntax_token_ink(SyntaxTokenKind::Punctuation),
            Token::InkDim
        );
        assert_eq!(syntax_token_ink(SyntaxTokenKind::Plain), Token::Ink);
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
        assert_eq!(
            diagnostics[0].message_key,
            "editor_diagnostic_unclosed_quote"
        );
        assert_eq!(diagnostics[0].message_arg, None);

        // Line 4 is missing routing target
        assert_eq!(diagnostics[1].line, 4);
        assert_eq!(diagnostics[1].severity, DiagnosticSeverity::Warning);
        assert_eq!(
            diagnostics[1].message_key,
            "editor_diagnostic_rule_without_target"
        );
        assert_eq!(
            diagnostics[1].message_arg.as_deref(),
            Some("- IP-CIDR,192.168.1.0/24")
        );
    }
}

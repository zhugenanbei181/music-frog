//! The rules workspace's JSON source view: the line-number gutter beside the
//! highlighted document.
//!
//! The editor's *state* is the shared editor widget's ([`CodeEditorState`]);
//! this module is the rules workspace's reading of a JSON document: a gutter
//! that names every line and a source that is highlighted by the shared syntax
//! vocabulary, so a reader can see the structure of the document instead of a
//! wall of text.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::feathers::theme::ThemeTextColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::editor::{SyntaxToken, SyntaxTokenKind, syntax_token_ink};
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::rem;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// The width of the JSON editor's line-number gutter (px).
const JSON_EDITOR_GUTTER_WIDTH_PX: f32 = 44.0;

/// The height of one highlighted source line (px).
const JSON_EDITOR_LINE_HEIGHT_PX: f32 = 20.0;

/// Marker on the JSON editor panel: the surface that reveals itself.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesJsonEditorPanel;

/// The semantic ink of one syntax token is the shared editor's own reading:
/// the JSON view and the YAML editor paint the same vocabulary alike, so the
/// source reads as code on every surface.

/// Tokenize one JSON source line into the shared syntax vocabulary.
///
/// A string that a colon follows is a key; every other string is a value.
/// The tokenizer is a pure function: the same line always yields the same
/// tokens.
pub fn tokenize_json_line(line: &str) -> Vec<SyntaxToken> {
    let mut tokens: Vec<SyntaxToken> = Vec::new();
    let mut rest = line;
    while !rest.is_empty() {
        let leading = rest.len() - rest.trim_start().len();
        if leading > 0 {
            let (space, tail) = rest.split_at(leading);
            tokens.push(SyntaxToken {
                text: space.to_owned(),
                kind: SyntaxTokenKind::Plain,
            });
            rest = tail;
            continue;
        }
        let Some(first) = rest.chars().next() else {
            break;
        };
        if first == '"' {
            let end = rest[1..]
                .find('"')
                .map(|offset| offset + 2)
                .unwrap_or(rest.len());
            let text = rest[..end].to_owned();
            let after = rest[end..].trim_start();
            let kind = if after.starts_with(':') {
                SyntaxTokenKind::Keyword
            } else {
                SyntaxTokenKind::StringLiteral
            };
            tokens.push(SyntaxToken { text, kind });
            rest = &rest[end..];
            continue;
        }
        if first.is_ascii_digit()
            || (first == '-' && rest[1..].starts_with(|c: char| c.is_ascii_digit()))
        {
            let end = rest
                .find(|c: char| {
                    !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e' || c == '+')
                })
                .unwrap_or(rest.len());
            let text = rest[..end].to_owned();
            tokens.push(SyntaxToken {
                text,
                kind: SyntaxTokenKind::NumberLiteral,
            });
            rest = &rest[end..];
            continue;
        }
        if rest.starts_with("true") || rest.starts_with("false") || rest.starts_with("null") {
            let end = rest
                .find(|c: char| !c.is_ascii_alphabetic())
                .unwrap_or(rest.len());
            let text = rest[..end].to_owned();
            tokens.push(SyntaxToken {
                text,
                kind: SyntaxTokenKind::Plain,
            });
            rest = &rest[end..];
            continue;
        }
        if rest.starts_with("//") {
            let text = rest.to_owned();
            tokens.push(SyntaxToken {
                text,
                kind: SyntaxTokenKind::Comment,
            });
            rest = "";
            continue;
        }
        let width = first.len_utf8();
        tokens.push(SyntaxToken {
            text: rest[..width].to_owned(),
            kind: SyntaxTokenKind::Punctuation,
        });
        rest = &rest[width..];
    }
    tokens
}

/// The JSON editor body: the line-number gutter beside the highlighted source.
pub fn json_editor_body_scene(state: &CodeEditorState) -> Box<dyn Scene> {
    let gutter_lines: Vec<Box<dyn Scene>> = (1..=state.line_count())
        .map(|number| {
            Box::new(bsn! {
                    Node {
                        height: rem(JSON_EDITOR_LINE_HEIGHT_PX),
                        justify_content: JustifyContent::FlexEnd,
                        padding: UiRect::horizontal(rem(space::S4)),
                    }
                    Children [
                        Text({ format!("{number}") })
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
            let token_nodes: Vec<Box<dyn Scene>> = tokenize_json_line(line)
                .into_iter()
                .map(|token| {
                    let ink = syntax_token_ink(token.kind);
                    Box::new(bsn! {
                            Text({ token.text })
                            TextRole(Role::Mono)
                            ThemeTextColor({ ink })
                    }) as Box<dyn Scene>
                })
                .collect();
            Box::new(bsn! {
                    Node {
                        height: rem(JSON_EDITOR_LINE_HEIGHT_PX),
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
                flex_direction: FlexDirection::Row,
            }
            Children [
                Node {
                    width: rem(JSON_EDITOR_GUTTER_WIDTH_PX),
                    flex_shrink: 0.0,
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::vertical(rem(space::S4)),
                }
                ThemeBackgroundColor({ Token::SurfaceElevated })
                Children [
                    { gutter_lines }
                ]
                --
                Node {
                    flex_grow: 1.0,
                    min_width: px(0.0),
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    { code_lines }
                ]
            ]
    })
}

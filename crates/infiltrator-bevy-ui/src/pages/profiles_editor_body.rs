//! DUAL-09-02/13/14: the bounded-window row renderer shared by the Bevy
//! editor's document panes (Profile and Mixin).
//!
//! Split out of `profiles_editor.rs` so the card module stays inside the
//! business line budget while the Mixin pane renders through exactly the same
//! rows, gutter, indentation rail and diagnostic wash as the profile document.

use bevy::color::Color;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, FlexDirection, JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::editor::{SyntaxTokenKind, tokenize_yaml_line};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

use crate::pages::profiles_editor_state::ProfileEditorState;

fn token_color(kind: SyntaxTokenKind, palette: &UiPalette) -> Color {
    match kind {
        SyntaxTokenKind::Comment => palette.ink_dim,
        SyntaxTokenKind::Keyword => palette.accent,
        SyntaxTokenKind::StringLiteral => palette.success,
        SyntaxTokenKind::NumberLiteral => palette.warning,
        SyntaxTokenKind::Punctuation => palette.ink_dim,
        SyntaxTokenKind::Plain => palette.ink,
    }
}

/// Gutter + code rows for the current window, with the cursor line washed and
/// the diagnostic line painted in the danger token. DUAL-09-02: the window,
/// the hidden-line counts and the indentation levels come from the shared
/// `EditorViewport`, so every pane and the Iced editor never drift.
pub(crate) fn editor_rows_scene(state: &ProfileEditorState, palette: &UiPalette) -> Box<dyn Scene> {
    let viewport = state.viewport();
    let diagnostic_row = state.diagnostic.as_ref().map(|diagnostic| diagnostic.line);
    let cursor_row = state.buffer.cursor_row + 1;
    let mut rows: Vec<Box<dyn Scene>> = Vec::with_capacity(viewport.rendered_len() + 2);
    if viewport.hidden_above() > 0 {
        rows.push(notice_row(
            &format!(
                "… 上方还有 {} 行未渲染（跟随光标的有界窗口）",
                viewport.hidden_above()
            ),
            palette,
        ));
    }
    for (number, line, indent_level) in state.rendered_lines() {
        let is_diagnostic = diagnostic_row == Some(number);
        let is_cursor = cursor_row == number && state.focused;
        let background = if is_diagnostic {
            palette.danger
        } else if is_cursor {
            palette.surface_elevated
        } else {
            palette.window_clear
        };
        let gutter_color = if is_diagnostic {
            palette.window_clear
        } else {
            palette.ink_dim
        };
        let tokens: Vec<Box<dyn Scene>> = tokenize_yaml_line(line)
            .into_iter()
            .map(|token| {
                let color = if is_diagnostic {
                    palette.window_clear
                } else {
                    token_color(token.kind, palette)
                };
                Box::new(bsn! {
                    (
                        Text({ token.text })
                        TextRole(Role::Mono)
                        TextColor({ color })
                    )
                }) as Box<dyn Scene>
            })
            .collect();
        rows.push(Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Row,
                column_gap: Val::Px(space::S8),
                min_height: px(16.0),
            }
            BackgroundColor({ background })
            Children [
                (
                    Node {
                        min_width: px(36.0),
                        justify_content: JustifyContent::FlexEnd,
                    }
                    Children [
                        (
                            Text({ format!("{number}") })
                            TextRole(Role::Mono)
                            TextColor({ gutter_color })
                        ),
                    ]
                ),
                ( { indent_rail(indent_level, palette) } ),
                { tokens },
            ]
        }));
    }
    if viewport.hidden_below() > 0 {
        rows.push(notice_row(
            &format!(
                "… 下方还有 {} 行未渲染（有界窗口，不是虚拟滚动）",
                viewport.hidden_below()
            ),
            palette,
        ));
    }
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
        }
        Children [
            { rows },
        ]
    })
}

/// DUAL-09-02: the shared indentation reference, rendered as a rail at the
/// start of the row (one tick per closed indentation level).
fn indent_rail(level: usize, palette: &UiPalette) -> Box<dyn Scene> {
    let color = palette.accent_container;
    let mut ticks: Vec<Box<dyn Scene>> = Vec::with_capacity(level);
    for _ in 0..level {
        ticks.push(Box::new(bsn! {
            Node {
                width: px(2.0),
                height: px(10.0),
                margin: UiRect::right(Val::Px(2.0)),
            }
            BackgroundColor({ color })
        }) as Box<dyn Scene>);
    }
    Box::new(bsn! {
        Node {
            flex_direction: FlexDirection::Row,
            min_width: px(4.0),
        }
        Children [
            { ticks },
        ]
    })
}

fn notice_row(text: &str, palette: &UiPalette) -> Box<dyn Scene> {
    let label = text.to_owned();
    let color = palette.warning;
    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
        }
        Children [
            ( Text({ label }) TextRole(Role::Caption) TextColor({ color }) ),
        ]
    })
}

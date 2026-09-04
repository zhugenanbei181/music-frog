//! Breakpoint-aware and multi-end smart text truncation engine.
//!
//! Provides Unicode-grapheme safe tail truncation, middle truncation (e.g. for URLs,
//! IPs, hashes, proxy node names), path truncation, and breakpoint-adaptive rule application.

use bevy::ecs::component::Component;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use unicode_segmentation::UnicodeSegmentation;

use crate::responsive::ResponsiveContext;
use crate::theme::Breakpoint;

/// Rules for smart text truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TruncateRule {
    /// Truncate at the end if length exceeds `max_chars` (e.g. "Long text..." -> "Long...").
    Tail { max_chars: usize },
    /// Middle truncation keeping specified head and tail characters (e.g. "192.168.1.1:8080" -> "192.168…:8080").
    Middle {
        head_chars: usize,
        tail_chars: usize,
    },
    /// Adaptive character limit based on active 4-tier breakpoint.
    Adaptive {
        compact: usize,
        medium: usize,
        expanded: usize,
        ultra: usize,
    },
    /// Adaptive middle truncation based on active 4-tier breakpoint.
    AdaptiveMiddle {
        compact: (usize, usize),
        medium: (usize, usize),
        expanded: (usize, usize),
        ultra: (usize, usize),
    },
}

impl Default for TruncateRule {
    fn default() -> Self {
        Self::Adaptive {
            compact: 16,
            medium: 28,
            expanded: 48,
            ultra: 80,
        }
    }
}

/// Unicode-safe tail truncation with ellipsis.
pub fn truncate_tail(text: &str, max_chars: usize) -> String {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    if graphemes.len() <= max_chars {
        text.to_owned()
    } else {
        let keep = max_chars.saturating_sub(1);
        let mut out = graphemes[..keep].concat();
        out.push('…');
        out
    }
}

/// Unicode-safe middle truncation with ellipsis.
pub fn truncate_middle(text: &str, head_chars: usize, tail_chars: usize) -> String {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    let total = graphemes.len();
    if total <= head_chars + tail_chars + 1 {
        text.to_owned()
    } else {
        let head = graphemes[..head_chars].concat();
        let tail = graphemes[total - tail_chars..].concat();
        format!("{head}…{tail}")
    }
}

/// Adaptive tail truncation across 4-tier breakpoints.
pub fn truncate_adaptive(
    text: &str,
    bp: Breakpoint,
    compact: usize,
    medium: usize,
    expanded: usize,
    ultra: usize,
) -> String {
    let max = match bp {
        Breakpoint::Compact => compact,
        Breakpoint::Medium => medium,
        Breakpoint::Expanded => expanded,
        Breakpoint::Ultra => ultra,
    };
    truncate_tail(text, max)
}

/// Adaptive middle truncation across 4-tier breakpoints.
pub fn truncate_adaptive_middle(
    text: &str,
    bp: Breakpoint,
    compact: (usize, usize),
    medium: (usize, usize),
    expanded: (usize, usize),
    ultra: (usize, usize),
) -> String {
    let (head, tail) = match bp {
        Breakpoint::Compact => compact,
        Breakpoint::Medium => medium,
        Breakpoint::Expanded => expanded,
        Breakpoint::Ultra => ultra,
    };
    truncate_middle(text, head, tail)
}

/// Apply a [`TruncateRule`] to a text slice under an active breakpoint.
pub fn apply_truncate_rule(text: &str, rule: &TruncateRule, bp: Breakpoint) -> String {
    match rule {
        TruncateRule::Tail { max_chars } => truncate_tail(text, *max_chars),
        TruncateRule::Middle {
            head_chars,
            tail_chars,
        } => truncate_middle(text, *head_chars, *tail_chars),
        TruncateRule::Adaptive {
            compact,
            medium,
            expanded,
            ultra,
        } => truncate_adaptive(text, bp, *compact, *medium, *expanded, *ultra),
        TruncateRule::AdaptiveMiddle {
            compact,
            medium,
            expanded,
            ultra,
        } => truncate_adaptive_middle(text, bp, *compact, *medium, *expanded, *ultra),
    }
}

/// Marker component for a text node whose content is dynamically truncated based on layout breakpoints.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct SmartTruncateText {
    /// Full, un-truncated original string.
    pub full_text: String,
    /// Truncation rule to apply.
    pub rule: TruncateRule,
}

impl SmartTruncateText {
    pub fn new(text: impl Into<String>, rule: TruncateRule) -> Self {
        Self {
            full_text: text.into(),
            rule,
        }
    }

    pub fn tail(text: impl Into<String>, max_chars: usize) -> Self {
        Self::new(text, TruncateRule::Tail { max_chars })
    }

    pub fn middle(text: impl Into<String>, head_chars: usize, tail_chars: usize) -> Self {
        Self::new(
            text,
            TruncateRule::Middle {
                head_chars,
                tail_chars,
            },
        )
    }

    pub fn adaptive(
        text: impl Into<String>,
        compact: usize,
        medium: usize,
        expanded: usize,
        ultra: usize,
    ) -> Self {
        Self::new(
            text,
            TruncateRule::Adaptive {
                compact,
                medium,
                expanded,
                ultra,
            },
        )
    }

    pub fn adaptive_middle(
        text: impl Into<String>,
        compact: (usize, usize),
        medium: (usize, usize),
        expanded: (usize, usize),
        ultra: (usize, usize),
    ) -> Self {
        Self::new(
            text,
            TruncateRule::AdaptiveMiddle {
                compact,
                medium,
                expanded,
                ultra,
            },
        )
    }
}

/// System to dynamically restamp [`Text`] nodes tagged with [`SmartTruncateText`].
pub fn sync_smart_truncate_text(
    ctx: Option<Res<ResponsiveContext>>,
    mut query: Query<(&mut Text, &SmartTruncateText)>,
) {
    let bp = ctx.map(|c| c.breakpoint).unwrap_or(Breakpoint::Expanded);
    for (mut text, smart) in &mut query {
        let want = apply_truncate_rule(&smart.full_text, &smart.rule, bp);
        if text.0 != want {
            text.0 = want;
        }
    }
}

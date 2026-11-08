//! The typed content structure of every shared shell page.
//!
//! A page's *shape* decides how much empty ground is legitimate. A page
//! whose body is one frame (a list whose rows scroll inside the frame) has
//! no room for empty ground at all: the frame fills the page body. A page
//! that lays its blocks out by content leaves ground below the last block,
//! and how much is legitimate depends on the peer product's behaviour for
//! the same shape, not on one global number.
//!
//! This registry is the single owner of that shape. The space-utilization
//! judge reads it to pick a per-page limit; a surface never decides for
//! itself how empty a page is allowed to be.

use crate::command_catalogue::ShellPage;

/// The page shape a capture slug renders.
///
/// The slug is the capture matrix's vocabulary; the shape is the contract's.
/// A workbench slug is a DIFFERENT shape from its mother page: a list page
/// frames its body and scrolls inside the frame, while a workbench lays out
/// its panels by content and ends where its last panel ends. Judging a
/// workbench by its mother page's frame rule would flag the reference
/// product's own layout as broken.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PageShape {
    pub page: ShellPage,
    pub structure: PageStructure,
}

/// Resolve a capture page slug onto the page and the shape it renders.
pub fn shape_for_slug(slug: &str) -> Option<PageShape> {
    let (page, structure) = match slug {
        "overview" => (ShellPage::Overview, PageStructure::Dense),
        "overview_workbench" => (ShellPage::Overview, PageStructure::Content),
        "proxies" => (ShellPage::Proxies, PageStructure::Content),
        "proxies_workbench" => (ShellPage::Proxies, PageStructure::Content),
        "profiles" => (ShellPage::Profiles, PageStructure::Content),
        "rules" => (ShellPage::Rules, PageStructure::Framed),
        "rules_workbench" => (ShellPage::Rules, PageStructure::Content),
        "connections" => (ShellPage::Connections, PageStructure::Framed),
        "connections_workbench" => (ShellPage::Connections, PageStructure::Content),
        "logs" => (ShellPage::Logs, PageStructure::Framed),
        "logs_workbench" => (ShellPage::Logs, PageStructure::Content),
        "dns" => (ShellPage::Dns, PageStructure::Content),
        "doctor" => (ShellPage::Doctor, PageStructure::Content),
        "app_routing" => (ShellPage::AppRouting, PageStructure::Content),
        "sync" => (ShellPage::Sync, PageStructure::Content),
        "settings" => (ShellPage::Settings, PageStructure::Content),
        "settings_workbench" | "shell_workbench" => (ShellPage::Settings, PageStructure::Content),
        "about" => (ShellPage::About, PageStructure::Content),
        "editor" => (ShellPage::Settings, PageStructure::Content),
        "filters" => (ShellPage::Settings, PageStructure::Content),
        "runtime" => (ShellPage::Overview, PageStructure::Content),
        "runtime_workbench" => (ShellPage::Overview, PageStructure::Content),
        _ => return None,
    };
    Some(PageShape { page, structure })
}

/// Emit the page-shape table for the cross-language space-utilization judge.
///
/// The judge is a Python script; it must not keep its own copy of the
/// limits. This JSON is the exported truth, and the judge reads it instead
/// of hard-coding a number.
pub fn page_structure_json() -> String {
    let pages: Vec<String> = ShellPage::ALL
        .iter()
        .map(|page| {
            let structure = page.structure();
            format!(
                "{{\"id\":\"{}\",\"structure\":\"{}\",\"ground_limit_pct\":{}}}",
                page.id(),
                structure.id(),
                ground_limit(structure)
            )
        })
        .collect();
    let slugs: Vec<String> = [
        "overview",
        "overview_workbench",
        "proxies",
        "proxies_workbench",
        "profiles",
        "rules",
        "rules_workbench",
        "connections",
        "connections_workbench",
        "logs",
        "logs_workbench",
        "dns",
        "doctor",
        "app_routing",
        "sync",
        "settings",
        "settings_workbench",
        "shell_workbench",
        "about",
        "editor",
        "filters",
        "runtime",
        "runtime_workbench",
    ]
    .iter()
    .filter_map(|slug| {
        shape_for_slug(slug).map(|shape| {
            format!(
                "{{\"slug\":\"{}\",\"page\":\"{}\",\"structure\":\"{}\",\"ground_limit_pct\":{}}}",
                slug,
                shape.page.id(),
                shape.structure.id(),
                ground_limit(shape.structure)
            )
        })
    })
    .collect();
    format!(
        "{{\"version\":1,\"pages\":[{}],\"slugs\":[{}]}}\n",
        pages.join(","),
        slugs.join(",")
    )
}

/// How a page fills its body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PageStructure {
    /// One card frames the body and its content scrolls inside; the frame
    /// reaches the bottom of the page body, so no ground may show below it.
    Framed,
    /// Blocks are laid out by content and the page ends where its last
    /// block ends.
    Content,
    /// Blocks are laid out by content, and the page carries a dense set of
    /// small modules that fill the body by design.
    Dense,
}

impl PageStructure {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Framed => "framed",
            Self::Content => "content",
            Self::Dense => "dense",
        }
    }
}

impl ShellPage {
    /// The content shape of this page.
    ///
    /// The judgement follows the peer product's measured behaviour for the
    /// same shape: a framed list page is filled to the bottom, a content
    /// page ends with its last block (the reference product's own agent and
    /// about pages leave ground below their last row), and the overview is a
    /// dense card grid.
    pub const fn structure(self) -> PageStructure {
        match self {
            Self::Overview => PageStructure::Dense,
            Self::Connections | Self::Logs | Self::Rules => PageStructure::Framed,
            Self::Proxies
            | Self::Profiles
            | Self::Dns
            | Self::Doctor
            | Self::AppRouting
            | Self::Sync
            | Self::Settings
            | Self::Tools
            | Self::About
            | Self::Disclaimer
            | Self::Licenses => PageStructure::Content,
        }
    }
}

/// The measured ground a peer product leaves below the last content row of
/// a content-shaped page, as a percentage of the page body.
///
/// Measured on the FlClash desktop reference (v0.9.3): its proxies page
/// ends at y357 of a 1849px body (80.6% empty) and its tools page lists its
/// sections by content. A content page may therefore leave ground below its
/// last block; the limit is the reference's own behaviour plus a margin, so
/// a page that empties more than the reference does still fails.
pub const CONTENT_PAGE_GROUND_LIMIT_PCT: f64 = 82.0;

/// A framed page may not leave measurable ground: the frame reaches the
/// bottom of the page body.
pub const FRAMED_PAGE_GROUND_LIMIT_PCT: f64 = 4.0;

/// A dense page is a card grid; only a card's own trailing air counts.
pub const DENSE_PAGE_GROUND_LIMIT_PCT: f64 = 12.0;

/// The ground limit for one page shape.
pub const fn ground_limit(structure: PageStructure) -> f64 {
    match structure {
        PageStructure::Framed => FRAMED_PAGE_GROUND_LIMIT_PCT,
        PageStructure::Dense => DENSE_PAGE_GROUND_LIMIT_PCT,
        PageStructure::Content => CONTENT_PAGE_GROUND_LIMIT_PCT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_shared_page_declares_a_structure() {
        for page in ShellPage::ALL {
            let structure = page.structure();
            assert!(!structure.id().is_empty());
            assert!(ground_limit(structure) > 0.0);
        }
    }

    #[test]
    fn a_framed_page_is_judged_stricter_than_a_content_page() {
        assert!(ground_limit(PageStructure::Framed) < ground_limit(PageStructure::Content));
    }

    #[test]
    fn the_list_pages_are_framed_and_the_about_page_is_content() {
        assert_eq!(ShellPage::Logs.structure(), PageStructure::Framed);
        assert_eq!(ShellPage::Connections.structure(), PageStructure::Framed);
        assert_eq!(ShellPage::Rules.structure(), PageStructure::Framed);
        assert_eq!(ShellPage::About.structure(), PageStructure::Content);
        assert_eq!(ShellPage::Proxies.structure(), PageStructure::Content);
        assert_eq!(ShellPage::Overview.structure(), PageStructure::Dense);
    }
}

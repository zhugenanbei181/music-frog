//! SPACEUTIL-008 / FLPARITY-053: the proxy node grid's column count comes
//! from the page body's width, not from the navigation column's labels.
//!
//! FlClash splits its pages by `getViewMode` (`lib/common/layout.dart`):
//! `viewWidth <= 600` is the mobile view, and the laptop range above it is
//! still the desktop page - only the navigation column collapses to icons
//! (`canExpandSidebar` is desktop-only). The node grid then asks the page
//! body's own width for its columns: `getProxiesColumns(max(width - 32, 0))`
//! divides that body by the 250dp floor and keeps at least two columns, with
//! no view-mode branch anywhere in it (`lib/views/proxies/tab.dart`).
//!
//! This surface used to answer "how many node cards fit" from whether the
//! tier owned a *labelled* navigation column. The laptop range owns an icon
//! rail instead, so a 624dp body was stacked one node per row where the
//! reference draws three columns, and the measured dead ground under the
//! single card was the surface's own fault. The cases below pin the rule to
//! the one exported lookup - `ViewportLadder.proxy_columns(form, page-width)`,
//! generated from the contract's `proxy_node_columns` - and pin the form to
//! one named tier fact, so neither the page nor the theme may keep a private
//! width number or a second opinion about which rule owns the grid.

use infiltrator_contract::responsive_viewport::{
    PROXY_MIN_COLUMNS, PROXY_MIN_ITEM_WIDTH_PX, ProxyGridForm, ViewportTier, proxy_node_columns,
};

const PAGE: &str = include_str!("../ui/pages/page_proxies.slint");
const THEME: &str = include_str!("../ui/components/theme.slint");

/// The surface's own declaration of the form, spelled the way the theme
/// spells it. The numeric cases below evaluate this exact text.
const SURFACE_FORM_RULE: &str =
    "root.viewport-tier == ViewportTier.compact ? ProxyGridForm.stacked : ProxyGridForm.desktop";

/// The page's own call into the exported lookup.
const SURFACE_COLUMN_RULE: &str =
    "ViewportLadder.proxy_columns(LumenTheme.proxy-grid-form, self.width)";

/// Drops `//` line comments so a rule cannot hide in prose.
fn strip_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| match line.find("//") {
            Some(at) => &line[..at],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The text a binding is bound to: everything between its declaration and
/// the terminating semicolon, with the line breaks folded away.
fn binding(source: &str, declaration: &str) -> Option<String> {
    let code = strip_comments(source);
    let start = code.find(declaration)? + declaration.len();
    let end = code[start..].find(';')? + start;
    Some(
        code[start..end]
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    )
}

/// The one tier fact that owns the grid's form.
fn form_fact(theme: &str) -> Option<String> {
    binding(theme, "out property <ProxyGridForm> proxy-grid-form:")
}

/// Everything the surface gets wrong about the proxy grid's column rule.
fn findings(theme: &str, page: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    let theme_code = strip_comments(theme);
    let page_code = strip_comments(page);

    let owned = theme_code.matches("proxy-grid-form").count();
    if owned != 1 {
        found.push(format!(
            "components/theme.slint: the proxy grid form is declared {owned} times; \
             one tier fact owns it"
        ));
    }

    match form_fact(theme) {
        None => found.push(
            "components/theme.slint: the proxy grid form has no tier fact of its own".to_string(),
        ),
        Some(rule) => {
            if rule != SURFACE_FORM_RULE {
                found.push(format!(
                    "components/theme.slint: the proxy grid form is `{rule}`, not the tier \
                     boundary `{SURFACE_FORM_RULE}`"
                ));
            }
            for retired in ["desktop-page", "labelled-sidebar"] {
                if rule.contains(retired) {
                    found.push(format!(
                        "components/theme.slint: the proxy grid form still answers `{retired}`, \
                         a navigation fact that leaves the laptop range one card per row"
                    ));
                }
            }
        }
    }

    match binding(page, "in property <int> node-columns:") {
        None => {
            found.push("pages/page_proxies.slint: the node grid lost its column count".to_string())
        }
        Some(rule) => {
            if rule != SURFACE_COLUMN_RULE {
                found.push(format!(
                    "pages/page_proxies.slint: the node grid's column count is `{rule}`, not \
                     the exported lookup `{SURFACE_COLUMN_RULE}`"
                ));
            }
            for retired in ["desktop-page", "labelled-sidebar"] {
                if rule.contains(retired) {
                    found.push(format!(
                        "pages/page_proxies.slint: the node grid's column count still follows \
                         `{retired}` instead of the page body's own width"
                    ));
                }
            }
            if rule.contains(&PROXY_MIN_ITEM_WIDTH_PX.to_string()) || rule.contains("32") {
                found.push(
                    "pages/page_proxies.slint: the node grid keeps a private width rule"
                        .to_string(),
                );
            }
        }
    }

    // The reference's 250dp card floor belongs to the contract; a page that
    // writes it down owns a second copy of the rule.
    if page_code.contains(&PROXY_MIN_ITEM_WIDTH_PX.to_string()) {
        found.push(
            "pages/page_proxies.slint: the reference's 250dp card floor is written down again"
                .to_string(),
        );
    }

    found
}

/// The rule the surface states, evaluated against the shared contract.
fn surface_columns(tier: ViewportTier, window_width_px: u16) -> u16 {
    let form = if tier == ViewportTier::Compact {
        ProxyGridForm::Stacked
    } else {
        ProxyGridForm::Desktop
    };
    proxy_node_columns(form, tier.page_body_width_px(window_width_px))
}

#[test]
fn the_node_grid_reads_the_exported_width_rule() {
    assert_eq!(findings(THEME, PAGE), Vec::<String>::new());
}

#[test]
fn the_page_body_width_decides_the_column_count() {
    // The published matrix's own windows. The body each one leaves is the
    // contract's `page_body_width_px`, so no width is written down here.
    let cases = [
        // A phone (400x800): the mobile view keeps its one readable column.
        (400.0_f32, ViewportTier::Compact, 1_u16),
        // A laptop window (720x480): FlClash's laptop range is the desktop
        // page with an icon rail, and its 624dp body draws three columns.
        (720.0_f32, ViewportTier::Medium, 3_u16),
        // A desktop window (1180x780): the 908dp body draws four columns.
        (1180.0_f32, ViewportTier::Expanded, 4_u16),
    ];

    for (window_width_px, tier, expected) in cases {
        assert_eq!(ViewportTier::from_width(window_width_px), tier);
        assert_eq!(
            surface_columns(tier, window_width_px as u16),
            expected,
            "the {tier:?} tier at {window_width_px}px"
        );
    }
}

#[test]
fn the_column_count_is_the_contracts_own_arithmetic() {
    // The expected numbers above are the contract's arithmetic, not a copy of
    // it: the laptop range's three columns are the 624dp body divided by the
    // exported 250dp floor, and the floor of two columns is the reference's.
    let medium_body = ViewportTier::Medium.page_body_width_px(720);
    assert_eq!(medium_body, 624);
    assert_eq!(
        proxy_node_columns(ProxyGridForm::Desktop, medium_body),
        medium_body
            .div_ceil(PROXY_MIN_ITEM_WIDTH_PX)
            .max(PROXY_MIN_COLUMNS)
    );
    assert_eq!(proxy_node_columns(ProxyGridForm::Stacked, medium_body), 1);
}

#[test]
fn a_grid_that_follows_the_navigation_column_is_caught() {
    let planted = PAGE.replace(
        SURFACE_COLUMN_RULE,
        "ViewportLadder.proxy_columns(LumenTheme.desktop-page ? \
         ProxyGridForm.desktop : ProxyGridForm.stacked, self.width)",
    );
    assert_ne!(planted, PAGE);
    let found = findings(THEME, &planted);
    assert!(
        found
            .iter()
            .any(|item| item.contains("navigation column") || item.contains("exported lookup")),
        "{found:?}"
    );
}

#[test]
fn a_private_width_rule_is_caught() {
    let planted = PAGE.replace(
        SURFACE_COLUMN_RULE,
        "Math.max(2, Math.ceil((self.width - 32 * LumenUiScale.unit) / 250 * LumenUiScale.unit))",
    );
    assert_ne!(planted, PAGE);
    let found = findings(THEME, &planted);
    assert!(
        found
            .iter()
            .any(|item| item.contains("private width rule") || item.contains("exported lookup")),
        "{found:?}"
    );
}

#[test]
fn a_form_that_follows_the_navigation_column_is_caught() {
    let planted = THEME.replace(
        SURFACE_FORM_RULE,
        "LumenTheme.desktop-page ? ProxyGridForm.desktop : ProxyGridForm.stacked",
    );
    assert_ne!(planted, THEME);
    let found = findings(&planted, PAGE);
    assert!(
        found
            .iter()
            .any(|item| item.contains("navigation fact") || item.contains("tier boundary")),
        "{found:?}"
    );
}

#[test]
fn a_second_form_fact_is_caught() {
    let planted = THEME.replace(
        "out property <ProxyGridForm> proxy-grid-form:",
        "out property <ProxyGridForm> proxy-grid-form:\
         out property <ProxyGridForm> proxy-grid-form:",
    );
    assert_ne!(planted, THEME);
    let found = findings(&planted, PAGE);
    assert!(
        found.iter().any(|item| item.contains("declared")),
        "{found:?}"
    );
}

//! The business panels of the editor workbench page.

use super::Panel;
use crate::surface::business::BusinessId;
use infiltrator_contract::surface_snapshot::SurfaceSnapshot;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::copy::TextCatalog;

/// The business panels the editor page carries.
pub(super) fn panels(snapshot: &SurfaceSnapshot, catalog: &Lang<'_>) -> Vec<Panel> {
    let diff = snapshot.yaml_ast_diff.clone();
    let differences = diff
        .as_ref()
        .map(|diff| diff.unified_lines.len())
        .unwrap_or(0);
    let source = diff
        .as_ref()
        .map(|diff| diff.source_id.clone())
        .unwrap_or_default();
    let snippets = YAML_SNIPPETS
        .iter()
        .map(|snippet| snippet.id.to_owned())
        .collect::<Vec<_>>();

    let mut snippet_panel = Panel::new(
        BusinessId::EditorSnippetInsertion,
        catalog.text("yaml_snippets_title"),
        catalog.text("profiles_snippet_quick_insert").into_owned(),
        vec![
            catalog.text("editor_viewport_label").into_owned(),
            catalog.text("common_details").into_owned(),
        ],
    );
    snippet_panel.facts.extend(snippets);

    vec![
        snippet_panel,
        Panel::new(
            BusinessId::EditorLargeDocumentWindow,
            catalog.text("editor_gutter_window"),
            catalog
                .text("editor_viewport_hidden_above")
                .replace("{count}", "0"),
            vec![
                catalog
                    .text("editor_viewport_hidden_below")
                    .replace("{count}", "0"),
                catalog.text("editor_history_refresh").into_owned(),
            ],
        ),
        Panel::new(
            BusinessId::EditorYamlErrorNavigation,
            catalog
                .text("editor_document_title")
                .replace("{lines}", &differences.to_string()),
            format!("{differences}"),
            vec![
                if source.is_empty() {
                    catalog.text("editor_apply_stage_committed").into_owned()
                } else {
                    source
                },
                catalog.text("editor_syntax_failed_label").into_owned(),
            ],
        ),
    ]
}

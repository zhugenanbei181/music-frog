//! The business panels of the editor workbench page.
//!
//! Every case drives the production Slint callbacks through the headless
//! surface harness, so a panel that lost its shared command or its shared
//! read model fails here.

#[path = "business_panels/fixture.rs"]
mod fixture;

use fixture::{assert_accepted, assert_refused, business_surface};
use infiltrator_slint::surface::business::BusinessId;

/// The page that carries the editor businesses.
const PAGE: &str = "Editor";

#[test]
fn the_editor_large_document_window_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::EditorLargeDocumentWindow);
}

#[test]
fn the_editor_snippet_insertion_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::EditorSnippetInsertion);
}

#[test]
fn the_editor_snippet_insertion_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::EditorSnippetInsertion);
}

#[test]
fn the_editor_yaml_error_navigation_success_path() {
    let surface = business_surface(PAGE);

    assert_accepted(&surface, BusinessId::EditorYamlErrorNavigation);
}

#[test]
fn the_editor_yaml_error_navigation_error_path() {
    let surface = business_surface(PAGE);

    assert_refused(&surface, BusinessId::EditorYamlErrorNavigation);
}

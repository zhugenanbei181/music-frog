//! DUAL-09-14: the Bevy editor's Mixin and Filter panes (state + scenes).
//!
//! The card edits three stored documents for the profile the shared
//! application has open:
//!
//! * the profile document itself (`profiles_editor.rs`);
//! * the **Mixin overlay** sidecar, rendered as a YAML buffer and committed
//!   through `ProfileOptionsApplication::save_mixin` — the same use-case the
//!   Iced Mixin pane calls;
//! * the **subscription filter** draft, rendered as the four shared
//!   `SubscriptionFilterDraft` fields and committed through
//!   `ProfileOptionsApplication::save_filter` (the shared pipeline runner).
//!
//! DUAL-10-05/14: the script console is now mirrored through the shared read
//! model: Iced runs `ScriptApplication` and publishes
//! `SurfaceSnapshot.script_sandbox`, and the Profiles page's script console
//! card renders the exact same projection. The pane row states the honest
//! engine fact (a directive DSL, not a JavaScript engine).

use crate::localized_widgets::{localized_field_scene, localized_pill_scene};
use crate::pages::profiles::ProfilesProjection;
use crate::pages::profiles_editor_filter::{
    EditorFilterDiscard, EditorFilterText, EditorFilterTransaction,
};
use crate::pages::profiles_editor_mixin_studio::MixinStudioBody;
use crate::pages::profiles_editor_state::{ProfileEditorState, diagnostic_line, status_line};
use crate::pages::profiles_editor_transactions::EditorMutationControl;
use crate::pages::profiles_editor_transactions::discard_scene;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::scene::{Scene, bsn};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, FlexWrap,
    JustifyContent, Node, UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_application::profile_editor_projection;
use infiltrator_application::subscription_filter_copy::status;
use infiltrator_application::subscription_filter_editor::SubscriptionFilterEditor;
use infiltrator_bevy_widgets::button::ButtonDisabled;
use infiltrator_bevy_widgets::editor::state::CodeEditorState;
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::native::NativeTextField;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::command::RequestId;
use infiltrator_contract::profile_source::ProfileSourceIdentity;
use infiltrator_contract::subscription_filter_form::{FilterField, FilterObservation};
use infiltrator_contract::subscription_import::SubscriptionFilterDedup;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;

/// Which document the editor card shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProfileEditorPane {
    #[default]
    Profile,
    Mixin,
    Filter,
}

impl ProfileEditorPane {
    pub const ALL: [Self; 3] = [Self::Profile, Self::Mixin, Self::Filter];

    pub const fn label_key(self) -> &'static str {
        match self {
            Self::Profile => "editor_pane_yaml",
            Self::Mixin => "editor_pane_mixin",
            Self::Filter => "editor_pane_filter",
        }
    }
}

/// The filter field root; its first child is the controlled `TextField`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorFilterField {
    pub kind: Option<FilterField>,
}

/// Pane switch button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorPaneButton {
    pub pane: ProfileEditorPane,
}

/// Areas that belong to exactly one pane; the visibility system toggles them.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProfileEditorPaneArea {
    pub pane: ProfileEditorPane,
}

/// Grab the keyboard for the Mixin buffer.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorFocusButton;

/// Reload the stored sidecar through the shared application.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorReloadButton;

/// Commit the Mixin buffer through the shared sidecar use-case.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorSaveButton;

/// One Mixin snippet button (index into the shared catalogue).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorSnippetButton {
    pub index: usize,
}

/// Container whose children are the Mixin pane's rendered rows.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorBody;

/// Restamped Mixin status line and diagnostic.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorStatusText;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorDiagnosticText;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MixinEditorDiagnosticPill;

/// Filter dedup strategy chip (index into the shared four-value vocabulary).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorFilterDedupButton {
    pub index: usize,
}

/// Run the shared filter pipeline over the stored document.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorFilterSaveButton;

/// Restamped filter-pane status (draft state + refusal reason).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorFilterStatusText;

/// Pane state that is not part of the profile document buffer.
#[derive(Resource, Clone, Debug)]
pub struct ProfileEditorOptionsState {
    pub pane: ProfileEditorPane,
    /// The Mixin overlay buffer — the same pure state machine the profile
    /// document uses, so the window/gutter/preflight rules cannot drift.
    pub mixin: ProfileEditorState,
    /// The last stored Mixin YAML adopted from the shared snapshot.
    pub mixin_loaded: Option<String>,
    /// The shared filter draft currently rendered by the Filter pane.
    pub filter: SubscriptionFilterEditor,
    /// The last stored draft adopted from the shared snapshot; compared so a
    /// re-published cache never clobbers an in-progress edit.
    pub filter_focus: Option<FilterField>,
    pub filter_request: Option<(RequestId, u64)>,
    pub filter_restore: bool,
    /// Last refusal/notice from a filter action.
    /// The profile whose sidecar has already been requested, so opening a
    /// pane does not spam the shared command pump.
    pub requested_for: Option<String>,
    /// DUAL-10-08/11: last (document, overlay) generations the shared-studio
    /// rows were rendered for, so the body rebuilds when either moves.
    pub studio_generation: (u64, u64),
}

impl Default for ProfileEditorOptionsState {
    fn default() -> Self {
        Self {
            pane: ProfileEditorPane::Profile,
            mixin: ProfileEditorState::default(),
            mixin_loaded: None,
            filter: SubscriptionFilterEditor::default(),
            filter_focus: None,
            filter_request: None,
            filter_restore: false,
            requested_for: None,
            studio_generation: (u64::MAX, u64::MAX),
        }
    }
}

impl ProfileEditorOptionsState {
    /// Initial state for the card scene: adopt whatever the shared projection
    /// already published, so a remount does not blank the panes.
    pub fn from_projection(projection: &ProfilesProjection) -> Self {
        let mut state = Self::default();
        if let Some(options) = projection.profile_options.as_ref() {
            state.adopt_snapshot(&options.source, &options.mixin_yaml, &options.filter);
        }
        state
    }

    /// Adopt the shared sidecar snapshot (first read or a changed stored fact).
    /// The comparison is against the last *stored* values, so a re-published
    /// cache never overwrites an in-progress edit.
    pub fn adopt_snapshot(
        &mut self,
        source: &ProfileSourceIdentity,
        mixin_yaml: &str,
        filter: &SubscriptionFilterDraft,
    ) {
        if self
            .mixin
            .session
            .observe(source, mixin_yaml, &self.mixin.buffer.full_text())
        {
            self.mixin.profile = source.profile.clone();
            self.mixin.buffer = CodeEditorState::new(mixin_yaml);
            self.mixin.loaded_content = Some(mixin_yaml.to_owned());
            self.mixin_loaded = Some(mixin_yaml.to_owned());
            self.mixin.dirty = false;
            self.mixin.notice = None;
            self.mixin.generation = self.mixin.generation.wrapping_add(1);
            self.mixin.refresh_preflight();
        }
        let before = self.filter.draft.clone();
        self.filter.observe_profile(
            &source.profile,
            Ok(FilterObservation {
                source: source.clone(),
                filter: filter.clone(),
            }),
        );
        self.filter_restore |= before != self.filter.draft;
    }
}

/// The pane switch row plus the honest note about the unmirrored Script pane.
pub fn pane_switch_scene(state: &ProfileEditorOptionsState, palette: &UiPalette) -> Box<dyn Scene> {
    let buttons: Vec<Box<dyn Scene>> = ProfileEditorPane::ALL
        .iter()
        .map(|pane| {
            let background = if *pane == state.pane {
                palette.accent
            } else {
                palette.surface_elevated
            };
            let label = LocalizedText::plain(pane.label_key());
            let pane = *pane;
            Box::new(bsn! {
                            Node {
                                min_height: px(24.0),
                                padding: UiRect::horizontal(Val::Px(space::S8)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                            }
                            BackgroundColor({ background })
                            Button
                            ProfileEditorPaneButton { pane }
                            Children [
                                label TextRole(Role::Caption)
                            ]
            }) as Box<dyn Scene>
        })
        .collect();
    let note_color = palette.ink_dim;
    Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(space::S4),
                row_gap: Val::Px(space::S4),
            }
            Children [
                { buttons }
                --
                LocalizedText::plain("profiles_script_console_hint")
                TextRole(Role::Caption)
                TextColor({ note_color })
            ]
    })
}

/// The Mixin pane: hint, status, shared preflight, the three-column workspace,
/// snippet bar and actions.
pub fn mixin_pane_scene(state: &ProfileEditorOptionsState, palette: &UiPalette) -> Box<dyn Scene> {
    let (diagnostic_text, has_error) = diagnostic_line(&state.mixin, UiLocale::default().code());
    let status = status_line(&state.mixin, None, UiLocale::default().code());
    let error_color = if has_error {
        palette.danger
    } else {
        palette.success
    };
    let pill_label = profile_editor_projection::syntax_label(has_error, UiLocale::default().code());
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            Children [
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                }
                MixinStudioBody
                --
                LocalizedText::plain("profiles_mixin_save_hint")
                TextRole(Role::Caption)
                --
                Text(status) MixinEditorStatusText TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S8),
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    Node {
                        padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(2.0), Val::Px(2.0)),
                        border_radius: BorderRadius::all(Val::Px(4.0)),
                    }
                    BackgroundColor({ error_color })
                    Children [
                        Text({ pill_label.to_owned() }) MixinEditorDiagnosticPill TextRole(Role::Caption)
                    ]
                    --
                    Text(diagnostic_text) MixinEditorDiagnosticText TextRole(Role::Mono)
                    --
                    @{ mixin_actions_scene(palette) }
                ]
                --
                @{ mixin_snippet_bar(palette) }
            ]
    })
}

fn mixin_actions_scene(palette: &UiPalette) -> Box<dyn Scene> {
    let background = palette.surface_elevated;
    let accent = palette.accent;
    Box::new(bsn! {
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S4),
            }
            Children [
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ background })
                Button
                EditorMutationControl
                MixinEditorFocusButton
                Children [
                    LocalizedText::plain("profiles_mixin_edit_action") TextRole(Role::Caption)
                ]
                --
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ background })
                Button
                MixinEditorReloadButton
                Children [
                    LocalizedText::plain("profiles_mixin_reload_action") TextRole(Role::Caption)
                ]
                --
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ accent })
                Button
                EditorMutationControl
                MixinEditorSaveButton
                Children [
                    LocalizedText::plain("profiles_mixin_save_action") TextRole(Role::Caption)
                ]
                --
                @{ discard_scene(true, palette) }
            ]
    })
}

fn mixin_snippet_bar(palette: &UiPalette) -> Box<dyn Scene> {
    let buttons: Vec<Box<dyn Scene>> = YAML_SNIPPETS
        .iter()
        .enumerate()
        .map(|(index, snippet)| {
            let label = LocalizedText::plain(snippet.label_key);
            let background = palette.surface_elevated;
            Box::new(bsn! {
                            Node {
                                min_height: px(20.0),
                                padding: UiRect::horizontal(Val::Px(space::S6)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                            }
                            BackgroundColor({ background })
                            Button
                            EditorMutationControl
                            MixinEditorSnippetButton { index }
                            Children [
                                label TextRole(Role::Caption)
                            ]
            }) as Box<dyn Scene>
        })
        .collect();
    Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(space::S4),
                row_gap: Val::Px(space::S4),
            }
            Children [
                LocalizedText::plain("profiles_snippet_insert_action") TextRole(Role::Caption)
                --
                { buttons }
            ]
    })
}

/// The Filter pane: the four shared draft fields, the dedup strategy and the
/// shared pipeline submit.
pub fn filter_pane_scene(state: &ProfileEditorOptionsState, palette: &UiPalette) -> Box<dyn Scene> {
    let fields: Vec<Box<dyn Scene>> = FilterField::ALL
        .iter()
        .filter(|kind| **kind != FilterField::Advanced)
        .map(|kind| filter_field_scene(*kind, kind.value(&state.filter.draft), palette))
        .collect();
    let chips: Vec<Box<dyn Scene>> = SubscriptionFilterDedup::ALL.into_iter().map(|mode| {
        let index = mode.index();
        let selected = state.filter.selected() == Some(mode);
        Box::new(bsn! { @{ localized_pill_scene(LocalizedText::plain(mode.label_key()), selected, palette) } EditorFilterDedupButton { index } ButtonDisabled(false) }) as Box<dyn Scene>
    }).collect();
    let status = filter_status_line(state);
    let border = palette.border;
    Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            Children [
                LocalizedText::plain("profiles_filter_save_hint")
                TextRole(Role::Caption)
                --
                { fields }
                --
                Node {width: percent(100), flex_direction: FlexDirection::Column, row_gap: px(space::S8), border: UiRect::all(px(1.0)), padding: UiRect::all(px(space::S6))}
                BorderColor::all(border)
                EditorFilterTransaction
                Children [
                    @{ filter_field_scene(FilterField::Advanced, FilterField::Advanced.value(&state.filter.draft), palette) }
                    --
                LocalizedText::plain("filter_advanced_help") TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S4),
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    LocalizedText::plain("profiles_filter_deduplicate") TextRole(Role::Caption)
                    --
                    { chips }
                ]
                --
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    Text(status) EditorFilterStatusText TextRole(Role::Caption)
                    --
                    @{ localized_pill_scene(LocalizedText::plain("filter_form_discard"), false, palette) } EditorFilterDiscard ButtonDisabled(false)
                    --
                    @{ localized_pill_scene(LocalizedText::plain("profiles_filter_apply_action"), false, palette) } EditorFilterSaveButton ButtonDisabled(false)

                ]
                ]
            ]
    })
}

fn filter_field_scene(kind: FilterField, value: &str, palette: &UiPalette) -> Box<dyn Scene> {
    let initial = value.to_owned();
    let placeholder = LocalizedText::plain(filter_placeholder_key(kind));
    Box::new(bsn! {
            Node {
                width: percent(100),
            }
            Button
            EditorFilterField { kind: { Some(kind) } }
            Children [
                @{ localized_field_scene(initial, placeholder, palette) } EditorFilterText(kind) NativeTextField({ match kind { FilterField::Include => 100, FilterField::Exclude => 101, FilterField::Protocols => 102, FilterField::Renames => 103, FilterField::Advanced => 104 } })
            ]
    })
}

/// Status text for the Filter pane: how much of the shared draft is active,
/// plus the last refusal.
pub fn filter_status_line(state: &ProfileEditorOptionsState) -> String {
    status(&state.filter, "zh-CN")
}

fn filter_placeholder_key(kind: FilterField) -> &'static str {
    match kind {
        FilterField::Include => "field_filter_include",
        FilterField::Exclude => "field_filter_exclude",
        FilterField::Protocols => "field_filter_protocols",
        FilterField::Renames => "field_filter_renames",
        FilterField::Advanced => "filter_advanced_ph",
    }
}

/// Palette token used by tests to assert the dedup chips restamp.
pub fn chip_background(selected: bool, palette: &UiPalette) -> Color {
    if selected {
        palette.accent
    } else {
        palette.surface_elevated
    }
}

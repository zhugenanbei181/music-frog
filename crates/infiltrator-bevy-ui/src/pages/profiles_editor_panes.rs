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
//! Honest boundary, stated in the UI: the Iced QuickJS script console stays a
//! single-surface feature. `ScriptApplication` has no surface-published read
//! model (the surface snapshot's `script_sandbox` is never published and Iced
//! runs the domain engine inline), so mirroring it would copy one surface's
//! bypass instead of a shared use-case. The pane row says so.

use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::resource::Resource;
use bevy::scene::{Scene, bsn, template_value};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, FlexWrap, JustifyContent, Node,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_bevy_widgets::editor::CodeEditorState;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::subscription_import::SubscriptionFilterDraft;
use infiltrator_contract::yaml_snippets::YAML_SNIPPETS;

use crate::pages::profiles::ProfilesProjection;
use crate::pages::profiles_editor_body::editor_rows_scene;
use crate::pages::profiles_editor_state::{ProfileEditorState, diagnostic_line, status_line};

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

    pub const fn label_zh(self) -> &'static str {
        match self {
            Self::Profile => "配置文档",
            Self::Mixin => "Mixin 覆盖",
            Self::Filter => "订阅过滤",
        }
    }
}

/// One editable field of the shared subscription-filter draft.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EditorFilterFieldKind {
    Include,
    Exclude,
    ExcludeTypes,
    Renames,
}

impl EditorFilterFieldKind {
    pub const ALL: [Self; 4] = [
        Self::Include,
        Self::Exclude,
        Self::ExcludeTypes,
        Self::Renames,
    ];

    pub const fn placeholder_zh(self) -> &'static str {
        match self {
            Self::Include => "包含关键字（逗号/换行分隔，支持正则）",
            Self::Exclude => "排除关键字（逗号/换行分隔，支持正则）",
            Self::ExcludeTypes => "协议排除（如 ss, vmess, trojan）",
            Self::Renames => "重命名规则（模式 => 替换，一行一条）",
        }
    }

    pub const fn draft_field(self) -> SubscriptionFilterDraftField {
        match self {
            Self::Include => SubscriptionFilterDraftField::Include,
            Self::Exclude => SubscriptionFilterDraftField::Exclude,
            Self::ExcludeTypes => SubscriptionFilterDraftField::ExcludeTypes,
            Self::Renames => SubscriptionFilterDraftField::Renames,
        }
    }
}

/// Which text field of the shared draft a surface is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubscriptionFilterDraftField {
    Include,
    Exclude,
    ExcludeTypes,
    Renames,
}

impl SubscriptionFilterDraftField {
    pub fn value(self, draft: &SubscriptionFilterDraft) -> &str {
        match self {
            Self::Include => &draft.include,
            Self::Exclude => &draft.exclude,
            Self::ExcludeTypes => &draft.exclude_types,
            Self::Renames => &draft.renames,
        }
    }

    pub fn set(self, draft: &mut SubscriptionFilterDraft, value: String) {
        match self {
            Self::Include => draft.include = value,
            Self::Exclude => draft.exclude = value,
            Self::ExcludeTypes => draft.exclude_types = value,
            Self::Renames => draft.renames = value,
        }
    }
}

/// The filter field root; its first child is the controlled `TextField`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EditorFilterField {
    pub kind: Option<EditorFilterFieldKind>,
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
    pub filter: SubscriptionFilterDraft,
    /// The last stored draft adopted from the shared snapshot; compared so a
    /// re-published cache never clobbers an in-progress edit.
    pub filter_loaded: Option<SubscriptionFilterDraft>,
    pub filter_focus: Option<EditorFilterFieldKind>,
    /// Last refusal/notice from a filter action.
    pub filter_notice: Option<String>,
    /// The profile whose sidecar has already been requested, so opening a
    /// pane does not spam the shared command pump.
    pub requested_for: Option<String>,
}

impl Default for ProfileEditorOptionsState {
    fn default() -> Self {
        Self {
            pane: ProfileEditorPane::Profile,
            mixin: ProfileEditorState::default(),
            mixin_loaded: None,
            filter: SubscriptionFilterDraft::default(),
            filter_loaded: None,
            filter_focus: None,
            filter_notice: None,
            requested_for: None,
        }
    }
}

impl ProfileEditorOptionsState {
    /// Initial state for the card scene: adopt whatever the shared projection
    /// already published, so a remount does not blank the panes.
    pub fn from_projection(projection: &ProfilesProjection) -> Self {
        let mut state = Self::default();
        if let Some(options) = projection.profile_options.as_ref() {
            state.adopt_snapshot(&options.profile, &options.mixin_yaml, &options.filter);
        }
        state
    }

    /// Adopt the shared sidecar snapshot (first read or a changed stored fact).
    /// The comparison is against the last *stored* values, so a re-published
    /// cache never overwrites an in-progress edit.
    pub fn adopt_snapshot(
        &mut self,
        profile: &str,
        mixin_yaml: &str,
        filter: &SubscriptionFilterDraft,
    ) {
        self.mixin.profile = profile.to_owned();
        if self.mixin_loaded.as_deref() != Some(mixin_yaml) {
            self.mixin.buffer = CodeEditorState::new(mixin_yaml);
            self.mixin.loaded_content = Some(mixin_yaml.to_owned());
            self.mixin_loaded = Some(mixin_yaml.to_owned());
            self.mixin.dirty = false;
            self.mixin.notice = None;
            self.mixin.generation = self.mixin.generation.wrapping_add(1);
            self.mixin.refresh_preflight();
        }
        if self.filter_loaded.as_ref() != Some(filter) {
            self.filter_loaded = Some(filter.clone());
            self.filter = filter.clone();
        }
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
            let label = pane.label_zh();
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
                template_value(ProfileEditorPaneButton { pane })
                Children [
                    ( Text({ label.to_owned() }) TextRole(Role::Caption) ),
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
            { buttons },
            (
                Text({ "脚本沙盒未镜像：Iced QuickJS 控制台在 update 内直跑引擎，共享 ScriptApplication 无 surface 读模型".to_owned() })
                TextRole(Role::Caption)
                bevy::text::TextColor({ note_color })
            ),
        ]
    })
}

/// The Mixin pane: hint, status, shared preflight, snippet bar, actions and
/// the bounded editor body.
pub fn mixin_pane_scene(state: &ProfileEditorOptionsState, palette: &UiPalette) -> Box<dyn Scene> {
    let (diagnostic_text, has_error) = diagnostic_line(&state.mixin);
    let status = status_line(&state.mixin, None);
    let rows = editor_rows_scene(&state.mixin, palette);
    let error_color = if has_error {
        palette.danger
    } else {
        palette.success
    };
    let pill_label = if has_error {
        "语法错误"
    } else {
        "语法通过"
    };
    let editor_background = palette.window_clear;
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S8),
        }
        Children [
            (
                Text({ "Mixin 覆盖：保存先剥离上一版注入的规则行，再经共享保真引擎合并并应用".to_owned() })
                TextRole(Role::Caption)
            ),
            ( Text(status) MixinEditorStatusText TextRole(Role::Caption) ),
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S8),
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    (
                        Node {
                            padding: UiRect::new(Val::Px(6.0), Val::Px(6.0), Val::Px(2.0), Val::Px(2.0)),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ error_color })
                        Children [
                            ( Text({ pill_label.to_owned() }) MixinEditorDiagnosticPill TextRole(Role::Caption) ),
                        ]
                    ),
                    ( Text(diagnostic_text) MixinEditorDiagnosticText TextRole(Role::Mono) ),
                    ( { mixin_actions_scene(palette) } ),
                ]
            ),
            ( { mixin_snippet_bar(palette) } ),
            (
                Node {
                    width: percent(100),
                    max_height: px(300.0),
                    padding: UiRect::all(Val::Px(space::S8)),
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    overflow: bevy::ui::prelude::Overflow::scroll_y(),
                }
                BackgroundColor({ editor_background })
                Children [
                    (
                        Node {
                            width: percent(100),
                            flex_direction: FlexDirection::Column,
                        }
                        MixinEditorBody
                        Children [
                            ( { rows } ),
                        ]
                    ),
                ]
            ),
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
            (
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ background })
                Button
                MixinEditorFocusButton
                Children [
                    ( Text({ "编辑 Mixin（键盘）".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
            (
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
                    ( Text({ "重新加载 Mixin".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
            (
                Node {
                    min_height: px(24.0),
                    padding: UiRect::horizontal(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    border_radius: BorderRadius::all(Val::Px(4.0)),
                }
                BackgroundColor({ accent })
                Button
                MixinEditorSaveButton
                Children [
                    ( Text({ "保存 Mixin（共享用例）".to_owned() }) TextRole(Role::Caption) ),
                ]
            ),
        ]
    })
}

fn mixin_snippet_bar(palette: &UiPalette) -> Box<dyn Scene> {
    let buttons: Vec<Box<dyn Scene>> = YAML_SNIPPETS
        .iter()
        .enumerate()
        .map(|(index, snippet)| {
            let label = snippet.label_zh.to_owned();
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
                template_value(MixinEditorSnippetButton { index })
                Children [
                    ( Text({ label }) TextRole(Role::Caption) ),
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
            ( Text({ "插入共享片段".to_owned() }) TextRole(Role::Caption) ),
            { buttons },
        ]
    })
}

/// The Filter pane: the four shared draft fields, the dedup strategy and the
/// shared pipeline submit.
pub fn filter_pane_scene(state: &ProfileEditorOptionsState, palette: &UiPalette) -> Box<dyn Scene> {
    let fields: Vec<Box<dyn Scene>> = EditorFilterFieldKind::ALL
        .iter()
        .map(|kind| filter_field_scene(*kind, kind.draft_field().value(&state.filter), palette))
        .collect();
    let chips: Vec<Box<dyn Scene>> = (0..4usize)
        .map(|index| {
            let label = ["关闭", "保留首个", "保留最后", "追加序号"][index];
            let selected = state.filter.dedup_index == index;
            let background = if selected {
                palette.accent
            } else {
                palette.surface_elevated
            };
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
                template_value(EditorFilterDedupButton { index })
                Children [
                    ( Text({ label.to_owned() }) TextRole(Role::Caption) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();
    let status = filter_status_line(state);
    let accent = palette.accent;
    Box::new(bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S8),
        }
        Children [
            (
                Text({ "订阅过滤：保存即用共享管道重跑当前配置，并持久化同一 draft".to_owned() })
                TextRole(Role::Caption)
            ),
            { fields },
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::S4),
                    row_gap: Val::Px(space::S4),
                }
                Children [
                    ( Text({ "重复节点去重".to_owned() }) TextRole(Role::Caption) ),
                    { chips },
                ]
            ),
            (
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    column_gap: Val::Px(space::S8),
                }
                Children [
                    ( Text(status) EditorFilterStatusText TextRole(Role::Caption) ),
                    (
                        Node {
                            min_height: px(26.0),
                            padding: UiRect::horizontal(Val::Px(space::S12)),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                        }
                        BackgroundColor({ accent })
                        Button
                        EditorFilterSaveButton
                        Children [
                            ( Text({ "应用过滤（共享管道）".to_owned() }) TextRole(Role::Body) ),
                        ]
                    ),
                ]
            ),
        ]
    })
}

fn filter_field_scene(
    kind: EditorFilterFieldKind,
    value: &str,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let initial = value.to_owned();
    let placeholder = kind.placeholder_zh().to_owned();
    Box::new(bsn! {
        Node {
            width: percent(100),
        }
        Button
        template_value(EditorFilterField { kind: Some(kind) })
        Children [
            ( { text_field_with_placeholder_scene(initial, placeholder, palette) } ),
        ]
    })
}

/// Status text for the Filter pane: how much of the shared draft is active,
/// plus the last refusal.
pub fn filter_status_line(state: &ProfileEditorOptionsState) -> String {
    let draft = &state.filter;
    let mut status = if draft.is_empty() {
        "过滤管道：未启用（draft 为空）".to_owned()
    } else {
        format!(
            "过滤管道：包含 `{}` · 排除 `{}` · 协议排除 `{}` · 重命名 {}",
            draft.include,
            draft.exclude,
            draft.exclude_types,
            if draft.renames.trim().is_empty() {
                "0 条".to_owned()
            } else {
                format!("{} 行", draft.renames.lines().count())
            }
        )
    };
    if let Some(notice) = state.filter_notice.as_deref() {
        status.push_str(" · ");
        status.push_str(notice);
    }
    status
}

/// Palette token used by tests to assert the dedup chips restamp.
pub fn chip_background(selected: bool, palette: &UiPalette) -> Color {
    if selected {
        palette.accent
    } else {
        palette.surface_elevated
    }
}

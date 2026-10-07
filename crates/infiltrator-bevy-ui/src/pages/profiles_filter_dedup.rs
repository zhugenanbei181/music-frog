//! Native four-way selection; no command is emitted before the existing filter save action.
use crate::localized_widgets::localized_pill_scene;
use crate::pages::profiles_filter_form::{self, FilterFormState};
use crate::pages::profiles_import_channels::SaveSubscriptionFilterButton;
use accesskit::{Role, Toggled};
use bevy::a11y::AccessibilityNode;
use bevy::app::{App, Plugin, PostUpdate, Update};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::{ChildOf, Children};
use bevy::ecs::observer::On;
use bevy::ecs::query::{QueryData, QueryFilter, With, Without};
use bevy::ecs::schedule::{ApplyDeferred, IntoScheduleConfigs};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{Display, FlexWrap, Node, px};
use bevy::ui_widgets::Activate;
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual, sync_button_disabled};
use infiltrator_bevy_widgets::button::{sync_control_labels, sync_control_visuals};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::text_input::render::sync_text_fields;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::subscription_import::SubscriptionFilterDedup;

#[derive(Component, Clone, Copy, Default)]
pub struct FilterFormRoot;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct FilterDedupChoice(pub SubscriptionFilterDedup);
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ReloadFilterDedup;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct FilterDedupSourceIssue;

pub(super) fn scene(index: usize, palette: &UiPalette) -> impl Scene + use<> {
    let selected = SubscriptionFilterDedup::from_index(index);
    let choices = SubscriptionFilterDedup::ALL
        .into_iter()
        .map(|mode| {
            Box::new((
                localized_pill_scene(
                    LocalizedText::plain(mode.label_key()),
                    selected == Some(mode),
                    palette,
                ),
                bsn! { FilterDedupChoice(mode) ButtonDisabled(false) },
            )) as Box<dyn Scene>
        })
        .collect::<Vec<_>>();
    bsn! {
        Node { flex_wrap: FlexWrap::Wrap, column_gap: px(space::S4), row_gap: px(space::S4) }
        FilterFormRoot
        AccessibilityNode({ accesskit::Node::new(Role::RadioGroup) })
        LocalizedLabel::plain("profiles_filter_deduplicate")
        Children [
            { choices }
            --
            @{ localized_pill_scene(LocalizedText::plain("filter_form_discard"), false, palette) } ReloadFilterDedup ButtonDisabled(false)
            --
            Node { display: Display::None }
            FilterDedupSourceIssue
            Children [ LocalizedText::plain("subscription_filter_source_changed") TextRole(text::Role::Caption) ]
        ]
    }
}
pub(super) struct FilterDedupPlugin;
impl Plugin for FilterDedupPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FilterFormState>();
        app.add_observer(profiles_filter_form::observe);
        app.add_observer(profiles_filter_form::initialize);
        app.add_observer(profiles_filter_form::observe_read_status);
        app.add_observer(profiles_filter_form::finish);
        app.add_observer(activate);
        app.add_systems(
            Update,
            (
                profiles_filter_form::receive,
                profiles_filter_form::replay,
                profiles_filter_form::render_status,
                sync,
            )
                .chain()
                .before(sync_control_visuals)
                .before(sync_control_labels)
                .before(sync_text_fields),
        );
        app.add_systems(
            PostUpdate,
            (
                profiles_filter_form::replay,
                profiles_filter_form::render_status,
                sync,
                ApplyDeferred,
            )
                .chain()
                .before(sync_button_disabled),
        );
    }
}
fn activate(
    event: On<Activate>,
    choices: Query<&FilterDedupChoice>,
    resets: Query<(), With<ReloadFilterDedup>>,
    mut state: ResMut<FilterFormState>,
) {
    if let Ok(choice) = choices.get(event.entity) {
        state.editor.pick(choice.0);
    } else if resets.contains(event.entity) && state.editor.cancel() {
        state.restore_fields = true;
        state.request = None;
    }
}
#[derive(QueryData)]
#[query_data(mutable)]
struct ChoiceControl {
    choice: &'static FilterDedupChoice,
    visual: &'static mut ControlVisual,
    disabled: &'static mut ButtonDisabled,
    accessibility: Option<&'static mut AccessibilityNode>,
}
#[derive(QueryFilter)]
struct ResetControls {
    reset: With<ReloadFilterDedup>,
    choice: Without<FilterDedupChoice>,
    save: Without<SaveSubscriptionFilterButton>,
}
fn sync(
    state: Res<FilterFormState>,
    mut choices: Query<ChoiceControl>,
    mut issues: Query<(&ChildOf, &mut Node), With<FilterDedupSourceIssue>>,
    mut saves: Query<
        &mut ButtonDisabled,
        (
            With<SaveSubscriptionFilterButton>,
            Without<FilterDedupChoice>,
        ),
    >,
    mut resets: Query<&mut ButtonDisabled, ResetControls>,
) {
    for mut control in &mut choices {
        let selected = state.editor.selected() == Some(control.choice.0);
        if control.visual.0 != selected {
            control.visual.0 = selected;
        }
        let blocked = !state.editor.can_edit();
        if control.disabled.0 != blocked {
            control.disabled.0 = blocked;
        }
        if let Some(mut node) = control.accessibility {
            node.set_role(Role::RadioButton);
            node.set_toggled(if selected {
                Toggled::True
            } else {
                Toggled::False
            });
        }
    }
    for (_, mut node) in &mut issues {
        node.display = if state.editor.stale() {
            Display::Flex
        } else {
            Display::None
        };
    }
    let can_save = state.editor.can_edit() && state.editor.selected().is_some();
    for mut disabled in &mut saves {
        if disabled.0 == can_save {
            disabled.0 = !can_save;
        }
    }
    for mut disabled in &mut resets {
        let blocked = state.editor.pending.is_some();
        if disabled.0 != blocked {
            disabled.0 = blocked;
        }
    }
}

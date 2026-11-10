//! The Settings page's appearance card: the theme preference selector and the
//! layout density selector.
//!
//! **Why here.** Appearance is a settings preference on every judged desktop
//! surface: Slint states it as the `AppPreferenceRow` "主题模式" that opens the
//! theme menu (`ui/pages/page_settings.slint`), Iced carries a selector in its
//! settings card, and FlClash keeps theme mode in settings too. The Bevy shell
//! used to park a theme pill and a density pill in the *sidebar footer*, which
//! made the navigation column carry non-navigation blocks (`chrome_band` 0%)
//! and left the settings page's own theme strip decorative — a hardcoded
//! four-skin list with a fixed selected index that dispatched nothing. Both
//! halves are retired here: the sidebar mounts navigation alone, and this card
//! is the one live appearance control.
//!
//! **One owner per fact.** The offered options are not this file's decision:
//! the theme list is [`ThemePreference::ALL`] with the contract's own label
//! keys, and the density list is [`Density::ALL`]. The strip is the shared
//! segmented control over the official `TabList`/`Tab` pair, so selection,
//! roving focus, keyboard navigation and the tab roles belong to the official
//! widgets; this page owns only the *controlled* selection, written back from
//! the preference owner ([`ThemeMode`] / [`ResponsiveContext`]) exactly the way
//! `rules_tabs` writes its strip from the shared rules model.
//!
//! Density has one writer: the page dispatches [`DensitySwitch`] and the shared
//! widget observer applies it to `ResponsiveContext`. The shell's old
//! `ShellLayoutState.density` was a second copy of that value which no layout
//! ever read, so it is gone rather than relocated.

use crate::a11y::{region_semantic_node, semantic_node};
use crate::appearance::{
    SystemAppearance, ThemeMode, persist_theme_preference, resolved_skin,
    tonal_ladder_preview_scene,
};
use crate::command::CommandSinkHandle;
use crate::localized_widgets::localized_segmented_scene;
use bevy::app::{App, Update};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Query, Res, ResMut, SystemParam};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{FlexDirection, Node, UiRect, percent};
use bevy::ui_widgets::{Activate, SelectedTab, Tab, ValueChange};
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::responsive::{Density, DensitySwitch, ResponsiveContext};
use infiltrator_bevy_widgets::switch::ThemeSwitch;
use infiltrator_bevy_widgets::tabs::SegmentedTab;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::design_resources;
use infiltrator_contract::shared_copy::SharedCopyKey;
use infiltrator_contract::theme::ThemePreference;
use infiltrator_shared::locales::{Lang, Localizer};

/// Marker on the appearance card root, which carries the shared grammar row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppearanceCard;

/// Marker on the theme preference strip (the official `TabList` entity).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppearanceStrip;

/// The preference one theme chip stands for. The shared control owns the chip
/// and its index; this marker is the page's typed identity for it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AppearanceChip(pub ThemePreference);

/// Marker on the layout density strip (the official `TabList` entity).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DensityStrip;

/// The density one density chip stands for.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DensityChip(pub Density);

/// The appearance card: the theme selector, the derived tonal-ladder preview
/// and the density selector.
///
/// The selected indices are the owners' defaults at mount; the first
/// [`sync_appearance_selection`] frame writes the live preference into the
/// official selection, so a host that seeded a pinned skin never shows a
/// stale chip.
pub fn appearance_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    let theme_keys: Vec<&'static str> = ThemePreference::ALL
        .iter()
        .map(|preference| preference.label_key())
        .collect();
    let density_keys: Vec<&'static str> = Density::ALL
        .iter()
        .map(|density| density.label_key())
        .collect();
    let appearance_node = semantic_node(ShellA11yNode::AppearanceSegment);
    let density_node = region_semantic_node(&Lang("zh-CN").tr(SharedCopyKey::LayoutDensity.key()));
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S6),
                padding: UiRect::top(rem(space::S4)),
            }
            AppearanceCard
            appearance_node
            LocalizedLabel::plain(ShellA11yNode::AppearanceSegment.label_key())
            Children [
                LocalizedText::plain("settings_interface_theme_title") TextRole(design_resources::TextRole::Caption)
                --
                @{ localized_segmented_scene(&theme_keys, ThemePreference::default().index(), palette) }
                AppearanceStrip
                --
                @{ tonal_ladder_preview_scene(palette) }
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S6),
                    padding: UiRect::top(rem(space::S8)),
                }
                density_node
                LocalizedLabel::plain(SharedCopyKey::LayoutDensity.key())
                Children [
                    LocalizedText::plain(SharedCopyKey::LayoutDensity.key()) TextRole(design_resources::TextRole::Caption)
                    --
                    @{ localized_segmented_scene(&density_keys, Density::default().index(), palette) }
                    DensityStrip
                ]
            ]
    }
}

type UnbridgedThemeChip<'w, 's> =
    Query<'w, 's, (Entity, &'static SegmentedTab), (With<Tab>, Without<AppearanceChip>)>;
type UnbridgedDensityChip<'w, 's> =
    Query<'w, 's, (Entity, &'static SegmentedTab), (With<Tab>, Without<DensityChip>)>;

/// One strip's official selection and the chips under it. The two strips are
/// one component family, so each system writes only its own entity: the strip
/// marker makes the access disjoint.
type StripSelection<'w, 's, Marker, Other> = Query<
    'w,
    's,
    (Entity, &'static mut SelectedTab, &'static Children),
    (With<Marker>, Without<Other>),
>;

/// The strips, their official selection and the preference owners they mirror.
#[derive(SystemParam)]
pub struct AppearanceSyncParams<'w, 's> {
    commands: Commands<'w, 's>,
    mode: Res<'w, ThemeMode>,
    context: Option<Res<'w, ResponsiveContext>>,
    // The two strips are one component family, so their mutable access is made
    // disjoint by the strip marker: each strip's official selection is written
    // from its own owner, and neither system touches the other's entity.
    theme_strips: StripSelection<'w, 's, AppearanceStrip, DensityStrip>,
    density_strips: StripSelection<'w, 's, DensityStrip, AppearanceStrip>,
    theme_chips: Query<'w, 's, (Entity, &'static AppearanceChip), With<Tab>>,
    density_chips: Query<'w, 's, (Entity, &'static DensityChip), With<Tab>>,
    unbridged_theme: UnbridgedThemeChip<'w, 's>,
    unbridged_density: UnbridgedDensityChip<'w, 's>,
}

/// Bridge the shared chips onto the page's typed identities and write the
/// official selection from the preference owners.
///
/// The bridge does not depend on either owner, so a chip is the page's own
/// typed marker from the first frame. The selection is written *from* the
/// owner: a click only requests a change, and an owner that refuses or
/// re-resolves the request (the published theme readback) keeps the strip
/// honest instead of letting the strip drift into a second truth.
pub fn sync_appearance_selection(params: AppearanceSyncParams) {
    let AppearanceSyncParams {
        mut commands,
        mode,
        context,
        mut theme_strips,
        mut density_strips,
        theme_chips,
        density_chips,
        unbridged_theme,
        unbridged_density,
    } = params;
    // The bridge is scoped to each strip's own children: a page mounts more
    // than one segmented control, and an unscoped `Without<AppearanceChip>`
    // query would stamp the theme identity onto the density chips (and onto
    // every other strip on the page).
    for (_, _, children) in &theme_strips {
        for child in children.iter() {
            if let Ok((entity, tab)) = unbridged_theme.get(*child) {
                let preference = ThemePreference::from_index(tab.0);
                commands.entity(entity).insert((
                    AppearanceChip(preference),
                    LocalizedLabel::plain(preference.label_key()),
                ));
            }
        }
    }
    for (_, _, children) in &density_strips {
        for child in children.iter() {
            if let Ok((entity, tab)) = unbridged_density.get(*child) {
                let density = Density::from_index(tab.0);
                commands.entity(entity).insert((
                    DensityChip(density),
                    LocalizedLabel::plain(density.label_key()),
                ));
            }
        }
    }
    let selected_theme = theme_chips
        .iter()
        .find(|(_, chip)| chip.0 == mode.0)
        .map(|(entity, _)| entity);
    for (_, mut selected, children) in &mut theme_strips {
        if let Some(target) = selected_theme
            && children.contains(&target)
            && selected.0 != Some(target)
        {
            selected.0 = Some(target);
        }
    }
    let Some(context) = context else {
        return;
    };
    let selected_density = density_chips
        .iter()
        .find(|(_, chip)| chip.0 == context.density)
        .map(|(entity, _)| entity);
    for (_, mut selected, children) in &mut density_strips {
        if let Some(target) = selected_density
            && children.contains(&target)
            && selected.0 != Some(target)
        {
            selected.0 = Some(target);
        }
    }
}

/// Adopt the requested theme preference: paint it, write it through to the
/// settings owner, and let the published readback confirm it.
fn select_theme_preference(
    preference: ThemePreference,
    mode: &mut ResMut<ThemeMode>,
    appearance: Option<&SystemAppearance>,
    sink: Option<&CommandSinkHandle>,
    commands: &mut Commands,
) {
    if mode.0 == preference {
        return;
    }
    mode.0 = preference;
    let system = appearance
        .map(|appearance| appearance.0)
        .unwrap_or_default();
    persist_theme_preference(sink, preference);
    commands.trigger(ThemeSwitch(resolved_skin(preference, system)));
}

/// The strip's official selection request.
pub fn on_appearance_value_changed(
    change: On<ValueChange<Option<Entity>>>,
    strips: Query<(), With<AppearanceStrip>>,
    chips: Query<&AppearanceChip>,
    mut mode: ResMut<ThemeMode>,
    appearance: Option<Res<SystemAppearance>>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    if !strips.contains(change.source) {
        return;
    }
    let Some(selected) = change.value else {
        return;
    };
    let Ok(chip) = chips.get(selected) else {
        return;
    };
    select_theme_preference(
        chip.0,
        &mut mode,
        appearance.as_deref(),
        sink.as_deref(),
        &mut commands,
    );
}

/// A theme chip addressed directly (the capture seam, a shell command).
pub fn on_appearance_chip_activated(
    activate: On<Activate>,
    chips: Query<&AppearanceChip>,
    mut mode: ResMut<ThemeMode>,
    appearance: Option<Res<SystemAppearance>>,
    sink: Option<Res<CommandSinkHandle>>,
    mut commands: Commands,
) {
    let Ok(chip) = chips.get(activate.entity) else {
        return;
    };
    select_theme_preference(
        chip.0,
        &mut mode,
        appearance.as_deref(),
        sink.as_deref(),
        &mut commands,
    );
}

/// Request a density. The shared widget observer is the one writer of
/// `ResponsiveContext.density`, so the page dispatches and never assigns.
pub fn on_density_value_changed(
    change: On<ValueChange<Option<Entity>>>,
    strips: Query<(), With<DensityStrip>>,
    chips: Query<&DensityChip>,
    context: Option<Res<ResponsiveContext>>,
    mut commands: Commands,
) {
    if !strips.contains(change.source) {
        return;
    }
    let Some(selected) = change.value else {
        return;
    };
    let Ok(chip) = chips.get(selected) else {
        return;
    };
    if context.is_some_and(|context| context.density == chip.0) {
        return;
    }
    commands.trigger(DensitySwitch(chip.0));
}

/// A density chip addressed directly.
pub fn on_density_chip_activated(
    activate: On<Activate>,
    chips: Query<&DensityChip>,
    context: Option<Res<ResponsiveContext>>,
    mut commands: Commands,
) {
    let Ok(chip) = chips.get(activate.entity) else {
        return;
    };
    if context.is_some_and(|context| context.density == chip.0) {
        return;
    }
    commands.trigger(DensitySwitch(chip.0));
}

/// Mount the card's wiring once at page assembly.
pub fn register(app: &mut App) {
    app.add_observer(on_appearance_value_changed);
    app.add_observer(on_appearance_chip_activated);
    app.add_observer(on_density_value_changed);
    app.add_observer(on_density_chip_activated);
    app.add_systems(Update, sync_appearance_selection);
}

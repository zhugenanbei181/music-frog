//! BEVY-027: the card render-strategy degradation switch in Settings.
//!
//! The control selects the widget layer's [`CardRenderStrategy`] (GPU
//! instanced batch / analytic CPU fallback / flat fill), persists the choice
//! through the existing `UpdateSetting` seam, and stamps the live resource the
//! widget sync path already honours. An unknown persisted value falls back to
//! the typed default instead of guessing.

use crate::command::{CommandSinkHandle, UiCommand};
use bevy::app::{App, Update};
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::shader_fx::CardRenderStrategy;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// The persisted setting key for the card render strategy.
pub const RENDER_STRATEGY_SETTING_KEY: &str = "card_render_strategy";

/// The UI-side mirror of the selected strategy. Persisted across page
/// remounts; the widget layer's `CardRenderStrategy` resource is stamped in
/// lockstep so the sync path honours the switch immediately.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStrategySelection(pub CardRenderStrategy);

/// One selectable strategy pill.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStrategyButton(pub CardRenderStrategy);

/// The stable persisted token for a strategy.
pub const fn render_strategy_setting_value(strategy: CardRenderStrategy) -> &'static str {
    match strategy {
        CardRenderStrategy::GpuInstanced => "gpu_instanced",
        CardRenderStrategy::CpuFallback => "cpu_fallback",
        CardRenderStrategy::Flat => "flat",
    }
}

/// Parse a persisted token, falling back to the typed default for anything
/// unrecognized or unsupported (never a fabricated fourth strategy).
pub fn parse_render_strategy(value: &str) -> CardRenderStrategy {
    match value {
        "gpu_instanced" => CardRenderStrategy::GpuInstanced,
        "cpu_fallback" => CardRenderStrategy::CpuFallback,
        "flat" => CardRenderStrategy::Flat,
        _ => CardRenderStrategy::default(),
    }
}

const STRATEGIES: [(CardRenderStrategy, &str); 3] = [
    (CardRenderStrategy::GpuInstanced, "GPU"),
    (CardRenderStrategy::CpuFallback, "CPU"),
    (CardRenderStrategy::Flat, "Flat"),
];

/// The Settings row: a caption plus the three strategy pills.
pub fn render_strategy_row_scene(palette: &UiPalette) -> impl Scene + use<> {
    let buttons: Vec<Box<dyn Scene>> = STRATEGIES
        .into_iter()
        .map(|(strategy, label)| {
            Box::new(strategy_button_scene(strategy, label, palette)) as Box<dyn Scene>
        })
        .collect();
    bsn! {
        Node {
            width: percent(100),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(space::S6),
            padding: UiRect::top(Val::Px(space::S4)),
        }
        Children [
            Text("Card render strategy") TextRole(Role::Caption)
            --
            Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::S4),
            }
            Children [
                { buttons }
            ]
        ]
    }
}

fn strategy_button_scene(
    strategy: CardRenderStrategy,
    label: &'static str,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            min_height: px(palette.control_height_px),
            padding: UiRect::horizontal(Val::Px(space::S8)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        RenderStrategyButton(strategy)
        Button
        Children [
            Text(label) TextRole(Role::Caption)
        ]
    }
}

/// Selecting a strategy stamps the UI mirror, the live widget resource (so the
/// sync path degrades immediately) and persists the choice.
pub fn on_render_strategy_activated(
    activate: On<Activate>,
    buttons: Query<&RenderStrategyButton>,
    handle: Option<Res<CommandSinkHandle>>,
    mut selection: ResMut<RenderStrategySelection>,
    mut strategy: Option<ResMut<CardRenderStrategy>>,
) {
    let Ok(button) = buttons.get(activate.entity) else {
        return;
    };
    let next = button.0;
    if selection.0 != next {
        selection.0 = next;
    }
    if let Some(strategy) = strategy.as_deref_mut()
        && *strategy != next
    {
        *strategy = next;
    }
    if let Some(handle) = handle {
        handle.submit(UiCommand::UpdateSetting {
            key: RENDER_STRATEGY_SETTING_KEY.to_owned(),
            value: render_strategy_setting_value(next).to_owned(),
        });
    }
}

/// Restamp the selected pill from the UI mirror (also covers a remount).
pub fn replay_render_strategy(
    selection: Res<RenderStrategySelection>,
    palette: Res<UiPalette>,
    mut buttons: Query<(&RenderStrategyButton, &mut BackgroundColor)>,
) {
    for (button, mut fill) in &mut buttons {
        let want = if button.0 == selection.0 {
            palette.accent
        } else {
            palette.surface_elevated
        };
        if fill.0 != want {
            fill.0 = want;
        }
    }
}

/// Registers the control's resource, observer and replay system.
pub fn register(app: &mut App) {
    app.init_resource::<RenderStrategySelection>();
    app.add_observer(on_render_strategy_activated);
    app.add_systems(Update, replay_render_strategy);
}

//! BEVY-040: dev/doctor chaos sandbox console.
//!
//! A seeded, bounded fault-injection explorer for the digital-twin sandbox. It
//! is explicitly opt-in: the console starts disabled and only runs when a
//! dev/doctor surface enables it, so the normal user path never injects a
//! fault. The same `(seed, config, steps)` always discovers the same states,
//! so a state found by the explorer can be replayed exactly.

use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::event::Event;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::ResMut;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderColor, BorderRadius, FlexDirection, JustifyContent, Node,
    UiRect, Val, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::chaos::{BoundedMonkeyExplorer, ChaosFaultConfig, DiscoveredState};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

/// The product surfaces the bounded monkey explorer can walk.
pub const CHAOS_ROUTES: [&str; 11] = [
    "Overview",
    "Proxies",
    "Profiles",
    "Rules",
    "Connections",
    "Logs",
    "DNS",
    "Doctor",
    "Routing",
    "Sync",
    "Settings",
];

/// Dev/doctor chaos console state. `enabled` is the explicit opt-in gate.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ChaosConsole {
    pub enabled: bool,
    pub seed: u64,
    pub max_steps: u64,
    pub config: ChaosFaultConfig,
    pub discovered: Vec<DiscoveredState>,
    pub runs: u64,
}

impl Default for ChaosConsole {
    fn default() -> Self {
        Self {
            enabled: false,
            seed: 0x5EED_5EED,
            max_steps: 64,
            config: ChaosFaultConfig::default(),
            discovered: Vec::new(),
            runs: 0,
        }
    }
}

impl ChaosConsole {
    /// An explicitly enabled console for a doctor/dev surface.
    pub fn enabled_for_doctor(seed: u64, max_steps: u64, config: ChaosFaultConfig) -> Self {
        Self {
            enabled: true,
            seed,
            max_steps,
            config,
            discovered: Vec::new(),
            runs: 0,
        }
    }

    pub fn enable(&mut self) {
        self.enabled = true;
    }

    pub fn disable(&mut self) {
        self.enabled = false;
    }

    /// Run one bounded, seeded exploration; a disabled console runs nothing and
    /// returns `0`. Returns the number of distinct discovered states.
    pub fn run(&mut self, routes: &[&str]) -> usize {
        if !self.enabled {
            return 0;
        }
        let mut explorer = BoundedMonkeyExplorer::new(self.max_steps);
        self.discovered = explorer.explore(routes, self.seed, self.config).to_vec();
        self.runs += 1;
        self.discovered.len()
    }
}

/// Run one bounded, seeded exploration on the doctor surface.
#[derive(Event, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunChaosExploration;

/// Marker on the mounted chaos console root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChaosConsoleRoot;

/// Marker on the enabled/disabled gate badge.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChaosConsoleGate;

/// Marker on one discovered-state row.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChaosStateRow(pub u64);

/// Observer: run the exploration only when the console is explicitly enabled.
pub fn on_run_chaos_exploration(
    _request: On<RunChaosExploration>,
    mut console: ResMut<ChaosConsole>,
) {
    console.run(&CHAOS_ROUTES);
}

/// The dev/doctor console scene: gate badge plus the discovered-state list.
pub fn chaos_console_scene(console: &ChaosConsole, palette: &UiPalette) -> impl Scene + use<> {
    let edge = palette.border;
    let gate = if console.enabled {
        "ENABLED"
    } else {
        "DISABLED"
    };
    let seed = format!(
        "seed {} · steps {} · runs {}",
        console.seed, console.max_steps, console.runs
    );
    let states: Vec<Box<dyn Scene>> = console
        .discovered
        .iter()
        .map(|state| {
            let fault = state
                .fault_mode
                .map_or_else(|| "none".to_owned(), |mode| format!("{mode:?}"));
            Box::new(bsn! {
                Node { width: percent(100), align_items: AlignItems::Center, justify_content: JustifyContent::SpaceBetween }
                ChaosStateRow({ state.step })
                Children [
                    Text({ format!("#{} {}", state.step, state.route) }) TextRole(Role::Caption)
                    --
                    Text(fault) TextRole(Role::Caption)
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
                padding: UiRect::all(Val::Px(space::S12)),
                border: UiRect::all(Val::Px(palette.hairline_px)),
                border_radius: BorderRadius::all(Val::Px(palette.card_radius_px)),
            }
            BackgroundColor({ palette.surface })
            BorderColor { top: edge, right: edge, bottom: edge, left: edge }
            ChaosConsoleRoot
            Children [
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                }
                Children [
                    Text({ "Chaos Sandbox".to_owned() }) TextRole(Role::Heading)
                    --
                    Text({ gate.to_owned() }) TextRole(Role::Caption) ChaosConsoleGate
                ]
                --
                Text(seed) TextRole(Role::Caption)
                --
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(2.0),
                }
                Children [
                    { states }
                ]
            ]
    }
}

/// Product assembly: the disabled-by-default console and its run observer.
pub struct ChaosConsolePlugin;

impl Plugin for ChaosConsolePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChaosConsole>();
        app.add_observer(on_run_chaos_exploration);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::MinimalPlugins;
    use bevy::app::App;
    use bevy::asset::{AssetApp, AssetPlugin};
    use bevy::image::Image;
    use bevy::scene::{CommandsSceneExt, ScenePlugin};
    use infiltrator_bevy_widgets::theme::Theme;

    fn packet_drop_config() -> ChaosFaultConfig {
        ChaosFaultConfig {
            is_enabled: true,
            packet_drop_rate: 0.5,
            ..ChaosFaultConfig::default()
        }
    }

    #[test]
    fn disabled_console_never_runs() {
        let mut console = ChaosConsole::default();
        assert!(!console.enabled, "the console is opt-in only");
        assert_eq!(console.run(&CHAOS_ROUTES), 0);
        assert!(console.discovered.is_empty());
        assert_eq!(console.runs, 0);
    }

    #[test]
    fn seeded_exploration_is_deterministic_and_bounded() {
        let mut first = ChaosConsole::enabled_for_doctor(7, 20, packet_drop_config());
        let mut second = ChaosConsole::enabled_for_doctor(7, 20, packet_drop_config());
        let discovered = first.run(&CHAOS_ROUTES);
        assert_eq!(discovered, second.run(&CHAOS_ROUTES));
        assert_eq!(first.discovered, second.discovered);
        assert!(discovered > 0);
        assert!(discovered <= 20);
        assert_eq!(first.runs, 1);
    }

    #[test]
    fn doctor_event_runs_the_exploration() {
        let mut app = App::new();
        app.add_plugins(ChaosConsolePlugin);
        app.world_mut().resource_mut::<ChaosConsole>().enable();
        app.world_mut().commands().trigger(RunChaosExploration);
        app.update();
        let console = app.world().resource::<ChaosConsole>();
        assert_eq!(console.runs, 1);
        assert!(!console.discovered.is_empty());
    }

    #[test]
    fn console_scene_shows_discovered_states() {
        let config = ChaosFaultConfig {
            is_enabled: true,
            packet_drop_rate: 1.0,
            ..ChaosFaultConfig::default()
        };
        let mut console = ChaosConsole::enabled_for_doctor(3, 16, config);
        console.run(&CHAOS_ROUTES);
        let palette = UiPalette::new(&Theme::dark());

        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.init_asset::<Image>();
        app.world_mut()
            .commands()
            .spawn_scene(chaos_console_scene(&console, &palette));
        app.update();

        let world = app.world_mut();
        let texts: Vec<String> = world
            .query::<&Text>()
            .iter(world)
            .map(|text| text.0.clone())
            .collect();
        assert!(texts.iter().any(|text| text.contains("Chaos Sandbox")));
        assert!(texts.iter().any(|text| text.contains("ENABLED")));
        assert!(
            texts.iter().any(|text| text.contains("PacketDrop")),
            "the discovered fault modes render: {texts:?}"
        );
    }
}

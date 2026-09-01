//! Shared headless composition for the widget tests.
//!
//! `MinimalPlugins` + (`AssetPlugin`, `ScenePlugin`) is the bsn! spawn
//! infrastructure; `TextPlugin` is the font-asset registration a windowed
//! `DefaultPlugins` composition contributes (the embedded font sources ride
//! its `Assets<Font>` store). `WidgetsPlugin` stays the only widget seam.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::scene::ScenePlugin;
use bevy::text::TextPlugin;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::theme::Theme;

pub fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin, TextPlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

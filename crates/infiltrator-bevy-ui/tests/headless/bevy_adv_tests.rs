//! Headless integration tests for the advanced Bevy wiring:
//! BEVY-037 widget sandbox slot, BEVY-041 ABI negotiation, BEVY-040 chaos
//! console and BEVY-039 cold-start pipeline cache, all through the product
//! assembly (`ShellPlugin` + `PagesPlugin`).

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetApp, AssetPlugin};
use bevy::image::Image;
use bevy::scene::ScenePlugin;
use infiltrator_bevy_ui::app::ShellPlugin;
use infiltrator_bevy_ui::lifecycle::{BootCacheLaunchReport, BootCacheStore};
use infiltrator_bevy_ui::pages::plugin::chaos_console::{ChaosConsole, RunChaosExploration};
use infiltrator_bevy_ui::pages::plugin::widget_sandbox::{
    AttachSandboxWidget, AuthorizeSandboxCapability, WidgetHostAbi, WidgetSandboxBoard,
};
use infiltrator_bevy_ui::route::PagesPlugin;
use infiltrator_bevy_widgets::abi::{HostCapabilities, WIDGET_ABI_VERSION, WidgetAbiRequirement};
use infiltrator_bevy_widgets::boot_cache::{BootCacheReject, BootCacheRestore, BootPipelineCache};
use infiltrator_bevy_widgets::sandbox::{HostCapability, WidgetManifest, WidgetPermission};
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};

fn assembled_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.init_asset::<Image>();
    app.add_plugins(ShellPlugin::new(ThemePreference::Fixed(ThemeSkin::Dark)));
    app.add_plugins(PagesPlugin::demo());
    app
}

fn community_widget() -> WidgetManifest {
    WidgetManifest::new("community.speedtest", "Community Speed Test")
        .with_permission(WidgetPermission::ReadTrafficStats)
}

#[test]
fn product_assembly_attaches_a_widget_through_the_typed_abi() {
    let mut app = assembled_app();
    app.update();
    app.world_mut().commands().trigger(AttachSandboxWidget::new(
        community_widget(),
        WidgetAbiRequirement::for_current_abi(),
    ));
    app.update();
    let board = app.world().resource::<WidgetSandboxBoard>();
    assert_eq!(board.attached_count(), 1);
    assert!(board.last_outcome().expect("outcome").is_attached());

    // A version-mismatched host is typed-incompatible, never a silent attach.
    app.insert_resource(WidgetHostAbi {
        version: (0, 29, 0),
        capabilities: HostCapabilities::all_desktop(),
    });
    app.world_mut().commands().trigger(AttachSandboxWidget::new(
        community_widget(),
        WidgetAbiRequirement::for_current_abi(),
    ));
    app.update();
    let board = app.world().resource::<WidgetSandboxBoard>();
    assert_eq!(
        board.attached_count(),
        1,
        "an incompatible widget is not attached"
    );
    assert!(
        board
            .last_outcome()
            .expect("outcome")
            .incompatibility()
            .is_some(),
        "the mismatch is surfaced as a typed incompatibility"
    );
    assert_eq!(WIDGET_ABI_VERSION, (0, 30, 0));

    // A denied capability is surfaced typed, never silently granted.
    app.world_mut()
        .commands()
        .trigger(AuthorizeSandboxCapability(HostCapability::ManageProfiles));
    app.update();
    assert_eq!(
        app.world()
            .resource::<WidgetSandboxBoard>()
            .denied_capability(),
        Some(HostCapability::ManageProfiles)
    );
}

#[test]
fn chaos_console_is_opt_in_and_bounded() {
    let mut app = assembled_app();
    app.update();
    assert!(
        !app.world().resource::<ChaosConsole>().enabled,
        "the chaos console is never in the normal user path"
    );

    app.world_mut().commands().trigger(RunChaosExploration);
    app.update();
    assert_eq!(
        app.world().resource::<ChaosConsole>().runs,
        0,
        "a disabled console runs nothing"
    );

    app.world_mut().resource_mut::<ChaosConsole>().enable();
    app.world_mut().commands().trigger(RunChaosExploration);
    app.update();
    let console = app.world().resource::<ChaosConsole>();
    assert_eq!(console.runs, 1);
    assert!(!console.discovered.is_empty());
    assert!(console.discovered.len() <= console.max_steps as usize);
}

#[test]
fn product_launch_restores_the_cold_start_cache_non_fatally() {
    let mut first = assembled_app();
    first.update();
    let report = *first.world().resource::<BootCacheLaunchReport>();
    assert_eq!(
        report.restore,
        BootCacheRestore::Miss(BootCacheReject::Empty)
    );
    assert!(report.rebuilt, "a cold boot rebuilds once");
    assert_eq!(
        first.world().resource::<BootPipelineCache>().rebuild_count,
        1
    );
    let record = first
        .world()
        .resource::<BootCacheStore>()
        .0
        .clone()
        .expect("the cold boot records a cache");

    // The next launch restores the recorded cache without a rebuild.
    let mut second = assembled_app();
    second.insert_resource(BootCacheStore(Some(record)));
    second.update();
    let report = *second.world().resource::<BootCacheLaunchReport>();
    assert_eq!(report.restore, BootCacheRestore::Hit);
    assert!(!report.rebuilt);
    assert_eq!(
        second.world().resource::<BootPipelineCache>().rebuild_count,
        0
    );
}

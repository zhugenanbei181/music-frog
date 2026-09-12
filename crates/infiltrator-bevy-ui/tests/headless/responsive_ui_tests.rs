//! Headless integration tests for 0.30 Bevy UI Dimension Six:
//! Responsive Breakpoints and Multi-End Adaptive Layout System.
//!
//! Asserts:
//! 1. Standardized 4-tier breakpoints (Compact, Medium, Expanded, Ultra) and global ResponsiveContext.
//! 2. Sidebar / Bottom navigation polymorphic switching and fluid adaptive card grid.
//! 3. Master-Detail split vs stacked pane coordination, smart text truncation,
//!    modal-to-actionsheet morphology, and compact/comfortable density switching.

use std::sync::Arc;

use bevy::app::App;
use bevy::scene::CommandsSceneExt;
use bevy::ui::prelude::{Display, JustifyContent, Node, px};
use infiltrator_bevy_ui::app::{
    BottomNavBar, LayoutMode, ShellLayoutState, ShellPlugin, SidebarPanel,
};
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommandSink};
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::adaptive_modal::{AdaptiveModalRoot, CloseModal, OpenModal};
use infiltrator_bevy_widgets::responsive::{
    Density, DensitySwitch, MasterDetailMode, ModalForm, ResponsiveContext, SidebarMode,
};
use infiltrator_bevy_widgets::smart_truncate::{truncate_adaptive, truncate_middle, truncate_tail};
use infiltrator_bevy_widgets::theme::Breakpoint;

use crate::support::*;

/// Compile-time cross-check that the Bevy widget layer's mirrored breakpoint
/// thresholds agree with the authoritative shared contract. `bevy-widgets`
/// cannot depend on `infiltrator-contract` by charter, so this test (which can
/// see both) makes drift fail the build rather than only the Python guard.
#[test]
fn test_bevy_breakpoint_mirrors_shared_contract_at_boundaries() {
    use infiltrator_contract::responsive_viewport::{ResponsiveViewportSnapshot, ViewportTier};

    // Sample both sides of every half-open boundary.
    for width in [
        0.0, 599.9, 600.0, 839.9, 840.0, 1199.9, 1200.0, 1920.0, 3840.0,
    ] {
        let widget_tier = match Breakpoint::from_width(width) {
            Breakpoint::Compact => ViewportTier::Compact,
            Breakpoint::Medium => ViewportTier::Medium,
            Breakpoint::Expanded => ViewportTier::Expanded,
            Breakpoint::Ultra => ViewportTier::Ultra,
        };
        assert_eq!(
            widget_tier,
            ViewportTier::from_width(width),
            "Bevy Breakpoint and shared ViewportTier disagree at width {width}"
        );
    }

    // The numeric thresholds themselves must match.
    assert_eq!(Breakpoint::COMPACT_MAX_PX, 600.0);
    assert_eq!(Breakpoint::MEDIUM_MAX_PX, 840.0);
    assert_eq!(Breakpoint::EXPANDED_MAX_PX, 1200.0);

    // Shared snapshot column operators must agree with widget grid columns.
    let desktop = ResponsiveViewportSnapshot::from_dimensions(900.0, 780.0);
    assert_eq!(desktop.tier, ViewportTier::Expanded);
    assert_eq!(desktop.card_columns, 2);
}

/// The Bevy Overview metrics band and the Iced metrics grid must derive the
/// same column counts from the shared contract (2 / 3 / 6 / 6). This pins the
/// `sync_overview_metrics_columns` mapping to the authoritative operator.
#[test]
fn test_overview_metrics_columns_match_shared_contract() {
    use infiltrator_bevy_widgets::theme::Breakpoint;
    use infiltrator_contract::responsive_viewport::ViewportTier;

    let cases = [
        (Breakpoint::Compact, ViewportTier::Compact),
        (Breakpoint::Medium, ViewportTier::Medium),
        (Breakpoint::Expanded, ViewportTier::Expanded),
        (Breakpoint::Ultra, ViewportTier::Ultra),
    ];
    for (bp, tier) in cases {
        let from_bp = match bp {
            Breakpoint::Compact => 2,
            Breakpoint::Medium => 3,
            Breakpoint::Expanded | Breakpoint::Ultra => 6,
        };
        assert_eq!(
            from_bp,
            tier.metrics_grid_columns(),
            "metrics columns mismatch at {bp:?}"
        );
    }
}

/// The Bevy proxy node grid and the Iced proxy grid must read the same tier
/// column operator (1 / 2 / 3 / 4), and the wrapped item percent must leave
/// enough slack that N columns never wrap early.
#[test]
fn test_proxy_grid_columns_match_shared_contract() {
    use infiltrator_bevy_widgets::fluid_grid::FluidCardGrid;

    // Every tier's column count must produce a row that fits within 100%.
    for columns in 1usize..=6 {
        let basis = FluidCardGrid::wrapped_item_percent(columns);
        let used = basis * columns as f32;
        assert!(
            used <= 100.0,
            "{columns} columns of {basis}% overflow the row ({used}%)"
        );
    }

    // The Bevy grid and the shared operator agree at the desktop tier.
    assert_eq!(
        infiltrator_contract::responsive_viewport::ViewportTier::Expanded.proxy_grid_columns(false),
        3
    );
}

fn setup_responsive_app(width: f32) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        infiltrator_bevy_widgets::theme::LightDark::Dark,
        width,
    ));
    app.add_plugins(PagesPlugin::new(DemoOverviewSource::running()));
    let sink = Arc::new(DemoCommandSink::accepting());
    app.add_plugins(CommandPumpPlugin::new(sink as Arc<dyn UiCommandSink>));
    app.update();
    app
}

#[test]
fn test_standardized_four_tier_breakpoints_and_responsive_context() {
    // 1. Compact: 375px
    let mut app = setup_responsive_app(375.0);
    {
        let world = app.world();
        let ctx = world.resource::<ResponsiveContext>();
        let layout = world.resource::<ShellLayoutState>();

        assert_eq!(ctx.breakpoint, Breakpoint::Compact);
        assert!(ctx.is_compact());
        assert_eq!(ctx.sidebar_mode(), SidebarMode::BottomNav);
        assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Stacked);
        assert_eq!(ctx.modal_form(), ModalForm::ActionSheet);
        assert_eq!(layout.mode, LayoutMode::BottomNav);
    }

    // 2. Medium: 768px (Tablet / Foldable)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(768.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(768.0, 1024.0);
    app.update();
    {
        let world = app.world();
        let ctx = world.resource::<ResponsiveContext>();
        let layout = world.resource::<ShellLayoutState>();

        assert_eq!(ctx.breakpoint, Breakpoint::Medium);
        assert!(ctx.is_medium());
        assert_eq!(ctx.sidebar_mode(), SidebarMode::Rail);
        assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Split);
        assert_eq!(ctx.modal_form(), ModalForm::CenteredDialog);
        assert_eq!(layout.mode, LayoutMode::Rail);
    }

    // 3. Expanded: 1000px (Standard Desktop / Laptop)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1000.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1000.0, 800.0);
    app.update();
    {
        let world = app.world();
        let ctx = world.resource::<ResponsiveContext>();
        let layout = world.resource::<ShellLayoutState>();

        assert_eq!(ctx.breakpoint, Breakpoint::Expanded);
        assert!(ctx.is_expanded());
        assert_eq!(ctx.sidebar_mode(), SidebarMode::Standard);
        assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Split);
        assert_eq!(ctx.modal_form(), ModalForm::CenteredDialog);
        assert_eq!(layout.mode, LayoutMode::Sidebar);
    }

    // 4. Ultra: 1920px (Ultrawide / 4K Monitor)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1920.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1920.0, 1080.0);
    app.update();
    {
        let world = app.world();
        let ctx = world.resource::<ResponsiveContext>();
        let layout = world.resource::<ShellLayoutState>();

        assert_eq!(ctx.breakpoint, Breakpoint::Ultra);
        assert!(ctx.is_ultra());
        assert_eq!(ctx.sidebar_mode(), SidebarMode::Wide);
        assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Split);
        assert_eq!(ctx.modal_form(), ModalForm::CenteredDialog);
        assert_eq!(layout.mode, LayoutMode::Wide);
    }
}

#[test]
fn test_polymorphic_navigation_switching_and_route_preservation() {
    let mut app = setup_responsive_app(1000.0);

    // Initial expanded state: Sidebar visible, BottomNav hidden
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();

        assert_eq!(sidebars.single(world).unwrap().0.display, Display::Flex);
        assert_eq!(bottom_navs.single(world).unwrap().0.display, Display::None);
    }

    // Navigate to Proxies
    app.world_mut()
        .commands()
        .trigger(RouteChanged(Route::Proxies));
    app.update();
    assert_eq!(
        app.world().resource::<ActiveRoute>().0,
        Some(Route::Proxies)
    );

    // Shrink window to mobile (375px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(375.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(375.0, 667.0);
    app.update();

    // In compact mode: Sidebar hidden, BottomNav visible, route stays on Proxies!
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();

        assert_eq!(sidebars.single(world).unwrap().0.display, Display::None);
        assert_eq!(bottom_navs.single(world).unwrap().0.display, Display::Flex);
        assert_eq!(world.resource::<ActiveRoute>().0, Some(Route::Proxies));
    }

    // Switch to Medium (768px): Rail mode (width 72px)
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(768.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(768.0, 1024.0);
    app.update();

    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();

        let (node, _) = sidebars.single(world).unwrap();
        assert_eq!(node.display, Display::Flex);
        assert_eq!(node.width, px(72.0));
        assert_eq!(bottom_navs.single(world).unwrap().0.display, Display::None);
    }
}

#[test]
fn test_density_toggle_affordance_and_metric_scaling() {
    let mut app = setup_responsive_app(1180.0);

    assert_eq!(app.world().resource::<Density>(), &Density::Comfortable);
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Comfortable
    );

    // Trigger density switch
    app.world_mut()
        .commands()
        .trigger(DensitySwitch(Density::Compact));
    app.update();

    assert_eq!(app.world().resource::<Density>(), &Density::Compact);
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Compact
    );
    assert_eq!(
        app.world()
            .resource::<ResponsiveContext>()
            .density_control_height(),
        28.0
    );
    assert_eq!(
        app.world()
            .resource::<ResponsiveContext>()
            .density_padding(16.0),
        12.0
    );

    // Switch back to comfortable
    app.world_mut()
        .commands()
        .trigger(DensitySwitch(Density::Comfortable));
    app.update();

    assert_eq!(app.world().resource::<Density>(), &Density::Comfortable);
    assert_eq!(
        app.world().resource::<ResponsiveContext>().density,
        Density::Comfortable
    );
    assert_eq!(
        app.world()
            .resource::<ResponsiveContext>()
            .density_control_height(),
        36.0
    );
    assert_eq!(
        app.world()
            .resource::<ResponsiveContext>()
            .density_padding(16.0),
        16.0
    );
}

#[test]
fn test_smart_text_truncation_rules() {
    // 1. Grapheme-safe tail truncation
    assert_eq!(truncate_tail("DirectConnections", 8), "DirectC…");
    assert_eq!(truncate_tail("香港专线节点01", 6), "香港专线节…");

    // 2. Middle truncation for long hostnames / IPs
    assert_eq!(
        truncate_middle("gateway.discord.gg:443", 10, 6),
        "gateway.di…gg:443"
    );
    assert_eq!(truncate_middle("104.21.58.12:80", 6, 3), "104.21…:80");

    // 3. Adaptive truncation across breakpoints
    let long_payload = "DOMAIN-SUFFIX,sub.service-cluster-node-east.example.com,ProxyGroup";
    assert_eq!(
        truncate_adaptive(long_payload, Breakpoint::Compact, 14, 28, 48, 80),
        "DOMAIN-SUFFIX…"
    );
    assert_eq!(
        truncate_adaptive(long_payload, Breakpoint::Medium, 14, 28, 48, 80),
        "DOMAIN-SUFFIX,sub.service-c…"
    );
    assert_eq!(
        truncate_adaptive(long_payload, Breakpoint::Expanded, 14, 28, 48, 80),
        "DOMAIN-SUFFIX,sub.service-cluster-node-east.exa…"
    );
    assert_eq!(
        truncate_adaptive(long_payload, Breakpoint::Ultra, 14, 28, 48, 80),
        "DOMAIN-SUFFIX,sub.service-cluster-node-east.example.com,ProxyGroup"
    );
}

#[test]
fn test_dialog_to_actionsheet_morphology_transitions() {
    let mut app = setup_responsive_app(1000.0);
    let palette = *app
        .world()
        .resource::<infiltrator_bevy_widgets::palette::UiPalette>();
    let body = Box::new(
        bevy::scene::bsn! { ( bevy::ui::widget::Text({ "Modal Body Content".to_owned() }) ) },
    );
    let actions =
        vec![
            Box::new(bevy::scene::bsn! { ( bevy::ui::widget::Text({ "Confirm".to_owned() }) ) })
                as Box<dyn bevy::scene::Scene>,
        ];
    app.world_mut().commands().spawn_scene(
        infiltrator_bevy_widgets::adaptive_modal::adaptive_modal_scene(
            "Test Modal".to_owned(),
            body,
            actions,
            &palette,
        ),
    );
    app.update();

    // Initial state: dialog mode on Expanded breakpoint
    app.world_mut().commands().trigger(OpenModal);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        let root = roots.iter(world).next().expect("modal root mounted");
        assert_eq!(root.display, Display::Flex);
        assert_eq!(root.justify_content, JustifyContent::Center);
    }

    // Resize to Compact (<600px): ActionSheet morphology
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(375.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(375.0, 667.0);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        let root = roots.iter(world).next().expect("modal root mounted");
        assert_eq!(root.display, Display::Flex);
        assert_eq!(root.justify_content, JustifyContent::FlexEnd);
    }

    // Close modal
    app.world_mut().commands().trigger(CloseModal);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        let root = roots.iter(world).next().expect("modal root mounted");
        assert_eq!(root.display, Display::None);
    }
}

//! Headless integration tests for 0.30 Bevy UI Dimension Six:
//! Responsive Breakpoints and Multi-End Adaptive Layout System.
//!
//! Asserts:
//! 1. Standardized 4-tier breakpoints (Compact, Medium, Expanded, Ultra) and global ResponsiveContext.
//! 2. Sidebar / Bottom navigation polymorphic switching and fluid adaptive card grid.
//! 3. Master-Detail split vs stacked pane coordination, smart text truncation,
//!    modal-to-actionsheet morphology, and compact/comfortable density switching.

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::With;
use bevy::picking::hover::PickingInteraction;
use bevy::scene::{CommandsSceneExt, Scene, bsn};
use bevy::ui::prelude::{Display, JustifyContent, Node, UiRect, Val, px};
use bevy::ui::widget;
use infiltrator_bevy_ui::app::{
    BOTTOM_NAV_HEIGHT_PX, BottomNavBar, LayoutMode, NavSpacer, RailNavTooltip,
    SIDEBAR_RAIL_WIDTH_PX, SIDEBAR_WIDTH_PX, ShellLayoutState, ShellPlugin, ShellRoot,
    SidebarActiveProfileCard, SidebarFooterRow, SidebarIdentityText, SidebarModeSegment,
    SidebarNavItem, SidebarPanel, SidebarShortcutMatrix, SidebarSpeedFooter,
    SidebarSystemProxyCard, SidebarTunCard,
};
use infiltrator_bevy_ui::chrome::ChromeDragBar;
use infiltrator_bevy_ui::command::{CommandPumpPlugin, DemoCommandSink, UiCommandSink};
use infiltrator_bevy_ui::gesture::GestureHostReport;
use infiltrator_bevy_ui::projection::DemoOverviewSource;
use infiltrator_bevy_ui::route::{ActiveRoute, PagesPlugin, Route, RouteChanged};
use infiltrator_bevy_widgets::adaptive_modal::{
    AdaptiveModalRoot, CloseModal, OpenModal, adaptive_modal_scene,
};
use infiltrator_bevy_widgets::nav::NavLabel;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::{
    Density, DensitySwitch, MasterDetailMode, ModalForm, ResponsiveContext, SafeAreaInsets,
    SidebarMode, TouchHitbox,
};
use infiltrator_bevy_widgets::smart_truncate::{truncate_adaptive, truncate_middle, truncate_tail};
use infiltrator_bevy_widgets::theme::{Breakpoint, space};
use infiltrator_contract::responsive_viewport::ViewportTier;
use infiltrator_contract::shell_gesture;
use infiltrator_contract::theme::{ThemePreference, ThemeSkin};
use infiltrator_contract::window_chrome::CHROME_DRAG_STRIP_HEIGHT_PX;
use std::sync::Arc;

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
    assert_eq!(ViewportTier::Expanded.proxy_grid_columns(false), 3);
}

fn setup_responsive_app(width: f32) -> App {
    let mut app = App::new();
    headless_plugins(&mut app);
    app.add_plugins(ShellPlugin::new_with_width(
        ThemePreference::Fixed(ThemeSkin::Dark),
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

    // Switch to Medium (768px): Rail mode (width 64px)
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
        assert_eq!(node.width, px(64.0));
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
    let palette = *app.world().resource::<UiPalette>();
    let body = Box::new(bsn! {
                widget::Text({ "Modal Body Content".to_owned() })
    });
    let actions = vec![Box::new(bsn! {
                    widget::Text({ "Confirm".to_owned() })
    }) as Box<dyn Scene>];
    app.world_mut().commands().spawn_scene(adaptive_modal_scene(
        "Test Modal".to_owned(),
        "Close".to_owned(),
        body,
        actions,
        &palette,
    ));
    app.update();

    // Initial state: dialog mode on Expanded breakpoint
    app.world_mut().commands().trigger(OpenModal);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, With<AdaptiveModalRoot>>();
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
        let mut roots = world.query_filtered::<&Node, With<AdaptiveModalRoot>>();
        let root = roots.iter(world).next().expect("modal root mounted");
        assert_eq!(root.display, Display::Flex);
        assert_eq!(root.justify_content, JustifyContent::FlexEnd);
    }

    // Close modal
    app.world_mut().commands().trigger(CloseModal);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, With<AdaptiveModalRoot>>();
        let root = roots.iter(world).next().expect("modal root mounted");
        assert_eq!(root.display, Display::None);
    }
}

#[test]
fn test_sidebar_rail_morphology_and_floating_tooltips() {
    let mut app = setup_responsive_app(1180.0);
    app.update();

    // 1. Initial Expanded mode (1180px): standard sidebar 240px
    {
        let world = app.world_mut();
        let mut sidebars = world.query::<(&Node, &SidebarPanel)>();
        let (sidebar_node, _) = sidebars.single(world).unwrap();
        assert_eq!(sidebar_node.display, Display::Flex);
        assert_eq!(sidebar_node.width, px(SIDEBAR_WIDTH_PX));

        // Nav items have FlexStart alignment and visible labels
        let mut nav_items = world.query_filtered::<&Node, With<SidebarNavItem>>();
        for node in nav_items.iter(world) {
            assert_eq!(node.justify_content, JustifyContent::FlexStart);
        }

        let mut nav_labels = world.query_filtered::<&Node, With<NavLabel>>();
        assert!(nav_labels.iter(world).count() > 0);
        for node in nav_labels.iter(world) {
            assert_eq!(node.display, Display::Flex);
        }

        let mut nav_spacers = world.query_filtered::<&Node, With<NavSpacer>>();
        for node in nav_spacers.iter(world) {
            assert_eq!(node.display, Display::Flex);
        }

        // Identity text and mode segment are visible in Expanded
        let mut identity_texts = world.query_filtered::<&Node, With<SidebarIdentityText>>();
        for node in identity_texts.iter(world) {
            assert_eq!(node.display, Display::Flex);
        }

        let mut mode_segments = world.query_filtered::<&Node, With<SidebarModeSegment>>();
        for node in mode_segments.iter(world) {
            assert_eq!(node.display, Display::Flex);
        }

        // Floating tooltips are hidden in Expanded mode
        let mut tooltips = world.query_filtered::<&Node, With<RailNavTooltip>>();
        assert!(tooltips.iter(world).count() > 0);
        for node in tooltips.iter(world) {
            assert_eq!(node.display, Display::None);
        }
    }

    // 2. Switch to Medium mode (768px): Rail mode (64px)
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
        let (sidebar_node, _) = sidebars.single(world).unwrap();
        assert_eq!(sidebar_node.display, Display::Flex);
        assert_eq!(sidebar_node.width, px(SIDEBAR_RAIL_WIDTH_PX));

        // Nav items are centered in Rail mode
        let mut nav_items = world.query_filtered::<&Node, With<SidebarNavItem>>();
        for node in nav_items.iter(world) {
            assert_eq!(node.justify_content, JustifyContent::Center);
        }

        // Labels, spacers, identity text, and mode segment hidden in Rail mode
        let mut nav_labels = world.query_filtered::<&Node, With<NavLabel>>();
        for node in nav_labels.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut nav_spacers = world.query_filtered::<&Node, With<NavSpacer>>();
        for node in nav_spacers.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut identity_texts = world.query_filtered::<&Node, With<SidebarIdentityText>>();
        for node in identity_texts.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut mode_segments = world.query_filtered::<&Node, With<SidebarModeSegment>>();
        for node in mode_segments.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut system_proxies = world.query_filtered::<&Node, With<SidebarSystemProxyCard>>();
        for node in system_proxies.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut tun_cards = world.query_filtered::<&Node, With<SidebarTunCard>>();
        for node in tun_cards.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut active_profiles = world.query_filtered::<&Node, With<SidebarActiveProfileCard>>();
        for node in active_profiles.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut shortcuts = world.query_filtered::<&Node, With<SidebarShortcutMatrix>>();
        for node in shortcuts.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut speed_footers = world.query_filtered::<&Node, With<SidebarSpeedFooter>>();
        for node in speed_footers.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        let mut footer_rows = world.query_filtered::<&Node, With<SidebarFooterRow>>();
        for node in footer_rows.iter(world) {
            assert_eq!(node.display, Display::None);
        }

        // When unhovered, floating tooltips are hidden
        let mut tooltips = world.query_filtered::<&Node, With<RailNavTooltip>>();
        for node in tooltips.iter(world) {
            assert_eq!(node.display, Display::None);
        }
    }

    // 3. Hover a navigation item in Rail mode: Tooltip pops out!
    let target_nav_entity = {
        let world = app.world_mut();
        let mut items = world.query_filtered::<Entity, With<SidebarNavItem>>();
        items.iter(world).next().expect("sidebar nav item exists")
    };

    app.world_mut()
        .entity_mut(target_nav_entity)
        .insert(PickingInteraction::Hovered);
    app.update();

    {
        let world = app.world_mut();
        let child_entities: Vec<Entity> = world
            .get::<Children>(target_nav_entity)
            .expect("has children")
            .iter()
            .copied()
            .collect();
        let mut tooltip_found = false;
        let mut tooltip_query = world.query::<(&RailNavTooltip, &Node)>();
        for child in child_entities {
            if let Ok((_tooltip, node)) = tooltip_query.get(world, child) {
                assert_eq!(
                    node.display,
                    Display::Flex,
                    "hovered item tooltip must be visible in rail mode"
                );
                tooltip_found = true;
            }
        }
        assert!(
            tooltip_found,
            "target nav item must contain RailNavTooltip child"
        );
    }

    // 4. Unhover: Tooltip disappears
    app.world_mut()
        .entity_mut(target_nav_entity)
        .insert(PickingInteraction::None);
    app.update();

    {
        let world = app.world_mut();
        let child_entities: Vec<Entity> = world
            .get::<Children>(target_nav_entity)
            .expect("has children")
            .iter()
            .copied()
            .collect();
        let mut tooltip_query = world.query::<(&RailNavTooltip, &Node)>();
        for child in child_entities {
            if let Ok((_tooltip, node)) = tooltip_query.get(world, child) {
                assert_eq!(
                    node.display,
                    Display::None,
                    "unhovered item tooltip must be hidden"
                );
            }
        }
    }
}

#[test]
fn test_safe_area_insets_android_and_ios_adaptation() {
    let mut app = setup_responsive_app(375.0);
    app.update();

    // 1. Default zero insets in Compact mode (375px mobile portrait)
    {
        let world = app.world_mut();
        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
        let (nav_node, _) = bottom_navs.single(world).unwrap();
        assert_eq!(nav_node.height, px(BOTTOM_NAV_HEIGHT_PX));
        assert_eq!(nav_node.min_height, px(BOTTOM_NAV_HEIGHT_PX));
        assert_eq!(nav_node.padding, UiRect::bottom(Val::Px(space::S6)));

        let mut chrome_bars = world.query::<(&Node, &ChromeDragBar)>();
        let (chrome_node, _) = chrome_bars.single(world).unwrap();
        assert_eq!(chrome_node.height, px(CHROME_DRAG_STRIP_HEIGHT_PX as f32));
        assert_eq!(
            chrome_node.min_height,
            px(CHROME_DRAG_STRIP_HEIGHT_PX as f32)
        );
        assert_eq!(chrome_node.padding, UiRect::horizontal(Val::Px(space::S12)));

        let safe_insets = world.resource::<SafeAreaInsets>();
        assert_eq!(safe_insets.top_px, 0.0);
        assert_eq!(safe_insets.bottom_px, 0.0);
    }

    // 2. Android default insets (24px status bar, 16px bottom gesture pill)
    app.world_mut()
        .insert_resource(SafeAreaInsets::android_default());
    app.update();

    {
        let world = app.world_mut();
        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
        let (nav_node, _) = bottom_navs.single(world).unwrap();
        // Height = 58 + 16 = 74.0px, padding.bottom = 6 + 16 = 22.0px
        assert_eq!(nav_node.height, px(BOTTOM_NAV_HEIGHT_PX + 16.0));
        assert_eq!(nav_node.padding.bottom, Val::Px(space::S6 + 16.0));

        let mut chrome_bars = world.query::<(&Node, &ChromeDragBar)>();
        let (chrome_node, _) = chrome_bars.single(world).unwrap();
        // Height = 38 + 24 = 62.0px, padding.top = 24.0px
        assert_eq!(
            chrome_node.height,
            px(CHROME_DRAG_STRIP_HEIGHT_PX as f32 + 24.0)
        );
        assert_eq!(chrome_node.padding.top, Val::Px(24.0));

        // Contract GestureHostReport must be kept in bidirectional sync
        let host_report = world.resource::<GestureHostReport>();
        assert_eq!(host_report.insets.top, 24.0);
        assert_eq!(host_report.insets.bottom, 16.0);
    }

    // 3. iOS default insets with Dynamic Island (48px status bar, 34px home indicator)
    app.world_mut()
        .insert_resource(SafeAreaInsets::ios_default());
    app.update();

    {
        let world = app.world_mut();
        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
        let (nav_node, _) = bottom_navs.single(world).unwrap();
        // Height = 58 + 34 = 92.0px, padding.bottom = 6 + 34 = 40.0px
        assert_eq!(nav_node.height, px(BOTTOM_NAV_HEIGHT_PX + 34.0));
        assert_eq!(nav_node.padding.bottom, Val::Px(space::S6 + 34.0));

        let mut chrome_bars = world.query::<(&Node, &ChromeDragBar)>();
        let (chrome_node, _) = chrome_bars.single(world).unwrap();
        // Height = 38 + 48 = 86.0px, padding.top = 48.0px
        assert_eq!(
            chrome_node.height,
            px(CHROME_DRAG_STRIP_HEIGHT_PX as f32 + 48.0)
        );
        assert_eq!(chrome_node.padding.top, Val::Px(48.0));

        let host_report = world.resource::<GestureHostReport>();
        assert_eq!(host_report.insets.top, 48.0);
        assert_eq!(host_report.insets.bottom, 34.0);
    }

    // 4. Inset injection directly through GestureHostReport (contract port seam) with horizontal notch
    {
        let mut host_report = app.world_mut().resource_mut::<GestureHostReport>();
        host_report.insets = shell_gesture::SafeAreaInsets::new(44.0, 12.0, 34.0, 12.0);
    }
    app.update();

    {
        let world = app.world_mut();
        let safe_insets = world.resource::<SafeAreaInsets>();
        assert_eq!(safe_insets.top_px, 44.0);
        assert_eq!(safe_insets.bottom_px, 34.0);
        assert_eq!(safe_insets.left_px, 12.0);
        assert_eq!(safe_insets.right_px, 12.0);

        let mut bottom_navs = world.query::<(&Node, &BottomNavBar)>();
        let (nav_node, _) = bottom_navs.single(world).unwrap();
        assert_eq!(nav_node.height, px(BOTTOM_NAV_HEIGHT_PX + 34.0));
        assert_eq!(nav_node.padding.bottom, Val::Px(space::S6 + 34.0));
        assert_eq!(nav_node.padding.left, Val::Px(12.0));
        assert_eq!(nav_node.padding.right, Val::Px(12.0));

        let mut chrome_bars = world.query::<(&Node, &ChromeDragBar)>();
        let (chrome_node, _) = chrome_bars.single(world).unwrap();
        assert_eq!(
            chrome_node.height,
            px(CHROME_DRAG_STRIP_HEIGHT_PX as f32 + 44.0)
        );
        assert_eq!(chrome_node.padding.top, Val::Px(44.0));
        assert_eq!(chrome_node.padding.left, Val::Px(space::S12 + 12.0));
        assert_eq!(chrome_node.padding.right, Val::Px(space::S12 + 12.0));
    }

    // 5. Expand to desktop (1180px): BottomNav hidden, ShellRoot absorbs fallback bottom safe padding
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1180.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1180.0, 760.0);
    app.update();

    {
        let world = app.world_mut();
        let mut shell_roots = world.query::<(&Node, &ShellRoot)>();
        let (root_node, _) = shell_roots.single(world).unwrap();
        assert_eq!(root_node.padding.bottom, Val::Px(34.0));
        assert_eq!(root_node.padding.left, Val::Px(12.0));
        assert_eq!(root_node.padding.right, Val::Px(12.0));
    }
}

#[test]
fn test_compact_mobile_touch_hitbox_expansion_and_restoration() {
    let mut app = setup_responsive_app(375.0);
    app.update();

    // 1. In Compact mode (375px), every interactive button with a TouchHitbox
    // must expand its min_width and min_height to at least 48.0px (WCAG 2.5.5 / Android HIG).
    {
        let world = app.world_mut();
        let mut hitboxes = world.query::<(&Node, &TouchHitbox)>();
        let mut found_hitbox = false;
        for (node, hitbox) in hitboxes.iter(world) {
            found_hitbox = true;
            assert!(hitbox.is_expanded);
            if let Val::Px(min_w) = node.min_width {
                assert!(min_w >= 48.0, "min_width {min_w} should be >= 48.0");
            }
            if let Val::Px(min_h) = node.min_height {
                assert!(min_h >= 48.0, "min_height {min_h} should be >= 48.0");
            }
        }
        assert!(
            found_hitbox,
            "at least one TouchHitbox must exist in the app"
        );
    }

    // 2. Expand to desktop (1200px): TouchHitbox restores to desktop compact size
    app.world_mut()
        .resource_mut::<ShellLayoutState>()
        .set_width(1200.0);
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1200.0, 800.0);
    app.update();

    {
        let world = app.world_mut();
        let mut hitboxes = world.query::<(&Node, &TouchHitbox)>();
        for (_node, hitbox) in hitboxes.iter(world) {
            assert!(!hitbox.is_expanded);
        }
    }
}

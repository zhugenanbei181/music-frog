/// Headless integration tests for the 4-tier responsive breakpoint and multi-end adaptive layout engine.
use bevy::MinimalPlugins;
use bevy::app::{App, Startup};
use bevy::asset::AssetPlugin;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::system::{Commands, Res};
use bevy::scene::{CommandsSceneExt, Scene, ScenePlugin, bsn};
use bevy::ui::prelude::{Display, FlexDirection, JustifyContent, Node, Val, percent, px};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::WidgetsPlugin;
use infiltrator_bevy_widgets::adaptive_modal::{
    AdaptiveModalRoot, CloseModal, ModalCard, OpenModal, adaptive_modal_scene,
};
use infiltrator_bevy_widgets::fluid_grid::{FluidGridConfig, FluidGridItem, fluid_card_grid_scene};
use infiltrator_bevy_widgets::master_detail::{
    DetailPane, MasterDetailState, MasterDetailView, MasterItemButton, MasterItemSelected,
    MasterPane, master_back_button_scene, master_detail_scene,
};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::{
    Density, DensitySwitch, MasterDetailMode, ModalForm, Orientation, ResponsiveContext,
    SidebarMode,
};
use infiltrator_bevy_widgets::smart_truncate::{
    SmartTruncateText, truncate_adaptive, truncate_adaptive_middle, truncate_middle, truncate_tail,
};
use infiltrator_bevy_widgets::theme::{Breakpoint, Theme, radius};

fn headless_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((AssetPlugin::default(), ScenePlugin));
    app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
    app
}

#[test]
fn four_tier_breakpoint_classification() {
    // 1. Compact: < 600px
    let bp_compact = Breakpoint::from_width(375.0);
    assert_eq!(bp_compact, Breakpoint::Compact);
    assert!(bp_compact.is_compact());
    assert!(bp_compact.is_mobile());
    assert!(!bp_compact.is_medium());
    assert!(!bp_compact.is_expanded());
    assert!(!bp_compact.is_ultra());
    assert_eq!(bp_compact.sidebar_width_px(), None);
    assert_eq!(bp_compact.default_grid_columns(), 1);

    let bp_compact_edge = Breakpoint::from_width(599.9);
    assert_eq!(bp_compact_edge, Breakpoint::Compact);

    // 2. Medium: 600px .. 1024px
    let bp_medium = Breakpoint::from_width(600.0);
    assert_eq!(bp_medium, Breakpoint::Medium);
    assert!(bp_medium.is_medium());
    assert!(bp_medium.is_tablet());
    assert!(!bp_medium.is_compact());
    assert!(!bp_medium.is_expanded());
    assert!(!bp_medium.is_ultra());
    assert_eq!(bp_medium.sidebar_width_px(), Some(72.0));
    assert_eq!(bp_medium.default_grid_columns(), 2);

    let bp_medium_edge = Breakpoint::from_width(1023.9);
    assert_eq!(bp_medium_edge, Breakpoint::Medium);

    // 3. Expanded: 1024px .. 1440px
    let bp_expanded = Breakpoint::from_width(1024.0);
    assert_eq!(bp_expanded, Breakpoint::Expanded);
    assert!(bp_expanded.is_expanded());
    assert!(bp_expanded.is_desktop());
    assert!(!bp_expanded.is_compact());
    assert!(!bp_expanded.is_medium());
    assert!(!bp_expanded.is_ultra());
    assert_eq!(bp_expanded.sidebar_width_px(), Some(240.0));
    assert_eq!(bp_expanded.default_grid_columns(), 3);

    let bp_expanded_edge = Breakpoint::from_width(1439.9);
    assert_eq!(bp_expanded_edge, Breakpoint::Expanded);

    // 4. Ultra: >= 1440px
    let bp_ultra = Breakpoint::from_width(1440.0);
    assert_eq!(bp_ultra, Breakpoint::Ultra);
    assert!(bp_ultra.is_ultra());
    assert!(bp_ultra.is_desktop());
    assert!(!bp_ultra.is_compact());
    assert!(!bp_ultra.is_medium());
    assert!(!bp_ultra.is_expanded());
    assert_eq!(bp_ultra.sidebar_width_px(), Some(280.0));
    assert_eq!(bp_ultra.default_grid_columns(), 4);

    let bp_ultra_4k = Breakpoint::from_width(3840.0);
    assert_eq!(bp_ultra_4k, Breakpoint::Ultra);
}

#[test]
fn responsive_context_derives_layout_modes() {
    let mut ctx = ResponsiveContext::new(375.0, 812.0);
    assert_eq!(ctx.breakpoint, Breakpoint::Compact);
    assert_eq!(ctx.orientation, Orientation::Portrait);
    assert_eq!(ctx.sidebar_mode(), SidebarMode::BottomNav);
    assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Stacked);
    assert_eq!(ctx.modal_form(), ModalForm::ActionSheet);

    ctx.set_dimensions(768.0, 1024.0);
    assert_eq!(ctx.breakpoint, Breakpoint::Medium);
    assert_eq!(ctx.orientation, Orientation::Portrait);
    assert_eq!(ctx.sidebar_mode(), SidebarMode::Rail);
    assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Split);
    assert_eq!(ctx.modal_form(), ModalForm::CenteredDialog);

    ctx.set_dimensions(1280.0, 800.0);
    assert_eq!(ctx.breakpoint, Breakpoint::Expanded);
    assert_eq!(ctx.orientation, Orientation::Landscape);
    assert_eq!(ctx.sidebar_mode(), SidebarMode::Standard);
    assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Split);
    assert_eq!(ctx.modal_form(), ModalForm::CenteredDialog);

    ctx.set_dimensions(1920.0, 1080.0);
    assert_eq!(ctx.breakpoint, Breakpoint::Ultra);
    assert_eq!(ctx.orientation, Orientation::Landscape);
    assert_eq!(ctx.sidebar_mode(), SidebarMode::Wide);
    assert_eq!(ctx.master_detail_mode(), MasterDetailMode::Split);
    assert_eq!(ctx.modal_form(), ModalForm::CenteredDialog);
}

#[test]
fn density_switching_scales_metrics() {
    let comfortable = Density::Comfortable;
    let compact = Density::Compact;

    assert_eq!(comfortable.scale_factor(), 1.0);
    assert_eq!(compact.scale_factor(), 0.85);

    assert_eq!(comfortable.padding(16.0), 16.0);
    assert_eq!(compact.padding(16.0), 12.0);

    assert_eq!(comfortable.gap(12.0), 12.0);
    assert_eq!(compact.gap(12.0), 9.0);

    assert_eq!(comfortable.control_height(), 36.0);
    assert_eq!(compact.control_height(), 28.0);
}

#[test]
fn density_switch_event_updates_context_in_place() {
    let mut app = headless_app();
    app.update();

    let world = app.world();
    let ctx = world.resource::<ResponsiveContext>();
    assert_eq!(ctx.density, Density::Comfortable);

    app.world_mut()
        .commands()
        .trigger(DensitySwitch(Density::Compact));
    app.update();

    let world = app.world();
    let ctx = world.resource::<ResponsiveContext>();
    assert_eq!(ctx.density, Density::Compact);
}

#[test]
fn smart_truncate_algorithms() {
    // Tail truncation
    assert_eq!(truncate_tail("HelloWorld", 10), "HelloWorld");
    assert_eq!(truncate_tail("HelloWorld123", 6), "Hello…");
    assert_eq!(truncate_tail("你好世界测试文本", 5), "你好世界…");

    // Middle truncation
    assert_eq!(truncate_middle("192.168.1.100:8080", 7, 5), "192.168…:8080");
    assert_eq!(
        truncate_middle("https://example.com/api/v1/users", 12, 6),
        "https://exam…/users"
    );

    // Adaptive truncation across 4 tiers
    let long_name = "HongKong-IEPL-HighSpeed-Gaming-01-UltraFast-VIP";
    assert_eq!(
        truncate_adaptive(long_name, Breakpoint::Compact, 12, 20, 30, 50),
        "HongKong-IE…"
    );
    assert_eq!(
        truncate_adaptive(long_name, Breakpoint::Medium, 12, 20, 30, 50),
        "HongKong-IEPL-HighS…"
    );
    assert_eq!(
        truncate_adaptive(long_name, Breakpoint::Expanded, 12, 20, 30, 50),
        "HongKong-IEPL-HighSpeed-Gamin…"
    );
    assert_eq!(
        truncate_adaptive(long_name, Breakpoint::Ultra, 12, 20, 30, 50),
        "HongKong-IEPL-HighSpeed-Gaming-01-UltraFast-VIP"
    );

    // Adaptive middle truncation across 4 tiers
    let url = "https://sub.provider-domain.example.com/clash/config/secret-token-xyz";
    assert_eq!(
        truncate_adaptive_middle(
            url,
            Breakpoint::Compact,
            (10, 8),
            (18, 12),
            (28, 18),
            (40, 30)
        ),
        "https://su…oken-xyz"
    );
}

#[test]
fn smart_truncate_ecs_restamps_text_in_place() {
    let mut app = headless_app();
    app.world_mut()
        .insert_resource(ResponsiveContext::new(375.0, 667.0));

    app.add_systems(Startup, |mut commands: Commands| {
        commands.spawn((
            Text("Placeholder".to_owned()),
            SmartTruncateText::adaptive(
                "SuperLongProxyNodeNameThatNeedsTruncationOnMobile",
                12,
                24,
                36,
                60,
            ),
        ));
    });

    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<&Text>();
    let text = texts.iter(world).next().expect("text mounted");
    assert_eq!(text.0, "SuperLongPr…");

    // Expand to Ultra width: text expands in place without entity recreation
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1920.0, 1080.0);
    app.update();

    let world = app.world_mut();
    let mut texts = world.query::<&Text>();
    let text = texts.iter(world).next().expect("text mounted");
    assert_eq!(text.0, "SuperLongProxyNodeNameThatNeedsTruncationOnMobile");
}

#[test]
fn fluid_card_grid_responsive_basis_and_gaps() {
    let mut app = headless_app();
    app.world_mut()
        .insert_resource(ResponsiveContext::new(1280.0, 800.0));

    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let cards = vec![
                Box::new(bsn! { ( Text({ "Card 1".to_owned() }) ) }) as Box<dyn Scene>,
                Box::new(bsn! { ( Text({ "Card 2".to_owned() }) ) }) as Box<dyn Scene>,
                Box::new(bsn! { ( Text({ "Card 3".to_owned() }) ) }) as Box<dyn Scene>,
            ];
            commands.spawn_scene(fluid_card_grid_scene(
                cards,
                FluidGridConfig::default(),
                &palette,
            ));
        },
    );

    app.update();

    // 1. Expanded (1280px): basis 31%
    {
        let world = app.world_mut();
        let mut items = world.query_filtered::<&Node, bevy::ecs::query::With<FluidGridItem>>();
        for node in items.iter(world) {
            assert_eq!(node.flex_basis, percent(31));
        }
    }

    // 2. Switch to Compact (375px): basis 100%
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(375.0, 667.0);
    app.update();

    {
        let world = app.world_mut();
        let mut items = world.query_filtered::<&Node, bevy::ecs::query::With<FluidGridItem>>();
        for node in items.iter(world) {
            assert_eq!(node.flex_basis, percent(100));
        }
    }

    // 3. Switch to Medium (768px): basis 48%
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(768.0, 1024.0);
    app.update();

    {
        let world = app.world_mut();
        let mut items = world.query_filtered::<&Node, bevy::ecs::query::With<FluidGridItem>>();
        for node in items.iter(world) {
            assert_eq!(node.flex_basis, percent(48));
        }
    }

    // 4. Switch to Ultra (1920px): basis 23%
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(1920.0, 1080.0);
    app.update();

    {
        let world = app.world_mut();
        let mut items = world.query_filtered::<&Node, bevy::ecs::query::With<FluidGridItem>>();
        for node in items.iter(world) {
            assert_eq!(node.flex_basis, percent(23));
        }
    }
}

#[test]
fn master_detail_split_and_stacked_navigation() {
    let mut app = headless_app();
    app.world_mut()
        .insert_resource(ResponsiveContext::new(1280.0, 800.0));

    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let master = Box::new(bsn! {
                Node {
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    ( Text({ "Master List".to_owned() }) ),
                    (
                        Node {}
                        MasterItemButton({ "item-42".to_owned() })
                    ),
                ]
            });
            let detail = Box::new(bsn! {
                Node {
                    flex_direction: FlexDirection::Column,
                }
                Children [
                    ( Text({ "Detail View".to_owned() }) ),
                    ( { master_back_button_scene("返回列表", &palette) } ),
                ]
            });
            commands.spawn_scene(master_detail_scene(master, detail, &palette));
        },
    );

    app.update();

    // 1. On Expanded (1280px): both Master and Detail panes are visible simultaneously
    {
        let world = app.world_mut();
        let mut master = world.query_filtered::<&Node, bevy::ecs::query::With<MasterPane>>();
        let mut detail = world.query_filtered::<&Node, bevy::ecs::query::With<DetailPane>>();

        assert_eq!(master.iter(world).next().unwrap().display, Display::Flex);
        assert_eq!(detail.iter(world).next().unwrap().display, Display::Flex);
    }

    // 2. Switch to Compact (375px): Stacked mode, initial view is Master
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(375.0, 667.0);
    app.update();

    {
        let world = app.world_mut();
        let mut master = world.query_filtered::<&Node, bevy::ecs::query::With<MasterPane>>();
        let mut detail = world.query_filtered::<&Node, bevy::ecs::query::With<DetailPane>>();

        assert_eq!(master.iter(world).next().unwrap().display, Display::Flex);
        assert_eq!(detail.iter(world).next().unwrap().display, Display::None);
    }

    // 3. User selects item-42: Detail becomes visible, Master hidden
    app.world_mut()
        .commands()
        .trigger(MasterItemSelected("item-42".to_owned()));
    app.update();

    {
        let world = app.world();
        let state = world.resource::<MasterDetailState>();
        assert_eq!(state.selected_item_id, Some("item-42".to_owned()));
        assert_eq!(state.active_view, MasterDetailView::Detail);

        let world_mut = app.world_mut();
        let mut master = world_mut.query_filtered::<&Node, bevy::ecs::query::With<MasterPane>>();
        let mut detail = world_mut.query_filtered::<&Node, bevy::ecs::query::With<DetailPane>>();

        assert_eq!(
            master.iter(world_mut).next().unwrap().display,
            Display::None
        );
        assert_eq!(
            detail.iter(world_mut).next().unwrap().display,
            Display::Flex
        );
    }

    // 4. User triggers back button
    app.world_mut().resource_mut::<MasterDetailState>().back();
    app.update();

    {
        let world = app.world();
        let state = world.resource::<MasterDetailState>();
        assert_eq!(state.active_view, MasterDetailView::Master);

        let world_mut = app.world_mut();
        let mut master = world_mut.query_filtered::<&Node, bevy::ecs::query::With<MasterPane>>();
        let mut detail = world_mut.query_filtered::<&Node, bevy::ecs::query::With<DetailPane>>();

        assert_eq!(
            master.iter(world_mut).next().unwrap().display,
            Display::Flex
        );
        assert_eq!(
            detail.iter(world_mut).next().unwrap().display,
            Display::None
        );
    }
}

#[test]
fn adaptive_modal_morphology_actionsheet_and_dialog() {
    let mut app = headless_app();
    app.world_mut()
        .insert_resource(ResponsiveContext::new(1280.0, 800.0));

    app.add_systems(
        Startup,
        |mut commands: Commands, palette: Res<UiPalette>| {
            let body = Box::new(bsn! { ( Text({ "Modal Body Content".to_owned() }) ) });
            let actions =
                vec![Box::new(bsn! { ( Text({ "Confirm".to_owned() }) ) }) as Box<dyn Scene>];
            commands.spawn_scene(adaptive_modal_scene(
                "Test Modal".to_owned(),
                body,
                actions,
                &palette,
            ));
        },
    );

    app.update();

    // 1. Initially closed: Display::None
    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        assert_eq!(roots.iter(world).next().unwrap().display, Display::None);
    }

    // 2. Open on Expanded (1280px): CenteredDialog (JustifyContent::Center, width 480px, radius::CARD all corners)
    app.world_mut().commands().trigger(OpenModal);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        let mut cards = world.query_filtered::<&Node, bevy::ecs::query::With<ModalCard>>();

        let root = roots.iter(world).next().unwrap();
        assert_eq!(root.display, Display::Flex);
        assert_eq!(root.justify_content, JustifyContent::Center);

        let card = cards.iter(world).next().unwrap();
        assert_eq!(card.width, px(480.0));
        assert_eq!(card.border_radius.top_left, Val::Px(radius::CARD));
        assert_eq!(card.border_radius.bottom_left, Val::Px(radius::CARD));
    }

    // 3. Resize to Compact (375px): ActionSheet morphology (JustifyContent::FlexEnd, width 100%, rounded top only)
    app.world_mut()
        .resource_mut::<ResponsiveContext>()
        .set_dimensions(375.0, 667.0);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        let mut cards = world.query_filtered::<&Node, bevy::ecs::query::With<ModalCard>>();

        let root = roots.iter(world).next().unwrap();
        assert_eq!(root.display, Display::Flex);
        assert_eq!(root.justify_content, JustifyContent::FlexEnd);

        let card = cards.iter(world).next().unwrap();
        assert_eq!(card.width, percent(100));
        assert_eq!(card.border_radius.top_left, Val::Px(radius::SHEET_TOP));
        assert_eq!(card.border_radius.bottom_left, Val::Px(0.0));
    }

    // 4. Close modal: Display::None again
    app.world_mut().commands().trigger(CloseModal);
    app.update();

    {
        let world = app.world_mut();
        let mut roots = world.query_filtered::<&Node, bevy::ecs::query::With<AdaptiveModalRoot>>();
        assert_eq!(roots.iter(world).next().unwrap().display, Display::None);
    }
}

#[test]
fn test_safe_area_insets_and_touch_target_policy() {
    let android_insets = infiltrator_bevy_widgets::responsive::SafeAreaInsets::android_default();
    assert_eq!(android_insets.top_px, 24.0);
    assert_eq!(android_insets.bottom_px, 16.0);

    let padded = android_insets.pad_base(16.0);
    assert_eq!(padded.top, Val::Px(40.0));
    assert_eq!(padded.bottom, Val::Px(32.0));
    assert_eq!(padded.left, Val::Px(16.0));
    assert_eq!(padded.right, Val::Px(16.0));

    // Touch target policy: compact mobile always guarantees 48px
    assert_eq!(
        infiltrator_bevy_widgets::responsive::TouchTargetPolicy::min_dimension(
            Density::Comfortable,
            true
        ),
        48.0
    );
    assert_eq!(
        infiltrator_bevy_widgets::responsive::TouchTargetPolicy::min_dimension(
            Density::Compact,
            true
        ),
        48.0
    );
    assert_eq!(
        infiltrator_bevy_widgets::responsive::TouchTargetPolicy::min_dimension(
            Density::Comfortable,
            false
        ),
        36.0
    );
    assert_eq!(
        infiltrator_bevy_widgets::responsive::TouchTargetPolicy::min_dimension(
            Density::Compact,
            false
        ),
        28.0
    );
}

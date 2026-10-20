//! Minimal Lurker Bevy 0.20 example (PUB-006.9).
//!
//! The example shows the shape of every Lurker surface:
//!
//! * the neutral contract owns the lifecycle vocabulary;
//! * the presentation kernel owns the projection to a renderable verdict;
//! * the surface owns nothing but the rendering and the typed user intent.
//!
//! Run it with:
//!
//! ```text
//! cargo run --example minimal-bevy
//! ```

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;
use lurker_core_contract::lifecycle::CoreLifecycle;
use lurker_core_contract::page::SurfaceOrigin;
use lurker_ui_presentation::core_status_projection::{
    OverviewBannerVerdict, project_banner_verdict,
};

/// The whole application state of the example: one typed lifecycle value.
#[derive(Resource, Clone)]
struct DemoState {
    lifecycle: CoreLifecycle,
}

/// The text node that renders the observed lifecycle.
#[derive(Component)]
struct StatusText;

/// The button that starts the core.
#[derive(Component)]
struct StartButton;

/// The button that stops the core.
#[derive(Component)]
struct StopButton;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(DemoState {
            lifecycle: CoreLifecycle::Stopped,
        })
        .add_systems(Startup, setup_ui)
        .add_systems(Update, handle_buttons);
    app.run();
}

/// Spawns the minimal surface: a title, the observed status and two actions.
fn setup_ui(mut commands: Commands) {
    commands.spawn(Camera2d);

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(16.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.12)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Lurker minimal Bevy example"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("Core lifecycle: Stopped"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                StatusText,
            ));

            parent
                .spawn((Node {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(12.0),
                    ..default()
                },))
                .with_children(|row| {
                    row.spawn((
                        Button,
                        Node {
                            padding: UiRect::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.2, 0.6, 0.3)),
                        StartButton,
                    ))
                    .with_children(|button| {
                        button.spawn((Text::new("Start core"), TextColor(Color::WHITE)));
                    });

                    row.spawn((
                        Button,
                        Node {
                            padding: UiRect::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.7, 0.2, 0.2)),
                        StopButton,
                    ))
                    .with_children(|button| {
                        button.spawn((Text::new("Stop core"), TextColor(Color::WHITE)));
                    });
                });
        });
}

/// Applies the typed user intent and republishes the projected status.
fn handle_buttons(
    mut state: ResMut<DemoState>,
    start_query: Query<Entity, (With<StartButton>, Added<Pressed>)>,
    stop_query: Query<Entity, (With<StopButton>, Added<Pressed>)>,
    mut status_text: Query<&mut Text, With<StatusText>>,
) {
    if start_query.iter().next().is_some() {
        state.lifecycle = CoreLifecycle::Running;
    } else if stop_query.iter().next().is_some() {
        state.lifecycle = CoreLifecycle::Stopped;
    }

    let verdict = project_banner_verdict(SurfaceOrigin::Live, &state.lifecycle, true, false);
    for mut text in status_text.iter_mut() {
        **text = format!(
            "Core lifecycle: {:?} · banner: {}",
            state.lifecycle,
            verdict_text(&verdict)
        );
    }
}

/// Renders the shared verdict vocabulary of the presentation kernel.
fn verdict_text(verdict: &OverviewBannerVerdict) -> &'static str {
    match verdict {
        OverviewBannerVerdict::Demo => "demo fixture",
        OverviewBannerVerdict::MissingHost => "missing host",
        OverviewBannerVerdict::CoreStopped => "core stopped",
        OverviewBannerVerdict::VpnUnauthorized => "vpn unauthorized",
        OverviewBannerVerdict::RealRunning => "real running",
        OverviewBannerVerdict::Transitioning => "transitioning",
    }
}

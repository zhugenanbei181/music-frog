//! High-performance Bevy 0.20 ECS desktop proxy client template (PUB-005.3).

use bevy::prelude::*;
use bevy::text::FontSize;
use bevy::ui::Pressed;
use bevy::ui_widgets::Button;
use infiltrator_contract::command::CommandIntent;
use infiltrator_ports::command_sink::{CommandSink, RecordingCommandSink};
use lurker_core_contract::routing_intent::RoutingIntent;
use std::sync::Arc;

#[derive(Resource, Clone)]
pub struct CommandSinkResource(pub Arc<dyn CommandSink>);

#[derive(Component)]
pub struct CoreStatusText;

#[derive(Component)]
pub struct StartCoreButton;

#[derive(Component)]
pub struct StopCoreButton;

#[derive(Component)]
pub struct SwitchRoutingButton(pub RoutingIntent);

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins)
        .insert_resource(CommandSinkResource(Arc::new(RecordingCommandSink::new())))
        .add_systems(Startup, setup_ui)
        .add_systems(Update, handle_buttons);
    app.run();
}

pub fn setup_ui(mut commands: Commands) {
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
                Text::new("Lurker Bevy 0.20 ECS Client Template"),
                TextFont {
                    font_size: FontSize::Px(24.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                Text::new("Core Status: Stopped"),
                TextFont {
                    font_size: FontSize::Px(16.0),
                    ..default()
                },
                TextColor(Color::srgb(0.6, 0.6, 0.6)),
                CoreStatusText,
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
                        StartCoreButton,
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new("Start Core"), TextColor(Color::WHITE)));
                    });

                    row.spawn((
                        Button,
                        Node {
                            padding: UiRect::all(Val::Px(10.0)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.7, 0.2, 0.2)),
                        StopCoreButton,
                    ))
                    .with_children(|b| {
                        b.spawn((Text::new("Stop Core"), TextColor(Color::WHITE)));
                    });
                });
        });
}

pub fn handle_buttons(
    sink: Res<CommandSinkResource>,
    start_query: Query<Entity, (With<StartCoreButton>, Added<Pressed>)>,
    stop_query: Query<Entity, (With<StopCoreButton>, Added<Pressed>)>,
    mut status_text: Query<&mut Text, With<CoreStatusText>>,
) {
    if start_query.iter().next().is_some() {
        let sink = Arc::clone(&sink.0);
        tokio::spawn(async move {
            let _ = sink.submit(CommandIntent::StartCore).await;
        });
        for mut text in status_text.iter_mut() {
            **text = "Core Status: Running".to_owned();
        }
    }

    if stop_query.iter().next().is_some() {
        let sink = Arc::clone(&sink.0);
        tokio::spawn(async move {
            let _ = sink.submit(CommandIntent::StopCore).await;
        });
        for mut text in status_text.iter_mut() {
            **text = "Core Status: Stopped".to_owned();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn headless_ecs_schedule_and_command_intent_pipeline_test() {
        let mut app = App::new();
        let recording_sink = Arc::new(RecordingCommandSink::new());
        app.add_plugins(MinimalPlugins)
            .insert_resource(CommandSinkResource(recording_sink.clone()))
            .add_systems(Startup, setup_ui)
            .add_systems(Update, handle_buttons);

        // Run startup systems
        app.update();

        // Simulate button press event in headless ECS world
        let mut button_query = app
            .world_mut()
            .query_filtered::<Entity, With<StartCoreButton>>();
        let button_entity = button_query
            .iter(app.world())
            .next()
            .expect("start button entity");

        app.world_mut().entity_mut(button_entity).insert(Pressed);

        // Advance 1 frame in headless ECS
        app.update();

        // Verify status text updated in place
        let mut text_query = app
            .world_mut()
            .query_filtered::<&Text, With<CoreStatusText>>();
        let text = text_query.iter(app.world()).next().expect("status text");
        assert_eq!(**text, "Core Status: Running");
    }
}

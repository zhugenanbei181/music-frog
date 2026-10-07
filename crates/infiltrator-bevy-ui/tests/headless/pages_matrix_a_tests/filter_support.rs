use super::*;

pub(super) fn filter_field_entity(app: &mut App, kind: FilterField) -> Entity {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::EditorFilterField;
    let mut query = app.world_mut().query::<(Entity, &EditorFilterField)>();
    query
        .iter(app.world())
        .find(|(_, field)| field.kind == Some(kind))
        .map(|(entity, _)| entity)
        .expect("filter field root")
}

pub(super) fn filter_field_text(app: &mut App, kind: FilterField) -> String {
    use infiltrator_bevy_ui::pages::profiles_editor_panes::EditorFilterField;
    use infiltrator_bevy_widgets::text_input::TextField;
    let child = {
        let mut query = app.world_mut().query::<(&EditorFilterField, &Children)>();
        query
            .iter(app.world())
            .find(|(field, _)| field.kind == Some(kind))
            .and_then(|(_, children)| children.iter().next().copied())
            .expect("filter field child")
    };
    app.world()
        .get::<TextField>(child)
        .map(|field| field.0.text().to_owned())
        .unwrap_or_default()
}

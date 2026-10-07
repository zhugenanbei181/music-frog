//! Register rendering assets before Startup scenes without unrestricted ECS access.

use crate::fonts::FontSources;
use crate::icon::{IconPlate, IconSources};
use crate::text::TextRole;
use bevy::asset::{AssetServer, Assets};
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::image::Image;
use bevy::text::{Font, FontSource, TextFont};
use bevy::ui::widget::ImageNode;

pub(crate) fn initialize_fonts(
    fonts: Option<ResMut<Assets<Font>>>,
    mut sources: ResMut<FontSources>,
    mut mounted: Query<(&TextRole, &mut TextFont)>,
) {
    let Some(mut fonts) = fonts else {
        return;
    };
    *sources = FontSources::embedded(&mut fonts);
    // A host can mount a scene before its first update; preserve those entities.
    for (role, mut font) in &mut mounted {
        font.font = FontSource::Handle(sources.face(role.0));
    }
}

pub(crate) fn initialize_icons(
    server: Option<Res<AssetServer>>,
    images: Option<Res<Assets<Image>>>,
    mut sources: ResMut<IconSources>,
    mut mounted: Query<(&IconPlate, &mut ImageNode)>,
) {
    let (Some(server), Some(_images)) = (server, images) else {
        return;
    };
    *sources = IconSources::load(&server);
    for (plate, mut image) in &mut mounted {
        image.image = sources.handle(plate.0).unwrap_or_default();
    }
}

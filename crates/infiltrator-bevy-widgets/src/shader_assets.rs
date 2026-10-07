//! App-scoped WESL libraries, retained explicitly instead of forgotten handles.

use bevy::app::{App, PreStartup};
use bevy::asset::{AssetServer, Handle, embedded_asset};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Res, ResMut};
use bevy::shader::Shader;

#[derive(Resource, Default)]
struct SurfaceShaderLibraries {
    handles: Vec<Handle<Shader>>,
}

pub(crate) fn install(app: &mut App) {
    embedded_asset!(app, "shaders/surface_uniform.wesl");
    embedded_asset!(app, "shaders/squircle.wesl");
    embedded_asset!(app, "shaders/shadow.wesl");
    app.init_resource::<SurfaceShaderLibraries>();
    app.add_systems(PreStartup, load);
}

fn load(server: Res<AssetServer>, mut libraries: ResMut<SurfaceShaderLibraries>) {
    if !libraries.handles.is_empty() {
        return;
    }
    // Embedded module imports use custom paths. Explicit handles keep the
    // dependencies available to the pinned Bevy linker for this App's lifetime.
    libraries.handles.extend([
        server.load("embedded://infiltrator_bevy_widgets/shaders/surface_uniform.wesl"),
        server.load("embedded://infiltrator_bevy_widgets/shaders/squircle.wesl"),
        server.load("embedded://infiltrator_bevy_widgets/shaders/shadow.wesl"),
    ]);
}

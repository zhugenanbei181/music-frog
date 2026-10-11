use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::{AssetPlugin, Assets};
use bevy::core_pipeline::tonemapping::lut_placeholder;
use bevy::image::{Image, ImagePlugin};
use bevy::scene::ScenePlugin;
use infiltrator_bevy_ui::app::ShellPlugin;

#[test]
fn shell_preserves_the_image_hosts_existing_lut_handles() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        ScenePlugin,
        ImagePlugin::default(),
    ));
    let lut = app
        .world_mut()
        .resource_mut::<Assets<Image>>()
        .add(lut_placeholder());
    app.add_plugins(ShellPlugin::default());
    app.update();
    let images = app.world().resource::<Assets<Image>>();
    let preserved = images.get(&lut).expect("host LUT must survive shell setup");
    assert_eq!(
        preserved.texture_descriptor.dimension,
        lut_placeholder().texture_descriptor.dimension
    );
    assert_eq!(preserved.data, lut_placeholder().data);
}

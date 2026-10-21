//! Inline content: the one seam between a text block and what flows inside it.
//!
//! Bevy 0.20 lays a text block out as a single paragraph. The root `Text`
//! node's own string, its `TextSpan` children and its inline boxes are one
//! flow: an inline box ([`InlineBox`]) reserves space in that flow —
//! [`InlineBoxKind::InFlow`] takes part in the layout and wraps with the
//! text, [`InlineBoxKind::OutOfFlow`] is placed at the text position
//! without consuming space — and an inline image ([`InlineImage`]) is an
//! inline box that draws a bitmap.
//!
//! Content that must flow *with* the sentence therefore lives inside the
//! text block. The banned shape is "icon node + text node + column gap": a
//! sibling node is laid out by the flexbox instead of the text, so it drifts
//! off the baseline and breaks the moment the sentence wraps.
//!
//! Sizes: upstream measures an inline box in logical pixels, while this
//! repository owns exactly one scaling entry — the root font size
//! ([`RemSize`]). [`InlineSize`] therefore carries the size in baseline
//! pixels (the scale the `space::`/`metrics::` tokens use) and
//! [`sync_inline_sizes`] re-resolves it against the root font size, so a
//! font-scale change re-measures the inline content instead of stranding it
//! at the size it was mounted with.

use bevy::asset::Handle;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::system::{Query, Res};
use bevy::image::Image;
use bevy::math::Vec2;
use bevy::scene::{Scene, bsn};
use bevy::text::{InlineBox, InlineBoxKind, RemSize};
use bevy::ui::widget::InlineImage;

use crate::theme::REM_BASE;

/// The size of an inline box, in the repository's rem channel.
///
/// The value is the box's size in *baseline* pixels — the same scale the
/// `space::`/`metrics::` tokens speak — so an inline box grows with the root
/// font size instead of floating at a fixed pixel size next to text that
/// grew.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct InlineSize {
    /// The box's width and height in baseline pixels.
    pub baseline: Vec2,
}

impl InlineSize {
    /// A square box of `baseline` baseline pixels on each edge.
    pub fn square(baseline: f32) -> Self {
        Self {
            baseline: Vec2::splat(baseline),
        }
    }

    /// The size in logical pixels for a given root font size.
    pub fn logical(self, rem: RemSize) -> Vec2 {
        self.baseline * (rem.0 / REM_BASE)
    }
}

/// Re-resolve every inline box's size against the root font size.
///
/// The inline box is the one place where the text flow reserves space, so a
/// stale size is a visible hole in the sentence. The root font size is the
/// repository's only scaling entry, so the box follows it.
pub fn sync_inline_sizes(rem: Res<RemSize>, mut boxes: Query<(&InlineSize, &mut InlineBox)>) {
    for (size, mut inline) in &mut boxes {
        let resolved = size.logical(*rem);
        if inline.size != resolved {
            inline.size = resolved;
        }
    }
}

/// Re-resolve every inline image's box against the root font size.
///
/// Upstream measures an inline image's box from the image itself, so the
/// repository states the box it wants in the rem channel and lets the
/// upstream system copy it into the inline box the text layout measures.
pub fn sync_inline_image_sizes(
    rem: Res<RemSize>,
    mut images: Query<(&InlineSize, &mut InlineImage)>,
) {
    for (size, mut image) in &mut images {
        let resolved = size.logical(*rem);
        if image.width != Some(resolved.x) || image.height != Some(resolved.y) {
            image.width = Some(resolved.x);
            image.height = Some(resolved.y);
        }
    }
}

/// A scene for a box that reserves space inside a text flow.
///
/// The placement is explicit: an [`InlineBoxKind::InFlow`] box takes part in
/// the layout and wraps with the text, an [`InlineBoxKind::OutOfFlow`] box
/// is placed at the text position without consuming space. An inline box is
/// a leaf of the text tree — its own children never join the text layout.
pub fn inline_box_scene(placement: InlineBoxKind, size: InlineSize) -> Box<dyn Scene> {
    Box::new(bsn! {
        InlineBox { kind: {placement}, size: {size.logical(RemSize(REM_BASE))} }
        InlineSize { baseline: {size.baseline} }
    })
}

/// The inline image of a graphic that flows inside a sentence.
///
/// An inline image *is* an inline box ([`InlineImage`] requires
/// [`InlineBox`]), so the graphic is measured and placed by the text layout
/// itself: it wraps with the sentence and rides its baseline instead of
/// being a sibling node glued next to it with a column gap.
///
/// The box measures the image, so the graphic's own pixel size is the
/// size — the rem channel applies to the *layout* around it, not to the
/// bitmap's intrinsic edge. A caller that wants the box on the rem channel
/// states it with [`InlineSize`], and [`sync_inline_image_sizes`] keeps it
/// there.
pub fn inline_image(image: Handle<Image>, tint: Color) -> InlineImage {
    InlineImage {
        color: tint,
        image,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WidgetsPlugin;
    use crate::theme::{Theme, space};
    use bevy::MinimalPlugins;
    use bevy::app::{App, Startup};
    use bevy::asset::AssetPlugin;
    use bevy::ecs::entity::Entity;
    use bevy::ecs::hierarchy::Children;
    use bevy::ecs::system::Commands;
    use bevy::scene::{CommandsSceneExt, ScenePlugin};
    use bevy::ui::widget::Text;

    /// The headless composition the widget tests run on: the widget seam
    /// plus the asset and scene stores the `bsn!` mount needs.
    fn headless_app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins((AssetPlugin::default(), ScenePlugin));
        app.add_plugins(WidgetsPlugin::new(&Theme::dark()));
        app
    }

    #[test]
    fn an_inline_box_declares_its_placement_and_keeps_the_rem_channel() {
        let mut app = headless_app();
        let size = InlineSize::square(space::S8);
        app.add_systems(Startup, move |mut commands: Commands| {
            commands.spawn_scene(bsn! {
                Text({ String::new() })
                Children [
                    @{ inline_box_scene(InlineBoxKind::InFlow, size) }
                ]
            });
        });
        app.update();

        let world = app.world_mut();
        let mut boxes = world.query::<(Entity, &InlineBox)>();
        let inline: Vec<(Entity, InlineBoxKind, Vec2)> = boxes
            .iter(world)
            .map(|(entity, inline)| (entity, inline.kind, inline.size))
            .collect();
        assert_eq!(inline.len(), 1, "the text block seats one inline box");
        let (entity, kind, measured) = inline[0];
        assert_eq!(
            kind,
            InlineBoxKind::InFlow,
            "the placement is an explicit typed state"
        );
        assert_eq!(
            measured,
            Vec2::splat(space::S8),
            "the box is measured in the root font size's pixels"
        );

        // The root font size is the only scaling entry: the box follows it.
        app.world_mut().insert_resource(RemSize(30.0));
        app.update();
        let resized = app
            .world()
            .get::<InlineBox>(entity)
            .expect("the inline box is still mounted")
            .size;
        assert_eq!(
            resized,
            Vec2::splat(space::S8 * 2.0),
            "a doubled root font size doubles the reserved space"
        );
    }
}

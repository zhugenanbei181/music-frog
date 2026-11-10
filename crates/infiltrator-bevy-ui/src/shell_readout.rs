//! Native ECS replay of the application-owned shell readout. No domain fold
//! lives here.
//!
//! The Bevy shell publishes one readout of its own: the chrome strip's duplex
//! rate capsule. Every other fact the shared readout offers already has an
//! owning page on this surface - the dashboard hero states the source caption
//! and the run state, the traffic card states the rates, the proxies, rules,
//! connections and DNS pages state their own counts, and the profiles page
//! states the active profile's name, kind and quota. The rail used to mount a
//! second copy of each of those; with the column reduced to navigation those
//! mounts are gone, and their markers go with them rather than surviving as a
//! dead vocabulary that invites a remount.
use crate::a11y::semantic_node;
use crate::app::GlobalStatusDot;
use crate::surface::LatestSurfaceSnapshot;
use bevy::asset::Handle;
use bevy::color::Color;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::system::{Query, Res};
use bevy::feathers::theme::{SurfaceLevel, UiTheme};
use bevy::image::Image;
use bevy::scene::{Scene, bsn};
use bevy::text::{InlineBox, InlineBoxKind};
use bevy::ui::widget::InlineImage;
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::inline::InlineSize;
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_presentation::shell_readout::rate_copy;
use infiltrator_shared::locales::Lang;
use lurker_core_contract::lifecycle::CoreLifecycle;

/// The one readout line the shell itself publishes. The other facts the
/// shared readout offers are owned by the page that shows them, so they are
/// not this vocabulary's business.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ShellReadoutText {
    /// The chrome capsule's up/down duplex line.
    #[default]
    TrafficCapsule,
}

/// Marker on the seat of the status point inside the title text block.
///
/// The point itself is an inline image, and an inline image carries a
/// runtime asset handle — a handle cannot be declared in a scene — so the
/// seat is declared here and the observer that owns the handle stamps the
/// image onto it.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusDotSlot;

/// The seat of the status point inside the title text block.
///
/// The point is *inside* the title text (upstream's inline content: an
/// inline image is an inline box), not a sibling node glued next to the
/// heading with a column gap — a sibling is laid out by the flexbox, so it
/// drifts off the baseline and breaks the moment the title wraps.
pub fn status_point_slot_scene() -> Box<dyn Scene> {
    let size = InlineSize::square(STATUS_DOT_BOX_PX);
    Box::new(bsn! {
        InlineBox { kind: {InlineBoxKind::InFlow} }
        StatusDotSlot
        size
        GlobalStatusDot
        semantic_node(ShellA11yNode::GlobalStatusDot)
    })
}

/// The inline image of the status point: the point's own plate, tinted.
pub fn status_point_image(image: Handle<Image>, tint: Color) -> InlineImage {
    InlineImage {
        color: tint,
        image,
        ..Default::default()
    }
}

/// The status point's box edge in baseline pixels.
///
/// The point is a raster plate, but its *box* is a layout size: stating it
/// in the rem channel keeps the dot in proportion to the heading it rides
/// when the root font size changes.
pub const STATUS_DOT_BOX_PX: f32 = 9.0;

pub fn sync_text(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    locale: Res<UiLocale>,
    mut texts: Query<(&ShellReadoutText, &mut Text)>,
) {
    let default = Default::default();
    let readout = snapshot
        .as_ref()
        .map_or(&default, |snapshot| &snapshot.0.shell_readout);
    let lang = locale.code();
    for (part, mut text) in &mut texts {
        let ShellReadoutText::TrafficCapsule = part;
        let up = rate_copy(&readout.upload_bps, &Lang(lang));
        let down = rate_copy(&readout.download_bps, &Lang(lang));
        let value = format!("↑ {up}  ↓ {down}");
        if text.0 != value {
            text.0 = value;
        }
    }
}

/// Repaint the status point from the core lifecycle.
///
/// The point is an inline image inside the title text block, so the tint is
/// the only thing that changes: the box, the position and the baseline are
/// the text layout's business.
pub fn sync_core_status(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    theme: Option<Res<UiTheme>>,
    mut dots: Query<&mut InlineImage, With<GlobalStatusDot>>,
) {
    let token = match snapshot.as_ref().map(|snapshot| &snapshot.0.core.lifecycle) {
        Some(CoreLifecycle::Running | CoreLifecycle::Ready) => Token::Success,
        Some(CoreLifecycle::Starting | CoreLifecycle::Stopping) => Token::Warning,
        Some(CoreLifecycle::Failed) => Token::Danger,
        Some(CoreLifecycle::Stopped) | None => Token::InkDim,
    };
    let Some(theme) = theme else {
        return;
    };
    let color = theme.context_color(&token.theme_token(), SurfaceLevel::Base);
    for mut dot in &mut dots {
        if dot.color != color {
            dot.color = color;
        }
    }
}

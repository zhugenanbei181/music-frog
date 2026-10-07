//! Native ECS replay of the application-owned shell readout. No domain fold lives here.
use crate::app::GlobalStatusDot;
use crate::surface::LatestSurfaceSnapshot;
use bevy::ecs::component::Component;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::ui::prelude::{BackgroundColor, Display, Node, Val};
use bevy::ui::widget::Text;
use infiltrator_application::shell_readout_projection::{
    count_copy, profile_kind, profile_name, profile_percent, profile_usage, rate_copy, rate_status,
};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_contract::snapshot::CoreLifecycle;
use infiltrator_contract::surface_snapshot::PageId;

#[derive(Component, Clone, Copy, Default)]
pub enum ShellReadoutText {
    Count(PageId),
    #[default]
    ProfileName,
    ProfileKind,
    ProfileUsage,
    ProfilePercent,
    Upload,
    Download,
    RateStatus,
}
#[derive(Component, Clone, Default)]
pub struct ShellQuotaFill;
#[derive(Component, Clone, Default)]
pub struct ShellQuotaTrack;

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
        let value = match part {
            ShellReadoutText::Count(page) => count_copy(readout, *page, lang),
            ShellReadoutText::ProfileName => profile_name(readout, lang),
            ShellReadoutText::ProfileUsage => profile_usage(readout, lang),
            ShellReadoutText::ProfilePercent => profile_percent(readout, lang),
            ShellReadoutText::ProfileKind => profile_kind(readout, lang),
            ShellReadoutText::Upload => rate_copy(&readout.upload_bps, lang),
            ShellReadoutText::Download => rate_copy(&readout.download_bps, lang),
            ShellReadoutText::RateStatus => rate_status(readout, lang),
        };
        if text.0 != value {
            text.0 = value;
        }
    }
}

pub fn sync_quota(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    mut fills: Query<&mut Node, (With<ShellQuotaFill>, Without<ShellQuotaTrack>)>,
    mut tracks: Query<&mut Node, (With<ShellQuotaTrack>, Without<ShellQuotaFill>)>,
) {
    let fraction = snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.0.shell_readout.profile.value.as_ref())
        .and_then(|profile| profile.usage_fraction);
    for mut node in &mut fills {
        node.width = Val::Percent(fraction.unwrap_or_default() * 100.0);
    }
    for mut node in &mut tracks {
        node.display = if fraction.is_some() {
            Display::Flex
        } else {
            Display::None
        };
    }
}

pub fn sync_core_status(
    snapshot: Option<Res<LatestSurfaceSnapshot>>,
    palette: Res<UiPalette>,
    mut dots: Query<&mut BackgroundColor, With<GlobalStatusDot>>,
) {
    let color = match snapshot.as_ref().map(|snapshot| &snapshot.0.core.lifecycle) {
        Some(CoreLifecycle::Running | CoreLifecycle::Ready) => palette.success,
        Some(CoreLifecycle::Starting | CoreLifecycle::Stopping) => palette.warning,
        Some(CoreLifecycle::Failed) => palette.danger,
        Some(CoreLifecycle::Stopped) | None => palette.ink_dim,
    };
    for mut dot in &mut dots {
        dot.0 = color;
    }
}

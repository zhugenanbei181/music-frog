//! Package names stay raw; availability and exemption copy follows the locale in place.
use crate::pages::app_routing::LastAppRoutingProjection;
use crate::pages::app_routing_uwp::{UwpEmptyLine, UwpPackageName, UwpPackageState, UwpStatusLine};
use bevy::ecs::query::{Has, Or, QueryData, QueryFilter, With};
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::routing_projection::{uwp_empty, uwp_state_key, uwp_summary};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_shared::locales::{Lang, Localizer};

#[derive(QueryData)]
#[query_data(mutable)]
pub struct UwpText {
    text: &'static mut Text,
    summary: Has<UwpStatusLine>,
    empty: Has<UwpEmptyLine>,
    name: Option<&'static UwpPackageName>,
    state: Option<&'static UwpPackageState>,
}
#[derive(QueryFilter)]
pub struct UwpStatusFilter {
    status: Or<(With<UwpStatusLine>, With<UwpEmptyLine>)>,
}
#[derive(QueryFilter)]
pub struct UwpPackageFilter {
    package: Or<(With<UwpPackageName>, With<UwpPackageState>)>,
}
#[derive(QueryFilter)]
pub struct UwpCopyFilter {
    uwp: Or<(UwpStatusFilter, UwpPackageFilter)>,
}
pub fn sync(
    last: Res<LastAppRoutingProjection>,
    locale: Res<UiLocale>,
    mut lines: Query<UwpText, UwpCopyFilter>,
) {
    let Some(projection) = &last.0 else {
        return;
    };
    let snapshot = &projection.uwp_loopback;
    for mut line in &mut lines {
        let value = if line.summary {
            uwp_summary(
                &snapshot.availability,
                snapshot
                    .packages
                    .iter()
                    .map(|package| package.loopback_exempt),
                locale.code(),
            )
        } else if line.empty {
            uwp_empty(&snapshot.availability, locale.code())
        } else if let Some(marker) = line.name {
            snapshot
                .packages
                .get(marker.0)
                .map(|package| package.display_name.clone())
                .unwrap_or_else(|| Lang(locale.code()).tr("shell_readout_unknown").into_owned())
        } else if let Some(marker) = line.state {
            Lang(locale.code())
                .tr(snapshot
                    .packages
                    .get(marker.0)
                    .map(|package| uwp_state_key(package.loopback_exempt))
                    .unwrap_or("shell_readout_unknown"))
                .into_owned()
        } else {
            continue;
        };
        if line.text.0 != value {
            line.text.0 = value;
        }
    }
}

//! The Overview page's public-IP probe card: address, location, ISP, provider
//! and the refresh affordance, projected from the shared probe snapshot.

use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::system::{Query, Res};
use bevy::scene::{Scene, bsn, template_value};
use bevy::text::TextColor;
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::{IconId, icon_scene};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::public_ip::PublicIpProbeSnapshot;

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::overview::{AccentContainerFill, SurfaceElevatedFill};

/// Public IP probe card (BEVY-GAP-023 / DUAL-03-11).
pub fn public_ip_probe_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    public_ip_probe_card_scene_with_snapshot(&PublicIpProbeSnapshot::demo_fixture(), palette)
}

/// Public IP probe card with real IP, location, ISP and refresh affordance.
pub fn public_ip_probe_card_scene_with_snapshot(
    snapshot: &PublicIpProbeSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut a11y = accesskit::Node::new(accesskit::Role::Region);
    a11y.set_label("公网 IP 隐私归属探针");
    let ip = public_ip_text_value(snapshot, PublicIpTextKind::Ip);
    let location = public_ip_text_value(snapshot, PublicIpTextKind::Location);
    let isp = public_ip_text_value(snapshot, PublicIpTextKind::Isp);
    let provider = public_ip_text_value(snapshot, PublicIpTextKind::Provider);
    let status = public_ip_text_value(snapshot, PublicIpTextKind::Status);

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::S8),
            }
            template_value(AccessibilityNode(a11y))
            PublicIpProbeCard
            Children [
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                    }
                    Children [
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                ( { icon_scene(IconId::Globe, 16.0, palette.accent) } ),
                                ( Text({ "当前公网 IP (Public IP Probe)".to_owned() }) TextRole(Role::Heading) ),
                            ]
                        ),
                        (
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                (
                                    Node {
                                        padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                    }
                                    BackgroundColor({ palette.surface_elevated })
                                    SurfaceElevatedFill
                                    Children [
                                        ( Text({ provider }) PublicIpText(PublicIpTextKind::Provider) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                                    ]
                                ),
                                (
                                    Button
                                    PublicIpRefreshButton
                                    Node {
                                        padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S4)),
                                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(space::S4),
                                    }
                                    BackgroundColor({ palette.surface_elevated })
                                    SurfaceElevatedFill
                                    Children [
                                        ( { icon_scene(IconId::Activity, 12.0, palette.ink) } ),
                                        ( Text({ "刷新".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink }) ),
                                    ]
                                ),
                            ]
                        ),
                    ]
                ),
                (
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        padding: UiRect::all(Val::Px(space::S8)),
                        border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                    }
                    BackgroundColor({ palette.surface_elevated })
                    SurfaceElevatedFill
                    Children [
                        (
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S2),
                            }
                            Children [
                                ( Text({ ip }) PublicIpText(PublicIpTextKind::Ip) TextRole(Role::BodyStrong) TextColor({ palette.ink }) ),
                                (
                                    Node {
                                        align_items: AlignItems::Center,
                                        column_gap: Val::Px(space::S8),
                                    }
                                    Children [
                                        ( Text({ location }) PublicIpText(PublicIpTextKind::Location) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                                        ( Text({ "·".to_owned() }) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                                        ( Text({ isp }) PublicIpText(PublicIpTextKind::Isp) TextRole(Role::Caption) TextColor({ palette.ink_dim }) ),
                                    ]
                                ),
                            ]
                        ),
                        (
                            Node {
                                padding: UiRect::axes(Val::Px(space::S8), Val::Px(space::S2)),
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent_container })
                            AccentContainerFill
                            Children [
                                ( Text({ status }) PublicIpText(PublicIpTextKind::Status) TextRole(Role::Caption) TextColor({ palette.accent }) ),
                            ]
                        ),
                    ]
                ),
            ]
        })],
        palette,
    )
}

/// Marker on the public IP and privacy probe card.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicIpProbeCard;

/// Marker on the public IP probe refresh button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicIpRefreshButton;

/// Marker on the public IP card's mutable facts.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublicIpText(pub PublicIpTextKind);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PublicIpTextKind {
    #[default]
    Ip,
    Location,
    Isp,
    Provider,
    Status,
}

pub(crate) fn public_ip_text_value(
    snapshot: &infiltrator_contract::public_ip::PublicIpProbeSnapshot,
    kind: PublicIpTextKind,
) -> String {
    match kind {
        PublicIpTextKind::Ip => snapshot.ip.clone().unwrap_or_else(|| "—".to_owned()),
        PublicIpTextKind::Location => {
            let country = snapshot.country_code.as_deref().unwrap_or("—");
            if let Some(city) = snapshot.city.as_deref() {
                format!("{country} · {city}")
            } else {
                country.to_owned()
            }
        }
        PublicIpTextKind::Isp => snapshot.isp.clone().unwrap_or_else(|| "—".to_owned()),
        PublicIpTextKind::Provider => snapshot
            .provider
            .as_deref()
            .unwrap_or("ipapi.is")
            .to_owned(),
        PublicIpTextKind::Status => match snapshot.status {
            infiltrator_contract::public_ip::PublicIpProbeStatus::Ready => {
                "probe · ready".to_owned()
            }
            infiltrator_contract::public_ip::PublicIpProbeStatus::Probing => {
                "probe · probing...".to_owned()
            }
            infiltrator_contract::public_ip::PublicIpProbeStatus::Empty => {
                "probe · empty".to_owned()
            }
            infiltrator_contract::public_ip::PublicIpProbeStatus::Unknown => {
                "probe · not requested".to_owned()
            }
            infiltrator_contract::public_ip::PublicIpProbeStatus::Unsupported => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "probe · unsupported".to_owned()),
            infiltrator_contract::public_ip::PublicIpProbeStatus::Failed => snapshot
                .failure
                .clone()
                .unwrap_or_else(|| "probe · failed".to_owned()),
        },
    }
}

pub(crate) fn on_overview_public_ip_refresh_activated(
    activate: On<Activate>,
    buttons: Query<&PublicIpRefreshButton>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if buttons.get(activate.entity).is_err() {
        return;
    }
    handle.submit(UiCommand::RefreshPublicIpProbe);
}

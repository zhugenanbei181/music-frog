//! Reversible preference controls replay the observed state in place.
use crate::pages::proxies::{
    LastProxiesProjection, NodePinButton, ProxiesScrollArea, ProxyNodeButton, ToggleViewModeButton,
};
use crate::pages::proxies_identity::node_for_entity;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::hierarchy::ChildOf;
use bevy::ecs::query::{Has, Or, QueryData, With, Without};
use bevy::ecs::system::{Query, Res};
use bevy::text::TextColor;
use bevy::ui::widget::Text;
use bevy::ui::{BackgroundColor, Node, Val};
use infiltrator_bevy_widgets::localization::{LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::responsive::ResponsiveContext;
use infiltrator_bevy_widgets::theme::space;

/// Short windows reduce empty inter-card space while retaining every input and action.
pub fn sync_proxy_vertical_spacing(
    context: Option<Res<ResponsiveContext>>,
    mut scrolls: Query<&mut Node, With<ProxiesScrollArea>>,
) {
    let Some(context) = context else { return };
    let gap = Val::Px(if context.height_px < 600.0 {
        space::S8
    } else {
        space::S16
    });
    for mut node in &mut scrolls {
        if node.row_gap != gap {
            node.row_gap = gap;
        }
    }
}

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct FilterAliveIndicator;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyViewLabel;

#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ProxyFavoriteStar;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct FavoriteStarParts {
    entity: Entity,
    text: &'static mut Text,
    color: &'static mut TextColor,
}
type FavoriteStarFilter = (With<ProxyFavoriteStar>, Without<NodePinButton>);

pub fn sync_proxy_favorites(
    last: Option<Res<LastProxiesProjection>>,
    palette: Res<UiPalette>,
    parents: Query<&ChildOf>,
    identities: Query<&ProxyNodeButton>,
    mut stars: Query<FavoriteStarParts, FavoriteStarFilter>,
    mut pins: Query<(&NodePinButton, &mut TextColor), Without<ProxyFavoriteStar>>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    for mut star in &mut stars {
        if let Some(node) = node_for_entity(star.entity, &parents, &identities, projection) {
            let caption = if node.favorite { "★" } else { "☆" };
            let ink = if node.favorite {
                palette.warning
            } else {
                palette.ink_dim
            };
            if star.text.0 != caption {
                star.text.0 = caption.into();
            }
            if star.color.0 != ink {
                star.color.0 = ink;
            }
        }
    }
    for (pin, mut color) in &mut pins {
        if let Some(node) = projection
            .groups
            .iter()
            .flat_map(|group| &group.proxies)
            .find(|node| node.name == pin.node_name)
        {
            let ink = if node.favorite {
                palette.warning
            } else {
                palette.ink_dim
            };
            if color.0 != ink {
                color.0 = ink;
            }
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
pub struct PreferenceFill {
    fill: &'static mut BackgroundColor,
    filter: Has<FilterAliveIndicator>,
}
type PreferenceControls = Or<(With<FilterAliveIndicator>, With<ToggleViewModeButton>)>;

pub fn sync_proxy_preferences(
    last: Option<Res<LastProxiesProjection>>,
    palette: Res<UiPalette>,
    locale: Res<UiLocale>,
    mut controls: Query<PreferenceFill, PreferenceControls>,
    mut labels: Query<(&mut LocalizedText, &mut Text), With<ProxyViewLabel>>,
) {
    let Some(projection) = last.as_ref().and_then(|last| last.0.as_ref()) else {
        return;
    };
    for mut control in &mut controls {
        let enabled = if control.filter {
            projection.filter_alive
        } else {
            projection.compact_view
        };
        control.fill.0 = if enabled {
            palette.accent_container
        } else {
            palette.surface_elevated
        };
    }
    let key = if projection.compact_view {
        "proxies_list_view"
    } else {
        "proxies_grid_view"
    };
    for (mut copy, mut text) in &mut labels {
        if copy.key != key {
            *copy = LocalizedText::plain(key);
        }
        text.0 = copy.render(&locale);
    }
}

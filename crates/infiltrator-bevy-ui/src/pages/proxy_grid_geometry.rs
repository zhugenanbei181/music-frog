//! The Bevy Proxies page consumes the shared FlClash page-width grid rule.

use bevy::ecs::component::Component;
use infiltrator_contract::responsive_viewport::{
    ProxyGridForm, ResponsiveViewportSnapshot, ViewportTier, proxy_node_columns,
};
use lurker_core_contract::outbound::preferences::OutboundLayout;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProxyGridGeometry {
    pub tier: ViewportTier,
    pub form: ProxyGridForm,
    pub page_body_width_px: u16,
    pub columns: usize,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProxyGridContainer(pub ProxyGridGeometry);

pub fn proxy_grid_geometry(
    viewport: &ResponsiveViewportSnapshot,
    layout: OutboundLayout,
) -> ProxyGridGeometry {
    let tier = ViewportTier::from_width(viewport.width_px);
    let form = if matches!(layout, OutboundLayout::CompactList) || !tier.has_labelled_sidebar() {
        ProxyGridForm::Stacked
    } else {
        ProxyGridForm::Desktop
    };
    let page_body_width_px = tier.page_body_width_px(viewport.width_px.round() as u16);
    let columns = proxy_node_columns(form, page_body_width_px).max(1) as usize;
    ProxyGridGeometry {
        tier,
        form,
        page_body_width_px,
        columns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_follows_the_shared_page_width_rule() {
        let cases = [
            (
                1180.0,
                OutboundLayout::Grid,
                ViewportTier::Expanded,
                ProxyGridForm::Desktop,
                908,
                4,
            ),
            (
                720.0,
                OutboundLayout::Grid,
                ViewportTier::Medium,
                ProxyGridForm::Stacked,
                624,
                1,
            ),
            (
                400.0,
                OutboundLayout::Grid,
                ViewportTier::Compact,
                ProxyGridForm::Stacked,
                368,
                1,
            ),
            (
                1920.0,
                OutboundLayout::Grid,
                ViewportTier::Ultra,
                ProxyGridForm::Desktop,
                1608,
                7,
            ),
            (
                1180.0,
                OutboundLayout::CompactList,
                ViewportTier::Expanded,
                ProxyGridForm::Stacked,
                908,
                1,
            ),
        ];
        for (width, layout, tier, form, page_body_width_px, columns) in cases {
            let viewport = ResponsiveViewportSnapshot::from_dimensions(width, 780.0);
            let geometry = proxy_grid_geometry(&viewport, layout);
            assert_eq!(geometry.tier, tier, "tier at {width}px");
            assert_eq!(geometry.form, form, "grid form at {width}px");
            assert_eq!(
                geometry.page_body_width_px, page_body_width_px,
                "page body at {width}px"
            );
            assert_eq!(geometry.columns, columns, "columns at {width}px");
            assert_eq!(
                geometry.columns as u16,
                proxy_node_columns(form, page_body_width_px),
                "shared rule at {width}px"
            );
        }
    }
}

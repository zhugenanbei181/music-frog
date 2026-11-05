//! About, Disclaimer, Licenses and Tools hub page scenes for Bevy UI.

use crate::route::{PageRoot, Route};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, FlexDirection, JustifyContent, Node, Overflow, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::ScrollArea;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::design_resources;
use infiltrator_shared::assets::{
    disclaimer_text_zh, harmonyos_font_license_text, third_party_notices_text,
};

/// Max height for scrollable license text containers.
const LICENSE_BOX_MAX_HEIGHT_PX: f32 = 180.0;
/// Large header icon tile dimension in logical baseline pixels.
const PAGE_HEADER_ICON_TILE_PX: f32 = 36.0;
/// Tool item icon tile dimension in logical baseline pixels.
const TOOL_ITEM_ICON_TILE_PX: f32 = 24.0;

/// Root marker on the About page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AboutPageRoot;

/// Root marker on the Disclaimer page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DisclaimerPageRoot;

/// Root marker on the Licenses page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LicensesPageRoot;

/// Root marker on the Tools hub page scene.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ToolsPageRoot;

/// The About page: product version, architecture and license overview.
pub fn about_page(palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("nav_about");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            height: percent(100),
            min_height: px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: rem(space::S16),
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(rem(space::S16)),
        }
        PageRoot(Route::About)
        AboutPageRoot
        ScrollArea
        Children [
            @{ about_header_scene(palette) }
            --
            @{ about_info_card_scene(palette) }
        ]
    }
}

fn about_header_scene(palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("nav_about");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: rem(space::S16),
            }
            AccessibilityNode(header_a11y) label
            Children [
                Node {
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S12),
                }
                Children [
                    @{ icon_tile_scene(IconId::FileText, PAGE_HEADER_ICON_TILE_PX, palette) }
                    --
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S4),
                    }
                    Children [
                        LocalizedText::plain("nav_about") TextRole(design_resources::TextRole::Heading)
                        --
                        Text({ format!("MusicFrog Infiltrator v{}", env!("CARGO_PKG_VERSION")) })
                            TextRole(design_resources::TextRole::Caption)
                    ]
                ]
            ]
        })],
        palette,
    )
}

fn about_info_card_scene(palette: &UiPalette) -> impl Scene + use<> {
    surface_scene(
        vec![Box::new(bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S12),
            }
            Children [
                LocalizedText::plain("about_app_info") TextRole(design_resources::TextRole::Heading)
                --
                LocalizedText::plain("app_description") TextRole(design_resources::TextRole::Body)
                --
                LocalizedText::plain("about_rem_architecture_notice") TextRole(design_resources::TextRole::Caption)
            ]
        })],
        palette,
    )
}

/// The Disclaimer page: legal terms and compliance requirements.
pub fn disclaimer_page(palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("nav_disclaimer");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            height: percent(100),
            min_height: px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: rem(space::S16),
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(rem(space::S16)),
        }
        PageRoot(Route::Disclaimer)
        DisclaimerPageRoot
        ScrollArea
        Children [
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S12),
                    }
                    AccessibilityNode(header_a11y) label
                    Children [
                        @{ icon_tile_scene(IconId::Shield, PAGE_HEADER_ICON_TILE_PX, palette) }
                        --
                        LocalizedText::plain("nav_disclaimer") TextRole(design_resources::TextRole::Heading)
                    ]
                })],
                palette,
            ) }
            --
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S12),
                    }
                    Children [
                        Text({ disclaimer_text_zh().to_owned() }) TextRole(design_resources::TextRole::Body)
                    ]
                })],
                palette,
            ) }
        ]
    }
}

/// The Licenses page: font attribution and third-party dependency notices.
pub fn licenses_page(palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("nav_licenses");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            height: percent(100),
            min_height: px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: rem(space::S16),
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(rem(space::S16)),
        }
        PageRoot(Route::Licenses)
        LicensesPageRoot
        ScrollArea
        Children [
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S12),
                    }
                    AccessibilityNode(header_a11y) label
                    Children [
                        @{ icon_tile_scene(IconId::FileText, PAGE_HEADER_ICON_TILE_PX, palette) }
                        --
                        LocalizedText::plain("nav_licenses") TextRole(design_resources::TextRole::Heading)
                    ]
                })],
                palette,
            ) }
            --
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S12),
                    }
                    Children [
                        LocalizedText::plain("settings_font_attribution_title") TextRole(design_resources::TextRole::Heading)
                        --
                        Node {
                            width: percent(100),
                            max_height: rem(LICENSE_BOX_MAX_HEIGHT_PX),
                            overflow: Overflow::scroll_y(),
                        }
                        Children [
                            Text({ harmonyos_font_license_text().to_owned() }) TextRole(design_resources::TextRole::Mono)
                        ]
                    ]
                })],
                palette,
            ) }
            --
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S12),
                    }
                    Children [
                        LocalizedText::plain("about_third_party_title") TextRole(design_resources::TextRole::Heading)
                        --
                        Node {
                            width: percent(100),
                            max_height: rem(LICENSE_BOX_MAX_HEIGHT_PX),
                            overflow: Overflow::scroll_y(),
                        }
                        Children [
                            Text({ third_party_notices_text().to_owned() }) TextRole(design_resources::TextRole::Mono)
                        ]
                    ]
                })],
                palette,
            ) }
        ]
    }
}

/// The Tools hub page: quick shortcuts to diagnostics, routing, sync, editor, and settings.
pub fn tools_page(palette: &UiPalette) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("nav_tools");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    bsn! {
        Node {
            width: percent(100),
            min_width: px(0.0),
            max_width: percent(100),
            height: percent(100),
            min_height: px(0.0),
            flex_direction: FlexDirection::Column,
            row_gap: rem(space::S16),
            overflow: Overflow::scroll_y(),
            padding: UiRect::all(rem(space::S16)),
        }
        PageRoot(Route::Tools)
        ToolsPageRoot
        ScrollArea
        Children [
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S12),
                    }
                    AccessibilityNode(header_a11y) label
                    Children [
                        @{ icon_tile_scene(IconId::Settings, PAGE_HEADER_ICON_TILE_PX, palette) }
                        --
                        LocalizedText::plain("nav_tools") TextRole(design_resources::TextRole::Heading)
                    ]
                })],
                palette,
            ) }
            --
            @{ surface_scene(
                vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        flex_direction: FlexDirection::Column,
                        row_gap: rem(space::S8),
                    }
                    Children [
                        @{ tool_item_scene(IconId::Activity, "nav_doctor", "tools_doctor_desc", palette) }
                        --
                        @{ tool_item_scene(IconId::Target, "nav_app_routing", "tools_app_routing_desc", palette) }
                        --
                        @{ tool_item_scene(IconId::Server, "nav_sync", "tools_sync_desc", palette) }
                        --
                        @{ tool_item_scene(IconId::Shield, "nav_dns", "tools_dns_desc", palette) }
                        --
                        @{ tool_item_scene(IconId::Settings, "nav_settings", "tools_settings_desc", palette) }
                        --
                        @{ tool_item_scene(IconId::FileText, "nav_about", "tools_about_desc", palette) }
                    ]
                })],
                palette,
            ) }
        ]
    }
}

fn tool_item_scene(
    icon: IconId,
    title_key: &'static str,
    desc_key: &'static str,
    palette: &UiPalette,
) -> impl Scene + use<> {
    bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            column_gap: rem(space::S12),
            padding: UiRect::all(rem(space::S8)),
        }
        Children [
            @{ icon_tile_scene(icon, TOOL_ITEM_ICON_TILE_PX, palette) }
            --
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S4),
            }
            Children [
                LocalizedText::plain(title_key) TextRole(design_resources::TextRole::Body)
                --
                LocalizedText::plain(desc_key) TextRole(design_resources::TextRole::Caption)
            ]
        ]
    }
}

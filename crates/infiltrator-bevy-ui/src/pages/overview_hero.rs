//! The Overview hero band: the run-state reading column (status dot, state
//! word, data-origin note, failure line, exit address and provenance chips)
//! and the action column (core orb plus the desktop probe group). Split out
//! of `overview.rs` so the page root carries only the page assembly.

use crate::pages::globe::GlobeSectionSlot;
use crate::pages::overview::overview_pulse::status_dot_scene;
use crate::pages::overview::{
    OverviewCardState, OverviewLine, OverviewLineKind, OverviewStatusCard, SurfaceElevatedFill,
    banner_note, banner_state_text, card_fill, status_semantic_node,
};
use crate::pages::overview_lifecycle::core_control_scene;
use crate::pages::overview_public_ip::{PublicIpText, PublicIpTextKind};
use crate::pages::overview_speedtest::{SPEEDTEST_GROUP_MAX_WIDTH_PX, speedtest_button_scene};
use crate::projection::OverviewProjection;
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::{ThemeBackgroundColor, ThemeTextColor};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, AlignSelf, BackgroundColor, BorderColor, BorderRadius, FlexDirection, FlexWrap,
    JustifyContent, Node, UiRect, percent, px,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::text::TextRole;
use infiltrator_bevy_widgets::theme::{radius, space};
use infiltrator_contract::design_resources;
use infiltrator_shared::locales::Lang;
use lurker_ui_presentation::core_status_projection::failure_copy;

const OVERVIEW_STATUS_TEXT_MIN_WIDTH_PX: f32 = 180.0;

/// The hero band: the reading column (status dot, state word, data-origin
/// note and failure line, then the exit address at display size with the
/// provenance chips under it) and the action column (the core orb and the
/// desktop probe group). This is the reference dashboard's composition: one
/// wide card whose left holds the facts the reader acts on and whose right
/// holds the controls, so no band of the card reads as an empty gap.
///
/// Every fact here rides the shared marker the page's one refresh pass
/// restamps. The exit address, the place and the organisation are the same
/// `PublicIpText` values the dedicated public-IP card shows, never a second
/// copy frozen at mount.
///
/// The current routing policy is **not** repeated here: the shell's status
/// capsule and the page's own routing-intent card already name it, and a
/// third copy of the same words in a pill only reads as a second control.
pub(crate) fn banner_scene(
    projection: &OverviewProjection,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let state = banner_state_text(projection, "en-US");
    let state_node = status_semantic_node(&state);
    let failure = failure_copy(&projection.lifecycle, projection.failure(), &Lang("en-US"));
    let note = banner_note(projection, "en-US");
    let stroke = palette.border;
    let exit_ip = projection
        .public_ip
        .ip
        .clone()
        .unwrap_or_else(|| "—".to_owned());
    let location = hero_exit_location(
        projection.public_ip.country_code.as_deref(),
        projection.public_ip.city.as_deref(),
    );
    let organization = projection
        .public_ip
        .isp
        .as_deref()
        .unwrap_or("—")
        .to_owned();
    bsn! {
            Node {
                width: percent(100),
                padding: UiRect::all(rem(space::S16)),
                align_items: AlignItems::FlexStart,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: rem(space::S16),
                row_gap: rem(space::S12),
                flex_wrap: FlexWrap::Wrap,
            border: UiRect::all(px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(palette.card_radius_px)),
            }
            BackgroundColor({ card_fill(Some(projection), &projection.lifecycle, palette) })
        BorderColor { top: stroke, right: stroke, bottom: stroke, left: stroke }
            OverviewStatusCard
            OverviewCardState({ Some(projection.lifecycle.clone()) })
            Children [
                Node {
                    flex_grow: 1.0,
                    flex_shrink: 1.0,
                    min_width: rem(OVERVIEW_STATUS_TEXT_MIN_WIDTH_PX),
                    flex_direction: FlexDirection::Column,
                    row_gap: rem(space::S8),
                }
                Children [
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: rem(space::S4),
                    }
                    Children [
                        @{ status_dot_scene(palette) }
                        --
                        Text({ state }) OverviewLine(OverviewLineKind::State) TextRole(design_resources::TextRole::Heading) state_node
                        --
                        Text({ note }) OverviewLine(OverviewLineKind::BannerNote) TextRole(design_resources::TextRole::Caption)
                        --
                        Text({ failure }) OverviewLine(OverviewLineKind::Failure) TextRole(design_resources::TextRole::Caption)
                    ]
                    --
                    @{ hero_exit_ip_scene(exit_ip) }
                    --
                    Node {
                        align_items: AlignItems::Center,
                        column_gap: rem(space::S8),
                        flex_wrap: FlexWrap::Wrap,
                        row_gap: rem(space::S6),
                    }
                    Children [
                        @{ hero_fact_chip_scene(location, PublicIpTextKind::Location, palette) }
                        --
                        @{ hero_fact_chip_scene(organization, PublicIpTextKind::Isp, palette) }
                    ]
                ]
                --
                Node {
                    flex_shrink: 0.0,
                    min_width: rem(SPEEDTEST_GROUP_MAX_WIDTH_PX),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: rem(space::S8),
                }
                Children [
                    @{ core_control_scene(palette) }
                    --
                    @{ speedtest_button_scene(palette) }
                ]
            ]
    }
}

/// The hero's exit address: the page's one large reading. It carries the
/// shared [`PublicIpText`] marker, so the page's single refresh pass restamps
/// it exactly like the dedicated public-IP card — one fact, one freshness.
pub(crate) fn hero_exit_ip_scene(value: String) -> impl Scene + use<> {
    bsn! {
            Node {
                min_width: px(0.0),
                max_width: percent(100),
            }
            Children [
                Text({ value })
                PublicIpText(PublicIpTextKind::Ip)
                TextRole(design_resources::TextRole::Display)
                ThemeTextColor({ Token::Ink })
            ]
    }
}

/// One hero provenance chip: the place and the network organisation the
/// shared probe actually reported. An absent fact stays an honest em dash;
/// the hero never labels a data centre the reader did not report.
pub(crate) fn hero_fact_chip_scene(
    value: String,
    kind: PublicIpTextKind,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let stroke = palette.border;
    bsn! {
            Node {
                padding: UiRect::axes(rem(space::S8), rem(space::S2)),
                border: UiRect::all(px(palette.hairline_px)),
                border_radius: BorderRadius::all(rem(radius::PILL)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            SurfaceElevatedFill
            BorderColor { top: stroke, right: stroke, bottom: stroke, left: stroke }
            Children [
                Text({ value }) PublicIpText(kind) TextRole(design_resources::TextRole::Caption) ThemeTextColor({ Token::InkDim })
            ]
    }
}

/// The hero location reads the shared public-IP projection; an absent fact
/// stays an honest em dash rather than becoming an invented place.
pub(crate) fn hero_exit_location(country: Option<&str>, city: Option<&str>) -> String {
    match (country, city) {
        (Some(country), Some(city)) => format!("{country} · {city}"),
        (Some(country), None) => country.to_owned(),
        (None, Some(city)) => city.to_owned(),
        (None, None) => "—".to_owned(),
    }
}

/// The Overview page's geo slot. The Globe section mounts here, so the page
/// owns where the section sits: the section never outranks the page's own
/// hero, and the compact viewport keeps the banner in view.
pub(crate) fn globe_slot_scene() -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(50),
                min_width: px(0.0),
                max_width: percent(100),
                align_self: AlignSelf::FlexEnd,
                flex_direction: FlexDirection::Column,
            }
            GlobeSectionSlot
    }
}

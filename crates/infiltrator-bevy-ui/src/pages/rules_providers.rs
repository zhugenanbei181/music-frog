//! The Rules workspace's providers partition: the MRS acceleration card and
//! the rule-provider lifecycle table.
//!
//! The partition is one business seam of the Rules page: the rule list and its
//! draft live in [`crate::pages::rules`], the rule tracer in
//! [`crate::pages::rules_tracer`], and the providers' own lifecycle lives
//! here. Every line below is a provider row or the card that holds them.

use crate::pages::rules::RuleProviderItem;
use crate::pages::rules_mrs::rules_mrs_scene;
use crate::pages::rules_outcome::{RuleProviderFailureText, rule_provider_refresh_scene};
use crate::pages::rules_projection::{
    ProviderCountText, ProviderNameText, ProviderUpdatedText, RulesLine, RulesLineKind,
};
use bevy::ecs::hierarchy::Children;
use bevy::feathers::theme::ThemeBackgroundColor;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, percent,
};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::feathers_theme::Token;
use infiltrator_bevy_widgets::localization::LocalizedText;
use infiltrator_bevy_widgets::palette::{UiPalette, rem};
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::mrs_acceleration::MrsAccelerationSnapshot;
use infiltrator_contract::provider_cache::{KernelEtagSupportSnapshot, RuleProviderCacheSnapshot};
use infiltrator_presentation::rule_provider_projection::{
    etag_support_line, provider_count, provider_fingerprint_line, provider_lifecycle_line,
};
use infiltrator_shared::locales::Lang;

/// One scene per published rule provider.
pub fn provider_scenes(providers: &[RuleProviderItem], palette: &UiPalette) -> Vec<Box<dyn Scene>> {
    providers
        .iter()
        .enumerate()
        .map(|(idx, provider)| {
            Box::new(provider_item_scene(idx, provider, palette)) as Box<dyn Scene>
        })
        .collect()
}

/// DUAL-11-14: the providers partition — the MRS acceleration card (which also
/// hosts the Geo database update entry) and the provider lifecycle table.
pub fn providers_partition(
    provider_scenes: Vec<Box<dyn Scene>>,
    palette: &UiPalette,
    mrs: &MrsAccelerationSnapshot,
    provider_cache: &RuleProviderCacheSnapshot,
    etag_support: &KernelEtagSupportSnapshot,
) -> impl Scene + use<> {
    bsn! {
            Node {
                width: percent(100),
                flex_direction: FlexDirection::Column,
                row_gap: rem(space::S16),
            }
            Children [
                @{ rules_mrs_scene(palette, mrs, provider_cache) }
                --
                @{ providers_card_scene(provider_scenes, etag_support, palette) }
            ]
    }
}

fn providers_card_scene(
    provider_scenes: Vec<Box<dyn Scene>>,
    etag_support: &KernelEtagSupportSnapshot,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let etag_line = etag_support_line(etag_support, &Lang("en-US"));
    surface_scene(
        vec![
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::SpaceBetween,
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                LocalizedText::plain("rules_providers_title") TextRole(Role::BodyStrong)
                                --
                                LocalizedText::plain("rules_providers_hint") TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                padding: UiRect::bottom(rem(space::S8)),
                            }
                            Children [
                                Text(etag_line) RulesLine(RulesLineKind::EtagSupport) TextRole(Role::Caption)
                            ]
            }),
            Box::new(bsn! {
                            Node {
                                width: percent(100),
                                flex_direction: FlexDirection::Column,
                                row_gap: rem(space::S8),
                            }
                            Children [
                                { provider_scenes }
                            ]
            }),
        ],
        palette,
    )
}

fn provider_item_scene(
    idx: usize,
    provider: &RuleProviderItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = provider.name.clone();
    let count_info = provider_count(provider.rule_count, &provider.behavior, &Lang("en-US"));
    let updated = provider_lifecycle_line(
        &provider.updated_at,
        provider.source_url.as_deref(),
        provider.refresh_interval_secs,
        &Lang("en-US"),
    );
    let updated = provider
        .cache_fingerprint
        .as_ref()
        .map(|observation| {
            format!(
                "{updated} · {}",
                provider_fingerprint_line(observation, &Lang("en-US"))
            )
        })
        .unwrap_or(updated);
    let failure = provider
        .failure
        .as_ref()
        .map(|failure| failure.message.clone())
        .unwrap_or_default();

    bsn! {
            Node {
                width: percent(100),
                padding: UiRect::all(rem(space::S8)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                border_radius: BorderRadius::all(rem(palette.control_radius_px)),
            }
            ThemeBackgroundColor({ Token::SurfaceElevated })
            Children [
                Node {
                    align_items: AlignItems::Center,
                    column_gap: rem(space::S8),
                }
                Children [
                    Text(name) ProviderNameText(idx) TextRole(Role::Body)
                    --
                    Text(count_info) ProviderCountText(idx) TextRole(Role::Caption)
                ]
                --
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    row_gap: rem(space::S4),
                }
                Children [
                    Text(updated) ProviderUpdatedText(idx) TextRole(Role::Caption)
                    --
                    Text({ failure }) RuleProviderFailureText(idx) TextRole(Role::Mono)
                    --
                    @{ rule_provider_refresh_scene(idx, provider.controller.is_some(), palette) }
                ]
            ]
    }
}

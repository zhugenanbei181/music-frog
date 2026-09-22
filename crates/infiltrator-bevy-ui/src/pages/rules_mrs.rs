//! MRS binary rule-set governance and deconstruction component (MRS 二进制规则集治理与解构).
//!
//! DUAL-11-03: this card renders the shared `MrsAccelerationSnapshot`
//! published by the surface reader. It never fabricates rule-set names or
//! counts; an offline/unsupported/failed snapshot renders an honest typed
//! status line instead.

use bevy::ecs::component::Component;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::observer::On;
use bevy::ecs::query::{With, Without};
use bevy::ecs::resource::Resource;
use bevy::ecs::system::{Query, Res, ResMut};
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::{Activate, Button};
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::mrs_acceleration::{
    MrsAccelerationSnapshot, MrsAccelerationStatus, MrsItemSnapshot,
};

use crate::command::{CommandSinkHandle, UiCommand};
use crate::pages::rules::RulesProjectionUpdated;

/// Marker for the MRS ruleset engine card root.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RulesMrsRoot;

/// Marker for the unpack rule provider button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UnpackRuleProviderButton;

/// DUAL-11-07: marker for the rule-provider cache purge button.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PurgeRuleProviderCacheButton;

/// DUAL-11-14: marker for the GeoIP/GeoSite database upgrade button. The
/// kernel owns the download; the button submits the shared
/// `UiCommand::UpgradeGeoDatabases` intent, the same trigger the Iced button
/// drives through the runtime gateway.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UpgradeGeoDatabasesButton;

/// DUAL-11-07: marker on the observed provider-cache fact line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProviderCacheText;

/// DUAL-11-06/07: the provider the unpack button targets, kept in sync with
/// the shared MRS projection so the button never names a fabricated provider.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct RulesMrsState {
    pub provider_name: Option<String>,
}

/// Honest one-line cache fact: directory, file count and bytes as observed.
pub(crate) fn provider_cache_line(
    cache: &infiltrator_contract::provider_cache::RuleProviderCacheSnapshot,
) -> String {
    use infiltrator_contract::provider_cache::RuleProviderCacheState;
    match cache.state {
        RuleProviderCacheState::Ready | RuleProviderCacheState::Empty => format!(
            "内核规则集缓存 · {} · {} 个文件 · {} 字节",
            cache.directory.as_deref().unwrap_or("目录未知"),
            cache.file_count,
            cache.total_bytes
        ),
        RuleProviderCacheState::Unsupported => format!(
            "规则集缓存：宿主未声明缓存目录{}",
            cache
                .failure
                .as_deref()
                .map(|reason| format!("（{reason}）"))
                .unwrap_or_default()
        ),
        RuleProviderCacheState::Failed => format!(
            "规则集缓存不可读：{}",
            cache.failure.as_deref().unwrap_or("未知错误")
        ),
        RuleProviderCacheState::Unknown => "规则集缓存状态未知".to_owned(),
    }
}

/// Marker on the MRS status/aggregate line.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MrsStatusText;

/// Marker on one MRS item row; the payload is the item index.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MrsItemText(pub usize);

/// Honest one-line status for the shared MRS read model.
pub(crate) fn mrs_status_line(mrs: &MrsAccelerationSnapshot) -> String {
    match mrs.status {
        MrsAccelerationStatus::Ready => format!(
            "MRS 加速就绪 · {} 个规则集 · {} 条规则 · 节省内存 {} · mmap {}",
            mrs.total_providers,
            mrs.total_accelerated_rules,
            mrs.total_memory_saved_bytes,
            if mrs.mmap_acceleration_active {
                "已启用"
            } else {
                "未启用"
            }
        ),
        MrsAccelerationStatus::Empty => "MRS 加速：当前配置未声明二进制规则集".to_owned(),
        MrsAccelerationStatus::Unsupported => format!(
            "MRS 加速不受支持：{}",
            mrs.failure.as_deref().unwrap_or("宿主未提供")
        ),
        MrsAccelerationStatus::Failed => format!(
            "MRS 加速失败：{}",
            mrs.failure.as_deref().unwrap_or("未知错误")
        ),
        MrsAccelerationStatus::Unknown => format!(
            "MRS 加速不可用：{}",
            mrs.failure.as_deref().unwrap_or("内核未运行")
        ),
    }
}

/// One MRS item line: name, behavior, count, compression and integrity facts.
pub(crate) fn mrs_item_label(item: &MrsItemSnapshot) -> String {
    let digest = item
        .sha256_digest
        .as_deref()
        .map(|value| format!("sha256 {}", value.chars().take(12).collect::<String>()))
        .unwrap_or_else(|| "sha256 —".to_owned());
    format!(
        "{} ({} 条目 · {} · {} · {})",
        item.name,
        item.rule_count,
        item.behavior.as_str(),
        if item.is_valid {
            "校验通过"
        } else {
            "校验失败"
        },
        digest
    )
}

/// Scene constructor for the MRS Ruleset Engine card.
pub fn rules_mrs_scene(
    palette: &UiPalette,
    mrs: &MrsAccelerationSnapshot,
    provider_cache: &infiltrator_contract::provider_cache::RuleProviderCacheSnapshot,
) -> impl Scene + use<> {
    let status_line = mrs_status_line(mrs);
    let cache_line = provider_cache_line(provider_cache);

    let item_scenes: Vec<Box<dyn Scene>> = mrs
        .items
        .iter()
        .enumerate()
        .map(|(idx, item)| {
            let label = mrs_item_label(item);
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::all(Val::Px(space::S8)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                }
                BackgroundColor({ palette.surface_elevated })
                Children [
                    ( Text({ label }) MrsItemText(idx) TextRole(Role::Body) ),
                ]
            }) as Box<dyn Scene>
        })
        .collect();

    surface_scene(
        vec![
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::SpaceBetween,
                    padding: UiRect::bottom(Val::Px(space::S8)),
                }
                RulesMrsRoot
                Children [
                    (
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            ( { icon_tile_scene(IconId::Activity, 24.0, palette) } ),
                            ( Text({ "MRS 二进制规则集治理与解构 (MRS Ruleset Engine)".to_owned() }) TextRole(Role::BodyStrong) ),
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
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.border })
                                Button
                                UpgradeGeoDatabasesButton
                                Children [
                                    ( Text({ "更新 Geo 数据库".to_owned() }) TextRole(Role::Body) ),
                                ]
                            ),
                            (
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.border })
                                Button
                                PurgeRuleProviderCacheButton
                                Children [
                                    ( Text({ "清理规则集本地缓存".to_owned() }) TextRole(Role::Body) ),
                                ]
                            ),
                            (
                                Node {
                                    min_height: px(palette.control_height_px),
                                    padding: UiRect::horizontal(Val::Px(space::S12)),
                                    align_items: AlignItems::Center,
                                    justify_content: JustifyContent::Center,
                                    border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                                }
                                BackgroundColor({ palette.accent })
                                Button
                                UnpackRuleProviderButton
                                Children [
                                    ( Text({ "一键解构导入为本地规则".to_owned() }) TextRole(Role::BodyStrong) ),
                                ]
                            ),
                        ]
                    ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    ( Text({ status_line }) MrsStatusText TextRole(Role::Caption) ),
                    ( Text({ cache_line }) ProviderCacheText TextRole(Role::Caption) ),
                ]
            }),
            Box::new(bsn! {
                Node {
                    width: percent(100),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::S8),
                    padding: UiRect::vertical(Val::Px(space::S4)),
                }
                Children [
                    { item_scenes },
                ]
            }),
        ],
        palette,
    )
}

/// Restamp the MRS status line and item rows from the shared snapshot.
pub fn apply_mrs_projection(
    update: On<RulesProjectionUpdated>,
    mut status: Query<(&mut Text, &MrsStatusText), Without<MrsItemText>>,
    mut items: Query<(&mut Text, &MrsItemText), Without<MrsStatusText>>,
    mut state: ResMut<RulesMrsState>,
) {
    let mrs = &update.0.mrs_acceleration;
    let want_status = mrs_status_line(mrs);
    for (mut text, _) in &mut status {
        if text.0 != want_status {
            text.0 = want_status.clone();
        }
    }
    for (mut text, marker) in &mut items {
        if let Some(item) = mrs.items.get(marker.0) {
            let want = mrs_item_label(item);
            if text.0 != want {
                text.0 = want;
            }
        }
    }
    // DUAL-11-06: the unpack target is the first provider the shared MRS read
    // model really reports; no provider means the button cannot name one.
    let want_provider = mrs.items.first().map(|item| item.name.clone());
    if state.provider_name != want_provider {
        state.provider_name = want_provider;
    }
}

/// DUAL-11-07: restamp the observed provider-cache fact line.
pub fn apply_provider_cache_projection(
    update: On<RulesProjectionUpdated>,
    mut cache_texts: Query<(&mut Text, &ProviderCacheText)>,
) {
    let want = provider_cache_line(&update.0.provider_cache);
    for (mut text, _) in &mut cache_texts {
        if text.0 != want {
            text.0 = want.clone();
        }
    }
}

/// DUAL-11-06/07: submit the shared unpack/purge intents for the MRS card.
pub(crate) fn on_rules_mrs_action_activated(
    activate: On<Activate>,
    unpack: Query<(), With<UnpackRuleProviderButton>>,
    purge: Query<(), With<PurgeRuleProviderCacheButton>>,
    geo: Query<(), With<UpgradeGeoDatabasesButton>>,
    state: Res<RulesMrsState>,
    handle: Option<Res<CommandSinkHandle>>,
) {
    let Some(handle) = handle else {
        return;
    };
    if purge.contains(activate.entity) {
        handle.submit(UiCommand::PurgeRuleProviderCache);
        return;
    }
    // DUAL-11-14: the same kernel trigger the Iced button drives.
    if geo.contains(activate.entity) {
        handle.submit(UiCommand::UpgradeGeoDatabases);
        return;
    }
    if unpack.contains(activate.entity)
        && let Some(provider) = state.provider_name.clone()
    {
        handle.submit(UiCommand::UnpackRuleProvider(provider));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::mrs_acceleration::MrsCompressionKind;

    fn item(name: &str) -> MrsItemSnapshot {
        MrsItemSnapshot {
            name: name.to_owned(),
            behavior: infiltrator_contract::mrs_acceleration::MrsBehaviorKind::Domain,
            format_version: 1,
            compression: MrsCompressionKind::None,
            rule_count: 42,
            payload_size_bytes: 128,
            file_size_bytes: 192,
            sha256_digest: Some("abcdef0123456789".to_owned()),
            crc32_checksum: Some(0x1234),
            is_mmap_accelerated: true,
            is_valid: true,
            description: String::new(),
            updated_at: String::new(),
            source_url: None,
            unpack_supported: true,
        }
    }

    #[test]
    fn status_line_is_honest_for_every_state() {
        let ready = MrsAccelerationSnapshot::ready(1, 1, vec![item("a.mrs")], true);
        assert!(mrs_status_line(&ready).contains("MRS 加速就绪"));
        assert!(mrs_status_line(&ready).contains("1 个规则集"));
        let empty = MrsAccelerationSnapshot::empty(1, 1);
        assert!(mrs_status_line(&empty).contains("未声明"));
        let unsupported = MrsAccelerationSnapshot::unsupported(1, 1, "无网关");
        assert!(mrs_status_line(&unsupported).contains("不受支持"));
        assert!(mrs_status_line(&unsupported).contains("无网关"));
        let failed = MrsAccelerationSnapshot::failed(1, 1, "解析失败");
        assert!(mrs_status_line(&failed).contains("失败"));
    }

    #[test]
    fn item_label_reports_integrity_and_digest_prefix() {
        let label = mrs_item_label(&item("geoip.mrs"));
        assert!(label.contains("geoip.mrs"));
        assert!(label.contains("42 条目"));
        assert!(label.contains("domain"));
        assert!(label.contains("校验通过"));
        assert!(label.contains("abcdef012345"));
    }
}

//! Bevy Settings scene piece for the local-only offline-startup preflight.
//!
//! Keeping the offline-first row beside its projection-specific scene keeps
//! `settings_core` below the source-size budget without moving any business
//! decision into the widget layer.

use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::BorderRadius;
use bevy::ui::prelude::{AlignItems, BackgroundColor, JustifyContent, Node, UiRect, Val, percent};
use bevy::ui::widget::Text;
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;
use infiltrator_contract::offline_startup::{
    LocalAssetStatus, OfflineStartupSnapshot, OfflineStartupState, StartupRemoteDependency,
};

use super::settings_core::{SettingsLine, SettingsLineKind};

pub(super) fn offline_startup_row_scene(
    snapshot: &OfflineStartupSnapshot,
    palette: &UiPalette,
) -> Box<dyn Scene> {
    let status = format_offline_startup(snapshot);
    Box::new(bsn! {
        Node {
            width: percent(100),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::SpaceBetween,
            padding: UiRect::all(Val::Px(space::S8)),
            border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
        }
        BackgroundColor({ palette.surface_elevated })
        Children [
            ( Text({ "离线启动 (Offline-first)".to_owned() }) TextRole(Role::Body) ),
            ( Text(status) SettingsLine(SettingsLineKind::OfflineStartup) TextRole(Role::Mono) ),
        ]
    })
}

pub(super) fn format_offline_startup(snapshot: &OfflineStartupSnapshot) -> String {
    let state = match snapshot.state {
        OfflineStartupState::Unknown => "未探测",
        OfflineStartupState::Checking => "校验中",
        OfflineStartupState::Ready => "可离线启动",
        OfflineStartupState::Degraded => "可启动但已降级",
        OfflineStartupState::Blocked => "已阻断",
    };
    let config = if snapshot.config_valid {
        "有效"
    } else {
        "无效"
    };
    let binary = if snapshot.binary_available {
        "可用"
    } else {
        "缺失"
    };
    let geoip = match snapshot.geoip {
        LocalAssetStatus::NotRequired => "不需要",
        LocalAssetStatus::Available => "本地可用",
        LocalAssetStatus::Missing => "缺失",
    };
    let remote = match snapshot.remote_dependency {
        StartupRemoteDependency::Optional => "远端可选",
    };
    let failure = snapshot
        .failure
        .as_ref()
        .map_or_else(String::new, |failure| {
            format!(
                " · {}",
                infiltrator_bevy_widgets::desktop::ClipboardPayload::sanitize_text(
                    &failure.message
                )
            )
        });
    format!(
        "离线优先 · {state} · 配置={config} · 内核={binary} · GeoIP={geoip} · {remote}{failure}"
    )
}

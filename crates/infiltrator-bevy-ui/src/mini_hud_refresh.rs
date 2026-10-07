//! Restamp the mounted HUD without destroying live control or texture entities.
use crate::a11y::switch_node;
use crate::mini_hud::{
    MiniHudModel, MiniHudQuickToggle, MiniHudText, MiniHudTextKind, exit_label, format_rate,
    pin_label,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Query, Res};
use bevy::ui::widget::Text;
use infiltrator_application::system_toggle_projection::{compact_label, compact_status_line};
use infiltrator_bevy_widgets::button::{ButtonDisabled, ControlVisual};
use infiltrator_bevy_widgets::localization::UiLocale;
use infiltrator_contract::a11y::ShellA11yNode;
use infiltrator_contract::system_toggle::SystemToggle;

#[derive(QueryData)]
#[query_data(mutable)]
pub struct HudToggleRow {
    marker: &'static MiniHudQuickToggle,
    children: &'static Children,
    visual: &'static mut ControlVisual,
    disabled: &'static mut ButtonDisabled,
    accessibility: &'static mut AccessibilityNode,
}

pub fn refresh_mini_hud(
    model: Res<MiniHudModel>,
    locale: Res<UiLocale>,
    mut texts: Query<&mut Text>,
    labels: Query<(&Children, &MiniHudText)>,
    mut toggles: Query<HudToggleRow>,
) {
    // Text wrappers retain their entity, preserving widget focus and pending pointer events.
    for (children, kind) in &labels {
        let value = match kind.0 {
            MiniHudTextKind::UpRate => format_rate(model.0.up_bytes_per_sec),
            MiniHudTextKind::DownRate => format_rate(model.0.down_bytes_per_sec),
            MiniHudTextKind::Mode => model.0.mode_zh.clone(),
            MiniHudTextKind::Node => exit_label(&model.0),
            MiniHudTextKind::ToggleLine => {
                compact_status_line(&model.0.system_proxy, &model.0.tun, locale.code())
            }
            MiniHudTextKind::Pin => pin_label(&model.0),
        };
        for child in children {
            if let Ok(mut text) = texts.get_mut(*child)
                && text.0 != value
            {
                text.0 = value.clone();
            }
        }
    }
    for mut row in &mut toggles {
        let toggle = row.marker.0;
        let (state, node) = match toggle {
            SystemToggle::SystemProxy => (
                &model.0.system_proxy,
                ShellA11yNode::MiniHudSystemProxySwitch,
            ),
            SystemToggle::Tun => (&model.0.tun, ShellA11yNode::MiniHudTunSwitch),
        };
        row.visual.0 = state.is_enabled();
        row.disabled.0 = model.0.next_value(toggle).is_none();
        *row.accessibility = switch_node(node, state.is_enabled());
        for child in row.children {
            if let Ok(mut text) = texts.get_mut(*child) {
                text.0 = compact_label(state, locale.code());
            }
        }
    }
}

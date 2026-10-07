//! Cards owner.

use crate::pages::profiles::{
    ActivateProfileButton, DeleteProfileButton, ProfileItem, ProfileNameText,
    ProfileProtectionText, ProfileScheduleText, ProfileStatusText, ProfileTimeText,
    ProfileTrafficText, ProfilesLine, ProfilesLineKind, UpdateAllSubscriptionsButton,
    UpdateProfileButton, profile_schedule_summary,
};
use bevy::a11y::AccessibilityNode;
use bevy::ecs::hierarchy::Children;
use bevy::scene::{Scene, bsn};
use bevy::ui::prelude::{
    AlignItems, BackgroundColor, BorderRadius, FlexDirection, JustifyContent, Node, UiRect, Val,
    percent, px,
};
use bevy::ui::widget::Text;
use bevy::ui_widgets::Button;
use infiltrator_application::profile_editor_projection::protection_label;
use infiltrator_application::profile_metadata_projection;
use infiltrator_bevy_widgets::button::ControlVisual;
use infiltrator_bevy_widgets::icon::IconId;
use infiltrator_bevy_widgets::icon_tile::icon_tile_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedText, UiLocale};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::surface::surface_scene;
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::theme::space;

pub(super) fn header_card_scene(
    summary: String,
    auto_update: String,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let mut header_a11y = accesskit::Node::new(accesskit::Role::Header);
    let label = LocalizedLabel::plain("profile_header_a11y");
    header_a11y.set_label(label.0.render(&UiLocale::default()));

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(space::S16),
                    }
                    AccessibilityNode(header_a11y) label
                    Children [
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S12),
                        }
                        Children [
                            @{ icon_tile_scene(IconId::FileText, 36.0, palette) }
                            --
                            Node {
                                flex_direction: FlexDirection::Column,
                                row_gap: Val::Px(space::S4),
                            }
                            Children [
                                Text(summary) ProfilesLine(ProfilesLineKind::Summary) TextRole(Role::Heading)
                                --
                                Text(auto_update) ProfilesLine(ProfilesLineKind::AutoUpdate) TextRole(Role::Caption)
                            ]
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            UpdateAllSubscriptionsButton
                            Children [
                                LocalizedText::plain("profiles_update_all") TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.accent })
                            Button
                            Children [
                                LocalizedText::plain("profiles_import_subscription_action") TextRole(Role::BodyStrong)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

pub(super) fn profile_card_scene(
    idx: usize,
    profile: &ProfileItem,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let name = profile.name.clone();
    let url = profile.url.clone();
    let locale = UiLocale::default();
    let updated = profile_metadata_projection::updated(&profile.updated_at, locale.code());
    let traffic_str = profile_metadata_projection::traffic_caption(
        &profile_metadata_projection::traffic(
            profile.upload_bytes,
            profile.download_bytes,
            profile.total_bytes,
        ),
        locale.code(),
    );
    let status_str = profile_metadata_projection::active_status(profile.is_active, locale.code());
    let schedule_str = profile_schedule_summary(profile, locale.code());
    let btn_bg = if profile.is_active {
        palette.success
    } else {
        palette.surface_elevated
    };

    surface_scene(
        vec![Box::new(bsn! {
                    Node {
                        width: percent(100),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::SpaceBetween,
                        column_gap: Val::Px(space::S16),
                    }
                    Children [
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(space::S4),
                        }
                        Children [
                            Node {
                                align_items: AlignItems::Center,
                                column_gap: Val::Px(space::S8),
                            }
                            Children [
                                Text(name) ProfileNameText(idx) TextRole(Role::BodyStrong)
                                --
                                Text(updated) ProfileTimeText(idx) TextRole(Role::Caption)
                            ]
                            --
                            Text(url) TextRole(Role::Caption)
                            --
                            Text({ protection_label(profile.write_protection, locale.code()) })
                            ProfileProtectionText(idx)
                            TextRole(Role::Caption)
                            --
                            Text(traffic_str) ProfileTrafficText(idx) TextRole(Role::Mono)
                            --
                            Text(schedule_str) ProfileScheduleText(idx) TextRole(Role::Caption)
                        ]
                        --
                        Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::S8),
                        }
                        Children [
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            Button
                            UpdateProfileButton(idx)
                            Children [
                                LocalizedText::plain("profiles_update_now") TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ btn_bg })
                            ControlVisual({ profile.is_active })
                            ActivateProfileButton {
                                profile_id: { profile.id.clone() },
                                profile_idx: { idx },
                            }
                            Button
                            Children [
                                Text(status_str) ProfileStatusText(idx) TextRole(Role::Body)
                            ]
                            --
                            Node {
                                min_height: px(palette.control_height_px),
                                padding: UiRect::horizontal(Val::Px(space::S12)),
                                align_items: AlignItems::Center,
                                justify_content: JustifyContent::Center,
                                border_radius: BorderRadius::all(Val::Px(palette.control_radius_px)),
                            }
                            BackgroundColor({ palette.surface_elevated })
                            ControlVisual({ !profile.is_active })
                            Button
                            DeleteProfileButton {
                                profile_id: { profile.id.clone() },
                                profile_idx: { idx },
                            }
                            Children [
                                LocalizedText::plain("profiles_delete_action") TextRole(Role::Body)
                            ]
                        ]
                    ]
        })],
        palette,
    )
}

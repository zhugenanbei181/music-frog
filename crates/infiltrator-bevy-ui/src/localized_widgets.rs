//! Native scene composition carries copy keys on the existing widget root and label.
use bevy::scene::{Scene, bsn};
use infiltrator_bevy_widgets::button::{
    ButtonLabel, ButtonSize, ButtonVariant, PillLabel, button_with_label_scene,
    pill_caption_with_label_scene,
};
use infiltrator_bevy_widgets::checkbox::checkbox_with_label_scene;
use infiltrator_bevy_widgets::localization::{LocalizedLabel, LocalizedPlaceholder, LocalizedText};
use infiltrator_bevy_widgets::palette::UiPalette;
use infiltrator_bevy_widgets::tabs::{SegmentedTabLabel, segmented_control_with_labels_scene};
use infiltrator_bevy_widgets::text::{Role, TextRole};
use infiltrator_bevy_widgets::text_input::text_field_with_placeholder_scene;

pub fn localized_pill_scene(
    copy: LocalizedText,
    selected: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let semantic = copy.clone();
    (
        pill_caption_with_label_scene(
            bsn! { LocalizedText { key: { copy.key }, params: { copy.params } } TextRole(Role::Caption) PillLabel },
            selected,
            palette,
        ),
        bsn! { LocalizedLabel(semantic) },
    )
}

pub fn localized_checkbox_scene(
    copy: LocalizedText,
    checked: bool,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let label = copy.clone();
    (
        checkbox_with_label_scene(
            bsn! { LocalizedText { key: { copy.key }, params: { copy.params } } TextRole(Role::Body) },
            checked,
            palette,
        ),
        bsn! { LocalizedLabel(label) },
    )
}
pub fn localized_field_scene(
    value: String,
    copy: LocalizedText,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let label = copy.clone();
    (
        text_field_with_placeholder_scene(value, String::new(), palette),
        bsn! { LocalizedPlaceholder(copy) LocalizedLabel(label) },
    )
}

/// Preserve the SDK button root, skin, disabled state and native localized label.
pub fn localized_button_scene(
    copy: LocalizedText,
    variant: ButtonVariant,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let label = copy.clone();
    (
        button_with_label_scene(
            bsn! { LocalizedText { key: { copy.key }, params: { copy.params } } TextRole(Role::Body) ButtonLabel },
            variant,
            ButtonSize::Md,
            palette,
        ),
        bsn! { LocalizedLabel(label) },
    )
}

/// Native segmented labels retain their keys across locale and state changes.
pub fn localized_segmented_scene(
    keys: &[&'static str],
    selected_index: usize,
    palette: &UiPalette,
) -> impl Scene + use<> {
    let labels = keys
        .iter()
        .map(|key| {
            let copy = LocalizedText::plain(key);
            Box::new(bsn! {
                LocalizedText { key: { copy.key }, params: { copy.params } }
                TextRole(Role::Caption) SegmentedTabLabel
            }) as Box<dyn Scene>
        })
        .collect();
    segmented_control_with_labels_scene(labels, selected_index, palette)
}

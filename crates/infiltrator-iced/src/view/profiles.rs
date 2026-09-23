//! Profiles & Subscriptions page view: remote subscription imports, local YAML imports,
//! auto-update scheduler settings, and profile card management with traffic quota and expiry tracking.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::modern_scrollable;
use crate::view::theme::{self, SP_MD};
use iced::widget::{Space, column};
use iced::{Element, Length};
use infiltrator_shared::locales::Lang;

mod helpers;
mod imports;
mod list;
mod subscription;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let lang = Lang(&state.shell.lang);
    let mut content_items: Vec<Element<'_, Message>> = vec![
        imports::header(state),
        Space::new().height(theme::SP_LG).into(),
    ];
    if let Some(alert) = imports::active_alert(state) {
        content_items.push(alert);
        content_items.push(Space::new().height(SP_MD).into());
    }
    content_items.push(imports::import_section(state));
    content_items.push(Space::new().height(SP_MD).into());
    content_items.push(imports::local_import_section(state));
    content_items.push(Space::new().height(SP_MD).into());
    content_items.push(subscription::subscription_section(state));
    content_items.push(Space::new().height(theme::SP_MD).into());
    content_items.push(crate::view::sub_quota_card::sub_quota_card(state, &lang));
    content_items.push(Space::new().height(theme::SP_LG).into());
    content_items.push(list::profiles_section(state));
    content_items.push(Space::new().height(theme::SP_XL).into());

    let content = column(content_items).spacing(10);
    modern_scrollable(content).height(Length::Fill).into()
}

#[cfg(test)]
#[path = "../../tests/gui/view_profiles_tests.rs"]
mod tests;

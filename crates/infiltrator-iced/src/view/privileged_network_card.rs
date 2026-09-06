//! Iced projection for the privileged network regression transaction.

use crate::state::AppState;
use crate::types::message::Message;
use crate::view::components::{BadgeKind, badge, card, style_accent};
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, button, column, row, text};
use iced::{Alignment, Element, Length, Theme};
use infiltrator_contract::privileged_network::{
    PrivilegedNetworkSnapshot, PrivilegedNetworkState,
};
use infiltrator_shared::locales::{Lang, Localizer};

pub fn privileged_network_card<'a>(
    state: &'a AppState,
    lang: &Lang<'_>,
) -> Element<'a, Message> {
    let snapshot = &state.runtime.privileged_network;
    let run = button(text(lang.tr("privileged_network_run").to_string()))
        .padding([4, 12])
        .style(style_accent);
    let run = if matches!(snapshot.state, PrivilegedNetworkState::Unsupported { .. }) {
        run
    } else {
        run.on_press(Message::RunPrivilegedNetworkRegression)
    };
    let details = format_details(snapshot);
    card(
        Some(lang.tr("privileged_network_title").to_string()),
        column![
            text(lang.tr("privileged_network_desc").to_string())
                .size(12)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_secondary),
                }),
            Space::new().height(theme::SP_XS),
            row![
                badge(format_status(&snapshot.state, lang), BadgeKind::Accent),
                Space::new().width(Length::Fill),
                text(details).size(11).font(MONO),
            ]
            .align_y(Alignment::Center),
            Space::new().height(theme::SP_XS),
            run,
        ]
        .spacing(theme::SP_SM),
    )
}

fn format_status(state: &PrivilegedNetworkState, lang: &Lang<'_>) -> String {
    match state {
        PrivilegedNetworkState::Idle => lang.tr("privileged_network_status_idle").to_string(),
        PrivilegedNetworkState::Injecting => {
            lang.tr("privileged_network_status_injecting").to_string()
        }
        PrivilegedNetworkState::Active => lang.tr("privileged_network_status_active").to_string(),
        PrivilegedNetworkState::RollingBack => {
            lang.tr("privileged_network_status_rolling_back").to_string()
        }
        PrivilegedNetworkState::Cleaned => {
            lang.tr("privileged_network_status_cleaned").to_string()
        }
        PrivilegedNetworkState::Unsupported { reason } => format!(
            "{} · {reason}",
            lang.tr("privileged_network_status_unsupported")
        ),
        PrivilegedNetworkState::Failed { failure } => format!(
            "{} · {}",
            lang.tr("privileged_network_status_failed"),
            failure.message
        ),
    }
}

fn format_details(snapshot: &PrivilegedNetworkSnapshot) -> String {
    format!(
        "operations={} · injected={} · cleanup={} · rollback={}",
        snapshot.operation_count,
        snapshot.injected,
        snapshot.cleanup_attempted,
        snapshot.rollback_attempted
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_contract::privileged_network::PrivilegedNetworkSnapshot;

    #[test]
    fn card_consumes_cleaned_readback_without_fabricating_active_state() {
        let (mut state, _) = AppState::new();
        let snapshot = PrivilegedNetworkSnapshot::cleaned(2, 3, false);
        let _ = state.update(Message::PrivilegedNetworkRegressionUpdated(Ok(
            snapshot.clone(),
        )));
        assert_eq!(state.runtime.privileged_network, snapshot);
        let lang = Lang(&state.shell.lang);
        let _ = privileged_network_card(&state, &lang);
    }
}

//! Native TEA sandbox inspector preserves unknown inputs until the user supplies them.
use crate::state::AppState;
use crate::types::message::Message;
use crate::types::rule_trace::RuleTraceAction;
use crate::view::component_forms::{form_input_style, style_ghost};
use iced::widget::{button, column, row, scrollable, text, text_input};
use iced::{Element, Length};
use infiltrator_application::rule_condition_projection::field_key;
use infiltrator_contract::rule_condition::TrafficField;
use infiltrator_shared::locales::{Lang, Localizer};
pub fn sandbox_view<'a>(state: &'a AppState, lang: &Lang<'_>) -> Element<'a, Message> {
    let model = &state.editor.rule_trace;
    let toggle = button(text(lang.tr("rule_trace_sandbox_toggle").to_string()))
        .style(style_ghost)
        .on_press_maybe(
            (!model.busy()).then_some(Message::RuleTrace(RuleTraceAction::ToggleSandbox)),
        );
    if !model.advanced_open {
        return toggle.into();
    }
    let rows: Vec<Element<'a, Message>> = TrafficField::SANDBOX
        .into_iter()
        .map(|field| {
            let value = model.sandbox.get(&field).map(String::as_str).unwrap_or("");
            row![
                text(lang.tr(field_key(field)).to_string()).width(Length::FillPortion(2)),
                text_input(lang.tr("rule_trace_sandbox_unknown").as_ref(), value)
                    .on_input_maybe((!model.busy()).then_some(move |value| Message::RuleTrace(
                        RuleTraceAction::Sandbox(field, value)
                    )))
                    .style(form_input_style)
                    .width(Length::FillPortion(3))
            ]
            .spacing(8)
            .into()
        })
        .collect();
    column![
        toggle,
        text(lang.tr("rule_trace_sandbox_notice").to_string()).size(11),
        scrollable(column(rows).spacing(8)).height(180)
    ]
    .spacing(8)
    .into()
}

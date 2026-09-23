//! DUAL-05-09/10/13: the custom-node dialer-hop and certificate-trust editors.
//!
//! Split from `custom_node_params.rs` to respect the business-file line
//! budget. Every rendered value comes from the shared studio snapshot: the
//! chain line and loop findings are the ones the shared analyzer published, and
//! the CA lines are what the host really did (an unsupported host never looks
//! loaded here).

use crate::types::message::Message;
use crate::view::theme::{self, MONO, tokens};
use iced::widget::{Space, column, text};
use iced::{Element, Length, Theme};
use infiltrator_contract::protocol_fidelity::{ProtocolDraft, ProtocolStudioSnapshot};
use infiltrator_shared::locales::{Lang, Localizer};

use super::custom_node_params::{edit, field, label_text, params_row};

/// DUAL-05-09/10/13: the dialer hop, the resolved chain (with loop warnings)
/// and the custom-CA / certificate-whitelist editors.
///
/// Everything rendered here comes from the shared studio snapshot: the chain
/// line and loop findings are the ones the shared analyzer published, and the
/// CA lines are what the host really did.
pub(super) fn hop_and_trust_params<'a>(
    draft: &'a ProtocolDraft,
    studio: &'a ProtocolStudioSnapshot,
    lang: &Lang<'_>,
) -> Vec<Element<'a, Message>> {
    let mut blocks: Vec<Element<'a, Message>> = Vec::new();

    // ---- DUAL-05-09: the hop this node dials through -----------------------
    blocks.push(params_row(vec![
        field(
            lang.tr("custom_node_dialer_proxy").to_string(),
            "gateway / proxy-group name",
            draft.dialer_proxy.as_str(),
            move |value| {
                edit(draft, move |next| {
                    next.dialer_proxy = value;
                })
            },
        ),
        column![
            label_text(lang.tr("custom_node_dialer_scan").to_string()),
            Space::new().height(4.0),
            crate::view::component_forms::text_btn(
                lang.tr("custom_node_dialer_scan").to_string(),
                crate::view::component_forms::style_ghost,
                Some(Message::ScanCustomNodeDialer),
            ),
        ]
        .width(Length::FillPortion(1))
        .into(),
    ]));

    // The shared chain for this node (and only this node).
    if let Some(chain) = studio.dialer.chain_for(draft.name.trim()) {
        let valid = chain.valid();
        blocks.push(
            text(format!(
                "{} {}",
                lang.tr("custom_node_dialer_chain"),
                chain.chain_line()
            ))
            .size(10)
            .font(MONO)
            .style(move |t: &Theme| text::Style {
                color: Some(if valid {
                    tokens(t).success
                } else {
                    tokens(t).warning
                }),
            })
            .into(),
        );
        for warning in chain.warning_lines().iter().take(2) {
            blocks.push(
                text(warning.clone())
                    .size(10)
                    .font(MONO)
                    .style(|t: &Theme| text::Style {
                        color: Some(tokens(t).warning),
                    })
                    .into(),
            );
        }
    }
    // DUAL-05-10: profile-wide loop findings are never hidden.
    for finding in studio.dialer.loops.iter().take(2) {
        blocks.push(
            text(finding.message.clone())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).warning),
                })
                .into(),
        );
    }
    for warning in studio.dialer.warnings.iter().take(2) {
        blocks.push(
            text(warning.clone())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                })
                .into(),
        );
    }

    // ---- DUAL-05-13: custom CA / certificate whitelist ---------------------
    let ca_path = draft.params.tls_trust.ca_path.as_str();
    let ca_str = draft.params.tls_trust.ca_str.as_str();
    let fingerprint = draft.params.tls_trust.fingerprint.as_str();
    blocks.push(params_row(vec![
        field(
            lang.tr("custom_node_ca_path").to_string(),
            "/etc/ssl/certs/ca.pem",
            ca_path,
            move |value| {
                edit(draft, move |next| {
                    next.params.tls_trust.ca_path = value;
                })
            },
        ),
        field(
            lang.tr("custom_node_ca_fingerprint").to_string(),
            "sha256 hex (64)",
            fingerprint,
            move |value| {
                edit(draft, move |next| {
                    next.params.tls_trust.fingerprint = value;
                })
            },
        ),
    ]));
    blocks.push(params_row(vec![
        field(
            lang.tr("custom_node_ca_str").to_string(),
            "-----BEGIN CERTIFICATE-----",
            ca_str,
            move |value| {
                edit(draft, move |next| {
                    next.params.tls_trust.ca_str = value;
                })
            },
        ),
        column![
            label_text(lang.tr("custom_node_ca_verify").to_string()),
            Space::new().height(4.0),
            crate::view::component_forms::text_btn(
                lang.tr("custom_node_ca_verify").to_string(),
                crate::view::component_forms::style_ghost,
                Some(Message::VerifyCustomNodeCertificateAuthority),
            ),
        ]
        .width(Length::FillPortion(1))
        .into(),
    ]));
    for line in studio.ca_trust.lines().iter().take(3) {
        blocks.push(
            text(line.clone())
                .size(10)
                .font(MONO)
                .style(|t: &Theme| text::Style {
                    color: Some(tokens(t).text_tertiary),
                })
                .into(),
        );
    }

    vec![column(blocks).spacing(theme::SP_XS).into()]
}

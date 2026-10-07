//! Once-folded facts rendered by both native protocol editors.
use infiltrator_contract::dialer_chain::DialerChainEnd;
use infiltrator_contract::protocol_fidelity::ProtocolStudioSnapshot;
use infiltrator_contract::protocol_form::ProtocolStudioSlot;

pub fn slot_text(
    slot: ProtocolStudioSlot,
    studio: &ProtocolStudioSnapshot,
    translate: impl Fn(&str) -> String,
    chinese: bool,
) -> String {
    match slot {
        ProtocolStudioSlot::Chips => studio
            .report
            .as_ref()
            .map(|r| r.all_chips().join(" · "))
            .unwrap_or_else(|| translate("protocol_form_no_draft")),
        ProtocolStudioSlot::Issues => {
            let issues = studio.issue_lines();
            if issues.is_empty() {
                translate("protocol_form_valid")
            } else {
                issues.join("; ")
            }
        }
        ProtocolStudioSlot::Notes => {
            let notes = studio
                .report
                .as_ref()
                .map(|r| r.params.notes.clone())
                .unwrap_or_default();
            if notes.is_empty() {
                translate("protocol_form_notes_empty")
            } else {
                notes.join("; ")
            }
        }
        ProtocolStudioSlot::Audit => match &studio.audit {
            Some(audit) => translate("protocol_form_audit")
                .replace("{detail}", &audit.detail)
                .replace("{nodes}", &audit.node_count.to_string())
                .replace("{unknown}", &audit.unknown_fields.len().to_string())
                .replace(
                    "{verdict}",
                    &translate(if audit.lossless {
                        "protocol_form_lossless"
                    } else {
                        "protocol_form_lossy"
                    }),
                ),
            None => translate("protocol_form_no_codec"),
        },
        ProtocolStudioSlot::UriPreview => studio
            .uri_preview
            .clone()
            .unwrap_or_else(|| translate("protocol_form_no_preview")),
        ProtocolStudioSlot::Gaps => {
            if studio.uri_gaps.is_empty() {
                translate("protocol_form_no_gaps")
            } else {
                translate("protocol_form_gaps").replace("{fields}", &studio.uri_gaps.join(" / "))
            }
        }
        ProtocolStudioSlot::Chain => {
            let name = studio
                .draft
                .as_ref()
                .map(|d| d.name.trim())
                .unwrap_or_default();
            match studio.dialer.chain_for(name) {
                Some(chain) => format!(
                    "{} · {}",
                    chain.chain_line(),
                    if chinese {
                        chain.end.label_zh()
                    } else {
                        match &chain.end {
                            DialerChainEnd::Complete => "Chain complete".to_owned(),
                            DialerChainEnd::MissingTarget { name } => {
                                format!("Missing dialer `{name}`")
                            }
                            DialerChainEnd::GroupBoundary { name } => {
                                format!("Group `{name}` selected at runtime")
                            }
                            DialerChainEnd::Cycle { path } => {
                                format!("Dependency cycle {}", path.join(" → "))
                            }
                        }
                    }
                ),
                None => {
                    let loops = studio.dialer.loop_lines();
                    if loops.is_empty() {
                        translate("protocol_form_chain_empty")
                    } else {
                        loops.join("; ")
                    }
                }
            }
        }
        ProtocolStudioSlot::CaTrust => {
            if studio.ca_trust.resolutions.is_empty() {
                translate("protocol_form_trust_empty")
            } else {
                studio.ca_trust.lines().join("; ")
            }
        }
    }
}

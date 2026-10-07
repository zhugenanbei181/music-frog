//! Read native widget layout after rendering. A route or state flag never proves visible controls.
use crate::state::AppState;
use crate::types::message::Message;
use crate::view::proxies::node_region_id;
use crate::view::proxies_controls::search_region_id;
use crate::view::rules::rules_list::row_region_id;
use crate::view::rules_tracer::TRACER_SCROLL_ID;
use crate::view::runtime::connection_search::{highlight_id, row_id};
use crate::view_root::interaction_regions::InteractionRegion;
use iced::advanced::widget::operation::{Outcome, Scrollable};
use iced::advanced::widget::{Id, Operation, operate};
use iced::widget::scrollable::AbsoluteOffset;
use iced::{Rectangle, Size, Task, Vector};
use infiltrator_contract::parity::FeatureId;
use std::sync::atomic::{AtomicBool, Ordering};

static CLIPPED_REGION_REPORTED: AtomicBool = AtomicBool::new(false);
static INCOMPLETE_REGION_REPORTED: AtomicBool = AtomicBool::new(false);

struct RevealScrollable {
    id: Id,
    offset: AbsoluteOffset<Option<f32>>,
}
impl Operation<Option<Rectangle>> for RevealScrollable {
    fn traverse(&mut self, visit: &mut dyn FnMut(&mut dyn Operation<Option<Rectangle>>)) {
        visit(self);
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        _: Rectangle,
        _: Rectangle,
        _: Vector,
        state: &mut dyn Scrollable,
    ) {
        if id == Some(&self.id) {
            state.scroll_to(self.offset);
        }
    }
    fn finish(&self) -> Outcome<Option<Rectangle>> {
        // Deliver a real UI event after changing native scroll state. The next
        // layout is measured separately; this operation never publishes pixels.
        Outcome::Some(None)
    }
}

struct RequiredRegion {
    id: Id,
    found: Option<Rectangle>,
    contained: bool,
    measured: Option<Rectangle>,
    visits: usize,
}
struct VisibleRegion {
    target: Id,
    clip: Rectangle,
    translation: Vector,
    pending_scroll: Option<(Rectangle, Vector)>,
    found: Option<Rectangle>,
    required: Vec<RequiredRegion>,
    target_visits: usize,
    clipped: Option<(Rectangle, Rectangle)>,
    target_clipped: Option<(Rectangle, Rectangle)>,
    reveal_scroll: Option<Id>,
    reveal_offset: Option<Vector>,
    reveal_clip: Option<Rectangle>,
}
impl VisibleRegion {
    fn new(target: Id, size: Size) -> Self {
        Self {
            target,
            clip: Rectangle::with_size(size),
            translation: Vector::ZERO,
            pending_scroll: None,
            found: None,
            required: Vec::new(),
            target_visits: 0,
            clipped: None,
            target_clipped: None,
            reveal_scroll: None,
            reveal_offset: None,
            reveal_clip: None,
        }
    }
    fn requiring(mut self, id: Id) -> Self {
        self.required.push(RequiredRegion {
            id,
            found: None,
            contained: true,
            measured: None,
            visits: 0,
        });
        self
    }
    fn revealing(mut self, id: Id) -> Self {
        self.reveal_scroll = Some(id);
        self
    }
    fn requiring_peer(mut self, id: Id) -> Self {
        self.required.push(RequiredRegion {
            id,
            found: None,
            contained: false,
            measured: None,
            visits: 0,
        });
        self
    }
    fn visual(&self, mut bounds: Rectangle) -> Rectangle {
        bounds.x -= self.translation.x;
        bounds.y -= self.translation.y;
        bounds
    }
}
impl Operation<Option<Rectangle>> for VisibleRegion {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<Option<Rectangle>>)) {
        let previous = (self.clip, self.translation);
        if let Some((bounds, offset)) = self.pending_scroll.take() {
            self.clip = self
                .clip
                .intersection(&self.visual(bounds))
                .unwrap_or_default();
            self.translation += offset;
        }
        operate(self);
        (self.clip, self.translation) = previous;
    }
    fn container(&mut self, id: Option<&Id>, bounds: Rectangle) {
        if id == Some(&self.target)
            || self
                .required
                .iter()
                .any(|required| id == Some(&required.id))
        {
            if id == Some(&self.target) {
                self.target_visits += 1;
            }
            for required in &mut self.required {
                if id == Some(&required.id) {
                    required.visits += 1;
                }
            }
            let bounds = self.visual(bounds);
            for required in &mut self.required {
                if id == Some(&required.id) {
                    required.measured = Some(bounds);
                }
            }
            if [bounds.x, bounds.y, bounds.width, bounds.height]
                .iter()
                .all(|v| v.is_finite())
                && bounds.width > 0.0
                && bounds.height > 0.0
                && bounds.x >= self.clip.x
                && bounds.y >= self.clip.y
                && bounds.x + bounds.width <= self.clip.x + self.clip.width
                && bounds.y + bounds.height <= self.clip.y + self.clip.height
            {
                if id == Some(&self.target) {
                    self.found = Some(bounds);
                } else {
                    for required in &mut self.required {
                        if id == Some(&required.id) {
                            required.found = Some(bounds);
                        }
                    }
                }
            } else if bounds.width > 0.0 && bounds.height > 0.0 {
                self.clipped = Some((bounds, self.clip));
                if id == Some(&self.target) {
                    self.target_clipped = self.clipped;
                }
            }
        }
    }
    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _content: Rectangle,
        translation: Vector,
        _state: &mut dyn Scrollable,
    ) {
        self.pending_scroll = Some((bounds, translation));
        if self.reveal_scroll.as_ref() == id && id.is_some() {
            self.reveal_offset = Some(translation);
            self.reveal_clip = self.clip.intersection(&self.visual(bounds));
        }
    }
    fn finish(&self) -> Outcome<Option<Rectangle>> {
        if let Some((bounds, clip)) = self.target_clipped.or(self.clipped)
            && let (Some(id), Some(offset)) = (&self.reveal_scroll, self.reveal_offset)
            && let clip = self.reveal_clip.unwrap_or(clip)
            && bounds.height <= clip.height
        {
            // Centre within the spare space instead of aligning to a clip edge.
            let delta = bounds.y - (clip.y + (clip.height - bounds.height) / 2.0);
            if delta != 0.0 {
                if !CLIPPED_REGION_REPORTED.swap(true, Ordering::Relaxed) {
                    eprintln!(
                        "native capture reveal {:?}: bounds={bounds:?} clip={clip:?} offset={offset:?} delta={delta}",
                        self.target
                    );
                }
                return Outcome::Chain(Box::new(RevealScrollable {
                    id: id.clone(),
                    offset: AbsoluteOffset {
                        x: None,
                        y: Some((offset.y + delta).max(0.0)),
                    },
                }));
            }
        }
        if let Some((bounds, clip)) = self.clipped
            && !CLIPPED_REGION_REPORTED.swap(true, Ordering::Relaxed)
        {
            eprintln!(
                "native capture region {:?} is clipped: bounds={bounds:?} clip={clip:?}",
                self.target
            );
        }
        let found = self.found.filter(|parent| {
            self.target_visits == 1
                && self.clipped.is_none()
                && self.required.iter().all(|required| {
                    required.visits == 1
                        && required.found.is_some_and(|child| {
                            !required.contained
                                || (child.x >= parent.x
                                    && child.y >= parent.y
                                    && child.x + child.width <= parent.x + parent.width
                                    && child.y + child.height <= parent.y + parent.height)
                        })
                })
        });
        if found.is_none() && !INCOMPLETE_REGION_REPORTED.swap(true, Ordering::Relaxed) {
            eprintln!(
                "native capture incomplete region {:?}: visits={} found={:?} required={:?}",
                self.target,
                self.target_visits,
                self.found,
                self.required
                    .iter()
                    .map(|region| (&region.id, region.visits, region.found, region.measured))
                    .collect::<Vec<_>>()
            );
        }
        Outcome::Some(found)
    }
}

impl AppState {
    pub(crate) fn capture_geometry_pending(&self) -> bool {
        self.shell.demo
            && self.shell.capture_marker.is_some()
            && !self.shell.capture_marker_written.load(Ordering::SeqCst)
    }

    pub(crate) fn capture_geometry_task(&self) -> Task<Message> {
        if !self.shell.demo || self.shell.capture_marker_written.load(Ordering::SeqCst) {
            return Task::none();
        }
        let Some(feature) = self.shell.capture_scenario else {
            return Task::none();
        };
        let target = match feature {
            FeatureId::ProxiesGroupExpanded | FeatureId::ProxiesSearchHighlight => {
                let Some((group, members)) = self.runtime.filtered_groups.first() else {
                    return Task::none();
                };
                let Some(node) = members.first() else {
                    return Task::none();
                };
                node_region_id(group, node)
            }
            FeatureId::ConnectionsCloseAllConfirm => InteractionRegion::Confirmation.id(),
            FeatureId::RulesOverrideEditor => InteractionRegion::Confirmation.id(),
            FeatureId::ConnectionsDetailsDrawer => InteractionRegion::Connection.id(),
            FeatureId::ConnectionsGrouping => InteractionRegion::ConnectionGroups.id(),
            FeatureId::LogsRedactedExport => InteractionRegion::LogExportDialog.id(),
            FeatureId::RuntimeTelemetryObservation => InteractionRegion::TrafficObservation.id(),
            FeatureId::ConnectionsSearchHighlight => row_id("group-0"),
            FeatureId::LogsSearchHighlight | FeatureId::LogsScrollLock => {
                Id::from("log-row-1".to_string())
            }
            FeatureId::ProxiesNodeDetailDrawer => InteractionRegion::ProxyInspection.id(),
            FeatureId::ProxiesProbeSettings => InteractionRegion::ProxyProbeSettings.id(),
            FeatureId::ProxiesGroupReorder => InteractionRegion::ProxyGroupOrder.id(),
            FeatureId::DnsLeakAlert => InteractionRegion::DnsLeakCard.id(),
            FeatureId::DnsHostsEditor => InteractionRegion::HostsEditor.id(),
            FeatureId::DnsQueryDetails => InteractionRegion::DnsQueryDialog.id(),
            FeatureId::RulesTracerDrawer => InteractionRegion::RuleTraceReport.id(),
            FeatureId::RulesStatisticsInspector => InteractionRegion::StatisticsReview.id(),
            FeatureId::RulesListEditor => InteractionRegion::RuleListEditor.id(),
            FeatureId::ProfilesFilterEditor => InteractionRegion::FilterTransaction.id(),
            FeatureId::DnsFakeIpFlushConfirm => InteractionRegion::CacheDialog.id(),
            FeatureId::DoctorFailureRecovery => InteractionRegion::DoctorReport.id(),
            FeatureId::SettingsLanguageChoice => InteractionRegion::LanguageChoices.id(),
            FeatureId::ProfilesSnapshotRestoreConfirm => InteractionRegion::RestoreReview.id(),
            FeatureId::ShellProxyModeControl | FeatureId::ShellProxyModeAuthentication => {
                InteractionRegion::ModeFailure.id()
            }
            FeatureId::ProxiesCustomNodeModal | FeatureId::ProxiesUriImportPreview => {
                InteractionRegion::Protocol.id()
            }
            FeatureId::SpeedtestDetailsModal => InteractionRegion::Speedtest.id(),
            FeatureId::ShellCommandPalette => InteractionRegion::Palette.id(),
            _ => Id::new("page-content"),
        };
        let mut probe = VisibleRegion::new(
            target.clone(),
            Size::new(self.shell.viewport.width_px, self.shell.viewport.height_px),
        );
        if feature == FeatureId::RulesTracerDrawer {
            probe = probe.revealing(Id::new(TRACER_SCROLL_ID));
        }
        if feature == FeatureId::RuntimeTelemetryObservation {
            probe = probe
                .revealing(Id::new("runtime-scroll"))
                .requiring(InteractionRegion::TrafficRates.id())
                .requiring(InteractionRegion::TrafficFailure.id());
        }
        if matches!(
            feature,
            FeatureId::ConnectionsGrouping | FeatureId::ConnectionsSearchHighlight
        ) {
            probe = probe
                .revealing(Id::new("runtime-scroll"))
                .requiring_peer(InteractionRegion::ConnectionGroupingControls.id())
                .requiring_peer(InteractionRegion::ConnectionSearch.id());
        }
        if feature == FeatureId::ConnectionsSearchHighlight {
            probe = probe.requiring(highlight_id("group-0"));
        }
        if feature == FeatureId::LogsScrollLock {
            probe = probe
                .revealing(Id::new("runtime-scroll"))
                .requiring_peer(InteractionRegion::LogFollow.id());
        }
        if feature == FeatureId::LogsRedactedExport {
            probe = probe
                .requiring(InteractionRegion::LogExportStatus.id())
                .requiring(InteractionRegion::LogExportDetails.id())
                .requiring(InteractionRegion::LogExportRetry.id())
                .requiring(InteractionRegion::LogExportCancel.id());
        }
        if feature == FeatureId::LogsSearchHighlight {
            probe = probe
                .revealing(Id::new("runtime-scroll"))
                .requiring_peer(InteractionRegion::LogsSearch.id());
        }
        if feature == FeatureId::ProfilesFilterEditor {
            probe = probe
                .revealing(Id::new("filter-scroll"))
                .requiring(InteractionRegion::FilterAdvanced.id())
                .requiring(InteractionRegion::FilterStatus.id())
                .requiring(InteractionRegion::FilterDiscard.id())
                .requiring(InteractionRegion::FilterSave.id());
        }
        if feature == FeatureId::RulesListEditor {
            let Some(row_id) = self.editor.rule_list.row_id(0) else {
                return Task::none();
            };
            probe = probe
                .requiring(InteractionRegion::RuleListStatus.id())
                .requiring(InteractionRegion::RuleListPreview.id())
                .requiring(InteractionRegion::RuleListSave.id())
                .requiring(InteractionRegion::RuleListDiscard.id())
                .requiring_peer(row_region_id(row_id));
        }
        if feature == FeatureId::RulesStatisticsInspector {
            probe = probe
                .requiring(InteractionRegion::ConfirmationAccept.id())
                .requiring(InteractionRegion::ConfirmationCancel.id());
        }
        if feature == FeatureId::RulesOverrideEditor {
            probe = probe
                .requiring(InteractionRegion::ConfirmationDetails.id())
                .requiring(InteractionRegion::ConfirmationAccept.id())
                .requiring(InteractionRegion::ConfirmationCancel.id());
        }
        if feature == FeatureId::ProxiesSearchHighlight {
            probe = probe.requiring_peer(search_region_id());
        }
        if feature == FeatureId::DnsFakeIpFlushConfirm {
            probe = probe
                .requiring(InteractionRegion::CacheConfirm.id())
                .requiring(InteractionRegion::CacheCancel.id());
        }
        if feature == FeatureId::DnsQueryDetails {
            probe = probe
                .requiring(InteractionRegion::DnsQueryTabs.id())
                .requiring(InteractionRegion::DnsQueryName.id())
                .requiring(InteractionRegion::DnsQueryRecords.id())
                .requiring(InteractionRegion::DnsQueryRun.id())
                .requiring(InteractionRegion::DnsQueryCancel.id());
        }
        if feature == FeatureId::DnsHostsEditor {
            probe = probe
                .requiring(InteractionRegion::HostsAddress.id())
                .requiring(InteractionRegion::HostsDomain.id())
                .requiring(InteractionRegion::HostsRows.id())
                .requiring(InteractionRegion::HostsApply.id());
        }
        if feature == FeatureId::DnsLeakAlert {
            probe = probe.requiring(InteractionRegion::DnsLeakRun.id());
        }
        if feature == FeatureId::DoctorFailureRecovery {
            probe = probe.requiring(InteractionRegion::DoctorRetry.id());
        }
        if feature == FeatureId::ProfilesSnapshotRestoreConfirm {
            probe = probe
                .requiring(InteractionRegion::RestoreDetails.id())
                .requiring(InteractionRegion::RestoreConfirm.id())
                .requiring(InteractionRegion::RestoreCancel.id());
        }
        if feature == FeatureId::SettingsLanguageChoice {
            probe = probe.requiring_peer(InteractionRegion::TunPermissionAction.id());
        }
        if matches!(
            feature,
            FeatureId::ShellProxyModeControl | FeatureId::ShellProxyModeAuthentication
        ) {
            probe = probe
                .requiring(InteractionRegion::ModeRetry.id())
                .requiring(InteractionRegion::ModeDismiss.id());
        }
        if feature == FeatureId::ShellProxyModeAuthentication {
            probe = probe.requiring(InteractionRegion::ModeSettings.id());
        }
        if feature == FeatureId::ProxiesGroupReorder {
            probe = probe.requiring(InteractionRegion::ProxyGroupOrderList.id());
        }
        if feature == FeatureId::ProxiesProbeSettings {
            probe = probe.requiring(InteractionRegion::ProxyProbeTimeout.id());
        }
        if feature == FeatureId::ProxiesNodeDetailDrawer {
            probe = probe.requiring(InteractionRegion::ProxyInspectionHistory.id());
        }
        operate(probe).map(move |bounds| {
            let bounds = bounds.map(|rect| {
                if target == Id::new("page-content") {
                    Rectangle {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width.min(480.0),
                        height: rect.height.min(260.0),
                    }
                } else {
                    rect
                }
            });
            Message::CaptureRegionMeasured(bounds)
        })
    }
}

#[cfg(test)]
#[path = "../../tests/gui/capture_geometry_tests.rs"]
mod tests;

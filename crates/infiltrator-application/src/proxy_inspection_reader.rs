//! Retain the last honest detail through read failures; only successful facts prove deletion.
use crate::proxy_inspection_projection::lookup_inspection;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::proxy_inspection::ProxyInspectionSnapshot;
use infiltrator_contract::surface_snapshot::{PageData, PageStatus, ProxiesPageSnapshot};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProxyInspectionReadState {
    pub detail: Option<ProxyInspectionSnapshot>,
    pub failure: Option<Failure>,
    pub loading: bool,
}
impl ProxyInspectionReadState {
    /// `true` means a successful complete page proves the selected identity is absent.
    pub fn observe(
        &mut self,
        selected: Option<&str>,
        page: &PageData<ProxiesPageSnapshot>,
    ) -> bool {
        let Some(name) = selected else {
            *self = Self::default();
            return false;
        };
        if self
            .detail
            .as_ref()
            .is_some_and(|detail| detail.name != name)
        {
            self.detail = None;
        }
        self.loading = false;
        match &page.status {
            PageStatus::Ready | PageStatus::Empty => {
                let Some(data) = &page.data else {
                    self.failure = Some(Failure::new(
                        ErrorCode::Internal,
                        "successful proxy page has no observation data",
                        true,
                    ));
                    return false;
                };
                self.failure = None;
                self.detail = lookup_inspection(data, name);
                self.detail.is_none()
            }
            PageStatus::Failed { failure } | PageStatus::Unavailable { failure } => {
                self.failure = Some(failure.clone());
                false
            }
            PageStatus::Loading => {
                self.loading = true;
                self.failure = None;
                false
            }
        }
    }
    pub fn can_probe(&self) -> bool {
        !self.loading
            && self.failure.is_none()
            && self.detail.as_ref().is_some_and(|detail| detail.can_probe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proxy_inspection_fixtures::{INSPECTION_NODE, observed_proxy};
    use crate::proxy_inspection_projection::project_proxy_inspection;
    #[test]
    fn failed_loading_and_unavailable_reads_retain_facts_and_only_successful_absence_closes() {
        let detail = project_proxy_inspection(INSPECTION_NODE, &observed_proxy());
        let mut page = ProxiesPageSnapshot {
            node_details: vec![detail.clone()],
            name_runs: Default::default(),
            search_query: String::new(),
            groups: vec![],
            testing: false,
            active_exit: String::new(),
            filter_alive: Default::default(),
            sort_order: Default::default(),
            compact_view: false,
            custom_node: Default::default(),
        };
        let mut state = ProxyInspectionReadState::default();
        assert!(!state.observe(Some(INSPECTION_NODE), &PageData::ready(page.clone())));
        assert!(state.can_probe());
        let failure = Failure::new(ErrorCode::Network, "controller read failed", true);
        for bad in [
            PageData::failed(failure.clone()),
            PageData::unavailable(failure.clone()),
            PageData::loading(),
        ] {
            assert!(!state.observe(Some(INSPECTION_NODE), &bad));
            assert_eq!(state.detail, Some(detail.clone()));
            assert!(!state.can_probe());
        }
        assert!(!state.observe(Some(INSPECTION_NODE), &PageData::ready(page.clone())));
        assert!(state.can_probe());
        page.node_details.clear();
        assert!(state.observe(Some(INSPECTION_NODE), &PageData::empty(page)));
        assert_eq!(state.detail, None);
        assert!(!state.can_probe());
    }
    #[test]
    fn failed_reads_cannot_reassign_cached_facts_to_a_different_identity_or_invent_deletion() {
        let mut state = ProxyInspectionReadState {
            detail: Some(project_proxy_inspection(INSPECTION_NODE, &observed_proxy())),
            ..Default::default()
        };
        let failure = Failure::new(ErrorCode::Permission, "read denied", false);
        assert!(!state.observe(Some("new identity"), &PageData::failed(failure)));
        assert_eq!(state.detail, None);
        assert!(!state.observe(
            Some("new identity"),
            &PageData {
                status: PageStatus::Ready,
                data: None
            }
        ));
        assert_eq!(state.failure.unwrap().code, ErrorCode::Internal);
    }
}

//! Native selection uses a stable proxy name and the shared detail fold.
use crate::state::AppState;
use infiltrator_application::proxy_inspection_projection::{
    lookup_inspection, project_proxy_inspection, with_egress_record,
};
use infiltrator_contract::proxy_inspection::ProxyInspectionSnapshot;
use infiltrator_contract::surface_snapshot::PageStatus;

impl AppState {
    pub fn proxy_inspection(&self, name: &str) -> Option<ProxyInspectionSnapshot> {
        if let Some(snapshot) = self.surface.latest() {
            let page = &snapshot.pages.proxies;
            if matches!(page.status, PageStatus::Ready | PageStatus::Empty) {
                return page
                    .data
                    .as_ref()
                    .and_then(|data| lookup_inspection(data, name));
            }
            return self
                .runtime
                .inspection_read
                .detail
                .as_ref()
                .filter(|detail| detail.name == name)
                .cloned();
        }
        self.runtime.proxies.get(name).map(|proxy| {
            let mut detail = project_proxy_inspection(name, proxy);
            with_egress_record(&mut detail, &self.diag.speedtest);
            detail
        })
    }
    pub(crate) fn reconcile_proxy_inspection(&mut self) {
        let selected = self.runtime.inspecting_proxy.as_deref();
        let deleted = if let Some(snapshot) = self.surface.latest() {
            self.runtime
                .inspection_read
                .observe(selected, &snapshot.pages.proxies)
        } else {
            selected.is_some_and(|name| !self.runtime.proxies.contains_key(name))
        };
        if deleted {
            self.runtime.inspecting_proxy = None;
            self.runtime.inspection_probe.dismiss();
            self.runtime.inspection_read = Default::default();
        }
    }
}

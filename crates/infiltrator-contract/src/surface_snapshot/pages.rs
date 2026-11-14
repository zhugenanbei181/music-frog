//! The page collection of the shared surface read model.
//!
//! `SurfacePages` is the one place that lists every page the surfaces can
//! read, so a page that exists in the catalogue and a page the snapshot
//! publishes cannot drift apart. It lives beside the snapshot instead of
//! inside it, and the snapshot owns it by name.

use super::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SurfacePages {
    pub overview: PageData<OverviewPageSnapshot>,
    pub profiles: PageData<ProfilesPageSnapshot>,
    pub rules: PageData<RulesPageSnapshot>,
    pub connections: PageData<ConnectionsPageSnapshot>,
    pub logs: PageData<LogsPageSnapshot>,
    pub dns: PageData<DnsPageSnapshot>,
    pub doctor: PageData<DoctorPageSnapshot>,
    pub app_routing: PageData<AppRoutingPageSnapshot>,
    pub sync: PageData<SyncPageSnapshot>,
    pub settings: PageData<SettingsPageSnapshot>,
}

impl SurfacePages {
    pub fn unavailable(failure: Failure) -> Self {
        Self {
            overview: PageData::unavailable(failure.clone()),
            profiles: PageData::unavailable(failure.clone()),
            rules: PageData::unavailable(failure.clone()),
            connections: PageData::unavailable(failure.clone()),
            logs: PageData::unavailable(failure.clone()),
            dns: PageData::unavailable(failure.clone()),
            doctor: PageData::unavailable(failure.clone()),
            app_routing: PageData::unavailable(failure.clone()),
            sync: PageData::unavailable(failure.clone()),
            settings: PageData::unavailable(failure),
        }
    }

    pub(super) fn status(&self, page: PageId) -> Option<&PageStatus> {
        match page {
            PageId::Overview => Some(&self.overview.status),
            PageId::Proxies
            | PageId::Tools
            | PageId::About
            | PageId::Disclaimer
            | PageId::Licenses => None,
            PageId::Profiles => Some(&self.profiles.status),
            PageId::Rules => Some(&self.rules.status),
            PageId::Connections => Some(&self.connections.status),
            PageId::Logs => Some(&self.logs.status),
            PageId::Dns => Some(&self.dns.status),
            PageId::Doctor => Some(&self.doctor.status),
            PageId::AppRouting => Some(&self.app_routing.status),
            PageId::Sync => Some(&self.sync.status),
            PageId::Settings => Some(&self.settings.status),
        }
    }
}

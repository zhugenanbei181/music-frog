//! Fold the observed subset once; unavailable, unsupported and no matches differ.
use infiltrator_contract::dns::{FakeIpMappingEntry, FakeIpMappingPool, FakeIpMappingSource};
use infiltrator_shared::i18n_interpolator::localize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DnsMappingDisplay {
    pub source: String,
    pub count: String,
    pub rows: Vec<FakeIpMappingEntry>,
    pub empty: String,
}
impl DnsMappingDisplay {
    pub fn listing(&self) -> String {
        if self.rows.is_empty() {
            return self.empty.clone();
        }
        self.rows
            .iter()
            .map(|row| format!("{} ↔ {}", row.address, row.domain))
            .collect::<Vec<_>>()
            .join("\n")
    }
}
pub fn project_mappings(pool: &FakeIpMappingPool, query: &str, code: &str) -> DnsMappingDisplay {
    let (source, rows, empty) = match &pool.source {
        FakeIpMappingSource::LiveConnections => (
            localize(code, "dns_fakeip_pool_observed", &[]),
            pool.filter(query).into_iter().cloned().collect(),
            localize(
                code,
                if pool.entries.is_empty() {
                    "dns_fakeip_pool_empty"
                } else {
                    "dns_fakeip_pool_no_matches"
                },
                &[],
            ),
        ),
        FakeIpMappingSource::Unsupported { reason } => {
            let source = localize(
                code,
                "dns_fakeip_pool_unsupported_detail",
                &[("reason", reason.clone())],
            );
            (source.clone(), Vec::new(), source)
        }
        FakeIpMappingSource::Unavailable { reason } => {
            let source = localize(
                code,
                "dns_fakeip_pool_unavailable",
                &[("reason", reason.clone())],
            );
            (source.clone(), Vec::new(), source)
        }
    };
    let count = if pool.is_observed_subset() {
        localize(
            code,
            "dns_fakeip_pool_count_range",
            &[
                ("shown", rows.len().to_string()),
                ("total", pool.total.to_string()),
                ("range", pool.range.clone()),
            ],
        )
    } else {
        source.clone()
    };
    DnsMappingDisplay {
        source,
        count,
        rows,
        empty,
    }
}

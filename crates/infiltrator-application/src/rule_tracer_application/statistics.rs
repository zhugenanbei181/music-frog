//! Source-bound local trace statistics, copied once for each reader projection.
use super::RuleTracerApplication;
use infiltrator_contract::error::{ErrorCode, Failure};
use infiltrator_contract::rule_hit_audit::{
    RuleDeadEntry, RuleDeadReason, RuleHitAuditSnapshot, RuleHitSummary,
};
use infiltrator_contract::rule_source::RuleSourceIdentity;
use infiltrator_contract::rule_statistics::RuleStatisticsResetReceipt;
use infiltrator_domain::rules::RuleEntry;
use infiltrator_domain::rules::analyzer::{ShadowReason, find_shadowed_rules};
use infiltrator_domain::rules::tracer::RuleTraceMatch;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, Default)]
pub(super) struct TraceStatistics {
    source: Option<RuleSourceIdentity>,
    revision: u64,
    rows: HashMap<usize, RuleHitSummary>,
    trace_count: u64,
    total_latency_us: u64,
    last_latency_us: Option<u64>,
}

impl TraceStatistics {
    pub(super) fn bind(&mut self, source: &RuleSourceIdentity) {
        if self.source.as_ref() != Some(source) {
            *self = Self {
                revision: self
                    .revision
                    .checked_add(1)
                    .expect("statistics revision exhausted"),
                source: Some(source.clone()),
                ..Self::default()
            };
        }
    }

    pub(super) fn record_trace(&mut self, matched: Option<&RuleTraceMatch>, latency_us: u64) {
        self.revision = self
            .revision
            .checked_add(1)
            .expect("statistics revision exhausted");
        self.trace_count = self
            .trace_count
            .checked_add(1)
            .expect("trace count exhausted");
        self.total_latency_us = self
            .total_latency_us
            .checked_add(latency_us)
            .expect("trace latency exhausted");
        self.last_latency_us = Some(latency_us);
        if let Some(matched) = matched {
            if let Some(root) = matched
                .path
                .iter()
                .find(|entry| entry.location.table.is_none())
            {
                self.record(root.location.index, &root.raw, None);
            } else if matched.location.table.is_none() {
                self.record(matched.location.index, &matched.raw, None);
            }
        }
    }

    fn record(&mut self, index: usize, raw: &str, payload_bytes: Option<u64>) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_secs());
        match self.rows.get_mut(&index) {
            Some(row) => {
                assert_eq!(
                    row.rule_raw, raw,
                    "source identity must fix a row's definition"
                );
                row.hit_count = row
                    .hit_count
                    .checked_add(1)
                    .expect("rule hit count exhausted");
                row.total_payload_bytes =
                    row.total_payload_bytes
                        .zip(payload_bytes)
                        .map(|(old, added)| {
                            old.checked_add(added)
                                .expect("rule payload count exhausted")
                        });
                row.last_hit_secs = timestamp.or(row.last_hit_secs);
            }
            None => {
                self.rows.insert(
                    index,
                    RuleHitSummary {
                        rule_index: Some(index),
                        rule_raw: raw.into(),
                        hit_count: 1,
                        total_payload_bytes: payload_bytes,
                        last_hit_secs: timestamp,
                    },
                );
            }
        }
    }
}

pub(crate) struct RuleStatisticsReadout {
    pub(crate) audit: RuleHitAuditSnapshot,
    rows: HashMap<usize, RuleHitSummary>,
}
impl RuleStatisticsReadout {
    pub(crate) fn row_count(&self, index: usize) -> u64 {
        self.rows.get(&index).map_or(0, |row| row.hit_count)
    }
    pub(crate) fn row_timestamp(&self, index: usize) -> Option<u64> {
        self.rows.get(&index).and_then(|row| row.last_hit_secs)
    }
}

impl RuleTracerApplication {
    pub(crate) fn statistics_for(
        &self,
        source: &RuleSourceIdentity,
        rules: &[RuleEntry],
    ) -> Option<RuleStatisticsReadout> {
        let statistics = self
            .statistics
            .lock()
            .expect("rule statistics lock")
            .clone();
        (statistics.source.as_ref() == Some(source)).then(|| RuleStatisticsReadout {
            audit: audit(&statistics, rules),
            rows: statistics.rows,
        })
    }

    /// Reset local hit counters while retaining source and actual latency observations.
    pub async fn clear_hits(
        &self,
        expected_source: &RuleSourceIdentity,
    ) -> Result<RuleStatisticsResetReceipt, Failure> {
        let _admission = self.admission.try_lock().ok_or_else(|| {
            Failure::new(ErrorCode::NotReady, "A rule simulation is running", true)
        })?;
        let port = self
            .override_port
            .get()
            .ok_or_else(|| Failure::unsupported("No profile rule source is composed"))?;
        let workspace = port.load_rule_workspace().await.map_err(Failure::from)?;
        let mut statistics = self.statistics.lock().expect("rule statistics lock");
        if &workspace.source != expected_source
            || statistics.source.as_ref() != Some(expected_source)
        {
            return Err(Failure::new(
                ErrorCode::NotReady,
                "The rule statistics source changed; refresh the rules page",
                false,
            ));
        }
        let receipt = RuleStatisticsResetReceipt {
            source: expected_source.clone(),
            revision: statistics
                .revision
                .checked_add(1)
                .expect("statistics revision exhausted"),
            removed_hits: statistics.rows.values().map(|row| row.hit_count).sum(),
            removed_rows: statistics.rows.len(),
        };
        statistics.rows.clear();
        statistics.revision = receipt.revision;
        Ok(receipt)
    }

    #[cfg(test)]
    pub(super) fn record_hits(&self, rules: &[RuleEntry], hits: &[(&str, u64)]) {
        let mut statistics = self.statistics.lock().expect("rule statistics lock");
        for &(raw, bytes) in hits {
            let index = rules
                .iter()
                .position(|rule| rule.rule == raw)
                .expect("fixture hit belongs to a real row");
            statistics.record(index, raw, Some(bytes));
        }
    }
    #[cfg(test)]
    pub(super) fn audit(&self, rules: &[RuleEntry]) -> RuleHitAuditSnapshot {
        audit(
            &self.statistics.lock().expect("rule statistics lock"),
            rules,
        )
    }
    #[cfg(test)]
    pub(super) fn hit_count_for(&self, raw: &str) -> u64 {
        self.statistics
            .lock()
            .expect("rule statistics lock")
            .rows
            .values()
            .filter(|row| row.rule_raw == raw)
            .map(|row| row.hit_count)
            .sum()
    }
}

fn audit(statistics: &TraceStatistics, rules: &[RuleEntry]) -> RuleHitAuditSnapshot {
    let mut top: Vec<_> = statistics.rows.iter().collect();
    top.sort_by(|(left_index, left), (right_index, right)| {
        right
            .hit_count
            .cmp(&left.hit_count)
            .then_with(|| left.rule_raw.cmp(&right.rule_raw))
            .then_with(|| left_index.cmp(right_index))
    });
    let last_hit = top
        .iter()
        .filter(|(_, row)| row.last_hit_secs.is_some())
        .max_by_key(|(_, row)| row.last_hit_secs);
    let last_hit_secs = last_hit.and_then(|(_, row)| row.last_hit_secs);
    let last_hit_rule = top
        .iter()
        .find(|(_, row)| row.last_hit_secs == last_hit_secs && last_hit_secs.is_some())
        .map(|(_, row)| row.rule_raw.clone());
    let warnings = find_shadowed_rules(rules);
    let shadow: HashMap<_, _> = warnings
        .iter()
        .map(|warning| (warning.index, warning))
        .collect();
    let mut dead_rules = Vec::new();
    let mut cidr_overlaps = Vec::new();
    for (index, rule) in rules.iter().enumerate() {
        let observed = statistics.rows.get(&index);
        let hit_count = observed.map_or(0, |row| row.hit_count);
        let last_hit_secs = observed.and_then(|row| row.last_hit_secs);
        if let Some(warning) = shadow.get(&index) {
            let entry = RuleDeadEntry {
                rule_index: Some(index),
                rule_raw: rule.rule.clone(),
                hit_count,
                reason: RuleDeadReason::Shadowed,
                shadowed_by: Some(warning.shadowed_by_rule.clone()),
                detail: Some(warning.reason.to_string()),
                last_hit_secs,
            };
            if matches!(warning.reason, ShadowReason::IpCidrShadowedByCidr) {
                cidr_overlaps.push(entry.clone());
            }
            dead_rules.push(entry);
        } else if hit_count == 0 && rule.enabled {
            dead_rules.push(RuleDeadEntry {
                rule_index: Some(index),
                rule_raw: rule.rule.clone(),
                hit_count,
                reason: RuleDeadReason::ZeroHits,
                shadowed_by: None,
                detail: None,
                last_hit_secs,
            });
        }
    }
    let total_hits = statistics.rows.values().map(|row| row.hit_count).sum();
    RuleHitAuditSnapshot {
        revision: statistics.revision,
        source: statistics.source.clone(),
        total_hits,
        tracked_rules: statistics.rows.len(),
        top_hits: top
            .into_iter()
            .take(20)
            .map(|(_, row)| row.clone())
            .collect(),
        dead_rules,
        cidr_overlaps,
        last_hit_rule,
        last_hit_secs,
        can_clear: total_hits > 0,
        trace_count: statistics.trace_count,
        avg_match_latency_us: (statistics.trace_count > 0)
            .then(|| statistics.total_latency_us as f64 / statistics.trace_count as f64),
        last_match_latency_us: statistics.last_latency_us,
    }
}

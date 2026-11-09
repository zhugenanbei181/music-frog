//! Resolve an owner-published update instant into localized relative copy.
//!
//! The Core session carries the owner's wall-clock start and the elapsed time
//! it measured at readback. Adding them yields the owner's observation instant;
//! no surface reads `SystemTime::now()` and no UI derives a second clock.
//! RFC3339 parsing and legacy fixture formats stay in this presentation layer
//! because `infiltrator-contract` has no chrono dependency.

use chrono::{DateTime, NaiveDateTime};
use infiltrator_contract::relative_time::{
    RelativeTimeBucket, RelativeTimeObservation, RelativeTimePresentation,
};
use infiltrator_contract::snapshot::CoreSessionObservation;
use lurker_ui_presentation::copy::{TextCatalog, interpolate};

/// The owner's observation instant, derived from its own session facts.
pub fn owner_observed_at_epoch_ms(session: &CoreSessionObservation) -> Option<u64> {
    match session {
        CoreSessionObservation::Active {
            started_at_epoch_ms,
            elapsed_ms,
        } => Some(started_at_epoch_ms.saturating_add(*elapsed_ms)),
        CoreSessionObservation::Unknown | CoreSessionObservation::Inactive => None,
    }
}

/// Parse an owner-published update timestamp into epoch milliseconds.
pub fn parse_updated_at_epoch_ms(updated_at: &str) -> Option<u64> {
    if updated_at.trim().is_empty() {
        return None;
    }
    if let Ok(timestamp) = DateTime::parse_from_rfc3339(updated_at) {
        return timestamp.timestamp_millis().try_into().ok();
    }
    let legacy = NaiveDateTime::parse_from_str(updated_at, "%Y-%m-%d %H:%M:%S")
        .or_else(|_| NaiveDateTime::parse_from_str(updated_at, "%Y-%m-%d %H:%M"))
        .ok()?;
    legacy.and_utc().timestamp_millis().try_into().ok()
}

/// Resolve an owner-published update instant into one typed age fact.
///
/// A missing instant, an unknown owner clock or an unparseable legacy string
/// all produce the same explicit `Unobserved` variant. No caller needs a
/// second `Option` or a private string fallback.
pub fn age_of(updated_at: &str, session: &CoreSessionObservation) -> ProfileUpdateAge {
    let Some(updated_at_epoch_ms) = parse_updated_at_epoch_ms(updated_at) else {
        return ProfileUpdateAge::Unobserved;
    };
    let Some(observed_at_epoch_ms) = owner_observed_at_epoch_ms(session) else {
        return ProfileUpdateAge::Unobserved;
    };
    ProfileUpdateAge::Observed {
        bucket: RelativeTimeObservation::new(updated_at_epoch_ms, observed_at_epoch_ms).bucket(),
    }
}

/// One typed profile update-age fact, shared by every dashboard surface.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProfileUpdateAge {
    Observed { bucket: RelativeTimeBucket },
    Unobserved,
}

impl ProfileUpdateAge {
    pub fn copy(self, presentation: RelativeTimePresentation, catalog: &dyn TextCatalog) -> String {
        let ProfileUpdateAge::Observed { bucket } = self else {
            return String::new();
        };
        let age = match bucket.count() {
            Some(count) => interpolate(
                catalog.text(presentation.template_key(bucket)).as_ref(),
                &[("count", &count.to_string())],
            ),
            None => catalog.text(presentation.template_key(bucket)).into_owned(),
        };
        match presentation {
            RelativeTimePresentation::Detailed => interpolate(
                catalog.text("relative_time_detailed_prefix").as_ref(),
                &[("time", &age)],
            ),
            RelativeTimePresentation::Compact => age,
        }
    }
}

/// Resolve a typed relative-time bucket from two owner facts.
pub fn bucket_of(updated_at: &str, session: &CoreSessionObservation) -> Option<RelativeTimeBucket> {
    match age_of(updated_at, session) {
        ProfileUpdateAge::Observed { bucket } => Some(bucket),
        ProfileUpdateAge::Unobserved => None,
    }
}

/// Localize one bucket through the shared text catalog.
pub fn copy_of(
    age: ProfileUpdateAge,
    presentation: RelativeTimePresentation,
    catalog: &dyn TextCatalog,
) -> String {
    age.copy(presentation, catalog)
}

#[cfg(test)]
mod tests {
    use super::*;
    use infiltrator_shared::locales::Lang;

    const MINUTE_MS: u64 = 60_000;

    #[test]
    fn owner_session_is_the_only_clock() {
        let session = CoreSessionObservation::Active {
            started_at_epoch_ms: 1_000_000,
            elapsed_ms: 5 * MINUTE_MS,
        };
        assert_eq!(
            owner_observed_at_epoch_ms(&session),
            Some(1_000_000 + 5 * MINUTE_MS)
        );
        assert_eq!(
            owner_observed_at_epoch_ms(&CoreSessionObservation::Unknown),
            None
        );
    }

    #[test]
    fn rfc3339_and_demo_local_timestamps_are_accepted() {
        assert_eq!(
            parse_updated_at_epoch_ms("1970-01-01T00:00:01Z"),
            Some(1000)
        );
        assert!(parse_updated_at_epoch_ms("2026-10-16 12:00:00").is_some());
        assert_eq!(
            age_of(
                "2023-11-14T07:48:09Z",
                &CoreSessionObservation::Active {
                    started_at_epoch_ms: 1_700_000_000_000,
                    elapsed_ms: 11_089_000,
                }
            ),
            ProfileUpdateAge::Observed {
                bucket: RelativeTimeBucket::Hours { count: 17 }
            }
        );
        assert_eq!(parse_updated_at_epoch_ms(""), None);
        assert_eq!(parse_updated_at_epoch_ms("not a time"), None);
    }

    #[test]
    fn a_bucket_resolves_through_both_product_catalogs() {
        let session = CoreSessionObservation::Active {
            started_at_epoch_ms: 10 * MINUTE_MS,
            elapsed_ms: 0,
        };
        let age = age_of("1970-01-01T00:05:00Z", &session);
        assert_eq!(
            age,
            ProfileUpdateAge::Observed {
                bucket: RelativeTimeBucket::Minutes { count: 5 }
            }
        );
        assert_eq!(
            copy_of(age, RelativeTimePresentation::Detailed, &Lang("zh-CN")),
            "更新于 5 分钟前"
        );
        assert_eq!(
            copy_of(age, RelativeTimePresentation::Compact, &Lang("zh-CN")),
            "5 分"
        );
        assert_eq!(
            copy_of(age, RelativeTimePresentation::Detailed, &Lang("en-US")),
            "Updated 5 minutes ago"
        );
        assert_eq!(
            copy_of(age, RelativeTimePresentation::Compact, &Lang("en-US")),
            "5m"
        );
    }

    #[test]
    fn unobserved_owner_facts_are_explicit() {
        assert_eq!(
            age_of("", &CoreSessionObservation::Unknown),
            ProfileUpdateAge::Unobserved
        );
        assert!(
            copy_of(
                ProfileUpdateAge::Unobserved,
                RelativeTimePresentation::Detailed,
                &Lang("zh-CN")
            )
            .is_empty()
        );
    }
}

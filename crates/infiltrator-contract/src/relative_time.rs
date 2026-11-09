//! Typed age buckets shared by every surface that reports "last updated".
//!
//! FlClash derives a relative timestamp from the difference between a profile
//! update instant and the application's clock. This contract does not read a
//! clock: it accepts two owner-published instants and returns an exhaustive
//! bucket. Localization owns the words, presentation owns RFC3339 parsing, and
//! a UI only consumes the resolved copy.

use serde::{Deserialize, Serialize};

/// One relative-time bucket, in the same coarse units FlClash publishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelativeTimeBucket {
    /// Less than one minute old.
    JustNow,
    /// At least one minute, less than one hour.
    Minutes { count: u64 },
    /// At least one hour, less than one day.
    Hours { count: u64 },
    /// At least one day, less than 30 days.
    Days { count: u64 },
    /// At least 30 days, less than 365 days.
    Months { count: u64 },
    /// At least 365 days old.
    Years { count: u64 },
}

impl RelativeTimeBucket {
    /// FlClash's compact card keeps only the age, without the "updated at" prefix.
    pub const fn compact_key(self) -> &'static str {
        match self {
            Self::JustNow => "relative_time_compact_just_now",
            Self::Minutes { .. } => "relative_time_compact_minutes_ago",
            Self::Hours { .. } => "relative_time_compact_hours_ago",
            Self::Days { .. } => "relative_time_compact_days_ago",
            Self::Months { .. } => "relative_time_compact_months_ago",
            Self::Years { .. } => "relative_time_compact_years_ago",
        }
    }

    /// The stable localization key for this bucket.
    pub const fn key(self) -> &'static str {
        match self {
            Self::JustNow => "relative_time_just_now",
            Self::Minutes { .. } => "relative_time_minutes_ago",
            Self::Hours { .. } => "relative_time_hours_ago",
            Self::Days { .. } => "relative_time_days_ago",
            Self::Months { .. } => "relative_time_months_ago",
            Self::Years { .. } => "relative_time_years_ago",
        }
    }

    /// The number interpolated into the bucket's copy, if it has one.
    pub const fn count(self) -> Option<u64> {
        match self {
            Self::JustNow => None,
            Self::Minutes { count }
            | Self::Hours { count }
            | Self::Days { count }
            | Self::Months { count }
            | Self::Years { count } => Some(count),
        }
    }
}

/// Which locale template a surface wants for one resolved age.
///
/// FlClash's `profiles.dart` renders `profileUpdatedAt(age)` at normal density
/// and the bare age when the card is compact. This enum travels with the typed
/// bucket so every surface selects the same copy without a second heuristic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelativeTimePresentation {
    Detailed,
    Compact,
}

impl RelativeTimePresentation {
    pub const fn template_key(self, bucket: RelativeTimeBucket) -> &'static str {
        match self {
            Self::Detailed => bucket.key(),
            Self::Compact => bucket.compact_key(),
        }
    }
}

/// An owner-published instant pair: when the fact was updated and when the
/// owner sampled the readback. Both are milliseconds since the Unix epoch.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelativeTimeObservation {
    pub updated_at_epoch_ms: u64,
    pub observed_at_epoch_ms: u64,
}

impl RelativeTimeObservation {
    pub const fn new(updated_at_epoch_ms: u64, observed_at_epoch_ms: u64) -> Self {
        Self {
            updated_at_epoch_ms,
            observed_at_epoch_ms,
        }
    }

    /// Resolve the age into one typed bucket.
    ///
    /// A future update timestamp is not a negative age: the owner clocks can
    /// disagree, so the honest answer is "just now" rather than a fabricated
    /// count. The 30/365 day approximations match the peer product's buckets.
    pub const fn bucket(self) -> RelativeTimeBucket {
        const MINUTE_MS: u64 = 60_000;
        const HOUR_MS: u64 = 60 * MINUTE_MS;
        const DAY_MS: u64 = 24 * HOUR_MS;
        const MONTH_MS: u64 = 30 * DAY_MS;
        const YEAR_MS: u64 = 365 * DAY_MS;

        let elapsed_ms = self
            .observed_at_epoch_ms
            .saturating_sub(self.updated_at_epoch_ms);
        if elapsed_ms < MINUTE_MS {
            RelativeTimeBucket::JustNow
        } else if elapsed_ms < HOUR_MS {
            RelativeTimeBucket::Minutes {
                count: elapsed_ms / MINUTE_MS,
            }
        } else if elapsed_ms < DAY_MS {
            RelativeTimeBucket::Hours {
                count: elapsed_ms / HOUR_MS,
            }
        } else if elapsed_ms < MONTH_MS {
            RelativeTimeBucket::Days {
                count: elapsed_ms / DAY_MS,
            }
        } else if elapsed_ms < YEAR_MS {
            RelativeTimeBucket::Months {
                count: elapsed_ms / MONTH_MS,
            }
        } else {
            RelativeTimeBucket::Years {
                count: elapsed_ms / YEAR_MS,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bucket_boundaries_follow_the_reference() {
        const MINUTE_MS: u64 = 60_000;
        const HOUR_MS: u64 = 60 * MINUTE_MS;
        const DAY_MS: u64 = 24 * HOUR_MS;
        const MONTH_MS: u64 = 30 * DAY_MS;
        const YEAR_MS: u64 = 365 * DAY_MS;
        let cases = [
            (0, RelativeTimeBucket::JustNow),
            (MINUTE_MS, RelativeTimeBucket::Minutes { count: 1 }),
            (HOUR_MS, RelativeTimeBucket::Hours { count: 1 }),
            (DAY_MS, RelativeTimeBucket::Days { count: 1 }),
            (MONTH_MS, RelativeTimeBucket::Months { count: 1 }),
            (YEAR_MS, RelativeTimeBucket::Years { count: 1 }),
        ];
        for (elapsed_ms, expected) in cases {
            assert_eq!(
                RelativeTimeObservation::new(0, elapsed_ms).bucket(),
                expected
            );
        }
        assert_eq!(
            RelativeTimeObservation::new(10_000, 0).bucket(),
            RelativeTimeBucket::JustNow
        );
    }

    #[test]
    fn every_bucket_has_one_key_and_one_count() {
        assert_eq!(RelativeTimeBucket::JustNow.count(), None);
        assert_eq!(RelativeTimeBucket::Days { count: 3 }.count(), Some(3));
        assert_eq!(
            RelativeTimeBucket::Days { count: 3 }.key(),
            "relative_time_days_ago"
        );
    }

    #[test]
    fn compact_presentation_owns_its_copy_selection() {
        let minutes = RelativeTimeBucket::Minutes { count: 3 };
        assert_eq!(
            RelativeTimePresentation::Detailed.template_key(minutes),
            "relative_time_minutes_ago"
        );
        assert_eq!(
            RelativeTimePresentation::Compact.template_key(minutes),
            "relative_time_compact_minutes_ago"
        );
        assert_eq!(
            RelativeTimePresentation::Compact.template_key(RelativeTimeBucket::JustNow),
            "relative_time_compact_just_now"
        );
    }
}

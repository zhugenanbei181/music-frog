use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

fn pseudo_random_u64(seed_mod: u64) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(42);
    nanos.wrapping_mul(6364136223846793005).wrapping_add(seed_mod ^ 1442695040888963407)
}

fn pseudo_random_range(max: u64, seed_mod: u64) -> u64 {
    if max == 0 {
        return 0;
    }
    pseudo_random_u64(seed_mod) % (max + 1)
}

/// Specifies the jitter mode to apply to the exponential backoff interval.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum JitterMode {
    /// No jitter applied.
    None,
    /// Delay is randomized between 0 and the current interval.
    Full,
    /// Delay is half the interval plus a random value between 0 and half the interval.
    Equal,
}

/// Configuration for the exponential backoff strategy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackoffConfig {
    /// The initial delay in milliseconds.
    pub initial_interval_ms: u64,
    /// The maximum delay in milliseconds.
    pub max_interval_ms: u64,
    /// The multiplier used to increase the delay.
    pub multiplier: f64,
    /// The maximum number of retries before giving up.
    pub max_retries: Option<u32>,
    /// The type of jitter to apply.
    pub jitter: JitterMode,
}

/// An exponential backoff and jitter strategy engine.
pub struct ExponentialBackoff {
    config: BackoffConfig,
    current_attempt: u32,
    current_interval_ms: f64,
}

impl ExponentialBackoff {
    /// Creates a new exponential backoff instance with the given configuration.
    pub fn new(config: BackoffConfig) -> Self {
        let current_interval_ms = config.initial_interval_ms as f64;
        Self {
            config,
            current_attempt: 0,
            current_interval_ms,
        }
    }

    /// Creates a backoff configuration designed for fast retries.
    /// (100ms base, 5s max, 2.0x multiplier, 5 retries).
    pub fn default_fast() -> Self {
        Self::new(BackoffConfig {
            initial_interval_ms: 100,
            max_interval_ms: 5000,
            multiplier: 2.0,
            max_retries: Some(5),
            jitter: JitterMode::Full,
        })
    }

    /// Creates a backoff configuration designed for network operations.
    /// (1000ms base, 30s max, 1.5x multiplier, 10 retries).
    pub fn default_network() -> Self {
        Self::new(BackoffConfig {
            initial_interval_ms: 1000,
            max_interval_ms: 30000,
            multiplier: 1.5,
            max_retries: Some(10),
            jitter: JitterMode::Full,
        })
    }

    /// Computes the next delay in milliseconds.
    ///
    /// Bounds the delay by `max_interval_ms`, applies jitter, increments attempts,
    /// and returns `None` if `max_retries` is exceeded.
    pub fn next_delay_ms(&mut self) -> Option<u64> {
        if let Some(max_retries) = self.config.max_retries
            && self.current_attempt >= max_retries {
                return None;
            }

        let mut delay = self.current_interval_ms as u64;
        if delay > self.config.max_interval_ms {
            delay = self.config.max_interval_ms;
        }

        // Compute next interval before returning
        self.current_interval_ms *= self.config.multiplier;
        if self.current_interval_ms > self.config.max_interval_ms as f64 {
            self.current_interval_ms = self.config.max_interval_ms as f64;
        }

        self.current_attempt += 1;

        let delay_with_jitter = match self.config.jitter {
            JitterMode::None => delay,
            JitterMode::Full => pseudo_random_range(delay, self.current_attempt as u64),
            JitterMode::Equal => {
                let half = delay / 2;
                half + pseudo_random_range(half, self.current_attempt as u64)
            }
        };

        Some(delay_with_jitter)
    }

    /// Resets the retry counter and internal interval state.
    pub fn reset(&mut self) {
        self.current_attempt = 0;
        self.current_interval_ms = self.config.initial_interval_ms as f64;
    }

    /// Returns the current retry attempt count.
    pub fn current_retry_count(&self) -> u32 {
        self.current_attempt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_and_multiplier() {
        let mut backoff = ExponentialBackoff::new(BackoffConfig {
            initial_interval_ms: 100,
            max_interval_ms: 1000,
            multiplier: 2.0,
            max_retries: Some(3),
            jitter: JitterMode::None,
        });

        assert_eq!(backoff.next_delay_ms(), Some(100));
        assert_eq!(backoff.next_delay_ms(), Some(200));
        assert_eq!(backoff.next_delay_ms(), Some(400));
        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_max_interval_clamp() {
        let mut backoff = ExponentialBackoff::new(BackoffConfig {
            initial_interval_ms: 100,
            max_interval_ms: 150,
            multiplier: 2.0,
            max_retries: Some(3),
            jitter: JitterMode::None,
        });

        assert_eq!(backoff.next_delay_ms(), Some(100));
        assert_eq!(backoff.next_delay_ms(), Some(150));
        assert_eq!(backoff.next_delay_ms(), Some(150));
        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_max_retries_exceeded() {
        let mut backoff = ExponentialBackoff::default_fast();
        backoff.config.jitter = JitterMode::None;
        
        for _ in 0..5 {
            assert!(backoff.next_delay_ms().is_some());
        }
        
        assert_eq!(backoff.next_delay_ms(), None);
    }

    #[test]
    fn test_reset() {
        let mut backoff = ExponentialBackoff::default_fast();
        backoff.config.jitter = JitterMode::None;
        
        assert_eq!(backoff.next_delay_ms(), Some(100));
        assert_eq!(backoff.current_retry_count(), 1);
        
        backoff.reset();
        
        assert_eq!(backoff.current_retry_count(), 0);
        assert_eq!(backoff.next_delay_ms(), Some(100));
    }
}

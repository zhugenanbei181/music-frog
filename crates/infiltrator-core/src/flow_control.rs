use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{Semaphore, watch};
use tokio::time::timeout;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DropPolicy {
    DropOldest,
    DropNewest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PushResult {
    Ok,
    EvictedOldest,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferStats {
    pub total_pushed: u64,
    pub total_dropped: u64,
    pub evicted_oldest_count: u64,
    pub rejected_count: u64,
    pub current_len: usize,
    pub capacity: usize,
    pub is_high_watermark: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatermarkConfig {
    pub high_watermark: usize,
    pub low_watermark: usize,
}

impl WatermarkConfig {
    pub fn default_for_capacity(capacity: usize) -> Self {
        Self {
            high_watermark: (capacity * 80) / 100,
            low_watermark: (capacity * 20) / 100,
        }
    }
}

struct BufferState<T> {
    queue: VecDeque<T>,
    capacity: usize,
    policy: DropPolicy,
    watermark: WatermarkConfig,
    total_pushed: u64,
    total_dropped: u64,
    evicted_oldest_count: u64,
    rejected_count: u64,
}

/// A high-performance bounded ring buffer supporting configurable drop policies,
/// watermark alerting, non-blocking batch drains, and comprehensive runtime statistics.
pub struct BoundedRingBuffer<T> {
    buffer: Arc<Mutex<BufferState<T>>>,
}

impl<T> Clone for BoundedRingBuffer<T> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
        }
    }
}

impl<T> BoundedRingBuffer<T> {
    /// Creates a bounded ring buffer with given capacity and overflow policy.
    pub fn new(capacity: usize, policy: DropPolicy) -> Self {
        let watermark = WatermarkConfig::default_for_capacity(capacity);
        Self {
            buffer: Arc::new(Mutex::new(BufferState {
                queue: VecDeque::with_capacity(capacity),
                capacity,
                policy,
                watermark,
                total_pushed: 0,
                total_dropped: 0,
                evicted_oldest_count: 0,
                rejected_count: 0,
            })),
        }
    }

    /// Creates a bounded ring buffer with custom high/low watermark thresholds.
    pub fn with_watermarks(
        capacity: usize,
        policy: DropPolicy,
        high_watermark: usize,
        low_watermark: usize,
    ) -> Self {
        let watermark = WatermarkConfig {
            high_watermark: high_watermark.min(capacity),
            low_watermark: low_watermark.min(high_watermark),
        };
        Self {
            buffer: Arc::new(Mutex::new(BufferState {
                queue: VecDeque::with_capacity(capacity),
                capacity,
                policy,
                watermark,
                total_pushed: 0,
                total_dropped: 0,
                evicted_oldest_count: 0,
                rejected_count: 0,
            })),
        }
    }

    /// Pushes an item into the buffer. Handles eviction or rejection according to the configured drop policy.
    pub fn push(&self, item: T) -> PushResult {
        let mut state = self.buffer.lock().unwrap();
        state.total_pushed += 1;

        if state.queue.len() < state.capacity {
            state.queue.push_back(item);
            PushResult::Ok
        } else {
            match state.policy {
                DropPolicy::DropOldest => {
                    state.queue.pop_front();
                    state.queue.push_back(item);
                    state.total_dropped += 1;
                    state.evicted_oldest_count += 1;
                    PushResult::EvictedOldest
                }
                DropPolicy::DropNewest => {
                    state.total_dropped += 1;
                    state.rejected_count += 1;
                    PushResult::Rejected
                }
            }
        }
    }

    /// Attempts to push without dropping. Returns `true` if item was accepted, `false` if buffer is full.
    pub fn try_push(&self, item: T) -> bool {
        let mut state = self.buffer.lock().unwrap();
        if state.queue.len() < state.capacity {
            state.total_pushed += 1;
            state.queue.push_back(item);
            true
        } else {
            false
        }
    }

    /// Pops the front item from the queue.
    pub fn pop(&self) -> Option<T> {
        let mut state = self.buffer.lock().unwrap();
        state.queue.pop_front()
    }

    /// Pops up to `max_count` items in a single lock acquisition.
    pub fn pop_batch(&self, max_count: usize) -> Vec<T> {
        let mut state = self.buffer.lock().unwrap();
        let take_n = max_count.min(state.queue.len());
        let mut result = Vec::with_capacity(take_n);
        for _ in 0..take_n {
            if let Some(item) = state.queue.pop_front() {
                result.push(item);
            }
        }
        result
    }

    /// Drains all items from the ring buffer.
    pub fn drain_all(&self) -> Vec<T> {
        let mut state = self.buffer.lock().unwrap();
        state.queue.drain(..).collect()
    }

    /// Returns the current number of elements in the buffer.
    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().queue.len()
    }

    /// Returns `true` if the buffer contains no elements.
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().queue.is_empty()
    }

    /// Returns the total capacity of the buffer.
    pub fn capacity(&self) -> usize {
        self.buffer.lock().unwrap().capacity
    }

    /// Returns `true` if current buffer occupancy is at or above the high watermark threshold.
    pub fn is_high_watermark(&self) -> bool {
        let state = self.buffer.lock().unwrap();
        state.queue.len() >= state.watermark.high_watermark
    }

    /// Returns `true` if current buffer occupancy is at or below the low watermark threshold.
    pub fn is_low_watermark(&self) -> bool {
        let state = self.buffer.lock().unwrap();
        state.queue.len() <= state.watermark.low_watermark
    }

    /// Retrieves cumulative buffer statistics.
    pub fn stats(&self) -> BufferStats {
        let state = self.buffer.lock().unwrap();
        let is_high = state.queue.len() >= state.watermark.high_watermark;
        BufferStats {
            total_pushed: state.total_pushed,
            total_dropped: state.total_dropped,
            evicted_oldest_count: state.evicted_oldest_count,
            rejected_count: state.rejected_count,
            current_len: state.queue.len(),
            capacity: state.capacity,
            is_high_watermark: is_high,
        }
    }
}

/// Token bucket based dual-rate bandwidth limiter and flow shaper.
pub struct TokenBucketRateLimiter {
    state: Mutex<RateLimiterState>,
}

struct RateLimiterState {
    rate_bps: f64,
    burst_capacity: f64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucketRateLimiter {
    /// Creates a rate limiter with committed information rate (bytes/sec) and burst capacity (bytes).
    pub fn new(rate_bps: u64, burst_capacity: u64) -> Self {
        Self {
            state: Mutex::new(RateLimiterState {
                rate_bps: rate_bps as f64,
                burst_capacity: burst_capacity as f64,
                tokens: burst_capacity as f64,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Non-blocking token consumption check. Returns `true` if enough tokens were available and consumed.
    pub fn try_consume(&self, bytes: usize) -> bool {
        let mut state = self.state.lock().unwrap();
        Self::refill_tokens_state(&mut state);
        let needed = bytes as f64;
        if state.tokens >= needed {
            state.tokens -= needed;
            true
        } else {
            false
        }
    }

    /// Asynchronous token consumption with cooperative sleep if tokens are depleted.
    pub async fn consume(&self, bytes: usize) {
        let needed = bytes as f64;
        loop {
            let wait_duration = {
                let mut state = self.state.lock().unwrap();
                Self::refill_tokens_state(&mut state);
                if state.tokens >= needed {
                    state.tokens -= needed;
                    return;
                }
                let deficit = needed - state.tokens;
                let secs_to_wait = deficit / state.rate_bps;
                Duration::from_secs_f64(secs_to_wait.max(0.001))
            };
            tokio::time::sleep(wait_duration).await;
        }
    }

    fn refill_tokens_state(state: &mut RateLimiterState) {
        let now = Instant::now();
        let elapsed = now.duration_since(state.last_refill).as_secs_f64();
        let new_tokens = elapsed * state.rate_bps;
        state.tokens = (state.tokens + new_tokens).min(state.burst_capacity);
        state.last_refill = now;
    }

    /// Updates the committed bandwidth rate in bytes per second.
    pub fn set_rate(&self, new_rate_bps: u64) {
        let mut state = self.state.lock().unwrap();
        Self::refill_tokens_state(&mut state);
        state.rate_bps = new_rate_bps as f64;
    }
}

/// Testing error scenarios for latency and throughput probes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DelayTestError {
    Timeout,
    Network(String),
    Cancelled,
}

/// Latency test result for a specific proxy node.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelayTestResult {
    pub proxy_name: String,
    pub latency_ms: u64,
}

/// Test outcome wrapping either success or detailed error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyTestOutcome {
    pub proxy_name: String,
    pub result: Result<DelayTestResult, DelayTestError>,
}

/// Concurrency-controlled batch delay testing executor.
pub struct BatchDelayTester {
    semaphore: Arc<Semaphore>,
    test_url: String,
    timeout: Duration,
}

impl BatchDelayTester {
    /// Creates a new batch delay tester.
    pub fn new(max_concurrent: usize, test_url: String, timeout: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            test_url,
            timeout,
        }
    }

    /// Executes concurrent delay tests across all specified proxies with cancellation support.
    pub async fn test_proxies<F, Fut>(
        &self,
        proxies: Vec<String>,
        test_fn: F,
        cancel_rx: watch::Receiver<bool>,
    ) -> Vec<ProxyTestOutcome>
    where
        F: Fn(String, String) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<u64, String>> + Send + 'static,
    {
        let mut tasks = Vec::new();
        let test_fn = Arc::new(test_fn);

        for proxy in proxies {
            let semaphore = self.semaphore.clone();
            let mut cancel_rx = cancel_rx.clone();
            let timeout_dur = self.timeout;
            let proxy_name = proxy.clone();
            let test_url = self.test_url.clone();
            let test_fn = test_fn.clone();

            let task = tokio::spawn(async move {
                if *cancel_rx.borrow() {
                    return ProxyTestOutcome {
                        proxy_name,
                        result: Err(DelayTestError::Cancelled),
                    };
                }

                let permit = tokio::select! {
                    p = semaphore.acquire() => {
                        match p {
                            Ok(permit) => permit,
                            Err(_) => return ProxyTestOutcome {
                                proxy_name,
                                result: Err(DelayTestError::Cancelled),
                            },
                        }
                    }
                    _ = cancel_rx.wait_for(|c| *c) => {
                        return ProxyTestOutcome {
                            proxy_name,
                            result: Err(DelayTestError::Cancelled),
                        };
                    }
                };

                let test_fut = test_fn(proxy_name.clone(), test_url);

                let result = tokio::select! {
                    _ = cancel_rx.wait_for(|c| *c) => {
                        Err(DelayTestError::Cancelled)
                    }
                    res = timeout(timeout_dur, test_fut) => {
                        match res {
                            Ok(Ok(latency)) => Ok(DelayTestResult { proxy_name: proxy_name.clone(), latency_ms: latency }),
                            Ok(Err(e)) => Err(DelayTestError::Network(e)),
                            Err(_) => Err(DelayTestError::Timeout),
                        }
                    }
                };

                drop(permit);

                ProxyTestOutcome { proxy_name, result }
            });

            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            if let Ok(outcome) = task.await {
                results.push(outcome);
            }
        }

        results
    }
}

/// Lifecycle phases of a comprehensive speedtest run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SpeedtestPhase {
    Idle,
    LatencyProbe,
    DownloadTesting,
    UploadTesting,
    Completed,
    Failed,
}

/// Configuration parameters for full throughput and latency speedtesting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedtestConfig {
    pub test_url: String,
    pub duration: Duration,
    pub max_download_bytes: u64,
    pub max_upload_bytes: u64,
    pub stream_concurrency: usize,
}

impl Default for SpeedtestConfig {
    fn default() -> Self {
        Self {
            test_url: "http://cachefly.cachefly.net/10mb.test".to_string(),
            duration: Duration::from_secs(5),
            max_download_bytes: 50 * 1024 * 1024,
            max_upload_bytes: 10 * 1024 * 1024,
            stream_concurrency: 4,
        }
    }
}

/// Real-time progress update emitted during speed testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpeedtestProgress {
    pub phase: SpeedtestPhase,
    pub progress_percent: f64,
    pub transferred_bytes: u64,
    pub instant_mbps: f64,
    pub ewma_mbps: f64,
    pub current_rtt_ms: Option<u64>,
}

/// Final comprehensive metrics report produced by the Speedtest engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpeedtestReport {
    pub proxy_name: String,
    pub avg_rtt_ms: u64,
    pub jitter_ms: f64,
    pub download_mbps: f64,
    pub upload_mbps: f64,
    pub total_downloaded_bytes: u64,
    pub total_uploaded_bytes: u64,
    pub duration_ms: u64,
}

/// Priority-ranked proxy descriptor for scheduled speedtesting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PriorityProxy {
    pub name: String,
    pub priority: u32,
    pub retry_budget: u32,
}

impl PriorityProxy {
    pub fn new(name: impl Into<String>, priority: u32) -> Self {
        Self {
            name: name.into(),
            priority,
            retry_budget: 2,
        }
    }
}

/// Concurrency-controlled, priority-scheduled batch speedtester with exponential retry backoff.
pub struct BatchSpeedtester {
    semaphore: Arc<Semaphore>,
    config: SpeedtestConfig,
    timeout: Duration,
}

impl BatchSpeedtester {
    pub fn new(max_concurrent: usize, config: SpeedtestConfig, timeout: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            config,
            timeout,
        }
    }

    pub async fn run_batch<F, Fut>(
        &self,
        mut proxies: Vec<PriorityProxy>,
        speedtest_fn: F,
        cancel_rx: watch::Receiver<bool>,
    ) -> Vec<Result<SpeedtestReport, DelayTestError>>
    where
        F: Fn(String, SpeedtestConfig) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<SpeedtestReport, String>> + Send + 'static,
    {
        // Sort highest priority first
        proxies.sort_by(|a, b| b.priority.cmp(&a.priority));

        let speedtest_fn = Arc::new(speedtest_fn);
        let mut tasks = Vec::new();

        for p in proxies {
            let sem = self.semaphore.clone();
            let mut cancel_rx = cancel_rx.clone();
            let timeout_dur = self.timeout;
            let cfg = self.config.clone();
            let test_fn = speedtest_fn.clone();
            let proxy_name = p.name.clone();
            let retries = p.retry_budget;

            let task = tokio::spawn(async move {
                if *cancel_rx.borrow() {
                    return Err(DelayTestError::Cancelled);
                }

                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => return Err(DelayTestError::Cancelled),
                };

                let mut attempts = 0;
                let mut last_err = DelayTestError::Timeout;

                while attempts <= retries {
                    if *cancel_rx.borrow() {
                        return Err(DelayTestError::Cancelled);
                    }

                    attempts += 1;
                    let fut = test_fn(proxy_name.clone(), cfg.clone());

                    let res = tokio::select! {
                        _ = cancel_rx.wait_for(|c| *c) => Err(DelayTestError::Cancelled),
                        r = timeout(timeout_dur, fut) => match r {
                            Ok(Ok(report)) => return Ok(report),
                            Ok(Err(e)) => Err(DelayTestError::Network(e)),
                            Err(_) => Err(DelayTestError::Timeout),
                        }
                    };

                    match res {
                        Ok(rep) => return Ok(rep),
                        Err(e) => {
                            last_err = e;
                            if attempts <= retries {
                                let backoff_ms = 10 * (1 << attempts);
                                tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                            }
                        }
                    }
                }

                Err(last_err)
            });

            tasks.push(task);
        }

        let mut results = Vec::new();
        for task in tasks {
            if let Ok(res) = task.await {
                results.push(res);
            }
        }
        results
    }
}

/// Adaptive window regulator implementing AIMD backpressure flow control.
#[derive(Debug, Clone, PartialEq)]
pub struct AdaptiveWindowRegulator {
    current_window: usize,
    min_window: usize,
    max_window: usize,
    ssthresh: usize,
    consecutive_success: usize,
}

impl AdaptiveWindowRegulator {
    pub fn new(min_window: usize, max_window: usize) -> Self {
        Self {
            current_window: min_window.max(1),
            min_window: min_window.max(1),
            max_window: max_window.max(min_window),
            ssthresh: max_window / 2,
            consecutive_success: 0,
        }
    }

    pub fn current_window(&self) -> usize {
        self.current_window
    }

    /// Called on successful probe or packet delivery.
    pub fn on_success(&mut self) {
        if self.current_window < self.ssthresh {
            // Slow start: exponential increase
            self.current_window = (self.current_window * 2).min(self.max_window);
        } else {
            // Congestion avoidance: additive increase
            self.consecutive_success += 1;
            if self.consecutive_success >= self.current_window {
                self.current_window = (self.current_window + 1).min(self.max_window);
                self.consecutive_success = 0;
            }
        }
    }

    /// Called on packet loss or buffer congestion.
    pub fn on_congestion(&mut self) {
        self.ssthresh = (self.current_window / 2).max(self.min_window);
        self.current_window = self.ssthresh;
        self.consecutive_success = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_bounded_ring_buffer_drop_oldest() {
        let buf = BoundedRingBuffer::new(3, DropPolicy::DropOldest);
        assert_eq!(buf.push(1), PushResult::Ok);
        assert_eq!(buf.push(2), PushResult::Ok);
        assert_eq!(buf.push(3), PushResult::Ok);
        assert_eq!(buf.push(4), PushResult::EvictedOldest);

        let stats = buf.stats();
        assert_eq!(stats.total_pushed, 4);
        assert_eq!(stats.total_dropped, 1);
        assert_eq!(stats.evicted_oldest_count, 1);
        assert_eq!(stats.rejected_count, 0);
        assert_eq!(stats.current_len, 3);

        assert_eq!(buf.drain_all(), vec![2, 3, 4]);
    }

    #[test]
    fn test_bounded_ring_buffer_drop_newest() {
        let buf = BoundedRingBuffer::new(3, DropPolicy::DropNewest);
        assert_eq!(buf.push(1), PushResult::Ok);
        assert_eq!(buf.push(2), PushResult::Ok);
        assert_eq!(buf.push(3), PushResult::Ok);
        assert_eq!(buf.push(4), PushResult::Rejected);

        let stats = buf.stats();
        assert_eq!(stats.total_pushed, 4);
        assert_eq!(stats.total_dropped, 1);
        assert_eq!(stats.rejected_count, 1);
        assert_eq!(stats.current_len, 3);

        assert_eq!(buf.drain_all(), vec![1, 2, 3]);
    }

    #[test]
    fn test_bounded_ring_buffer_watermarks_and_batch() {
        let buf = BoundedRingBuffer::with_watermarks(10, DropPolicy::DropOldest, 8, 2);
        for i in 0..7 {
            assert!(buf.try_push(i));
        }
        assert!(!buf.is_high_watermark());

        buf.push(7); // len = 8 -> hits high watermark
        assert!(buf.is_high_watermark());

        let batch = buf.pop_batch(4);
        assert_eq!(batch, vec![0, 1, 2, 3]);
        assert_eq!(buf.len(), 4);
        assert!(!buf.is_high_watermark());

        let rest = buf.pop_batch(10);
        assert_eq!(rest, vec![4, 5, 6, 7]);
        assert!(buf.is_low_watermark());
    }

    #[test]
    fn test_bounded_ring_buffer_concurrent() {
        let buf = BoundedRingBuffer::new(1000, DropPolicy::DropNewest);
        let mut handles = vec![];

        for i in 0..10 {
            let b = buf.clone();
            handles.push(std::thread::spawn(move || {
                for j in 0..100 {
                    b.push(i * 100 + j);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(buf.len(), 1000);
        assert_eq!(buf.stats().total_pushed, 1000);
        assert_eq!(buf.stats().total_dropped, 0);
    }

    #[tokio::test]
    async fn test_token_bucket_limiter() {
        let limiter = TokenBucketRateLimiter::new(1000, 500);
        assert!(limiter.try_consume(200));
        assert!(limiter.try_consume(200));
        assert!(!limiter.try_consume(200)); // deficit (only 100 left)

        // Async consume should succeed after brief wait
        limiter.consume(150).await;
    }

    #[tokio::test]
    async fn test_adaptive_window_regulator() {
        let mut reg = AdaptiveWindowRegulator::new(2, 16);
        assert_eq!(reg.current_window(), 2);

        // Slow start
        reg.on_success();
        assert_eq!(reg.current_window(), 4);
        reg.on_success();
        assert_eq!(reg.current_window(), 8);

        // Congestion
        reg.on_congestion();
        assert_eq!(reg.current_window(), 4);
    }

    #[tokio::test]
    async fn test_batch_delay_tester_success() {
        let tester =
            BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_secs(1));
        let (_tx, rx) = watch::channel(false);

        let proxies = vec!["p1".to_string(), "p2".to_string(), "p3".to_string()];

        let results = tester
            .test_proxies(proxies, |_name, _url| async move { Ok(100) }, rx)
            .await;

        assert_eq!(results.len(), 3);
        for r in results {
            assert!(r.result.is_ok());
            assert_eq!(r.result.unwrap().latency_ms, 100);
        }
    }

    #[tokio::test]
    async fn test_batch_delay_tester_timeout() {
        let tester =
            BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_millis(100));
        let (_tx, rx) = watch::channel(false);

        let proxies = vec!["p1".to_string()];

        let results = tester
            .test_proxies(
                proxies,
                |_name, _url| async move {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    Ok(100)
                },
                rx,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Err(DelayTestError::Timeout));
    }

    #[tokio::test]
    async fn test_batch_delay_tester_cancel() {
        let tester =
            BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_secs(2));
        let (tx, rx) = watch::channel(false);

        let proxies = vec!["p1".to_string()];

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });

        let results = tester
            .test_proxies(
                proxies,
                |_name, _url| async move {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    Ok(100)
                },
                rx,
            )
            .await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Err(DelayTestError::Cancelled));
    }

    #[tokio::test]
    async fn test_batch_delay_tester_concurrency() {
        let tester =
            BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_secs(2));
        let (_tx, rx) = watch::channel(false);

        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));

        let mut proxies = Vec::new();
        for i in 0..5 {
            proxies.push(format!("p{}", i));
        }

        let active_clone = active.clone();
        let max_clone = max_active.clone();

        let results = tester
            .test_proxies(
                proxies,
                move |_name, _url| {
                    let active = active_clone.clone();
                    let max = max_clone.clone();
                    async move {
                        let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                        let mut current_max = max.load(Ordering::SeqCst);
                        while current > current_max {
                            match max.compare_exchange_weak(
                                current_max,
                                current,
                                Ordering::SeqCst,
                                Ordering::SeqCst,
                            ) {
                                Ok(_) => break,
                                Err(actual) => current_max = actual,
                            }
                        }

                        tokio::time::sleep(Duration::from_millis(100)).await;
                        active.fetch_sub(1, Ordering::SeqCst);
                        Ok(100)
                    }
                },
                rx,
            )
            .await;

        assert_eq!(results.len(), 5);
        assert_eq!(max_active.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_batch_speedtester_priority_and_retry() {
        let tester = BatchSpeedtester::new(
            2,
            SpeedtestConfig::default(),
            Duration::from_secs(1),
        );
        let (_tx, rx) = watch::channel(false);

        let proxies = vec![
            PriorityProxy::new("low-prio", 1),
            PriorityProxy::new("high-prio", 10),
        ];

        let results = tester
            .run_batch(
                proxies,
                |name, _cfg| async move {
                    Ok(SpeedtestReport {
                        proxy_name: name,
                        avg_rtt_ms: 30,
                        jitter_ms: 2.5,
                        download_mbps: 150.0,
                        upload_mbps: 50.0,
                        total_downloaded_bytes: 10_000_000,
                        total_uploaded_bytes: 3_000_000,
                        duration_ms: 1000,
                    })
                },
                rx,
            )
            .await;

        assert_eq!(results.len(), 2);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
    }
}

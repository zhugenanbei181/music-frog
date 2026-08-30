use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::{Semaphore, watch};
use tokio::time::timeout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPolicy {
    DropOldest,
    DropNewest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushResult {
    Ok,
    EvictedOldest,
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferStats {
    pub total_pushed: u64,
    pub total_dropped: u64,
    pub current_len: usize,
    pub capacity: usize,
}

struct BufferState<T> {
    queue: VecDeque<T>,
    capacity: usize,
    policy: DropPolicy,
    total_pushed: u64,
    total_dropped: u64,
}

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
    pub fn new(capacity: usize, policy: DropPolicy) -> Self {
        Self {
            buffer: Arc::new(Mutex::new(BufferState {
                queue: VecDeque::with_capacity(capacity),
                capacity,
                policy,
                total_pushed: 0,
                total_dropped: 0,
            })),
        }
    }

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
                    PushResult::EvictedOldest
                }
                DropPolicy::DropNewest => {
                    state.total_dropped += 1;
                    PushResult::Rejected
                }
            }
        }
    }

    pub fn pop(&self) -> Option<T> {
        let mut state = self.buffer.lock().unwrap();
        state.queue.pop_front()
    }

    pub fn drain_all(&self) -> Vec<T> {
        let mut state = self.buffer.lock().unwrap();
        state.queue.drain(..).collect()
    }

    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().queue.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.buffer.lock().unwrap().capacity
    }

    pub fn stats(&self) -> BufferStats {
        let state = self.buffer.lock().unwrap();
        BufferStats {
            total_pushed: state.total_pushed,
            total_dropped: state.total_dropped,
            current_len: state.queue.len(),
            capacity: state.capacity,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelayTestError {
    Timeout,
    Network(String),
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelayTestResult {
    pub proxy_name: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyTestOutcome {
    pub proxy_name: String,
    pub result: Result<DelayTestResult, DelayTestError>,
}

use std::future::Future;

pub struct BatchDelayTester {
    semaphore: Arc<Semaphore>,
    test_url: String,
    timeout: Duration,
}

impl BatchDelayTester {
    pub fn new(max_concurrent: usize, test_url: String, timeout: Duration) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
            test_url,
            timeout,
        }
    }

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
                
                ProxyTestOutcome {
                    proxy_name,
                    result,
                }
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
        assert_eq!(stats.current_len, 3);
        
        assert_eq!(buf.drain_all(), vec![1, 2, 3]);
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
    async fn test_batch_delay_tester_success() {
        let tester = BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_secs(1));
        let (_tx, rx) = watch::channel(false);
        
        let proxies = vec!["p1".to_string(), "p2".to_string(), "p3".to_string()];
        
        let results = tester.test_proxies(proxies, |_name, _url| async move {
            Ok(100)
        }, rx).await;
        
        assert_eq!(results.len(), 3);
        for r in results {
            assert!(r.result.is_ok());
            assert_eq!(r.result.unwrap().latency_ms, 100);
        }
    }

    #[tokio::test]
    async fn test_batch_delay_tester_timeout() {
        let tester = BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_millis(100));
        let (_tx, rx) = watch::channel(false);
        
        let proxies = vec!["p1".to_string()];
        
        let results = tester.test_proxies(proxies, |_name, _url| async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            Ok(100)
        }, rx).await;
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Err(DelayTestError::Timeout));
    }

    #[tokio::test]
    async fn test_batch_delay_tester_cancel() {
        let tester = BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_secs(2));
        let (tx, rx) = watch::channel(false);
        
        let proxies = vec!["p1".to_string()];
        
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let _ = tx.send(true);
        });
        
        let results = tester.test_proxies(proxies, |_name, _url| async move {
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok(100)
        }, rx).await;
        
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].result, Err(DelayTestError::Cancelled));
    }

    #[tokio::test]
    async fn test_batch_delay_tester_concurrency() {
        let tester = BatchDelayTester::new(2, "http://test.com".to_string(), Duration::from_secs(2));
        let (_tx, rx) = watch::channel(false);
        
        let active = Arc::new(AtomicUsize::new(0));
        let max_active = Arc::new(AtomicUsize::new(0));
        
        let mut proxies = Vec::new();
        for i in 0..5 {
            proxies.push(format!("p{}", i));
        }
        
        let active_clone = active.clone();
        let max_clone = max_active.clone();
        
        let results = tester.test_proxies(proxies, move |_name, _url| {
            let active = active_clone.clone();
            let max = max_clone.clone();
            async move {
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                let mut current_max = max.load(Ordering::SeqCst);
                while current > current_max {
                    match max.compare_exchange_weak(current_max, current, Ordering::SeqCst, Ordering::SeqCst) {
                        Ok(_) => break,
                        Err(actual) => current_max = actual,
                    }
                }
                
                tokio::time::sleep(Duration::from_millis(100)).await;
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(100)
            }
        }, rx).await;
        
        assert_eq!(results.len(), 5);
        assert_eq!(max_active.load(Ordering::SeqCst), 2);
    }
}

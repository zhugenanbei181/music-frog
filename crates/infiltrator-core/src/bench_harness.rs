use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Result of a benchmark execution.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BenchResult {
    /// Name of the benchmark.
    pub name: String,
    /// Number of iterations executed.
    pub iterations: u64,
    /// Total elapsed time in microseconds.
    pub total_elapsed_micros: u64,
    /// Average time per operation in microseconds.
    pub avg_micros_per_op: f64,
    /// Operations per second.
    pub ops_per_sec: f64,
}

/// Harness for executing synchronous and asynchronous benchmarks.
pub struct BenchHarness;

impl BenchHarness {
    /// Benchmarks a synchronous operation.
    pub fn bench_sync_op<F: FnMut()>(name: &str, iterations: u64, mut op: F) -> BenchResult {
        if iterations == 0 {
            return BenchResult {
                name: name.to_string(),
                iterations: 0,
                total_elapsed_micros: 0,
                avg_micros_per_op: 0.0,
                ops_per_sec: 0.0,
            };
        }

        let start = Instant::now();
        for _ in 0..iterations {
            op();
        }
        let elapsed = start.elapsed();
        let total_elapsed_micros = elapsed.as_micros() as u64;

        let avg_micros_per_op = total_elapsed_micros as f64 / iterations as f64;
        let ops_per_sec = if total_elapsed_micros > 0 {
            (iterations as f64 / total_elapsed_micros as f64) * 1_000_000.0
        } else {
            f64::INFINITY
        };

        BenchResult {
            name: name.to_string(),
            iterations,
            total_elapsed_micros,
            avg_micros_per_op,
            ops_per_sec,
        }
    }

    /// Benchmarks an asynchronous operation.
    pub async fn bench_async_op<F, Fut>(name: &str, iterations: u64, mut op: F) -> BenchResult
    where
        F: FnMut() -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        if iterations == 0 {
            return BenchResult {
                name: name.to_string(),
                iterations: 0,
                total_elapsed_micros: 0,
                avg_micros_per_op: 0.0,
                ops_per_sec: 0.0,
            };
        }

        let start = Instant::now();
        for _ in 0..iterations {
            op().await;
        }
        let elapsed = start.elapsed();
        let total_elapsed_micros = elapsed.as_micros() as u64;

        let avg_micros_per_op = total_elapsed_micros as f64 / iterations as f64;
        let ops_per_sec = if total_elapsed_micros > 0 {
            (iterations as f64 / total_elapsed_micros as f64) * 1_000_000.0
        } else {
            f64::INFINITY
        };

        BenchResult {
            name: name.to_string(),
            iterations,
            total_elapsed_micros,
            avg_micros_per_op,
            ops_per_sec,
        }
    }

    /// Formats the benchmark result into a human-readable report string.
    pub fn format_report(result: &BenchResult) -> String {
        format!(
            "Benchmark: {}\n\
             Iterations: {}\n\
             Total Time: {} µs\n\
             Avg Time/Op: {:.2} µs\n\
             Ops/Sec: {:.2}",
            result.name,
            result.iterations,
            result.total_elapsed_micros,
            result.avg_micros_per_op,
            result.ops_per_sec
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_bench() {
        let res = BenchHarness::bench_sync_op("test_sync", 100, || {
            let _ = 1 + 1;
        });
        assert_eq!(res.name, "test_sync");
        assert_eq!(res.iterations, 100);
        assert!(res.ops_per_sec > 0.0);
    }

    #[tokio::test]
    async fn test_async_bench() {
        let res = BenchHarness::bench_async_op("test_async", 100, || async {
            let _ = 1 + 1;
        }).await;
        assert_eq!(res.name, "test_async");
        assert_eq!(res.iterations, 100);
        assert!(res.ops_per_sec > 0.0);
    }

    #[test]
    fn test_zero_iterations() {
        let res = BenchHarness::bench_sync_op("test_zero", 0, || {});
        assert_eq!(res.iterations, 0);
        assert_eq!(res.total_elapsed_micros, 0);
        assert_eq!(res.avg_micros_per_op, 0.0);
        assert_eq!(res.ops_per_sec, 0.0);
    }

    #[tokio::test]
    async fn test_zero_iterations_async() {
        let res = BenchHarness::bench_async_op("test_zero", 0, || async {}).await;
        assert_eq!(res.iterations, 0);
        assert_eq!(res.total_elapsed_micros, 0);
        assert_eq!(res.avg_micros_per_op, 0.0);
        assert_eq!(res.ops_per_sec, 0.0);
    }

    #[test]
    fn test_format_report() {
        let result = BenchResult {
            name: "format_test".to_string(),
            iterations: 1000,
            total_elapsed_micros: 2000,
            avg_micros_per_op: 2.0,
            ops_per_sec: 500000.0,
        };
        let report = BenchHarness::format_report(&result);
        assert!(report.contains("Benchmark: format_test"));
        assert!(report.contains("Iterations: 1000"));
        assert!(report.contains("Total Time: 2000 µs"));
        assert!(report.contains("Avg Time/Op: 2.00 µs"));
        assert!(report.contains("Ops/Sec: 500000.00"));
    }
}

//! Simple benchmark runner to test our lock contention improvements
//!
//! This module provides a CLI interface to run benchmarks and generate reports.

use crate::lock_contention_benchmark::{
    benchmark_std_rwlock, benchmark_tokio_rwlock, benchmark_message_passing,
    BenchmarkConfig, ContentionBenchmarkResults, print_benchmark_results
};
use tracing::info;

/// Run a quick benchmark to test current performance
pub async fn run_quick_benchmark() -> Vec<ContentionBenchmarkResults> {
    info!("Running quick lock contention benchmark");
    
    let config = BenchmarkConfig {
        thread_count: 4,
        operations_per_thread: 500,
        read_write_ratio: 0.8,
        hold_duration_ms: 1,
        timeout_ms: 50,
    };
    
    let mut results = Vec::new();
    
    // Test std RwLock
    info!("Benchmarking std::RwLock");
    results.push(benchmark_std_rwlock(config.clone()).await);
    
    // Test tokio RwLock
    info!("Benchmarking tokio::RwLock");
    results.push(benchmark_tokio_rwlock(config.clone()).await);
    
    // Test message passing
    info!("Benchmarking message passing");
    results.push(benchmark_message_passing(config.clone()).await);
    
    results
}

/// Generate a summary report of benchmark results
pub fn generate_summary_report(results: &[ContentionBenchmarkResults]) -> String {
    let mut report = String::new();
    report.push_str("# Lock Contention Benchmark Summary\n\n");
    
    // Find the best performing scenario
    let best_contention = results.iter()
        .min_by(|a, b| a.contention_percentage.partial_cmp(&b.contention_percentage).unwrap());
    
    let best_throughput = results.iter()
        .max_by(|a, b| a.throughput_ops_per_sec.partial_cmp(&b.throughput_ops_per_sec).unwrap());
    
    if let Some(best) = best_contention {
        report.push_str(&format!("**Best Contention Performance:** {} ({:.2}% contention)\n", 
            best.scenario, best.contention_percentage));
    }
    
    if let Some(best) = best_throughput {
        report.push_str(&format!("**Highest Throughput:** {} ({:.0} ops/sec)\n", 
            best.scenario, best.throughput_ops_per_sec));
    }
    
    // Check goal achievement
    let low_contention_count = results.iter()
        .filter(|r| r.contention_percentage < 1.0)
        .count();
    
    report.push_str(&format!("\n**Goal Achievement:** {}/{} scenarios achieved <1% contention\n", 
        low_contention_count, results.len()));
    
    // Detailed results
    report.push_str("\n## Detailed Results\n\n");
    report.push_str("| Scenario | Threads | Ops/sec | Avg Wait (μs) | Max Wait (μs) | Contention % | Failed Ops |\n");
    report.push_str("|----------|---------|---------|---------------|---------------|--------------|------------|\n");
    
    for result in results {
        report.push_str(&format!("| {} | {} | {:.0} | {:.0} | {:.0} | {:.2} | {} |\n",
            result.scenario,
            result.thread_count,
            result.throughput_ops_per_sec,
            result.avg_lock_wait_time.as_micros(),
            result.max_lock_wait_time.as_micros(),
            result.contention_percentage,
            result.failed_operations
        ));
    }
    
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_quick_benchmark() {
        let results = run_quick_benchmark().await;
        assert_eq!(results.len(), 3);
        
        // Verify we have all three scenarios
        let scenarios: Vec<String> = results.iter().map(|r| r.scenario.clone()).collect();
        assert!(scenarios.contains(&"std::RwLock".to_string()));
        assert!(scenarios.contains(&"tokio::RwLock".to_string()));
        assert!(scenarios.contains(&"Message Passing".to_string()));
    }

    #[test]
    fn test_summary_report_generation() {
        let test_results = vec![
            ContentionBenchmarkResults {
                scenario: "Test1".to_string(),
                thread_count: 4,
                operation_count: 1000,
                total_duration: Duration::from_secs(1),
                avg_lock_wait_time: Duration::from_micros(100),
                max_lock_wait_time: Duration::from_micros(500),
                contention_percentage: 2.5,
                throughput_ops_per_sec: 800.0,
                failed_operations: 25,
            },
            ContentionBenchmarkResults {
                scenario: "Test2".to_string(),
                thread_count: 4,
                operation_count: 1000,
                total_duration: Duration::from_secs(1),
                avg_lock_wait_time: Duration::from_micros(50),
                max_lock_wait_time: Duration::from_micros(200),
                contention_percentage: 0.5,
                throughput_ops_per_sec: 950.0,
                failed_operations: 5,
            },
        ];
        
        let report = generate_summary_report(&test_results);
        assert!(report.contains("Best Contention Performance"));
        assert!(report.contains("Highest Throughput"));
        assert!(report.contains("Test2"));
    }
}
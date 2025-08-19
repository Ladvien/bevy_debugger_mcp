/// Performance Measurement Utilities
/// 
/// Tools for measuring and analyzing performance during tests,
/// validating that optimizations meet their targets.

use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde_json::{json, Value};

/// Performance measurement context
pub struct PerformanceMeasurement {
    name: String,
    start_time: Instant,
    measurements: Vec<Measurement>,
    targets: PerformanceTargets,
}

#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub duration: Duration,
    pub timestamp: Duration,
    pub metadata: HashMap<String, Value>,
}

/// Performance targets for validation
#[derive(Debug, Clone)]
pub struct PerformanceTargets {
    pub max_latency_ms: f64,
    pub max_memory_mb: f64,
    pub min_throughput_ops_per_sec: f64,
    pub max_cpu_percent: f64,
}

impl Default for PerformanceTargets {
    fn default() -> Self {
        Self {
            max_latency_ms: 100.0,  // 100ms max latency
            max_memory_mb: 50.0,    // 50MB max memory overhead
            min_throughput_ops_per_sec: 100.0, // 100 ops/sec min throughput
            max_cpu_percent: 10.0,  // 10% max CPU usage
        }
    }
}

impl PerformanceTargets {
    /// Create targets based on BEVDBG-012 requirements
    pub fn bevdbg_012_targets() -> Self {
        Self {
            max_latency_ms: 1.0,    // < 1ms p99 command processing
            max_memory_mb: 50.0,    // < 50MB memory overhead
            min_throughput_ops_per_sec: 1000.0, // High throughput
            max_cpu_percent: 3.0,   // < 3% CPU overhead
        }
    }

    /// Create relaxed targets for testing environment
    pub fn testing_targets() -> Self {
        Self {
            max_latency_ms: 10.0,   // 10ms max in testing
            max_memory_mb: 100.0,   // 100MB max in testing
            min_throughput_ops_per_sec: 50.0,  // Lower throughput acceptable
            max_cpu_percent: 15.0,  // 15% max in testing
        }
    }
}

impl PerformanceMeasurement {
    /// Create a new performance measurement context
    pub fn new(name: &str) -> Self {
        Self::with_targets(name, PerformanceTargets::default())
    }

    /// Create with specific performance targets
    pub fn with_targets(name: &str, targets: PerformanceTargets) -> Self {
        Self {
            name: name.to_string(),
            start_time: Instant::now(),
            measurements: Vec::new(),
            targets,
        }
    }

    /// Record a single measurement
    pub fn record(&mut self, operation_name: &str, duration: Duration) {
        self.record_with_metadata(operation_name, duration, HashMap::new());
    }

    /// Record a measurement with metadata
    pub fn record_with_metadata(
        &mut self, 
        operation_name: &str, 
        duration: Duration,
        metadata: HashMap<String, Value>
    ) {
        let measurement = Measurement {
            name: operation_name.to_string(),
            duration,
            timestamp: self.start_time.elapsed(),
            metadata,
        };
        
        self.measurements.push(measurement);
    }

    /// Measure a synchronous operation
    pub fn measure<F, R>(&mut self, operation_name: &str, operation: F) -> R
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = operation();
        let duration = start.elapsed();
        
        self.record(operation_name, duration);
        result
    }

    /// Measure an async operation
    pub async fn measure_async<F, Fut, R>(&mut self, operation_name: &str, operation: F) -> R
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let result = operation().await;
        let duration = start.elapsed();
        
        self.record(operation_name, duration);
        result
    }

    /// Get all measurements
    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }

    /// Calculate statistics for a specific operation
    pub fn operation_stats(&self, operation_name: &str) -> Option<OperationStats> {
        let measurements: Vec<_> = self.measurements
            .iter()
            .filter(|m| m.name == operation_name)
            .collect();

        if measurements.is_empty() {
            return None;
        }

        let durations: Vec<Duration> = measurements.iter().map(|m| m.duration).collect();
        let mut sorted_durations = durations.clone();
        sorted_durations.sort();

        let total_duration: Duration = durations.iter().sum();
        let count = durations.len();
        let avg_duration = total_duration / count as u32;

        let min_duration = *sorted_durations.first().unwrap();
        let max_duration = *sorted_durations.last().unwrap();

        // Calculate percentiles
        let p50_duration = sorted_durations[count / 2];
        let p95_duration = sorted_durations[(count as f64 * 0.95) as usize.min(count - 1)];
        let p99_duration = sorted_durations[(count as f64 * 0.99) as usize.min(count - 1)];

        Some(OperationStats {
            operation_name: operation_name.to_string(),
            count,
            total_duration,
            avg_duration,
            min_duration,
            max_duration,
            p50_duration,
            p95_duration,
            p99_duration,
        })
    }

    /// Get overall performance summary
    pub fn performance_summary(&self) -> PerformanceSummary {
        let mut operation_stats = HashMap::new();
        let mut operation_names = std::collections::HashSet::new();

        for measurement in &self.measurements {
            operation_names.insert(measurement.name.clone());
        }

        for operation_name in operation_names {
            if let Some(stats) = self.operation_stats(&operation_name) {
                operation_stats.insert(operation_name, stats);
            }
        }

        let total_duration = self.start_time.elapsed();
        let throughput = self.measurements.len() as f64 / total_duration.as_secs_f64();

        PerformanceSummary {
            measurement_name: self.name.clone(),
            total_duration,
            total_operations: self.measurements.len(),
            throughput_ops_per_sec: throughput,
            operation_stats,
            targets: self.targets.clone(),
        }
    }

    /// Check if performance meets targets
    pub fn meets_targets(&self) -> bool {
        let summary = self.performance_summary();
        
        // Check overall throughput
        if summary.throughput_ops_per_sec < self.targets.min_throughput_ops_per_sec {
            return false;
        }

        // Check individual operation latencies
        for (_, stats) in &summary.operation_stats {
            if stats.p99_duration.as_millis() as f64 > self.targets.max_latency_ms {
                return false;
            }
        }

        true
    }

    /// Generate a detailed performance report
    pub fn generate_report(&self) -> String {
        let summary = self.performance_summary();
        
        let mut report = format!(
            "Performance Report: {}\n\
             Total Duration: {:.2}s\n\
             Total Operations: {}\n\
             Throughput: {:.2} ops/sec\n\
             Meets Targets: {}\n\n",
            summary.measurement_name,
            summary.total_duration.as_secs_f64(),
            summary.total_operations,
            summary.throughput_ops_per_sec,
            if self.meets_targets() { "✓" } else { "✗" }
        );

        report.push_str("Targets:\n");
        report.push_str(&format!("  Max Latency: {:.1}ms\n", self.targets.max_latency_ms));
        report.push_str(&format!("  Max Memory: {:.1}MB\n", self.targets.max_memory_mb));
        report.push_str(&format!("  Min Throughput: {:.1} ops/sec\n", self.targets.min_throughput_ops_per_sec));
        report.push_str(&format!("  Max CPU: {:.1}%\n", self.targets.max_cpu_percent));

        report.push_str("\nOperation Statistics:\n");
        for (operation_name, stats) in &summary.operation_stats {
            report.push_str(&format!(
                "  {}:\n\
                     Count: {}\n\
                     Avg: {:.2}ms\n\
                     P50: {:.2}ms\n\
                     P95: {:.2}ms\n\
                     P99: {:.2}ms\n\
                     Min: {:.2}ms\n\
                     Max: {:.2}ms\n",
                operation_name,
                stats.count,
                stats.avg_duration.as_millis(),
                stats.p50_duration.as_millis(),
                stats.p95_duration.as_millis(),
                stats.p99_duration.as_millis(),
                stats.min_duration.as_millis(),
                stats.max_duration.as_millis(),
            ));

            // Check against targets
            let p99_ms = stats.p99_duration.as_millis() as f64;
            if p99_ms > self.targets.max_latency_ms {
                report.push_str(&format!("    ⚠️  P99 latency exceeds target ({:.2}ms > {:.1}ms)\n", 
                                       p99_ms, self.targets.max_latency_ms));
            }
        }

        report
    }
}

/// Statistics for a specific operation
#[derive(Debug, Clone)]
pub struct OperationStats {
    pub operation_name: String,
    pub count: usize,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub p50_duration: Duration,
    pub p95_duration: Duration,
    pub p99_duration: Duration,
}

/// Overall performance summary
#[derive(Debug, Clone)]
pub struct PerformanceSummary {
    pub measurement_name: String,
    pub total_duration: Duration,
    pub total_operations: usize,
    pub throughput_ops_per_sec: f64,
    pub operation_stats: HashMap<String, OperationStats>,
    pub targets: PerformanceTargets,
}

/// Convenient function to measure an async operation
pub async fn measure_async<F, Fut, R>(operation_name: &str, operation: F) -> (R, Duration)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = R>,
{
    let start = Instant::now();
    let result = operation().await;
    let duration = start.elapsed();
    
    println!("Operation '{}' completed in {:?}", operation_name, duration);
    (result, duration)
}

/// Performance regression detector
pub struct RegressionDetector {
    baseline_measurements: HashMap<String, OperationStats>,
    threshold_percent: f64,
}

impl RegressionDetector {
    /// Create a new regression detector
    pub fn new(threshold_percent: f64) -> Self {
        Self {
            baseline_measurements: HashMap::new(),
            threshold_percent,
        }
    }

    /// Set baseline measurements
    pub fn set_baseline(&mut self, baseline: &PerformanceSummary) {
        self.baseline_measurements = baseline.operation_stats.clone();
    }

    /// Check for regressions compared to baseline
    pub fn check_regression(&self, current: &PerformanceSummary) -> RegressionReport {
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();

        for (operation_name, current_stats) in &current.operation_stats {
            if let Some(baseline_stats) = self.baseline_measurements.get(operation_name) {
                let current_p99_ms = current_stats.p99_duration.as_millis() as f64;
                let baseline_p99_ms = baseline_stats.p99_duration.as_millis() as f64;
                
                if baseline_p99_ms > 0.0 {
                    let change_percent = ((current_p99_ms - baseline_p99_ms) / baseline_p99_ms) * 100.0;
                    
                    if change_percent > self.threshold_percent {
                        regressions.push(PerformanceRegression {
                            operation_name: operation_name.clone(),
                            baseline_p99_ms,
                            current_p99_ms,
                            change_percent,
                        });
                    } else if change_percent < -self.threshold_percent {
                        improvements.push(PerformanceImprovement {
                            operation_name: operation_name.clone(),
                            baseline_p99_ms,
                            current_p99_ms,
                            improvement_percent: -change_percent,
                        });
                    }
                }
            }
        }

        RegressionReport {
            regressions,
            improvements,
            threshold_percent: self.threshold_percent,
        }
    }
}

/// Performance regression information
#[derive(Debug, Clone)]
pub struct PerformanceRegression {
    pub operation_name: String,
    pub baseline_p99_ms: f64,
    pub current_p99_ms: f64,
    pub change_percent: f64,
}

/// Performance improvement information
#[derive(Debug, Clone)]
pub struct PerformanceImprovement {
    pub operation_name: String,
    pub baseline_p99_ms: f64,
    pub current_p99_ms: f64,
    pub improvement_percent: f64,
}

/// Regression analysis report
#[derive(Debug, Clone)]
pub struct RegressionReport {
    pub regressions: Vec<PerformanceRegression>,
    pub improvements: Vec<PerformanceImprovement>,
    pub threshold_percent: f64,
}

impl RegressionReport {
    /// Check if any regressions were found
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Generate a human-readable report
    pub fn generate_report(&self) -> String {
        let mut report = format!(
            "Performance Regression Analysis (threshold: {:.1}%):\n\n",
            self.threshold_percent
        );

        if self.regressions.is_empty() {
            report.push_str("✅ No performance regressions detected\n\n");
        } else {
            report.push_str(&format!("❌ {} performance regressions found:\n", self.regressions.len()));
            for regression in &self.regressions {
                report.push_str(&format!(
                    "  • {}: {:.2}ms → {:.2}ms ({:+.1}%)\n",
                    regression.operation_name,
                    regression.baseline_p99_ms,
                    regression.current_p99_ms,
                    regression.change_percent
                ));
            }
            report.push('\n');
        }

        if !self.improvements.is_empty() {
            report.push_str(&format!("🚀 {} performance improvements found:\n", self.improvements.len()));
            for improvement in &self.improvements {
                report.push_str(&format!(
                    "  • {}: {:.2}ms → {:.2}ms ({:.1}% faster)\n",
                    improvement.operation_name,
                    improvement.baseline_p99_ms,
                    improvement.current_p99_ms,
                    improvement.improvement_percent
                ));
            }
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[test]
    fn test_performance_measurement_basic() {
        let mut measurement = PerformanceMeasurement::new("test");
        
        // Measure a simple operation
        let result = measurement.measure("test_op", || {
            std::thread::sleep(Duration::from_millis(10));
            42
        });
        
        assert_eq!(result, 42);
        assert_eq!(measurement.measurements().len(), 1);
        
        let stats = measurement.operation_stats("test_op").unwrap();
        assert_eq!(stats.count, 1);
        assert!(stats.avg_duration >= Duration::from_millis(9)); // Allow some variance
    }

    #[tokio::test]
    async fn test_async_measurement() {
        let mut measurement = PerformanceMeasurement::new("async_test");
        
        let result = measurement.measure_async("async_op", || async {
            sleep(Duration::from_millis(5)).await;
            "done"
        }).await;
        
        assert_eq!(result, "done");
        assert_eq!(measurement.measurements().len(), 1);
        
        let stats = measurement.operation_stats("async_op").unwrap();
        assert!(stats.avg_duration >= Duration::from_millis(4));
    }

    #[test]
    fn test_performance_targets() {
        let targets = PerformanceTargets::bevdbg_012_targets();
        let mut measurement = PerformanceMeasurement::with_targets("strict_test", targets);
        
        // Add a measurement that meets targets
        measurement.record("fast_op", Duration::from_nanos(500_000)); // 0.5ms
        
        // Add a measurement that exceeds targets
        measurement.record("slow_op", Duration::from_millis(5)); // 5ms
        
        // Overall should not meet targets due to slow operation
        assert!(!measurement.meets_targets());
        
        let report = measurement.generate_report();
        assert!(report.contains("✗")); // Should show failure
        assert!(report.contains("P99 latency exceeds target"));
    }

    #[test]
    fn test_regression_detector() {
        // Create baseline
        let mut baseline_measurement = PerformanceMeasurement::new("baseline");
        baseline_measurement.record("op1", Duration::from_millis(10));
        baseline_measurement.record("op2", Duration::from_millis(5));
        let baseline = baseline_measurement.performance_summary();
        
        // Create current measurements with regression
        let mut current_measurement = PerformanceMeasurement::new("current");
        current_measurement.record("op1", Duration::from_millis(15)); // 50% slower
        current_measurement.record("op2", Duration::from_millis(3));  // 40% faster
        let current = current_measurement.performance_summary();
        
        // Check for regressions
        let mut detector = RegressionDetector::new(20.0); // 20% threshold
        detector.set_baseline(&baseline);
        let report = detector.check_regression(&current);
        
        assert!(report.has_regressions());
        assert_eq!(report.regressions.len(), 1);
        assert_eq!(report.improvements.len(), 1);
        
        let report_str = report.generate_report();
        assert!(report_str.contains("performance regressions found"));
        assert!(report_str.contains("performance improvements found"));
    }

    #[tokio::test]
    async fn test_measure_async_function() {
        let (result, duration) = measure_async("test_operation", || async {
            sleep(Duration::from_millis(1)).await;
            "completed"
        }).await;
        
        assert_eq!(result, "completed");
        assert!(duration >= Duration::from_millis(1));
    }
}
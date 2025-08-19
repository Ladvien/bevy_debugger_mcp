/// Memory Usage Tracking
/// 
/// Utilities for tracking memory usage during tests to validate
/// that performance optimizations are working correctly.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

/// Simple memory usage tracker
pub struct MemoryUsageTracker {
    start_time: Instant,
    measurements: Vec<MemoryMeasurement>,
    baseline_memory: usize,
}

#[derive(Debug, Clone)]
pub struct MemoryMeasurement {
    pub timestamp: Duration,
    pub memory_usage: usize,
    pub allocation_count: Option<usize>,
    pub deallocation_count: Option<usize>,
}

impl MemoryUsageTracker {
    /// Create a new memory usage tracker
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            measurements: Vec::new(),
            baseline_memory: Self::get_current_memory_usage(),
        }
    }

    /// Get current memory usage (simplified implementation)
    pub fn current_usage(&self) -> usize {
        Self::get_current_memory_usage()
    }

    /// Record a memory measurement
    pub fn record_measurement(&mut self) {
        let measurement = MemoryMeasurement {
            timestamp: self.start_time.elapsed(),
            memory_usage: Self::get_current_memory_usage(),
            allocation_count: None,
            deallocation_count: None,
        };
        
        self.measurements.push(measurement);
    }

    /// Get baseline memory usage (memory at start)
    pub fn baseline(&self) -> usize {
        self.baseline_memory
    }

    /// Get peak memory usage
    pub fn peak_usage(&self) -> usize {
        self.measurements
            .iter()
            .map(|m| m.memory_usage)
            .max()
            .unwrap_or(self.baseline_memory)
    }

    /// Get current memory overhead compared to baseline
    pub fn current_overhead(&self) -> usize {
        self.current_usage().saturating_sub(self.baseline_memory)
    }

    /// Get peak memory overhead compared to baseline
    pub fn peak_overhead(&self) -> usize {
        self.peak_usage().saturating_sub(self.baseline_memory)
    }

    /// Get all measurements
    pub fn measurements(&self) -> &[MemoryMeasurement] {
        &self.measurements
    }

    /// Calculate memory growth rate (bytes per second)
    pub fn memory_growth_rate(&self) -> f64 {
        if self.measurements.len() < 2 {
            return 0.0;
        }

        let first = &self.measurements[0];
        let last = &self.measurements[self.measurements.len() - 1];
        
        let memory_diff = last.memory_usage as f64 - first.memory_usage as f64;
        let time_diff = (last.timestamp - first.timestamp).as_secs_f64();
        
        if time_diff > 0.0 {
            memory_diff / time_diff
        } else {
            0.0
        }
    }

    /// Check if memory usage is stable (not growing continuously)
    pub fn is_memory_stable(&self, threshold_bytes_per_sec: f64) -> bool {
        self.memory_growth_rate().abs() <= threshold_bytes_per_sec
    }

    /// Generate a memory usage report
    pub fn generate_report(&self) -> String {
        let current = self.current_usage();
        let peak = self.peak_usage();
        let baseline = self.baseline_memory;
        let growth_rate = self.memory_growth_rate();

        format!(
            "Memory Usage Report:\n\
             Baseline: {} bytes ({:.2} MB)\n\
             Current: {} bytes ({:.2} MB)\n\
             Peak: {} bytes ({:.2} MB)\n\
             Current Overhead: {} bytes ({:.2} MB)\n\
             Peak Overhead: {} bytes ({:.2} MB)\n\
             Growth Rate: {:.2} bytes/sec ({:.2} KB/sec)\n\
             Measurements: {}\n\
             Duration: {:.2}s",
            baseline,
            baseline as f64 / 1_048_576.0,
            current,
            current as f64 / 1_048_576.0,
            peak,
            peak as f64 / 1_048_576.0,
            self.current_overhead(),
            self.current_overhead() as f64 / 1_048_576.0,
            self.peak_overhead(),
            self.peak_overhead() as f64 / 1_048_576.0,
            growth_rate,
            growth_rate / 1024.0,
            self.measurements.len(),
            self.start_time.elapsed().as_secs_f64()
        )
    }

    /// Get memory usage estimate (platform-specific implementations would go here)
    fn get_current_memory_usage() -> usize {
        // This is a simplified implementation
        // In a real implementation, you would use platform-specific APIs:
        // - On Linux: parse /proc/self/status
        // - On macOS: use task_info
        // - On Windows: use GetProcessMemoryInfo
        
        // For testing, we'll simulate memory usage
        use std::sync::OnceLock;
        static SIMULATED_MEMORY: OnceLock<AtomicUsize> = OnceLock::new();
        
        let memory = SIMULATED_MEMORY.get_or_init(|| AtomicUsize::new(50_000_000)); // 50MB baseline
        memory.load(Ordering::Relaxed)
    }

    /// Simulate memory allocation (for testing)
    pub fn simulate_allocation(&self, bytes: usize) {
        use std::sync::OnceLock;
        static SIMULATED_MEMORY: OnceLock<AtomicUsize> = OnceLock::new();
        
        let memory = SIMULATED_MEMORY.get_or_init(|| AtomicUsize::new(50_000_000));
        memory.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Simulate memory deallocation (for testing)
    pub fn simulate_deallocation(&self, bytes: usize) {
        use std::sync::OnceLock;
        static SIMULATED_MEMORY: OnceLock<AtomicUsize> = OnceLock::new();
        
        let memory = SIMULATED_MEMORY.get_or_init(|| AtomicUsize::new(50_000_000));
        memory.fetch_sub(bytes, Ordering::Relaxed);
    }
}

/// Memory pressure test utilities
pub struct MemoryPressureTest {
    tracker: MemoryUsageTracker,
    allocation_size: usize,
    max_allocations: usize,
    allocations: Vec<Vec<u8>>,
}

impl MemoryPressureTest {
    /// Create a new memory pressure test
    pub fn new(allocation_size: usize, max_allocations: usize) -> Self {
        Self {
            tracker: MemoryUsageTracker::new(),
            allocation_size,
            max_allocations,
            allocations: Vec::new(),
        }
    }

    /// Apply memory pressure by allocating memory
    pub fn apply_pressure(&mut self, num_allocations: usize) {
        let allocations_to_make = num_allocations.min(
            self.max_allocations.saturating_sub(self.allocations.len())
        );

        for _ in 0..allocations_to_make {
            let allocation = vec![0u8; self.allocation_size];
            self.allocations.push(allocation);
            self.tracker.simulate_allocation(self.allocation_size);
        }

        self.tracker.record_measurement();
        
        println!(
            "Applied memory pressure: {} allocations of {} bytes each (total: {} MB)",
            allocations_to_make,
            self.allocation_size,
            (allocations_to_make * self.allocation_size) as f64 / 1_048_576.0
        );
    }

    /// Release memory pressure
    pub fn release_pressure(&mut self, num_deallocations: usize) {
        let deallocations_to_make = num_deallocations.min(self.allocations.len());

        for _ in 0..deallocations_to_make {
            if let Some(allocation) = self.allocations.pop() {
                let size = allocation.len();
                drop(allocation);
                self.tracker.simulate_deallocation(size);
            }
        }

        self.tracker.record_measurement();
        
        println!(
            "Released memory pressure: {} deallocations (remaining: {} allocations)",
            deallocations_to_make,
            self.allocations.len()
        );
    }

    /// Get the current memory tracker
    pub fn tracker(&self) -> &MemoryUsageTracker {
        &self.tracker
    }

    /// Get mutable access to the memory tracker
    pub fn tracker_mut(&mut self) -> &mut MemoryUsageTracker {
        &mut self.tracker
    }

    /// Run a memory pressure test scenario
    pub async fn run_pressure_scenario(&mut self) -> MemoryPressureResults {
        println!("Starting memory pressure test scenario");
        
        let start_time = Instant::now();
        let mut phase_results = Vec::new();

        // Phase 1: Gradual pressure increase
        println!("Phase 1: Gradual pressure increase");
        for step in 1..=5 {
            self.apply_pressure(10 * step);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        phase_results.push(("Pressure Increase".to_string(), self.tracker.current_usage()));

        // Phase 2: Sustained pressure
        println!("Phase 2: Sustained pressure");
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            self.tracker.record_measurement();
        }
        phase_results.push(("Sustained Pressure".to_string(), self.tracker.current_usage()));

        // Phase 3: Pressure spike
        println!("Phase 3: Pressure spike");
        self.apply_pressure(100);
        tokio::time::sleep(Duration::from_secs(3)).await;
        phase_results.push(("Pressure Spike".to_string(), self.tracker.current_usage()));

        // Phase 4: Gradual release
        println!("Phase 4: Gradual release");
        for _ in 0..3 {
            self.release_pressure(50);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        phase_results.push(("Pressure Release".to_string(), self.tracker.current_usage()));

        // Phase 5: Recovery
        println!("Phase 5: Recovery");
        self.release_pressure(self.allocations.len());
        tokio::time::sleep(Duration::from_secs(2)).await;
        self.tracker.record_measurement();
        phase_results.push(("Recovery".to_string(), self.tracker.current_usage()));

        MemoryPressureResults {
            duration: start_time.elapsed(),
            baseline_memory: self.tracker.baseline(),
            peak_memory: self.tracker.peak_usage(),
            final_memory: self.tracker.current_usage(),
            phase_results,
            measurements: self.tracker.measurements().to_vec(),
        }
    }
}

/// Results from a memory pressure test
#[derive(Debug)]
pub struct MemoryPressureResults {
    pub duration: Duration,
    pub baseline_memory: usize,
    pub peak_memory: usize,
    pub final_memory: usize,
    pub phase_results: Vec<(String, usize)>,
    pub measurements: Vec<MemoryMeasurement>,
}

impl MemoryPressureResults {
    /// Calculate maximum memory overhead during the test
    pub fn max_overhead(&self) -> usize {
        self.peak_memory.saturating_sub(self.baseline_memory)
    }

    /// Calculate final memory overhead
    pub fn final_overhead(&self) -> usize {
        self.final_memory.saturating_sub(self.baseline_memory)
    }

    /// Check if memory was properly released (within acceptable threshold)
    pub fn memory_properly_released(&self, threshold_mb: f64) -> bool {
        let final_overhead_mb = self.final_overhead() as f64 / 1_048_576.0;
        final_overhead_mb <= threshold_mb
    }

    /// Generate a detailed test report
    pub fn generate_report(&self) -> String {
        let mut report = format!(
            "Memory Pressure Test Results:\n\
             Duration: {:.2}s\n\
             Baseline Memory: {:.2} MB\n\
             Peak Memory: {:.2} MB\n\
             Final Memory: {:.2} MB\n\
             Max Overhead: {:.2} MB\n\
             Final Overhead: {:.2} MB\n\
             Memory Properly Released: {}\n\n",
            self.duration.as_secs_f64(),
            self.baseline_memory as f64 / 1_048_576.0,
            self.peak_memory as f64 / 1_048_576.0,
            self.final_memory as f64 / 1_048_576.0,
            self.max_overhead() as f64 / 1_048_576.0,
            self.final_overhead() as f64 / 1_048_576.0,
            if self.memory_properly_released(10.0) { "✓" } else { "✗" }
        );

        report.push_str("Phase Results:\n");
        for (phase_name, memory_usage) in &self.phase_results {
            report.push_str(&format!(
                "  {}: {:.2} MB\n",
                phase_name,
                *memory_usage as f64 / 1_048_576.0
            ));
        }

        report.push_str(&format!("\nTotal Measurements: {}\n", self.measurements.len()));

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_tracker_basic() {
        let mut tracker = MemoryUsageTracker::new();
        let baseline = tracker.baseline();
        
        assert!(baseline > 0, "Baseline memory should be positive");
        assert_eq!(tracker.current_overhead(), 0, "Initial overhead should be zero");
        
        // Simulate some allocations
        tracker.simulate_allocation(1000);
        tracker.record_measurement();
        
        assert!(tracker.current_overhead() >= 1000, "Should track allocation");
        
        tracker.simulate_deallocation(500);
        tracker.record_measurement();
        
        assert!(tracker.current_overhead() >= 500, "Should track partial deallocation");
    }

    #[tokio::test]
    async fn test_memory_pressure_test() {
        let mut pressure_test = MemoryPressureTest::new(1024, 100);
        
        pressure_test.apply_pressure(10);
        assert!(pressure_test.tracker().current_overhead() >= 10 * 1024);
        
        pressure_test.release_pressure(5);
        let overhead_after_release = pressure_test.tracker().current_overhead();
        assert!(overhead_after_release < 10 * 1024);
        
        // Run full scenario
        let results = pressure_test.run_pressure_scenario().await;
        assert!(results.duration > Duration::from_secs(10));
        assert!(results.peak_memory > results.baseline_memory);
        
        println!("{}", results.generate_report());
    }
}
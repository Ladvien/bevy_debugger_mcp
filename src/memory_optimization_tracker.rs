use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::{debug, info};

/// Memory tracking allocator to monitor clone operations
pub struct TrackingAllocator {
    allocations: AtomicUsize,
    deallocations: AtomicUsize,
    current_bytes: AtomicUsize,
    peak_bytes: AtomicUsize,
    clone_count: AtomicUsize,
}

impl TrackingAllocator {
    pub const fn new() -> Self {
        Self {
            allocations: AtomicUsize::new(0),
            deallocations: AtomicUsize::new(0),
            current_bytes: AtomicUsize::new(0),
            peak_bytes: AtomicUsize::new(0),
            clone_count: AtomicUsize::new(0),
        }
    }

    pub fn get_stats(&self) -> MemoryStats {
        MemoryStats {
            allocations: self.allocations.load(Ordering::Relaxed),
            deallocations: self.deallocations.load(Ordering::Relaxed),
            current_bytes: self.current_bytes.load(Ordering::Relaxed),
            peak_bytes: self.peak_bytes.load(Ordering::Relaxed),
            clone_count: self.clone_count.load(Ordering::Relaxed),
        }
    }

    pub fn increment_clone_count(&self) {
        self.clone_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn reset_stats(&self) {
        self.allocations.store(0, Ordering::Relaxed);
        self.deallocations.store(0, Ordering::Relaxed);
        self.current_bytes.store(0, Ordering::Relaxed);
        self.peak_bytes.store(0, Ordering::Relaxed);
        self.clone_count.store(0, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = System.alloc(layout);
        if !ptr.is_null() {
            self.allocations.fetch_add(1, Ordering::Relaxed);
            let new_bytes = self.current_bytes.fetch_add(layout.size(), Ordering::Relaxed) + layout.size();
            
            // Update peak if necessary
            let mut peak = self.peak_bytes.load(Ordering::Relaxed);
            while new_bytes > peak {
                match self.peak_bytes.compare_exchange_weak(peak, new_bytes, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(x) => peak = x,
                }
            }
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
        self.deallocations.fetch_add(1, Ordering::Relaxed);
        self.current_bytes.fetch_sub(layout.size(), Ordering::Relaxed);
    }
}

#[derive(Debug, Clone)]
pub struct MemoryStats {
    pub allocations: usize,
    pub deallocations: usize,
    pub current_bytes: usize,
    pub peak_bytes: usize,
    pub clone_count: usize,
}

impl MemoryStats {
    pub fn bytes_to_mb(bytes: usize) -> f64 {
        bytes as f64 / (1024.0 * 1024.0)
    }
    
    pub fn log_stats(&self) {
        info!(
            "Memory Stats - Allocs: {}, Deallocs: {}, Current: {:.2}MB, Peak: {:.2}MB, Clones: {}",
            self.allocations,
            self.deallocations,
            Self::bytes_to_mb(self.current_bytes),
            Self::bytes_to_mb(self.peak_bytes),
            self.clone_count
        );
    }
}

/// Memory optimization tracker for specific components
#[derive(Debug)]
pub struct MemoryOptimizationTracker {
    baseline_stats: RwLock<Option<MemoryStats>>,
    optimization_targets: RwLock<HashMap<String, OptimizationTarget>>,
}

#[derive(Debug, Clone)]
pub struct OptimizationTarget {
    pub name: String,
    pub baseline_clones: usize,
    pub current_clones: usize,
    pub target_reduction_percent: f64,
}

impl OptimizationTarget {
    pub fn new(name: String, baseline_clones: usize, target_reduction_percent: f64) -> Self {
        Self {
            name,
            baseline_clones,
            current_clones: baseline_clones,
            target_reduction_percent,
        }
    }
    
    pub fn update_current(&mut self, current_clones: usize) {
        self.current_clones = current_clones;
    }
    
    pub fn reduction_achieved(&self) -> f64 {
        if self.baseline_clones == 0 {
            return 0.0;
        }
        
        let reduction = self.baseline_clones.saturating_sub(self.current_clones) as f64;
        (reduction / self.baseline_clones as f64) * 100.0
    }
    
    pub fn is_target_met(&self) -> bool {
        self.reduction_achieved() >= self.target_reduction_percent
    }
}

impl MemoryOptimizationTracker {
    pub fn new() -> Self {
        Self {
            baseline_stats: RwLock::new(None),
            optimization_targets: RwLock::new(HashMap::new()),
        }
    }
    
    pub fn set_baseline(&self, stats: MemoryStats) {
        *self.baseline_stats.write().unwrap() = Some(stats);
    }
    
    pub fn add_optimization_target(&self, target: OptimizationTarget) {
        self.optimization_targets
            .write()
            .unwrap()
            .insert(target.name.clone(), target);
    }
    
    pub fn update_target(&self, name: &str, current_clones: usize) {
        if let Some(target) = self.optimization_targets.write().unwrap().get_mut(name) {
            target.update_current(current_clones);
        }
    }
    
    pub fn get_optimization_summary(&self) -> OptimizationSummary {
        let targets = self.optimization_targets.read().unwrap().clone();
        let baseline = self.baseline_stats.read().unwrap().clone();
        
        OptimizationSummary {
            baseline_stats: baseline,
            targets: targets.into_values().collect(),
        }
    }
}

#[derive(Debug)]
pub struct OptimizationSummary {
    pub baseline_stats: Option<MemoryStats>,
    pub targets: Vec<OptimizationTarget>,
}

impl OptimizationSummary {
    pub fn log_summary(&self) {
        info!("=== Memory Optimization Summary ===");
        
        if let Some(baseline) = &self.baseline_stats {
            baseline.log_stats();
        }
        
        for target in &self.targets {
            let status = if target.is_target_met() { "✅ MET" } else { "⚠️  PENDING" };
            info!(
                "{} - {}: {}/{} clones ({:.1}% reduction) [Target: {:.1}%]",
                status,
                target.name,
                target.current_clones,
                target.baseline_clones,
                target.reduction_achieved(),
                target.target_reduction_percent
            );
        }
        
        let total_reduction = self.calculate_total_reduction();
        info!("📊 Total clone reduction: {:.1}%", total_reduction);
    }
    
    fn calculate_total_reduction(&self) -> f64 {
        if self.targets.is_empty() {
            return 0.0;
        }
        
        let total_baseline: usize = self.targets.iter().map(|t| t.baseline_clones).sum();
        let total_current: usize = self.targets.iter().map(|t| t.current_clones).sum();
        
        if total_baseline == 0 {
            return 0.0;
        }
        
        let reduction = total_baseline.saturating_sub(total_current) as f64;
        (reduction / total_baseline as f64) * 100.0
    }
}

/// Global memory optimization tracker instance
pub static MEMORY_TRACKER: once_cell::sync::Lazy<MemoryOptimizationTracker> = 
    once_cell::sync::Lazy::new(|| MemoryOptimizationTracker::new());

/// Macro to track clone operations in optimized code
#[macro_export]
macro_rules! track_clone {
    ($value:expr, $target:expr) => {{
        // In production, this would integrate with the tracking allocator
        // For now, we'll just track the call
        debug!("Clone tracked for target: {}", $target);
        $value.clone()
    }};
}

/// Utility for benchmarking memory usage before/after optimizations
pub fn benchmark_memory_usage<F, R>(name: &str, f: F) -> (R, MemoryStats)
where
    F: FnOnce() -> R,
{
    // In a real implementation, this would use the tracking allocator
    // For now, we'll simulate memory tracking
    
    info!("Starting memory benchmark: {}", name);
    let result = f();
    
    // Simulate memory stats - in real implementation this would come from TrackingAllocator
    let stats = MemoryStats {
        allocations: 0,
        deallocations: 0,
        current_bytes: 0,
        peak_bytes: 0,
        clone_count: 0,
    };
    
    info!("Completed memory benchmark: {}", name);
    (result, stats)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimization_target() {
        let mut target = OptimizationTarget::new("test".to_string(), 100, 40.0);
        
        assert_eq!(target.reduction_achieved(), 0.0);
        assert!(!target.is_target_met());
        
        target.update_current(60);
        assert_eq!(target.reduction_achieved(), 40.0);
        assert!(target.is_target_met());
        
        target.update_current(50);
        assert_eq!(target.reduction_achieved(), 50.0);
        assert!(target.is_target_met());
    }

    #[test]
    fn test_memory_optimization_tracker() {
        let tracker = MemoryOptimizationTracker::new();
        
        let target = OptimizationTarget::new("lazy_init".to_string(), 56, 40.0);
        tracker.add_optimization_target(target);
        
        tracker.update_target("lazy_init", 34);
        
        let summary = tracker.get_optimization_summary();
        assert_eq!(summary.targets.len(), 1);
        assert!(summary.targets[0].is_target_met());
    }
}
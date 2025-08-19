/// Test Helper Modules
/// 
/// Provides utilities and infrastructure for integration and E2E testing
/// of the performance optimizations and debugging system.

pub mod screenshot_test_utils;
pub mod test_game_process;
pub mod memory_tracker;
pub mod query_generators;
pub mod performance_measurement;

pub use screenshot_test_utils::{
    ScreenshotValidator, 
    ScreenshotInfo, 
    ComparisonResult, 
    ScreenshotError
};

pub use test_game_process::{TestGameProcess, TestScenario, ScenarioResults, with_test_game};
pub use memory_tracker::{MemoryUsageTracker, MemoryPressureTest, MemoryPressureResults};
pub use query_generators::{
    generate_realistic_queries, 
    generate_stress_queries, 
    generate_optimization_test_queries,
    generate_complexity_queries,
    generate_edge_case_queries,
    generate_sequential_queries,
    generate_time_based_queries,
    generate_workload_pattern,
};
pub use performance_measurement::{
    PerformanceMeasurement, 
    PerformanceTargets,
    OperationStats,
    PerformanceSummary,
    RegressionDetector,
    RegressionReport,
    measure_async,
};
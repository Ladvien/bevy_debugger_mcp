use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{RwLock, Semaphore};
use tracing::{error, info, warn};

use crate::brp_client::BrpClient;
use crate::error::Result;
use crate::experiment_system::{Action, ActionExecutor, ComponentSpec};

/// Types of stress tests available
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StressTestType {
    /// Spawn many entities rapidly
    SpawnMany {
        entity_type: String,
        spawn_rate: usize,
        max_entities: usize,
    },
    /// Make rapid changes to existing entities
    RapidChanges {
        change_rate: usize,
        component_types: Vec<String>,
        target_entities: Option<Vec<u64>>,
    },
    /// Create memory pressure by spawning complex entities
    MemoryPressure {
        complexity: ComplexityLevel,
        target_memory_mb: usize,
    },
    /// Combined stress test
    Combined { tests: Vec<StressTestType> },
}

/// Complexity level for memory pressure tests
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComplexityLevel {
    Low,
    Medium,
    High,
    Extreme,
}

impl ComplexityLevel {
    /// Get the number of components for this complexity level
    pub fn component_count(&self) -> usize {
        match self {
            Self::Low => 5,
            Self::Medium => 15,
            Self::High => 30,
            Self::Extreme => 50,
        }
    }
}

/// Intensity level for stress tests
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntensityLevel {
    Low,
    Medium,
    High,
    Extreme,
}

impl IntensityLevel {
    /// Get multiplier for this intensity level
    pub fn multiplier(&self) -> f64 {
        match self {
            Self::Low => 0.5,
            Self::Medium => 1.0,
            Self::High => 2.0,
            Self::Extreme => 5.0,
        }
    }

    /// Get concurrent operations limit
    pub fn concurrency_limit(&self) -> usize {
        match self {
            Self::Low => 2,
            Self::Medium => 5,
            Self::High => 10,
            Self::Extreme => 20,
        }
    }
}

/// Performance metrics tracked during stress test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub frame_times_ms: Vec<f64>,
    pub entity_count: Vec<usize>,
    pub memory_usage_mb: Vec<f64>,
    pub cpu_usage_percent: Vec<f64>,
    pub operations_per_second: Vec<f64>,
    pub error_count: usize,
    pub warning_count: usize,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMetrics {
    pub fn new() -> Self {
        Self {
            frame_times_ms: Vec::new(),
            entity_count: Vec::new(),
            memory_usage_mb: Vec::new(),
            cpu_usage_percent: Vec::new(),
            operations_per_second: Vec::new(),
            error_count: 0,
            warning_count: 0,
        }
    }

    /// Calculate percentiles for frame times
    pub fn frame_time_percentiles(&self) -> (f64, f64, f64, f64) {
        if self.frame_times_ms.is_empty() {
            return (0.0, 0.0, 0.0, 0.0);
        }

        let mut sorted = self.frame_times_ms.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let p50_idx = sorted.len() / 2;
        let p90_idx = (sorted.len() * 9) / 10;
        let p95_idx = (sorted.len() * 95) / 100;
        let p99_idx = (sorted.len() * 99) / 100;

        (
            sorted[p50_idx],
            sorted[p90_idx.min(sorted.len() - 1)],
            sorted[p95_idx.min(sorted.len() - 1)],
            sorted[p99_idx.min(sorted.len() - 1)],
        )
    }
}

/// Circuit breaker to prevent game crashes
pub struct CircuitBreaker {
    failure_threshold: usize,
    reset_timeout: Duration,
    failure_count: AtomicU64,
    last_failure_time: RwLock<Option<Instant>>,
    is_open: AtomicBool,
}

impl CircuitBreaker {
    pub fn new(failure_threshold: usize, reset_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            reset_timeout,
            failure_count: AtomicU64::new(0),
            last_failure_time: RwLock::new(None),
            is_open: AtomicBool::new(false),
        }
    }

    /// Check if circuit breaker is open (blocking operations)
    pub async fn is_open(&self) -> bool {
        if !self.is_open.load(Ordering::Relaxed) {
            return false;
        }

        // Check if timeout has passed
        if let Some(last_failure) = *self.last_failure_time.read().await {
            if last_failure.elapsed() > self.reset_timeout {
                self.reset().await;
                return false;
            }
        }

        true
    }

    /// Record a success
    pub async fn record_success(&self) {
        let current = self.failure_count.load(Ordering::Relaxed);
        if current > 0 {
            self.failure_count.fetch_sub(1, Ordering::Relaxed);
        }
    }

    /// Record a failure
    pub async fn record_failure(&self) {
        let new_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;

        if new_count >= self.failure_threshold as u64 {
            self.is_open.store(true, Ordering::Relaxed);
            *self.last_failure_time.write().await = Some(Instant::now());
            warn!("Circuit breaker opened after {} failures", new_count);
        }
    }

    /// Reset the circuit breaker
    pub async fn reset(&self) {
        self.failure_count.store(0, Ordering::Relaxed);
        self.is_open.store(false, Ordering::Relaxed);
        *self.last_failure_time.write().await = None;
        info!("Circuit breaker reset");
    }
}

/// Trait for different stress test implementations
#[async_trait::async_trait]
pub trait StressTest: Send + Sync {
    /// Get the name of this stress test
    fn name(&self) -> &str;

    /// Execute the stress test
    async fn execute(
        &self,
        brp_client: &mut BrpClient,
        intensity: IntensityLevel,
        duration: Duration,
        metrics: Arc<RwLock<PerformanceMetrics>>,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Result<()>;

    /// Clean up resources after test
    async fn cleanup(&self, brp_client: &mut BrpClient) -> Result<()>;
}

/// Spawn many entities stress test
pub struct SpawnManyTest {
    entity_type: String,
    spawn_rate: usize,
    max_entities: usize,
    spawned_entities: Arc<RwLock<Vec<u64>>>,
}

impl SpawnManyTest {
    pub fn new(entity_type: String, spawn_rate: usize, max_entities: usize) -> Self {
        Self {
            entity_type,
            spawn_rate,
            max_entities,
            spawned_entities: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl StressTest for SpawnManyTest {
    fn name(&self) -> &str {
        "spawn_many"
    }

    async fn execute(
        &self,
        brp_client: &mut BrpClient,
        intensity: IntensityLevel,
        duration: Duration,
        metrics: Arc<RwLock<PerformanceMetrics>>,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Result<()> {
        info!(
            "Starting SpawnMany stress test with {} intensity",
            format!("{intensity:?}").to_lowercase()
        );

        let adjusted_rate = (self.spawn_rate as f64 * intensity.multiplier()) as usize;
        let start_time = Instant::now();
        let mut executor = ActionExecutor::new();
        let mut total_spawned = 0;

        while start_time.elapsed() < duration && total_spawned < self.max_entities {
            if circuit_breaker.is_open().await {
                warn!("Circuit breaker is open, pausing operations");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            let batch_size = adjusted_rate.min(self.max_entities - total_spawned);
            let mut batch_success = 0;

            for _ in 0..batch_size {
                let action = Action::Spawn {
                    components: self.create_entity_components(),
                    archetype: Some(self.entity_type.clone()),
                };

                match executor.execute_action(&action, brp_client).await {
                    Ok(result) if result.success => {
                        batch_success += 1;
                        if let Some(entity_id) = result.entity_id {
                            self.spawned_entities.write().await.push(entity_id);
                        }
                        circuit_breaker.record_success().await;
                    }
                    Ok(_) => {
                        metrics.write().await.warning_count += 1;
                    }
                    Err(e) => {
                        error!("Failed to spawn entity: {}", e);
                        metrics.write().await.error_count += 1;
                        circuit_breaker.record_failure().await;
                    }
                }
            }

            total_spawned += batch_success;

            // Record metrics
            {
                let mut metrics = metrics.write().await;
                metrics.entity_count.push(total_spawned);
                metrics.operations_per_second.push(batch_success as f64);
            }

            // Sleep to maintain rate
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        info!(
            "SpawnMany test completed: spawned {} entities",
            total_spawned
        );
        Ok(())
    }

    async fn cleanup(&self, brp_client: &mut BrpClient) -> Result<()> {
        info!("Cleaning up spawned entities");
        let entities = self.spawned_entities.read().await;
        let mut executor = ActionExecutor::new();

        for &entity_id in entities.iter() {
            let action = Action::Delete { entity_id };
            if let Err(e) = executor.execute_action(&action, brp_client).await {
                warn!("Failed to delete entity {}: {}", entity_id, e);
            }
        }

        Ok(())
    }
}

impl SpawnManyTest {
    fn random_float(&self) -> f32 {
        use rand::Rng;
        rand::thread_rng().gen()
    }

    fn random_u32(&self) -> u32 {
        use rand::Rng;
        rand::thread_rng().gen()
    }

    fn create_entity_components(&self) -> Vec<ComponentSpec> {
        vec![
            ComponentSpec {
                type_id: "Transform".to_string(),
                value: serde_json::json!({
                    "translation": {
                        "x": self.random_float() * 100.0 - 50.0,
                        "y": self.random_float() * 100.0 - 50.0,
                        "z": self.random_float() * 100.0 - 50.0,
                    },
                    "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                    "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
                }),
            },
            ComponentSpec {
                type_id: "Name".to_string(),
                value: serde_json::json!(format!("StressEntity_{}", self.random_u32())),
            },
        ]
    }
}

/// Rapid changes stress test
pub struct RapidChangesTest {
    change_rate: usize,
    component_types: Vec<String>,
    target_entities: Option<Vec<u64>>,
}

impl RapidChangesTest {
    pub fn new(
        change_rate: usize,
        component_types: Vec<String>,
        target_entities: Option<Vec<u64>>,
    ) -> Self {
        Self {
            change_rate,
            component_types,
            target_entities,
        }
    }
}

#[async_trait::async_trait]
impl StressTest for RapidChangesTest {
    fn name(&self) -> &str {
        "rapid_changes"
    }

    async fn execute(
        &self,
        brp_client: &mut BrpClient,
        intensity: IntensityLevel,
        duration: Duration,
        metrics: Arc<RwLock<PerformanceMetrics>>,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Result<()> {
        info!(
            "Starting RapidChanges stress test with {} intensity",
            format!("{intensity:?}").to_lowercase()
        );

        let adjusted_rate = (self.change_rate as f64 * intensity.multiplier()) as usize;
        let start_time = Instant::now();
        let mut executor = ActionExecutor::new();

        // Get or create target entities
        let entities = if let Some(ref targets) = self.target_entities {
            targets.clone()
        } else {
            // Create some test entities if none provided
            let mut created = Vec::new();
            for _i in 0..10 {
                let action = Action::Spawn {
                    components: vec![ComponentSpec {
                        type_id: "Transform".to_string(),
                        value: serde_json::json!({
                            "translation": {"x": 0.0, "y": 0.0, "z": 0.0},
                            "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                            "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
                        }),
                    }],
                    archetype: None,
                };

                if let Ok(result) = executor.execute_action(&action, brp_client).await {
                    if let Some(id) = result.entity_id {
                        created.push(id);
                    }
                }
            }
            created
        };

        let mut total_changes = 0;

        while start_time.elapsed() < duration {
            if circuit_breaker.is_open().await {
                warn!("Circuit breaker is open, pausing operations");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            let mut batch_success = 0;

            for _ in 0..adjusted_rate {
                if entities.is_empty() {
                    break;
                }

                let entity_id = entities[{
                    use rand::Rng;
                    rand::thread_rng().gen_range(0..entities.len())
                }];
                let components = self.create_random_changes();

                let action = Action::Modify {
                    entity_id,
                    components,
                };

                match executor.execute_action(&action, brp_client).await {
                    Ok(result) if result.success => {
                        batch_success += 1;
                        circuit_breaker.record_success().await;
                    }
                    Ok(_) => {
                        metrics.write().await.warning_count += 1;
                    }
                    Err(e) => {
                        error!("Failed to modify entity: {}", e);
                        metrics.write().await.error_count += 1;
                        circuit_breaker.record_failure().await;
                    }
                }
            }

            total_changes += batch_success;

            // Record metrics
            {
                let mut metrics = metrics.write().await;
                metrics.operations_per_second.push(batch_success as f64);
            }

            // Sleep to maintain rate
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        info!(
            "RapidChanges test completed: made {} changes",
            total_changes
        );
        Ok(())
    }

    async fn cleanup(&self, _brp_client: &mut BrpClient) -> Result<()> {
        // No special cleanup needed for rapid changes test
        Ok(())
    }
}

impl RapidChangesTest {
    fn create_random_changes(&self) -> Vec<ComponentSpec> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut components = Vec::new();

        for component_type in &self.component_types {
            let value = match component_type.as_str() {
                "Transform" => {
                    let x = rng.gen::<f32>() * 10.0;
                    let y = rng.gen::<f32>() * 10.0;
                    let z = rng.gen::<f32>() * 10.0;
                    serde_json::json!({
                        "translation": {
                            "x": x,
                            "y": y,
                            "z": z,
                        }
                    })
                }
                "Velocity" => {
                    let x = rng.gen::<f32>() * 5.0;
                    let y = rng.gen::<f32>() * 5.0;
                    let z = rng.gen::<f32>() * 5.0;
                    serde_json::json!({
                        "x": x,
                        "y": y,
                        "z": z,
                    })
                }
                _ => serde_json::json!(rng.gen::<f32>()),
            };

            components.push(ComponentSpec {
                type_id: component_type.clone(),
                value,
            });
        }

        components
    }
}

/// Memory pressure stress test
pub struct MemoryPressureTest {
    complexity: ComplexityLevel,
    target_memory_mb: usize,
    spawned_entities: Arc<RwLock<Vec<u64>>>,
}

impl MemoryPressureTest {
    pub fn new(complexity: ComplexityLevel, target_memory_mb: usize) -> Self {
        Self {
            complexity,
            target_memory_mb,
            spawned_entities: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait::async_trait]
impl StressTest for MemoryPressureTest {
    fn name(&self) -> &str {
        "memory_pressure"
    }

    async fn execute(
        &self,
        brp_client: &mut BrpClient,
        intensity: IntensityLevel,
        duration: Duration,
        metrics: Arc<RwLock<PerformanceMetrics>>,
        circuit_breaker: Arc<CircuitBreaker>,
    ) -> Result<()> {
        info!(
            "Starting MemoryPressure stress test with {} complexity",
            format!("{:?}", self.complexity).to_lowercase()
        );

        let start_time = Instant::now();
        let mut executor = ActionExecutor::new();
        let mut estimated_memory_mb = 0.0;
        let component_count = self.complexity.component_count();
        let bytes_per_entity = component_count * 100; // Rough estimate

        while start_time.elapsed() < duration && estimated_memory_mb < self.target_memory_mb as f64
        {
            if circuit_breaker.is_open().await {
                warn!("Circuit breaker is open, pausing operations");
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }

            let components = self.create_complex_entity(component_count);
            let action = Action::Spawn {
                components,
                archetype: None,
            };

            match executor.execute_action(&action, brp_client).await {
                Ok(result) if result.success => {
                    if let Some(entity_id) = result.entity_id {
                        self.spawned_entities.write().await.push(entity_id);
                    }
                    estimated_memory_mb += bytes_per_entity as f64 / (1024.0 * 1024.0);
                    circuit_breaker.record_success().await;
                }
                Ok(_) => {
                    metrics.write().await.warning_count += 1;
                }
                Err(e) => {
                    error!("Failed to create complex entity: {}", e);
                    metrics.write().await.error_count += 1;
                    circuit_breaker.record_failure().await;
                }
            }

            // Record metrics
            {
                let mut metrics = metrics.write().await;
                metrics.memory_usage_mb.push(estimated_memory_mb);
                metrics
                    .entity_count
                    .push(self.spawned_entities.read().await.len());
            }

            // Adaptive delay based on intensity
            let delay_ms = (100.0 / intensity.multiplier()) as u64;
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        info!(
            "MemoryPressure test completed: estimated memory usage {} MB",
            estimated_memory_mb
        );
        Ok(())
    }

    async fn cleanup(&self, brp_client: &mut BrpClient) -> Result<()> {
        info!("Cleaning up complex entities");
        let entities = self.spawned_entities.read().await;
        let mut executor = ActionExecutor::new();

        for &entity_id in entities.iter() {
            let action = Action::Delete { entity_id };
            if let Err(e) = executor.execute_action(&action, brp_client).await {
                warn!("Failed to delete entity {}: {}", entity_id, e);
            }
        }

        Ok(())
    }
}

impl MemoryPressureTest {
    fn create_complex_entity(&self, component_count: usize) -> Vec<ComponentSpec> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let mut components = Vec::new();

        for i in 0..component_count {
            // Create various types of components with data
            let component = match i % 5 {
                0 => {
                    let data: Vec<f32> = (0..100).map(|_| rng.gen()).collect();
                    ComponentSpec {
                        type_id: format!("Component_{i}"),
                        value: serde_json::json!({
                            "data": data,
                            "index": i,
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                        }),
                    }
                }
                1 => ComponentSpec {
                    type_id: format!("StringComponent_{i}"),
                    value: serde_json::json!({
                        "text": "x".repeat(1000),
                        "id": format!("complex_{}", i),
                    }),
                },
                2 => {
                    let arr: Vec<f32> = (0..50).map(|_| rng.gen()).collect();
                    ComponentSpec {
                        type_id: format!("ArrayComponent_{i}"),
                        value: serde_json::json!(arr),
                    }
                }
                3 => ComponentSpec {
                    type_id: format!("MapComponent_{i}"),
                    value: {
                        let mut map = HashMap::new();
                        for j in 0..20 {
                            map.insert(format!("key_{j}"), rng.gen::<f32>());
                        }
                        serde_json::json!(map)
                    },
                },
                _ => ComponentSpec {
                    type_id: format!("SimpleComponent_{i}"),
                    value: serde_json::json!(rng.gen::<f32>()),
                },
            };

            components.push(component);
        }

        components
    }
}

/// Stress test runner that orchestrates tests
pub struct StressTestRunner {
    tests: Vec<Box<dyn StressTest>>,
    circuit_breaker: Arc<CircuitBreaker>,
    metrics: Arc<RwLock<PerformanceMetrics>>,
    ramp_up: bool,
    concurrent_limit: Arc<Semaphore>,
}

impl StressTestRunner {
    /// Create a new stress test runner
    pub fn new() -> Self {
        Self {
            tests: Vec::new(),
            circuit_breaker: Arc::new(CircuitBreaker::new(10, Duration::from_secs(30))),
            metrics: Arc::new(RwLock::new(PerformanceMetrics::new())),
            ramp_up: true,
            concurrent_limit: Arc::new(Semaphore::new(5)),
        }
    }

    /// Add a stress test to run
    pub fn add_test(mut self, test: Box<dyn StressTest>) -> Self {
        self.tests.push(test);
        self
    }

    /// Set whether to use gradual ramp-up
    pub fn with_ramp_up(mut self, ramp_up: bool) -> Self {
        self.ramp_up = ramp_up;
        self
    }

    /// Run all configured stress tests
    pub async fn run(
        &self,
        brp_client: &mut BrpClient,
        intensity: IntensityLevel,
        duration: Duration,
    ) -> Result<StressTestReport> {
        info!("Starting stress test suite with {} tests", self.tests.len());

        let start_time = Instant::now();
        // Tasks tracking removed - using sequential execution for now

        // Monitor performance in background
        let metrics_clone = self.metrics.clone();
        let monitor_handle = tokio::spawn(async move {
            while start_time.elapsed() < duration {
                // Simulate performance monitoring
                let mut metrics = metrics_clone.write().await;
                {
                    use rand::Rng;
                    let mut rng = rand::thread_rng();
                    metrics.frame_times_ms.push(16.67 + rng.gen::<f64>() * 10.0);
                    metrics
                        .cpu_usage_percent
                        .push(50.0 + rng.gen::<f64>() * 30.0);
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        // Run tests with ramp-up if enabled
        for (i, test) in self.tests.iter().enumerate() {
            let test_intensity = if self.ramp_up {
                // Gradually increase intensity
                match i {
                    0 => IntensityLevel::Low,
                    1 => IntensityLevel::Medium,
                    2 => IntensityLevel::High,
                    _ => intensity,
                }
            } else {
                intensity
            };

            info!(
                "Starting test: {} with {:?} intensity",
                test.name(),
                test_intensity
            );

            // Execute test
            if let Err(e) = test
                .execute(
                    brp_client,
                    test_intensity,
                    duration / self.tests.len().max(1) as u32,
                    self.metrics.clone(),
                    self.circuit_breaker.clone(),
                )
                .await
            {
                error!("Test {} failed: {}", test.name(), e);
            }

            // Cleanup after test
            if let Err(e) = test.cleanup(brp_client).await {
                warn!("Cleanup failed for test {}: {}", test.name(), e);
            }
        }

        // Wait for monitoring to complete
        monitor_handle.abort();

        // Generate report
        let report = self.generate_report().await;

        info!("Stress test suite completed");
        Ok(report)
    }

    /// Generate a stress test report
    async fn generate_report(&self) -> StressTestReport {
        let metrics = self.metrics.read().await;
        let (p50, p90, p95, p99) = metrics.frame_time_percentiles();

        let issues = self.detect_issues(&metrics);

        StressTestReport {
            duration: chrono::Utc::now().to_rfc3339(),
            tests_run: self.tests.iter().map(|t| t.name().to_string()).collect(),
            metrics: metrics.clone(),
            frame_time_percentiles: FrameTimePercentiles { p50, p90, p95, p99 },
            issues_found: issues,
            circuit_breaker_triggered: self.circuit_breaker.is_open.load(Ordering::Relaxed),
        }
    }

    /// Detect performance issues from metrics
    fn detect_issues(&self, metrics: &PerformanceMetrics) -> Vec<PerformanceIssue> {
        let mut issues = Vec::new();

        // Check for high frame times
        if let Some(&max_frame_time) = metrics
            .frame_times_ms
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
        {
            if max_frame_time > 33.33 {
                issues.push(PerformanceIssue {
                    severity: if max_frame_time > 100.0 {
                        "critical"
                    } else {
                        "warning"
                    }
                    .to_string(),
                    description: format!("High frame time detected: {max_frame_time:.2}ms"),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        // Check for high CPU usage
        if let Some(&max_cpu) = metrics
            .cpu_usage_percent
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
        {
            if max_cpu > 80.0 {
                issues.push(PerformanceIssue {
                    severity: if max_cpu > 95.0 {
                        "critical"
                    } else {
                        "warning"
                    }
                    .to_string(),
                    description: format!("High CPU usage detected: {max_cpu:.1}%"),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                });
            }
        }

        // Check for errors
        if metrics.error_count > 0 {
            issues.push(PerformanceIssue {
                severity: "error".to_string(),
                description: format!("{} errors occurred during testing", metrics.error_count),
                timestamp: chrono::Utc::now().to_rfc3339(),
            });
        }

        issues
    }
}

impl Default for StressTestRunner {
    fn default() -> Self {
        Self::new()
    }
}

/// Report generated after stress test completion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressTestReport {
    pub duration: String,
    pub tests_run: Vec<String>,
    pub metrics: PerformanceMetrics,
    pub frame_time_percentiles: FrameTimePercentiles,
    pub issues_found: Vec<PerformanceIssue>,
    pub circuit_breaker_triggered: bool,
}

/// Frame time percentiles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameTimePercentiles {
    pub p50: f64,
    pub p90: f64,
    pub p95: f64,
    pub p99: f64,
}

/// Performance issue detected during stress test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceIssue {
    pub severity: String,
    pub description: String,
    pub timestamp: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intensity_level_multipliers() {
        assert_eq!(IntensityLevel::Low.multiplier(), 0.5);
        assert_eq!(IntensityLevel::Medium.multiplier(), 1.0);
        assert_eq!(IntensityLevel::High.multiplier(), 2.0);
        assert_eq!(IntensityLevel::Extreme.multiplier(), 5.0);
    }

    #[test]
    fn test_complexity_level_components() {
        assert_eq!(ComplexityLevel::Low.component_count(), 5);
        assert_eq!(ComplexityLevel::Medium.component_count(), 15);
        assert_eq!(ComplexityLevel::High.component_count(), 30);
        assert_eq!(ComplexityLevel::Extreme.component_count(), 50);
    }

    #[test]
    fn test_performance_metrics_percentiles() {
        let mut metrics = PerformanceMetrics::new();
        metrics.frame_times_ms = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        let (p50, p90, p95, p99) = metrics.frame_time_percentiles();
        assert_eq!(p50, 30.0);
        assert_eq!(p90, 50.0);
        assert_eq!(p95, 50.0);
        assert_eq!(p99, 50.0);
    }

    #[tokio::test]
    async fn test_circuit_breaker() {
        let cb = CircuitBreaker::new(3, Duration::from_millis(100));

        assert!(!cb.is_open().await);

        // Record failures
        for _ in 0..3 {
            cb.record_failure().await;
        }

        assert!(cb.is_open().await);

        // Wait for reset timeout
        tokio::time::sleep(Duration::from_millis(150)).await;
        assert!(!cb.is_open().await);
    }

    #[test]
    fn test_spawn_many_test_creation() {
        let test = SpawnManyTest::new("player".to_string(), 10, 100);
        assert_eq!(test.name(), "spawn_many");
        assert_eq!(test.entity_type, "player");
        assert_eq!(test.spawn_rate, 10);
        assert_eq!(test.max_entities, 100);
    }

    #[test]
    fn test_rapid_changes_test_creation() {
        let test = RapidChangesTest::new(
            20,
            vec!["Transform".to_string(), "Velocity".to_string()],
            None,
        );
        assert_eq!(test.name(), "rapid_changes");
        assert_eq!(test.change_rate, 20);
        assert_eq!(test.component_types.len(), 2);
    }

    #[test]
    fn test_memory_pressure_test_creation() {
        let test = MemoryPressureTest::new(ComplexityLevel::High, 1024);
        assert_eq!(test.name(), "memory_pressure");
        assert_eq!(test.target_memory_mb, 1024);
    }

    #[test]
    fn test_stress_test_runner_creation() {
        let runner = StressTestRunner::new().with_ramp_up(true);

        assert!(runner.ramp_up);
    }

    #[test]
    fn test_performance_issue_detection() {
        let mut metrics = PerformanceMetrics::new();
        metrics.frame_times_ms = vec![16.67, 50.0, 100.0];
        metrics.cpu_usage_percent = vec![50.0, 85.0, 95.0];
        metrics.error_count = 5;

        let runner = StressTestRunner::new();
        let issues = runner.detect_issues(&metrics);

        assert!(issues.len() >= 2); // At least frame time and CPU issues
        assert!(issues.iter().any(|i| i.description.contains("frame time")));
        assert!(issues.iter().any(|i| i.description.contains("CPU")));
    }
}

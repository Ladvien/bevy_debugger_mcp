/// Test Game Process Management
/// 
/// Utilities for managing Bevy game processes during integration testing,
/// including starting, stopping, and communicating with test games.

use std::process::{Child, Command, Stdio};
use std::time::Duration;
use tokio::time::sleep;
use serde_json::{json, Value};

use bevy_debugger_mcp::{
    config::Config,
    brp_client::BrpClient,
    error::{Error, Result},
};

/// Manages a Bevy game process for testing
pub struct TestGameProcess {
    pub game_name: String,
    pub process: Option<Child>,
    pub brp_client: Option<BrpClient>,
    pub config: Config,
}

impl TestGameProcess {
    /// Create a new test game process manager
    pub async fn new(game_name: &str) -> Self {
        let config = Config::default();

        Self {
            game_name: game_name.to_string(),
            process: None,
            brp_client: None,
            config,
        }
    }

    /// Start the test game process
    pub async fn start(&mut self) -> Result<()> {
        // For testing, we'll simulate a game process
        // In a real implementation, this would start an actual Bevy game
        
        println!("Starting test game: {}", self.game_name);
        
        // Simulate starting the game process
        // In practice, you would run something like:
        // let child = Command::new("cargo")
        //     .args(&["run", "--bin", &self.game_name])
        //     .stdout(Stdio::piped())
        //     .stderr(Stdio::piped())
        //     .spawn()?;
        // self.process = Some(child);

        // Initialize BRP client for communication
        self.brp_client = Some(BrpClient::new(&self.config));
        
        // Wait for game to be ready
        self.wait_for_ready().await?;
        
        Ok(())
    }

    /// Wait for the game to be ready for testing
    pub async fn wait_for_ready(&mut self) -> Result<()> {
        let max_attempts = 30;
        let mut attempts = 0;

        while attempts < max_attempts {
            if let Some(ref mut client) = self.brp_client {
                // Try to connect to the game
                match client.connect_with_retry().await {
                    Ok(_) => {
                        println!("Test game {} is ready", self.game_name);
                        return Ok(());
                    }
                    Err(_) => {
                        attempts += 1;
                        sleep(Duration::from_secs(1)).await;
                    }
                }
            } else {
                return Err(Error::Connection("No BRP client initialized".to_string()));
            }
        }

        Err(Error::Timeout(format!(
            "Test game {} failed to start within {} seconds", 
            self.game_name, max_attempts
        )))
    }

    /// Send a command to the game via BRP
    pub async fn send_command(&mut self, method: &str, params: Value) -> Result<Value> {
        if let Some(ref mut client) = self.brp_client {
            // In a real implementation, this would send actual BRP requests
            // For testing, we'll simulate responses based on the method
            match method {
                "bevy_debugger/get_entity_count" => {
                    Ok(json!({
                        "total": 150,
                        "simple": 50,
                        "moving": 60,
                        "complex": 40
                    }))
                }
                "bevy_debugger/get_performance_metrics" => {
                    Ok(json!({
                        "average_frame_time": 0.016,
                        "fps": 60.0,
                        "entity_count": 150,
                        "uptime": 30.0
                    }))
                }
                "bevy_debugger/spawn_entities" => {
                    let count = params.get("count").and_then(|c| c.as_u64()).unwrap_or(10);
                    Ok(json!({
                        "spawned": count,
                        "success": true
                    }))
                }
                "bevy_debugger/stress_test" => {
                    let test_type = params.get("type").and_then(|t| t.as_str()).unwrap_or("entity_spawn");
                    let intensity = params.get("intensity").and_then(|i| i.as_u64()).unwrap_or(1);
                    
                    Ok(json!({
                        "stress_test": test_type,
                        "intensity": intensity,
                        "entities_affected": intensity * 50,
                        "success": true
                    }))
                }
                "bevy_debugger/query_entities" => {
                    Ok(json!({
                        "entities": [
                            {"id": "entity_1", "type": "warrior", "health": 100.0},
                            {"id": "entity_2", "type": "merchant", "health": 80.0},
                            {"id": "entity_3", "type": "villager", "health": 90.0}
                        ],
                        "total_found": 3
                    }))
                }
                _ => {
                    Ok(json!({
                        "method": method,
                        "params": params,
                        "simulated": true,
                        "timestamp": std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    }))
                }
            }
        } else {
            Err(Error::Connection("No BRP client available".to_string()))
        }
    }

    /// Check if the game process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut process) = self.process {
            match process.try_wait() {
                Ok(Some(_)) => false, // Process has exited
                Ok(None) => true,     // Process is still running
                Err(_) => false,      // Error checking process
            }
        } else {
            // For simulated processes, always return true during testing
            true
        }
    }

    /// Get the game's current performance metrics
    pub async fn get_performance_metrics(&mut self) -> Result<Value> {
        self.send_command("bevy_debugger/get_performance_metrics", json!({})).await
    }

    /// Spawn entities in the test game
    pub async fn spawn_entities(&mut self, entity_type: &str, count: usize) -> Result<Value> {
        self.send_command("bevy_debugger/spawn_entities", json!({
            "type": entity_type,
            "count": count
        })).await
    }

    /// Run a stress test scenario
    pub async fn run_stress_test(&mut self, test_type: &str, intensity: usize) -> Result<Value> {
        self.send_command("bevy_debugger/stress_test", json!({
            "type": test_type,
            "intensity": intensity
        })).await
    }

    /// Query entities with filters
    pub async fn query_entities(&mut self, filters: Value) -> Result<Value> {
        self.send_command("bevy_debugger/query_entities", filters).await
    }

    /// Wait for a specific condition to be met
    pub async fn wait_for_condition<F>(&mut self, condition: F, timeout: Duration) -> Result<()>
    where
        F: Fn(&Value) -> bool + Send,
    {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            match self.get_performance_metrics().await {
                Ok(metrics) => {
                    if condition(&metrics) {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Continue trying even if individual requests fail
                }
            }
            
            sleep(Duration::from_millis(100)).await;
        }
        
        Err(Error::Timeout("Condition not met within timeout".to_string()))
    }

    /// Cleanup and stop the test game
    pub async fn cleanup(&mut self) -> Result<()> {
        println!("Cleaning up test game: {}", self.game_name);
        
        if let Some(mut process) = self.process.take() {
            // Attempt graceful shutdown first
            if let Err(e) = process.kill() {
                println!("Failed to kill test game process: {}", e);
            }
            
            // Wait for process to exit
            if let Err(e) = process.wait() {
                println!("Error waiting for test game process to exit: {}", e);
            }
        }
        
        self.brp_client = None;
        
        Ok(())
    }
}

impl Drop for TestGameProcess {
    fn drop(&mut self) {
        // Ensure cleanup happens even if not called explicitly
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
        }
    }
}

/// Helper for running tests with a managed game process
pub async fn with_test_game<F, Fut, R>(game_name: &str, test_fn: F) -> Result<R>
where
    F: FnOnce(TestGameProcess) -> Fut,
    Fut: std::future::Future<Output = Result<R>>,
{
    let mut game_process = TestGameProcess::new(game_name).await;
    
    // Start the game
    game_process.start().await?;
    
    // Run the test
    let result = test_fn(game_process).await;
    
    // Cleanup is handled by Drop trait
    result
}

/// Configuration for test scenarios
#[derive(Debug, Clone)]
pub struct TestScenario {
    pub name: String,
    pub entity_count: usize,
    pub duration: Duration,
    pub stress_level: usize,
    pub expected_fps: f32,
    pub max_memory_mb: usize,
}

impl TestScenario {
    /// Create a basic performance test scenario
    pub fn basic_performance() -> Self {
        Self {
            name: "Basic Performance".to_string(),
            entity_count: 100,
            duration: Duration::from_secs(30),
            stress_level: 1,
            expected_fps: 30.0,
            max_memory_mb: 100,
        }
    }

    /// Create a high-stress test scenario
    pub fn high_stress() -> Self {
        Self {
            name: "High Stress".to_string(),
            entity_count: 1000,
            duration: Duration::from_secs(60),
            stress_level: 3,
            expected_fps: 15.0,
            max_memory_mb: 200,
        }
    }

    /// Create a memory pressure test scenario
    pub fn memory_pressure() -> Self {
        Self {
            name: "Memory Pressure".to_string(),
            entity_count: 2000,
            duration: Duration::from_secs(45),
            stress_level: 2,
            expected_fps: 10.0,
            max_memory_mb: 300,
        }
    }

    /// Run this scenario against a test game
    pub async fn run_against_game(&self, game_process: &mut TestGameProcess) -> Result<ScenarioResults> {
        println!("Running scenario: {}", self.name);
        
        let start_time = std::time::Instant::now();
        let mut measurements = Vec::new();
        
        // Spawn initial entities
        game_process.spawn_entities("complex", self.entity_count).await?;
        
        // Run stress test if specified
        if self.stress_level > 0 {
            game_process.run_stress_test("continuous_spawn", self.stress_level).await?;
        }
        
        // Collect measurements during the test
        while start_time.elapsed() < self.duration {
            let metrics = game_process.get_performance_metrics().await?;
            measurements.push(metrics);
            
            sleep(Duration::from_secs(1)).await;
        }
        
        Ok(ScenarioResults {
            scenario_name: self.name.clone(),
            duration: start_time.elapsed(),
            measurements,
            target_fps: self.expected_fps,
            max_memory_mb: self.max_memory_mb,
        })
    }
}

/// Results from running a test scenario
#[derive(Debug)]
pub struct ScenarioResults {
    pub scenario_name: String,
    pub duration: Duration,
    pub measurements: Vec<Value>,
    pub target_fps: f32,
    pub max_memory_mb: usize,
}

impl ScenarioResults {
    /// Calculate average FPS from measurements
    pub fn average_fps(&self) -> f32 {
        let total_fps: f32 = self.measurements
            .iter()
            .filter_map(|m| m.get("fps").and_then(|f| f.as_f64()).map(|f| f as f32))
            .sum();
        
        if self.measurements.is_empty() {
            0.0
        } else {
            total_fps / self.measurements.len() as f32
        }
    }

    /// Calculate minimum FPS
    pub fn min_fps(&self) -> f32 {
        self.measurements
            .iter()
            .filter_map(|m| m.get("fps").and_then(|f| f.as_f64()).map(|f| f as f32))
            .fold(f32::INFINITY, |min, fps| min.min(fps))
    }

    /// Calculate maximum entity count
    pub fn max_entities(&self) -> usize {
        self.measurements
            .iter()
            .filter_map(|m| m.get("entity_count").and_then(|c| c.as_u64()).map(|c| c as usize))
            .max()
            .unwrap_or(0)
    }

    /// Check if scenario passed performance targets
    pub fn meets_targets(&self) -> bool {
        let avg_fps = self.average_fps();
        let min_fps = self.min_fps();
        
        avg_fps >= self.target_fps && min_fps >= self.target_fps * 0.8
    }

    /// Generate a performance report
    pub fn generate_report(&self) -> String {
        format!(
            "Scenario: {}\n\
             Duration: {:.2}s\n\
             Average FPS: {:.1} (target: {:.1})\n\
             Min FPS: {:.1}\n\
             Max Entities: {}\n\
             Measurements: {}\n\
             Passed: {}",
            self.scenario_name,
            self.duration.as_secs_f32(),
            self.average_fps(),
            self.target_fps,
            self.min_fps(),
            self.max_entities(),
            self.measurements.len(),
            if self.meets_targets() { "✓" } else { "✗" }
        )
    }
}
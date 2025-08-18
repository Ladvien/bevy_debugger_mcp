use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::brp_client::BrpClient;
use crate::error::{Error, Result};
use crate::stress_test_system::{
    ComplexityLevel, IntensityLevel, MemoryPressureTest, RapidChangesTest, SpawnManyTest,
    StressTestRunner, StressTestType,
};

/// Handle stress tool requests
pub async fn handle(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    debug!("Stress tool called with arguments: {}", arguments);

    // Check connection
    let is_connected = {
        let client = brp_client.read().await;
        client.is_connected()
    };

    if !is_connected {
        warn!("BRP client not connected");
        return Ok(json!({
            "error": "BRP client not connected",
            "message": "Cannot run stress test - not connected to Bevy game",
            "brp_connected": false
        }));
    }

    // Parse action parameter
    let action_str = arguments
        .get("action")
        .and_then(|a| a.as_str())
        .unwrap_or("run");

    match action_str {
        "run" => handle_run(arguments, brp_client).await,
        "quick" => handle_quick_test(arguments, brp_client).await,
        "spawn_many" => handle_spawn_many(arguments, brp_client).await,
        "rapid_changes" => handle_rapid_changes(arguments, brp_client).await,
        "memory_pressure" => handle_memory_pressure(arguments, brp_client).await,
        "combined" => handle_combined(arguments, brp_client).await,
        _ => Ok(json!({
            "error": "Unknown action",
            "message": format!("Unknown action: {}", action_str),
            "available_actions": ["run", "quick", "spawn_many", "rapid_changes", "memory_pressure", "combined"]
        })),
    }
}

/// Handle run action - run configured stress tests
async fn handle_run(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    // Parse test configuration
    let test_type = parse_test_type(&arguments)?;
    let intensity = parse_intensity(&arguments)?;
    let duration = parse_duration(&arguments)?;
    let ramp_up = arguments
        .get("ramp_up")
        .and_then(|r| r.as_bool())
        .unwrap_or(true);

    info!(
        "Running stress test: {:?} with {:?} intensity for {:?}",
        test_type, intensity, duration
    );

    // Create runner
    let mut runner = StressTestRunner::new().with_ramp_up(ramp_up);

    // Add tests based on type
    runner = add_tests_to_runner(runner, &test_type);

    // Run tests
    let mut client = brp_client.write().await;
    let report = runner.run(&mut client, intensity, duration).await?;

    // Convert report to JSON
    Ok(json!({
        "success": true,
        "report": {
            "duration": report.duration,
            "tests_run": report.tests_run,
            "metrics": {
                "frame_times": {
                    "p50": report.frame_time_percentiles.p50,
                    "p90": report.frame_time_percentiles.p90,
                    "p95": report.frame_time_percentiles.p95,
                    "p99": report.frame_time_percentiles.p99,
                },
                "error_count": report.metrics.error_count,
                "warning_count": report.metrics.warning_count,
                "operations": report.metrics.operations_per_second.len(),
            },
            "issues_found": report.issues_found,
            "circuit_breaker_triggered": report.circuit_breaker_triggered,
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Handle quick test - run a brief stress test
async fn handle_quick_test(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    // Override with quick test parameters
    let mut args = arguments.clone();
    if let Some(obj) = args.as_object_mut() {
        obj.insert("intensity".to_string(), json!("low"));
        obj.insert("duration_seconds".to_string(), json!(10));
        obj.insert("ramp_up".to_string(), json!(false));

        // Default to spawn_many if no type specified
        if !obj.contains_key("test_type") {
            obj.insert("test_type".to_string(), json!("spawn_many"));
        }
    }

    handle_run(args, brp_client).await
}

/// Handle spawn_many test specifically
async fn handle_spawn_many(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    let entity_type = arguments
        .get("entity_type")
        .and_then(|e| e.as_str())
        .unwrap_or("player")
        .to_string();

    let spawn_rate = arguments
        .get("spawn_rate")
        .and_then(|r| r.as_u64())
        .unwrap_or(10) as usize;

    let max_entities = arguments
        .get("max_entities")
        .and_then(|m| m.as_u64())
        .unwrap_or(100) as usize;

    let intensity = parse_intensity(&arguments)?;
    let duration = parse_duration(&arguments)?;

    let test = SpawnManyTest::new(entity_type, spawn_rate, max_entities);
    let runner = StressTestRunner::new().add_test(Box::new(test));

    let mut client = brp_client.write().await;
    let report = runner.run(&mut client, intensity, duration).await?;

    Ok(json!({
        "success": true,
        "test": "spawn_many",
        "parameters": {
            "spawn_rate": spawn_rate,
            "max_entities": max_entities,
        },
        "results": {
            "entities_spawned": report.metrics.entity_count.last().unwrap_or(&0),
            "errors": report.metrics.error_count,
            "warnings": report.metrics.warning_count,
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Handle rapid_changes test specifically
async fn handle_rapid_changes(
    arguments: Value,
    brp_client: Arc<RwLock<BrpClient>>,
) -> Result<Value> {
    let change_rate = arguments
        .get("change_rate")
        .and_then(|r| r.as_u64())
        .unwrap_or(20) as usize;

    let component_types = arguments
        .get("component_types")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_else(|| vec!["Transform".to_string(), "Velocity".to_string()]);

    let target_entities = arguments
        .get("target_entities")
        .and_then(|t| t.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect());

    let intensity = parse_intensity(&arguments)?;
    let duration = parse_duration(&arguments)?;

    let test = RapidChangesTest::new(change_rate, component_types.clone(), target_entities);
    let runner = StressTestRunner::new().add_test(Box::new(test));

    let mut client = brp_client.write().await;
    let report = runner.run(&mut client, intensity, duration).await?;

    Ok(json!({
        "success": true,
        "test": "rapid_changes",
        "parameters": {
            "change_rate": change_rate,
            "component_types": component_types,
        },
        "results": {
            "total_operations": report.metrics.operations_per_second.iter().sum::<f64>() as usize,
            "errors": report.metrics.error_count,
            "warnings": report.metrics.warning_count,
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Handle memory_pressure test specifically
async fn handle_memory_pressure(
    arguments: Value,
    brp_client: Arc<RwLock<BrpClient>>,
) -> Result<Value> {
    let complexity = parse_complexity(&arguments)?;
    let target_memory_mb = arguments
        .get("target_memory_mb")
        .and_then(|t| t.as_u64())
        .unwrap_or(512) as usize;

    let intensity = parse_intensity(&arguments)?;
    let duration = parse_duration(&arguments)?;

    let test = MemoryPressureTest::new(complexity, target_memory_mb);
    let runner = StressTestRunner::new().add_test(Box::new(test));

    let mut client = brp_client.write().await;
    let report = runner.run(&mut client, intensity, duration).await?;

    Ok(json!({
        "success": true,
        "test": "memory_pressure",
        "parameters": {
            "complexity": format!("{:?}", complexity),
            "target_memory_mb": target_memory_mb,
        },
        "results": {
            "estimated_memory_mb": report.metrics.memory_usage_mb.last().unwrap_or(&0.0),
            "entities_created": report.metrics.entity_count.last().unwrap_or(&0),
            "errors": report.metrics.error_count,
            "warnings": report.metrics.warning_count,
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Handle combined stress test
async fn handle_combined(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    let intensity = parse_intensity(&arguments)?;
    let duration = parse_duration(&arguments)?;
    let ramp_up = arguments
        .get("ramp_up")
        .and_then(|r| r.as_bool())
        .unwrap_or(true);

    // Create all test types
    let spawn_test = SpawnManyTest::new("player".to_string(), 5, 50);
    let change_test = RapidChangesTest::new(10, vec!["Transform".to_string()], None);
    let memory_test = MemoryPressureTest::new(ComplexityLevel::Medium, 256);

    let runner = StressTestRunner::new()
        .with_ramp_up(ramp_up)
        .add_test(Box::new(spawn_test))
        .add_test(Box::new(change_test))
        .add_test(Box::new(memory_test));

    let mut client = brp_client.write().await;
    let report = runner.run(&mut client, intensity, duration).await?;

    Ok(json!({
        "success": true,
        "test": "combined",
        "tests_run": report.tests_run,
        "report": {
            "frame_times": {
                "p50": report.frame_time_percentiles.p50,
                "p90": report.frame_time_percentiles.p90,
                "p95": report.frame_time_percentiles.p95,
                "p99": report.frame_time_percentiles.p99,
            },
            "issues_found": report.issues_found,
            "circuit_breaker_triggered": report.circuit_breaker_triggered,
            "total_errors": report.metrics.error_count,
            "total_warnings": report.metrics.warning_count,
        },
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Parse test type from arguments
fn parse_test_type(arguments: &Value) -> Result<StressTestType> {
    let test_type_str = arguments
        .get("test_type")
        .and_then(|t| t.as_str())
        .unwrap_or("spawn_many");

    match test_type_str {
        "spawn_many" => {
            let entity_type = arguments
                .get("entity_type")
                .and_then(|e| e.as_str())
                .unwrap_or("player")
                .to_string();

            let spawn_rate = arguments
                .get("spawn_rate")
                .and_then(|r| r.as_u64())
                .unwrap_or(10) as usize;

            let max_entities = arguments
                .get("max_entities")
                .and_then(|m| m.as_u64())
                .unwrap_or(100) as usize;

            Ok(StressTestType::SpawnMany {
                entity_type,
                spawn_rate,
                max_entities,
            })
        }
        "rapid_changes" => {
            let change_rate = arguments
                .get("change_rate")
                .and_then(|r| r.as_u64())
                .unwrap_or(20) as usize;

            let component_types = arguments
                .get("component_types")
                .and_then(|c| c.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_else(|| vec!["Transform".to_string()]);

            let target_entities = arguments
                .get("target_entities")
                .and_then(|t| t.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_u64()).collect());

            Ok(StressTestType::RapidChanges {
                change_rate,
                component_types,
                target_entities,
            })
        }
        "memory_pressure" => {
            let complexity = parse_complexity(arguments)?;
            let target_memory_mb = arguments
                .get("target_memory_mb")
                .and_then(|t| t.as_u64())
                .unwrap_or(512) as usize;

            Ok(StressTestType::MemoryPressure {
                complexity,
                target_memory_mb,
            })
        }
        _ => Err(Error::Validation(format!(
            "Unknown test type: {test_type_str}"
        ))),
    }
}

/// Parse intensity level from arguments
fn parse_intensity(arguments: &Value) -> Result<IntensityLevel> {
    let intensity_str = arguments
        .get("intensity")
        .and_then(|i| i.as_str())
        .unwrap_or("medium");

    match intensity_str {
        "low" => Ok(IntensityLevel::Low),
        "medium" => Ok(IntensityLevel::Medium),
        "high" => Ok(IntensityLevel::High),
        "extreme" => Ok(IntensityLevel::Extreme),
        _ => Err(Error::Validation(format!(
            "Unknown intensity level: {intensity_str}"
        ))),
    }
}

/// Parse complexity level from arguments
fn parse_complexity(arguments: &Value) -> Result<ComplexityLevel> {
    let complexity_str = arguments
        .get("complexity")
        .and_then(|c| c.as_str())
        .unwrap_or("medium");

    match complexity_str {
        "low" => Ok(ComplexityLevel::Low),
        "medium" => Ok(ComplexityLevel::Medium),
        "high" => Ok(ComplexityLevel::High),
        "extreme" => Ok(ComplexityLevel::Extreme),
        _ => Err(Error::Validation(format!(
            "Unknown complexity level: {complexity_str}"
        ))),
    }
}

/// Parse duration from arguments
fn parse_duration(arguments: &Value) -> Result<Duration> {
    let duration_secs = arguments
        .get("duration_seconds")
        .and_then(|d| d.as_u64())
        .unwrap_or(30);

    Ok(Duration::from_secs(duration_secs))
}

/// Add tests to runner based on test type
fn add_tests_to_runner(
    mut runner: StressTestRunner,
    test_type: &StressTestType,
) -> StressTestRunner {
    match test_type {
        StressTestType::SpawnMany {
            entity_type,
            spawn_rate,
            max_entities,
        } => runner.add_test(Box::new(SpawnManyTest::new(
            entity_type.clone(),
            *spawn_rate,
            *max_entities,
        ))),
        StressTestType::RapidChanges {
            change_rate,
            component_types,
            target_entities,
        } => runner.add_test(Box::new(RapidChangesTest::new(
            *change_rate,
            component_types.clone(),
            target_entities.clone(),
        ))),
        StressTestType::MemoryPressure {
            complexity,
            target_memory_mb,
        } => runner.add_test(Box::new(MemoryPressureTest::new(
            *complexity,
            *target_memory_mb,
        ))),
        StressTestType::Combined { tests } => {
            for test in tests {
                runner = add_tests_to_runner(runner, test);
            }
            runner
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_handle_without_connection() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));

        let args = json!({
            "action": "run",
            "test_type": "spawn_many"
        });

        let result = handle(args, brp_client).await.unwrap();
        assert_eq!(result.get("error").unwrap(), "BRP client not connected");
    }

    #[test]
    fn test_parse_intensity() {
        let args = json!({"intensity": "high"});
        let intensity = parse_intensity(&args).unwrap();
        assert!(matches!(intensity, IntensityLevel::High));

        let args = json!({});
        let intensity = parse_intensity(&args).unwrap();
        assert!(matches!(intensity, IntensityLevel::Medium));
    }

    #[test]
    fn test_parse_complexity() {
        let args = json!({"complexity": "extreme"});
        let complexity = parse_complexity(&args).unwrap();
        assert!(matches!(complexity, ComplexityLevel::Extreme));

        let args = json!({});
        let complexity = parse_complexity(&args).unwrap();
        assert!(matches!(complexity, ComplexityLevel::Medium));
    }

    #[test]
    fn test_parse_duration() {
        let args = json!({"duration_seconds": 60});
        let duration = parse_duration(&args).unwrap();
        assert_eq!(duration, Duration::from_secs(60));

        let args = json!({});
        let duration = parse_duration(&args).unwrap();
        assert_eq!(duration, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_test_type_spawn_many() {
        let args = json!({
            "test_type": "spawn_many",
            "entity_type": "enemy",
            "spawn_rate": 20,
            "max_entities": 200
        });

        let test_type = parse_test_type(&args).unwrap();
        match test_type {
            StressTestType::SpawnMany {
                entity_type,
                spawn_rate,
                max_entities,
            } => {
                assert_eq!(entity_type, "enemy");
                assert_eq!(spawn_rate, 20);
                assert_eq!(max_entities, 200);
            }
            _ => panic!("Wrong test type"),
        }
    }

    #[test]
    fn test_parse_test_type_rapid_changes() {
        let args = json!({
            "test_type": "rapid_changes",
            "change_rate": 50,
            "component_types": ["Transform", "Health"]
        });

        let test_type = parse_test_type(&args).unwrap();
        match test_type {
            StressTestType::RapidChanges {
                change_rate,
                component_types,
                ..
            } => {
                assert_eq!(change_rate, 50);
                assert_eq!(component_types.len(), 2);
            }
            _ => panic!("Wrong test type"),
        }
    }
}

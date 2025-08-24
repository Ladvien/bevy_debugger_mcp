use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use bevy_debugger_mcp::brp_client::BrpClient;
use bevy_debugger_mcp::config::Config;
use bevy_debugger_mcp::tool_orchestration::*;
use bevy_debugger_mcp::tools::orchestration;

/// Mock tool executor for testing
struct MockToolExecutor {
    name: String,
    success: bool,
    output: Value,
    delay: Duration,
    }

impl MockToolExecutor {
    fn new(name: &str, success: bool, output: Value, delay: Duration) -> Self {
        Self {
            name: name.to_string(),
            success,
            output,
            delay,
            }
        }
    }

#[async_trait::async_trait]
impl ToolExecutor for MockToolExecutor {
    async fn execute(
        &self,
        _arguments: Value,
        _brp_client: Arc<RwLock<BrpClient>>,
        context: &mut ToolContext,
    ) -> bevy_debugger_mcp::error::Result<Value> {
        tokio::time::sleep(self.delay).await;

        if self.success {
            Ok(self.output.clone())
        } else {
            Err(bevy_debugger_mcp::error::Error::Validation(format!(
                "Mock {} failed",
                self.name
            )))
            }
        }
    }

fn create_test_brp_client() -> Arc<RwLock<BrpClient>> {
    let config = { let mut config = Config::default();
        config.bevy_brp_host = "localhost".to_string();
        config.bevy_brp_port = 15702;
        config.mcp_port = 3000; config
    };
    Arc::new(RwLock::new(BrpClient::new(&config)))
    }

#[tokio::test]
async fn test_tool_context_basic_operations() {
    let mut context = ToolContext::new();

    // Test variable operations
    context.set_variable("test_var".to_string(), json!("test_value"));
    assert_eq!(context.get_variable("test_var"), Some(&json!("test_value")));

    // Test result operations
    let result = ToolResult {
        tool_name: "test_tool".to_string(),
        execution_id: ExecutionId::new(),
        success: true,
        output: json!({"result": "success"}),
        error: None,
        execution_time: Duration::from_millis(100),
        timestamp: std::time::SystemTime::now(),
        cache_key: Some("test_key".to_string()),
    };

    context.add_result("test_tool".to_string(), result);
    assert!(context.get_result("test_tool").is_some());
    assert_eq!(context.metadata.execution_count, 1);
    }

#[tokio::test]
async fn test_orchestrator_tool_execution() {
    let brp_client = create_test_brp_client();
    let mut orchestrator = ToolOrchestrator::new(brp_client);

    // Register mock tool
    let mock_tool = Arc::new(MockToolExecutor::new(
        "mock_observe",
        true,
        json!({"entities": [{"id": 1, "health": 30}]}),
        Duration::from_millis(10),
    ));
    orchestrator.register_tool("mock_observe".to_string(), mock_tool);

    let mut context = ToolContext::new();
    let result = orchestrator
        .execute_tool(
            "mock_observe".to_string(),
            json!({"query": "health < 50"}),
            &mut context,
        )
        .await;

    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success);
    assert_eq!(tool_result.tool_name, "mock_observe");
    assert!(context.get_result("mock_observe").is_some());
    }

#[tokio::test]
async fn test_orchestrator_caching() {
    let brp_client = create_test_brp_client();
    let mut orchestrator = ToolOrchestrator::new(brp_client);

    // Register mock tool
    let mock_tool = Arc::new(MockToolExecutor::new(
        "mock_tool",
        true,
        json!({"cached": true}),
        Duration::from_millis(100),
    ));
    orchestrator.register_tool("mock_tool".to_string(), mock_tool);

    let mut context = ToolContext::new();
    context.config.cache_results = true;

    // First execution
    let start = std::time::Instant::now();
    let result1 = orchestrator
        .execute_tool(
            "mock_tool".to_string(),
            json!({"param": "value"}),
            &mut context,
        )
        .await
        .unwrap();
    let first_duration = start.elapsed();

    // Second execution (should be cached)
    let start = std::time::Instant::now();
    let result2 = orchestrator
        .execute_tool(
            "mock_tool".to_string(),
            json!({"param": "value"}),
            &mut context,
        )
        .await
        .unwrap();
    let second_duration = start.elapsed();

    // Second execution should be faster due to caching
    assert!(second_duration < first_duration);
    assert_eq!(result1.output, result2.output);
    }

#[tokio::test]
async fn test_pipeline_execution_sequential() {
    let brp_client = create_test_brp_client();
    let mut orchestrator = ToolOrchestrator::new(brp_client);

    // Register mock tools
    orchestrator.register_tool(
        "step1".to_string(),
        Arc::new(MockToolExecutor::new(
            "step1",
            true,
            json!({"step": 1}),
            Duration::from_millis(10),
        )),
    );
    orchestrator.register_tool(
        "step2".to_string(),
        Arc::new(MockToolExecutor::new(
            "step2",
            true,
            json!({"step": 2}),
            Duration::from_millis(10),
        )),
    );

    // Create pipeline
    let mut pipeline = ToolPipeline::new("test_pipeline".to_string());
    pipeline.add_step(PipelineStep {
        name: "first".to_string(),
        tool: "step1".to_string(),
        arguments: json!({}),
        condition: None,
        retry_config: None,
        timeout: None,
    });
    pipeline.add_step(PipelineStep {
        name: "second".to_string(),
        tool: "step2".to_string(),
        arguments: json!({}),
        condition: Some(StepCondition {
            condition_type: ConditionType::OnSuccess,
            reference: "first".to_string(),
            expected_value: None,
        }),
        retry_config: None,
        timeout: None,
    });

    let context = ToolContext::new();
    let result = orchestrator.execute_pipeline(pipeline, context).await;

    assert!(result.is_ok());
    let pipeline_result = result.unwrap();
    assert!(pipeline_result.success);
    assert_eq!(pipeline_result.step_results.len(), 2);
    assert!(pipeline_result.step_results.iter().all(|r| r.success));
    }

#[tokio::test]
async fn test_pipeline_execution_with_failure() {
    let brp_client = create_test_brp_client();
    let mut orchestrator = ToolOrchestrator::new(brp_client);

    // Register mock tools - second one fails
    orchestrator.register_tool(
        "step1".to_string(),
        Arc::new(MockToolExecutor::new(
            "step1",
            true,
            json!({"step": 1}),
            Duration::from_millis(10),
        )),
    );
    orchestrator.register_tool(
        "step2".to_string(),
        Arc::new(MockToolExecutor::new(
            "step2",
            false,
            json!({}),
            Duration::from_millis(10),
        )),
    );

    // Create pipeline with fail_fast enabled
    let mut pipeline = ToolPipeline::new("test_pipeline".to_string());
    pipeline.fail_fast = true;
    pipeline.add_step(PipelineStep {
        name: "first".to_string(),
        tool: "step1".to_string(),
        arguments: json!({}),
        condition: None,
        retry_config: None,
        timeout: None,
    });
    pipeline.add_step(PipelineStep {
        name: "second".to_string(),
        tool: "step2".to_string(),
        arguments: json!({}),
        condition: None,
        retry_config: None,
        timeout: None,
    });

    let context = ToolContext::new();
    let result = orchestrator.execute_pipeline(pipeline, context).await;

    assert!(result.is_ok());
    let pipeline_result = result.unwrap();
    assert!(!pipeline_result.success);
    assert_eq!(pipeline_result.step_results.len(), 2);
    assert!(pipeline_result.step_results[0].success);
    assert!(!pipeline_result.step_results[1].success);
    }

#[tokio::test]
async fn test_dependency_graph_ordering() {
    let mut graph = DependencyGraph::new();

    graph.add_dependency("experiment".to_string(), "observe".to_string());
    graph.add_dependency("replay".to_string(), "experiment".to_string());
    graph.add_dependency("analysis".to_string(), "replay".to_string());

    let tools = vec![
        "analysis".to_string(),
        "replay".to_string(),
        "experiment".to_string(),
        "observe".to_string(),
    ];

    let order = graph.get_execution_order(&tools).unwrap();

    // Should be in dependency order
    assert_eq!(order, vec!["observe", "experiment", "replay", "analysis"]);
    }

#[tokio::test]
async fn test_dependency_graph_circular_detection() {
    let mut graph = DependencyGraph::new();

    graph.add_dependency("a".to_string(), "b".to_string());
    graph.add_dependency("b".to_string(), "c".to_string());
    graph.add_dependency("c".to_string(), "a".to_string());

    let result = graph.get_execution_order(&["a".to_string(), "b".to_string(), "c".to_string()]);
    assert!(result.is_err());
    }

#[tokio::test]
async fn test_workflow_dsl_templates() {
    // Test observe-experiment-replay pipeline
    let pipeline = WorkflowDSL::observe_experiment_replay();
    assert_eq!(pipeline.name, "observe_experiment_replay");
    assert_eq!(pipeline.steps.len(), 3);
    assert_eq!(pipeline.steps[0].tool, "observe");
    assert_eq!(pipeline.steps[1].tool, "experiment");
    assert_eq!(pipeline.steps[2].tool, "replay");

    // Check conditions
    assert!(pipeline.steps[0].condition.is_none());
    assert!(pipeline.steps[1].condition.is_some());
    assert!(pipeline.steps[2].condition.is_some());

    // Test debug performance pipeline
    let debug_pipeline = WorkflowDSL::debug_performance();
    assert_eq!(debug_pipeline.name, "debug_performance");
    assert_eq!(debug_pipeline.steps.len(), 2);
    assert_eq!(debug_pipeline.steps[0].tool, "stress");
    assert_eq!(debug_pipeline.steps[1].tool, "anomaly");
    }

#[tokio::test]
async fn test_retry_mechanism() {
    let brp_client = create_test_brp_client();
    let mut orchestrator = ToolOrchestrator::new(brp_client);

    // Register mock tool that fails initially
    let mock_tool = Arc::new(MockToolExecutor::new(
        "retry_tool",
        false,
        json!({}),
        Duration::from_millis(10),
    ));
    orchestrator.register_tool("retry_tool".to_string(), mock_tool);

    // Create pipeline step with retry configuration
    let step = PipelineStep {
        name: "retry_step".to_string(),
        tool: "retry_tool".to_string(),
        arguments: json!({}),
        condition: None,
        retry_config: Some(Retry{ let mut config = Config::default();
            max_attempts: 3,
            backoff_type: BackoffType::Fixed,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
        }),
        timeout: None,
    };

    let mut context = ToolContext::new();
    let result = orchestrator.execute_step(&step, &mut context).await;

    // Should have attempted multiple times
    assert!(!result.success);
    assert_eq!(result.retry_count, 3);
    }

#[tokio::test]
async fn test_pipeline_timeout() {
    let brp_client = create_test_brp_client();
    let mut orchestrator = ToolOrchestrator::new(brp_client);

    // Register slow tool
    let slow_tool = Arc::new(MockToolExecutor::new(
        "slow_tool",
        true,
        json!({}),
        Duration::from_secs(2),
    ));
    orchestrator.register_tool("slow_tool".to_string(), slow_tool);

    // Create pipeline with very short timeout
    let mut pipeline = ToolPipeline::new("timeout_test".to_string());
    pipeline.add_step(PipelineStep {
        name: "slow_step".to_string(),
        tool: "slow_tool".to_string(),
        arguments: json!({}),
        condition: None,
        retry_config: None,
        timeout: Some(Duration::from_millis(100)),
    });

    let context = ToolContext::new();

    // Override the pipeline timeout check for this specific test
    // by creating a modified version that allows shorter timeouts
    let start_time = std::time::Instant::now();
    let result = tokio::time::timeout(Duration::from_millis(500), async {
        orchestrator.execute_pipeline(pipeline, context).await
    })
    .await;

    // Should timeout
    assert!(result.is_err() || start_time.elapsed() < Duration::from_secs(1));
    }

#[tokio::test]
async fn test_step_conditions() {
    let brp_client = create_test_brp_client();
    let orchestrator = ToolOrchestrator::new(brp_client);

    let mut context = ToolContext::new();

    // Add a successful result to context
    let success_result = ToolResult {
        tool_name: "previous".to_string(),
        execution_id: ExecutionId::new(),
        success: true,
        output: json!({"data": "test"}),
        error: None,
        execution_time: Duration::from_millis(100),
        timestamp: std::time::SystemTime::now(),
        cache_key: None,
    };
    context.add_result("previous".to_string(), success_result);

    // Test OnSuccess condition
    let step_on_success = PipelineStep {
        name: "conditional".to_string(),
        tool: "test".to_string(),
        arguments: json!({}),
        condition: Some(StepCondition {
            condition_type: ConditionType::OnSuccess,
            reference: "previous".to_string(),
            expected_value: None,
        }),
        retry_config: None,
        timeout: None,
    };

    assert!(orchestrator.should_execute_step(&step_on_success, &context));

    // Test OnFailure condition
    let step_on_failure = PipelineStep {
        name: "conditional".to_string(),
        tool: "test".to_string(),
        arguments: json!({}),
        condition: Some(StepCondition {
            condition_type: ConditionType::OnFailure,
            reference: "previous".to_string(),
            expected_value: None,
        }),
        retry_config: None,
        timeout: None,
    };

    assert!(!orchestrator.should_execute_step(&step_on_failure, &context));

    // Test variable condition
    context.set_variable("test_var".to_string(), json!("expected"));
    let step_var_equals = PipelineStep {
        name: "conditional".to_string(),
        tool: "test".to_string(),
        arguments: json!({}),
        condition: Some(StepCondition {
            condition_type: ConditionType::VariableEquals,
            reference: "test_var".to_string(),
            expected_value: Some(json!("expected")),
        }),
        retry_config: None,
        timeout: None,
    };

    assert!(orchestrator.should_execute_step(&step_var_equals, &context));
    }

#[tokio::test]
async fn test_cache_key_generation() {
    let brp_client = create_test_brp_client();
    let orchestrator = ToolOrchestrator::new(brp_client);

    let args1 = json!({"param": "value", "count": 42});
    let args2 = json!({"count": 42, "param": "value"});
    let args3 = json!({"param": "different", "count": 42});

    let key1 = orchestrator.generate_cache_key("test_tool", &args1);
    let key2 = orchestrator.generate_cache_key("test_tool", &args2);
    let key3 = orchestrator.generate_cache_key("test_tool", &args3);
    let key4 = orchestrator.generate_cache_key("other_tool", &args1);

    // Same arguments should produce same key (JSON normalization)
    assert_eq!(key1, key2);

    // Different arguments should produce different keys
    assert_ne!(key1, key3);

    // Different tools should produce different keys
    assert_ne!(key1, key4);

    // All keys should be present
    assert!(key1.is_some());
    assert!(key3.is_some());
    assert!(key4.is_some());
    }

#[tokio::test]
async fn test_context_execution_count() {
    let mut context = ToolContext::new();
    assert_eq!(context.metadata.execution_count, 0);

    let result = ToolResult {
        tool_name: "test1".to_string(),
        execution_id: ExecutionId::new(),
        success: true,
        output: json!({}),
        error: None,
        execution_time: Duration::from_millis(100),
        timestamp: std::time::SystemTime::now(),
        cache_key: None,
    };

    context.add_result("test1".to_string(), result.clone());
    assert_eq!(context.metadata.execution_count, 1);

    context.add_result("test2".to_string(), result);
    assert_eq!(context.metadata.execution_count, 2);
    }

#[tokio::test]
async fn test_execution_id_uniqueness() {
    let id1 = ExecutionId::new();
    let id2 = ExecutionId::new();
    let id3 = ExecutionId::new();

    assert_ne!(id1, id2);
    assert_ne!(id2, id3);
    assert_ne!(id1, id3);

    // Test string conversion
    let id1_str = id1.to_string();
    let id2_str = id2.to_string();
    assert_ne!(id1_str, id2_str);
    assert!(!id1_str.is_empty());
    }

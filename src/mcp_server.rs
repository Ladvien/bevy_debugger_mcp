use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::brp_client::BrpClient;
use crate::checkpoint::{CheckpointConfig, CheckpointManager};
use crate::config::Config;
use crate::dead_letter_queue::{DeadLetterConfig, DeadLetterQueue};
use crate::diagnostics::{create_bug_report, DiagnosticCollector};
use crate::error::{Error, ErrorContext, ErrorSeverity, Result};
use crate::resource_manager::{ResourceConfig, ResourceManager};
use crate::tool_orchestration::{ToolContext, ToolOrchestrator, ToolPipeline};
use crate::tools::{anomaly, experiment, hypothesis, observe, orchestration, replay, stress};

pub struct McpServer {
    config: Config,
    brp_client: Arc<RwLock<BrpClient>>,
    orchestrator: Arc<RwLock<ToolOrchestrator>>,
    resource_manager: Arc<RwLock<ResourceManager>>,
    dead_letter_queue: Arc<RwLock<DeadLetterQueue>>,
    diagnostic_collector: Arc<DiagnosticCollector>,
    checkpoint_manager: Arc<RwLock<CheckpointManager>>,
    debug_mode: bool,
}

impl McpServer {
    pub fn new(config: Config, brp_client: Arc<RwLock<BrpClient>>) -> Self {
        let orchestrator = orchestration::create_orchestrator(brp_client.clone());
        let resource_manager = ResourceManager::new(ResourceConfig::default());

        // Initialize error recovery and diagnostic systems
        let dead_letter_queue = DeadLetterQueue::new(DeadLetterConfig::default());
        let diagnostic_collector = Arc::new(DiagnosticCollector::new(100)); // Keep 100 recent errors
        let checkpoint_manager = CheckpointManager::new(CheckpointConfig::default());

        // Check for debug mode from environment
        let debug_mode = std::env::var("DEBUG_MODE")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        if debug_mode {
            info!("Debug mode enabled - verbose logging and diagnostics active");
        }

        McpServer {
            config,
            brp_client,
            orchestrator: Arc::new(RwLock::new(orchestrator)),
            resource_manager: Arc::new(RwLock::new(resource_manager)),
            dead_letter_queue: Arc::new(RwLock::new(dead_letter_queue)),
            diagnostic_collector,
            checkpoint_manager: Arc::new(RwLock::new(checkpoint_manager)),
            debug_mode,
        }
    }

    pub async fn start(&self) -> Result<()> {
        // Start all systems
        {
            let mut rm = self.resource_manager.write().await;
            rm.start_monitoring().await?;
        }

        {
            let mut dlq = self.dead_letter_queue.write().await;
            dlq.start().await?;
        }

        {
            let mut cm = self.checkpoint_manager.write().await;
            cm.start().await?;
        }

        info!("MCP Server started with error recovery and diagnostic systems");
        if self.debug_mode {
            info!("Debug mode active - enhanced logging enabled");
        }
        Ok(())
    }

    pub async fn run(&self, listener: TcpListener) -> Result<()> {
        loop {
            match listener.accept().await {
                Ok((stream, addr)) => {
                    info!("New MCP connection from: {}", addr);
                    let server = self.clone();
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(stream).await {
                            error!("Error handling MCP connection: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to accept connection: {}", e);
                }
            }
        }
    }

    async fn handle_connection(&self, _stream: TcpStream) -> Result<()> {
        debug!("Handling MCP connection - performing handshake");

        let capabilities = json!({
            "capabilities": {
                "tools": {
                    "listChanged": false
                }
            },
            "serverInfo": {
                "name": "bevy-debugger-mcp",
                "version": "0.1.0"
            }
        });

        debug!(
            "MCP handshake completed with capabilities: {}",
            capabilities
        );

        Ok(())
    }

    pub async fn handle_tool_call(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        debug!("Handling tool call: {} with args: {}", tool_name, arguments);

        // Clone arguments for error reporting later
        let args_for_error = arguments.clone();

        let result = match tool_name {
            "observe" => observe::handle(arguments, self.brp_client.clone()).await,
            "experiment" => experiment::handle(arguments, self.brp_client.clone()).await,
            "hypothesis" => hypothesis::handle(arguments, self.brp_client.clone()).await,
            "stress" => stress::handle(arguments, self.brp_client.clone()).await,
            "replay" => replay::handle(arguments, self.brp_client.clone()).await,
            "anomaly" => anomaly::handle(arguments, self.brp_client.clone()).await,
            "orchestrate" => self.handle_orchestration(arguments).await,
            "pipeline" => self.handle_pipeline_execution(arguments).await,
            "resource_metrics" => self.handle_resource_metrics(arguments).await,
            "performance_dashboard" => self.handle_performance_dashboard(arguments).await,
            "health_check" => self.handle_health_check(arguments).await,
            // New diagnostic and error recovery endpoints
            "dead_letter_queue" => self.handle_dead_letter_queue(arguments).await,
            "diagnostic_report" => self.handle_diagnostic_report(arguments).await,
            "checkpoint" => self.handle_checkpoint(arguments).await,
            "bug_report" => self.handle_bug_report(arguments).await,
            _ => Err(Error::Mcp(format!("Unknown tool: {tool_name}"))),
        };

        // Record errors for diagnostics
        if let Err(ref error) = result {
            let error_context = ErrorContext::new(tool_name, "mcp_server")
                .add_cause(&error.to_string())
                .add_context("tool", tool_name)
                .add_context("arguments", &format!("{args_for_error}"))
                .set_retryable(true)
                .set_severity(ErrorSeverity::Error);

            self.diagnostic_collector.record_error(error_context);

            if self.debug_mode {
                warn!("Tool call failed: {} - {}", tool_name, error);
            }
        }

        result
    }

    /// Handle orchestration tool calls
    async fn handle_orchestration(&self, arguments: Value) -> Result<Value> {
        let mut context = ToolContext::new();

        // Extract tool and arguments from request
        let tool = arguments
            .get("tool")
            .and_then(|t| t.as_str())
            .ok_or_else(|| Error::Validation("Missing 'tool' field".to_string()))?;

        let tool_args = arguments.get("arguments").unwrap_or(&Value::Null).clone();

        // Apply context configuration if provided
        if let Some(config) = arguments.get("config") {
            if let Some(auto_record) = config.get("auto_record").and_then(|v| v.as_bool()) {
                context.config.auto_record = auto_record;
            }
            if let Some(auto_experiment) = config.get("auto_experiment").and_then(|v| v.as_bool()) {
                context.config.auto_experiment = auto_experiment;
            }
            if let Some(cache_results) = config.get("cache_results").and_then(|v| v.as_bool()) {
                context.config.cache_results = cache_results;
            }
        }

        let mut orchestrator = self.orchestrator.write().await;
        let result = orchestrator
            .execute_tool(tool.to_string(), tool_args, &mut context)
            .await?;

        // Sanitize context before returning - remove sensitive data
        let sanitized_context = json!({
            "execution_id": context.execution_id,
            "execution_count": context.metadata.execution_count,
            "result_count": context.results.len(),
            "variable_count": context.variables.len(),
            "config": {
                "auto_record": context.config.auto_record,
                "auto_experiment": context.config.auto_experiment,
                "cache_results": context.config.cache_results
            }
        });

        Ok(json!({
            "tool_result": result,
            "context": sanitized_context
        }))
    }

    /// Handle pipeline execution
    async fn handle_pipeline_execution(&self, arguments: Value) -> Result<Value> {
        let context = ToolContext::new();

        // Check if this is a template pipeline or custom pipeline
        if let Some(template_name) = arguments.get("template").and_then(|t| t.as_str()) {
            let mut orchestrator = self.orchestrator.write().await;

            // Get pipeline template (this would need to be implemented in orchestrator)
            let pipeline = match template_name {
                "observe_experiment_replay" => {
                    crate::tool_orchestration::WorkflowDSL::observe_experiment_replay()
                }
                "debug_performance" => crate::tool_orchestration::WorkflowDSL::debug_performance(),
                _ => {
                    return Err(Error::Validation(format!(
                        "Unknown pipeline template: {template_name}"
                    )))
                }
            };

            let result = orchestrator.execute_pipeline(pipeline, context).await?;

            Ok(json!({
                "pipeline_result": result
            }))
        } else if let Some(pipeline_data) = arguments.get("pipeline") {
            // Custom pipeline execution with validation
            let pipeline: ToolPipeline = serde_json::from_value(pipeline_data.clone())
                .map_err(|e| Error::Validation(format!("Invalid pipeline format: {e}")))?;

            // Validate pipeline constraints
            if pipeline.steps.len() > 50 {
                return Err(Error::Validation(
                    "Pipeline too complex: maximum 50 steps allowed".to_string(),
                ));
            }

            for step in &pipeline.steps {
                if step.timeout.unwrap_or(Duration::from_secs(300)) > Duration::from_secs(600) {
                    return Err(Error::Validation(format!(
                        "Step '{}' timeout too long: maximum 10 minutes allowed",
                        step.name
                    )));
                }

                // Validate tool names against known tools
                if ![
                    "observe",
                    "experiment",
                    "hypothesis",
                    "stress",
                    "replay",
                    "anomaly",
                ]
                .contains(&step.tool.as_str())
                {
                    return Err(Error::Validation(format!(
                        "Unknown tool '{}' in step '{}'",
                        step.tool, step.name
                    )));
                }
            }

            let mut orchestrator = self.orchestrator.write().await;
            let result = orchestrator.execute_pipeline(pipeline, context).await?;

            Ok(json!({
                "pipeline_result": result
            }))
        } else {
            Err(Error::Validation(
                "Missing 'template' or 'pipeline' field".to_string(),
            ))
        }
    }

    /// Handle resource metrics requests
    async fn handle_resource_metrics(&self, _arguments: Value) -> Result<Value> {
        let resource_manager = self.resource_manager.read().await;
        let metrics = resource_manager.get_metrics().await;

        serde_json::to_value(metrics)
            .map_err(|e| Error::Validation(format!("Failed to serialize metrics: {e}")))
    }

    /// Handle performance dashboard requests
    async fn handle_performance_dashboard(&self, _arguments: Value) -> Result<Value> {
        let resource_manager = self.resource_manager.read().await;
        let dashboard = resource_manager.get_performance_dashboard().await;

        Ok(dashboard)
    }

    /// Handle health check requests
    async fn handle_health_check(&self, _arguments: Value) -> Result<Value> {
        let resource_manager = self.resource_manager.read().await;
        let metrics = resource_manager.get_metrics().await;

        // Determine overall health status
        let cpu_ok = metrics.cpu_percent < 80.0; // 80% threshold
        let memory_ok = metrics.memory_bytes < 100 * 1024 * 1024; // 100MB threshold
        let circuit_ok = !metrics.circuit_breaker_open;

        let status = if cpu_ok && memory_ok && circuit_ok {
            "healthy"
        } else if !circuit_ok {
            "circuit_breaker_open"
        } else {
            "degraded"
        };

        Ok(json!({
            "status": status,
            "timestamp": SystemTime::now().duration_since(UNIX_EPOCH)
                .unwrap_or_default().as_secs(),
            "checks": {
                "cpu": {
                    "status": if cpu_ok { "ok" } else { "warning" },
                    "value": metrics.cpu_percent,
                    "threshold": 80.0
                },
                "memory": {
                    "status": if memory_ok { "ok" } else { "warning" },
                    "value_mb": metrics.memory_bytes / (1024 * 1024),
                    "threshold_mb": 100
                },
                "circuit_breaker": {
                    "status": if circuit_ok { "ok" } else { "error" },
                    "open": metrics.circuit_breaker_open
                }
            },
            "uptime_seconds": metrics.timestamp.duration_since(UNIX_EPOCH)
                .unwrap_or_default().as_secs()
        }))
    }

    /// Handle dead letter queue operations
    async fn handle_dead_letter_queue(&self, arguments: Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("list");

        match action {
            "list" => {
                let dlq = self.dead_letter_queue.read().await;
                let operations = dlq.get_failed_operations().await;
                Ok(json!({
                    "failed_operations": operations,
                    "total_count": operations.len()
                }))
            }
            "stats" => {
                let dlq = self.dead_letter_queue.read().await;
                let stats = dlq.get_statistics().await;
                Ok(serde_json::to_value(stats)?)
            }
            "remove" => {
                let id = arguments
                    .get("id")
                    .and_then(|i| i.as_str())
                    .ok_or_else(|| Error::Validation("Missing 'id' field".to_string()))?;

                let dlq = self.dead_letter_queue.read().await;
                let removed = dlq.remove_failed_operation(id).await?;

                Ok(json!({
                    "removed": removed.is_some(),
                    "operation": removed
                }))
            }
            _ => Err(Error::Validation(format!(
                "Unknown dead letter queue action: {action}"
            ))),
        }
    }

    /// Handle diagnostic report generation
    async fn handle_diagnostic_report(&self, arguments: Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("generate");

        match action {
            "generate" => {
                let dlq = self.dead_letter_queue.read().await;
                let report = self
                    .diagnostic_collector
                    .generate_report(Some(&*dlq))
                    .await?;
                Ok(serde_json::to_value(report)?)
            }
            "export" => {
                let dlq = self.dead_letter_queue.read().await;
                let report = self
                    .diagnostic_collector
                    .generate_report(Some(&*dlq))
                    .await?;
                let json_export = self
                    .diagnostic_collector
                    .export_report_json(&report)
                    .await?;

                Ok(json!({
                    "report_json": json_export,
                    "report_id": report.report_id
                }))
            }
            _ => Err(Error::Validation(format!(
                "Unknown diagnostic report action: {action}"
            ))),
        }
    }

    /// Handle checkpoint operations
    async fn handle_checkpoint(&self, arguments: Value) -> Result<Value> {
        let action = arguments
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("list");

        match action {
            "create" => {
                let name = arguments
                    .get("name")
                    .and_then(|n| n.as_str())
                    .ok_or_else(|| Error::Validation("Missing 'name' field".to_string()))?;

                let description = arguments
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("");

                let operation_type = arguments
                    .get("operation_type")
                    .and_then(|o| o.as_str())
                    .unwrap_or("manual");

                let state_data = arguments.get("state_data").cloned().unwrap_or(json!({}));

                let checkpoint = crate::checkpoint::Checkpoint::new(
                    name,
                    description,
                    operation_type,
                    "mcp_server",
                    state_data,
                );

                let cm = self.checkpoint_manager.read().await;
                let checkpoint_id = cm.create_checkpoint(checkpoint).await?;

                Ok(json!({
                    "checkpoint_id": checkpoint_id,
                    "created": true
                }))
            }
            "restore" => {
                let checkpoint_id = arguments
                    .get("checkpoint_id")
                    .and_then(|id| id.as_str())
                    .ok_or_else(|| {
                        Error::Validation("Missing 'checkpoint_id' field".to_string())
                    })?;

                let cm = self.checkpoint_manager.read().await;
                let checkpoint = cm.restore_checkpoint(checkpoint_id).await?;

                Ok(serde_json::to_value(checkpoint)?)
            }
            "list" => {
                let cm = self.checkpoint_manager.read().await;
                let checkpoints = cm.list_checkpoints().await;

                Ok(json!({
                    "checkpoints": checkpoints,
                    "total_count": checkpoints.len()
                }))
            }
            "delete" => {
                let checkpoint_id = arguments
                    .get("checkpoint_id")
                    .and_then(|id| id.as_str())
                    .ok_or_else(|| {
                        Error::Validation("Missing 'checkpoint_id' field".to_string())
                    })?;

                let cm = self.checkpoint_manager.read().await;
                cm.delete_checkpoint(checkpoint_id).await?;

                Ok(json!({
                    "deleted": true,
                    "checkpoint_id": checkpoint_id
                }))
            }
            "stats" => {
                let cm = self.checkpoint_manager.read().await;
                let stats = cm.get_statistics().await;

                Ok(serde_json::to_value(stats)?)
            }
            _ => Err(Error::Validation(format!(
                "Unknown checkpoint action: {action}"
            ))),
        }
    }

    /// Handle bug report creation
    async fn handle_bug_report(&self, arguments: Value) -> Result<Value> {
        let description = arguments
            .get("description")
            .and_then(|d| d.as_str())
            .ok_or_else(|| Error::Validation("Missing 'description' field".to_string()))?;

        let steps_to_reproduce = arguments
            .get("steps_to_reproduce")
            .and_then(|s| s.as_str())
            .unwrap_or("No steps provided");

        let dlq = self.dead_letter_queue.read().await;
        let diagnostic_report = self
            .diagnostic_collector
            .generate_report(Some(&*dlq))
            .await?;

        let bug_report = create_bug_report(&diagnostic_report, description, steps_to_reproduce);

        // Optionally save to file (with path validation)
        if let Some(file_path) = arguments.get("save_to_file").and_then(|f| f.as_str()) {
            // Validate and sanitize file path to prevent path traversal
            let path = std::path::Path::new(file_path);
            if path.is_absolute() || path.to_string_lossy().contains("..") {
                return Err(Error::Validation(
                    "Invalid file path: must be relative and not contain '..'".to_string(),
                ));
            }

            // Restrict to a safe directory
            let safe_dir = std::path::Path::new("./bug_reports");
            tokio::fs::create_dir_all(&safe_dir).await?;
            let full_path = safe_dir.join(path);

            tokio::fs::write(&full_path, &bug_report).await?;
        }

        Ok(json!({
            "bug_report": bug_report,
            "diagnostic_report_id": diagnostic_report.report_id,
            "generated_at": diagnostic_report.generated_at
        }))
    }
}

impl Clone for McpServer {
    fn clone(&self) -> Self {
        McpServer {
            config: self.config.clone(),
            brp_client: self.brp_client.clone(),
            orchestrator: self.orchestrator.clone(),
            resource_manager: self.resource_manager.clone(),
            dead_letter_queue: self.dead_letter_queue.clone(),
            diagnostic_collector: self.diagnostic_collector.clone(),
            checkpoint_manager: self.checkpoint_manager.clone(),
            debug_mode: self.debug_mode,
        }
    }
}

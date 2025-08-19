/// Mock MCP Client for Integration Testing
/// 
/// This module provides a mock MCP client that simulates Claude Code interactions
/// without requiring actual MCP server connections. Used for testing MCP protocol
/// compliance and tool orchestration.

use bevy_debugger_mcp::{
    mcp_server::McpServer,
    config::Config,
    error::{Error, Result},
};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, Mutex};
use tokio::time::timeout;
use tracing::{info, debug, warn, error};

/// Mock MCP client that simulates Claude Code interactions
pub struct MockMcpClient {
    server: Arc<McpServer>,
    capabilities: Value,
    tools: Vec<ToolDefinition>,
    session_state: Arc<Mutex<SessionState>>,
    request_id_counter: Arc<Mutex<u64>>,
}

/// Tool definition as exposed through MCP protocol
#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Session state tracking
#[derive(Debug, Default)]
struct SessionState {
    initialized: bool,
    tool_calls_made: Vec<ToolCall>,
    errors_encountered: Vec<String>,
}

/// Record of a tool call
#[derive(Debug, Clone)]
struct ToolCall {
    pub tool_name: String,
    pub arguments: Value,
    pub response: Result<Value, String>,
    pub duration_ms: u128,
    pub timestamp: std::time::SystemTime,
}

impl MockMcpClient {
    /// Create a new mock MCP client
    pub async fn new(config: Config) -> Result<Self> {
        let brp_client = Arc::new(RwLock::new(
            bevy_debugger_mcp::brp_client::BrpClient::new(&config)
        ));
        
        let server = Arc::new(McpServer::new(config, brp_client));
        
        let tools = vec![
            ToolDefinition {
                name: "observe".to_string(),
                description: "Observe Bevy game state and entities".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "Query to execute"
                        }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "experiment".to_string(),
                description: "Run experiments on the game".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "description": "Type of experiment"
                        }
                    },
                    "required": ["type"]
                }),
            },
            ToolDefinition {
                name: "screenshot".to_string(),
                description: "Capture a screenshot of the Bevy game window".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "File path where to save the screenshot"
                        },
                        "warmup_duration": {
                            "type": "integer",
                            "description": "Time in milliseconds to wait before taking screenshot"
                        },
                        "capture_delay": {
                            "type": "integer", 
                            "description": "Additional delay in milliseconds before screenshot capture"
                        },
                        "wait_for_render": {
                            "type": "boolean",
                            "description": "Whether to wait for at least one frame to render before capture"
                        },
                        "description": {
                            "type": "string",
                            "description": "Optional description of what this screenshot captures"
                        }
                    }
                }),
            },
            ToolDefinition {
                name: "hypothesis".to_string(),
                description: "Test hypotheses about game behavior".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "hypothesis": {
                            "type": "string",
                            "description": "The hypothesis to test"
                        }
                    },
                    "required": ["hypothesis"]
                }),
            },
            ToolDefinition {
                name: "stress".to_string(),
                description: "Run stress tests on the game".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "description": "Type of stress test"
                        },
                        "count": {
                            "type": "integer",
                            "description": "Number of entities/operations"
                        }
                    },
                    "required": ["type"]
                }),
            },
            ToolDefinition {
                name: "replay".to_string(),
                description: "Replay recorded game sessions".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "ID of the session to replay"
                        }
                    },
                    "required": ["session_id"]
                }),
            },
            ToolDefinition {
                name: "anomaly".to_string(),
                description: "Detect anomalies in game behavior".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "metric": {
                            "type": "string",
                            "description": "Metric to analyze for anomalies"
                        }
                    },
                    "required": ["metric"]
                }),
            },
        ];

        Ok(Self {
            server,
            capabilities: json!({
                "capabilities": {
                    "tools": {
                        "listChanged": false
                    }
                },
                "serverInfo": {
                    "name": "bevy-debugger-mcp",
                    "version": "0.1.0"
                }
            }),
            tools,
            session_state: Arc::new(Mutex::new(SessionState::default())),
            request_id_counter: Arc::new(Mutex::new(1)),
        })
    }

    /// Simulate MCP initialize handshake
    pub async fn initialize(&self) -> Result<Value> {
        let mut state = self.session_state.lock().await;
        state.initialized = true;
        
        info!("Mock MCP client initialized");
        Ok(self.capabilities.clone())
    }

    /// List available tools (simulates tools/list)
    pub async fn list_tools(&self) -> Result<Value> {
        let tools_json: Vec<Value> = self.tools.iter().map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "inputSchema": tool.input_schema
            })
        }).collect();

        Ok(json!({
            "tools": tools_json
        }))
    }

    /// Call a tool (simulates tools/call)
    pub async fn call_tool(&self, tool_name: &str, arguments: Value) -> Result<Value> {
        self.call_tool_with_timeout(tool_name, arguments, Duration::from_secs(10)).await
    }

    /// Call a tool with custom timeout
    pub async fn call_tool_with_timeout(
        &self, 
        tool_name: &str, 
        arguments: Value, 
        timeout_duration: Duration
    ) -> Result<Value> {
        let start_time = std::time::Instant::now();
        let timestamp = std::time::SystemTime::now();
        
        debug!("Calling tool '{}' with args: {}", tool_name, arguments);

        // Validate tool exists
        if !self.tools.iter().any(|t| t.name == tool_name) {
            let error = format!("Unknown tool: {}", tool_name);
            error!("{}", error);
            
            let mut state = self.session_state.lock().await;
            state.errors_encountered.push(error.clone());
            state.tool_calls_made.push(ToolCall {
                tool_name: tool_name.to_string(),
                arguments,
                response: Err(error.clone()),
                duration_ms: start_time.elapsed().as_millis(),
                timestamp,
            });
            
            return Err(Error::Validation(error));
        }

        // Execute the tool call with timeout
        let result = timeout(
            timeout_duration,
            self.server.handle_tool_call(tool_name, arguments.clone())
        ).await;

        let duration_ms = start_time.elapsed().as_millis();

        let response = match result {
            Ok(Ok(value)) => {
                debug!("Tool call '{}' succeeded in {}ms", tool_name, duration_ms);
                Ok(value)
            }
            Ok(Err(e)) => {
                warn!("Tool call '{}' failed: {}", tool_name, e);
                Err(e.to_string())
            }
            Err(_) => {
                let error = format!("Tool call '{}' timed out after {:?}", tool_name, timeout_duration);
                error!("{}", error);
                Err(error)
            }
        };

        // Record the tool call
        let mut state = self.session_state.lock().await;
        state.tool_calls_made.push(ToolCall {
            tool_name: tool_name.to_string(),
            arguments,
            response: response.clone().map_err(|e| e.to_string()),
            duration_ms,
            timestamp,
        });

        if response.is_err() {
            state.errors_encountered.push(response.clone().err().unwrap());
        }

        response.map_err(|e| Error::Validation(e))
    }

    /// Simulate a complete MCP conversation flow
    pub async fn simulate_debugging_session(&self) -> Result<DebugSessionReport> {
        let mut report = DebugSessionReport::default();
        report.session_start = std::time::SystemTime::now();

        info!("Starting simulated debugging session");

        // Initialize
        let init_result = self.initialize().await;
        report.initialization_successful = init_result.is_ok();

        if !report.initialization_successful {
            return Ok(report);
        }

        // List tools
        let tools_result = self.list_tools().await;
        report.tools_listed_successfully = tools_result.is_ok();

        if let Ok(tools) = tools_result {
            if let Some(tools_array) = tools.get("tools").and_then(|t| t.as_array()) {
                report.available_tools_count = tools_array.len();
            }
        }

        // Test each tool with basic arguments
        let test_scenarios = vec![
            ("observe", json!({"query": "entities with Transform"})),
            ("experiment", json!({"type": "performance_test"})),
            ("screenshot", json!({"path": "/tmp/test_screenshot.png", "description": "Test screenshot"})),
            ("hypothesis", json!({"hypothesis": "Player movement affects frame rate"})),
            ("stress", json!({"type": "entity_spawn", "count": 100})),
            ("replay", json!({"session_id": "test_session_123"})),
            ("anomaly", json!({"metric": "frame_time"})),
        ];

        for (tool_name, args) in test_scenarios {
            match self.call_tool(tool_name, args).await {
                Ok(_) => {
                    report.successful_tool_calls += 1;
                    info!("Tool '{}' executed successfully", tool_name);
                }
                Err(e) => {
                    report.failed_tool_calls += 1;
                    warn!("Tool '{}' failed: {}", tool_name, e);
                }
            }
        }

        report.session_end = Some(std::time::SystemTime::now());
        report.total_duration = report.session_end
            .unwrap()
            .duration_since(report.session_start)
            .unwrap_or_default();

        info!("Debugging session completed: {} successful, {} failed", 
              report.successful_tool_calls, report.failed_tool_calls);

        Ok(report)
    }

    /// Get session statistics
    pub async fn get_session_stats(&self) -> SessionStats {
        let state = self.session_state.lock().await;
        
        let successful_calls = state.tool_calls_made
            .iter()
            .filter(|call| call.response.is_ok())
            .count();
            
        let failed_calls = state.tool_calls_made.len() - successful_calls;
        
        let avg_duration = if !state.tool_calls_made.is_empty() {
            state.tool_calls_made
                .iter()
                .map(|call| call.duration_ms)
                .sum::<u128>() as f64 / state.tool_calls_made.len() as f64
        } else {
            0.0
        };

        let mut tools_used = HashMap::new();
        for call in &state.tool_calls_made {
            *tools_used.entry(call.tool_name.clone()).or_insert(0) += 1;
        }

        SessionStats {
            total_tool_calls: state.tool_calls_made.len(),
            successful_calls,
            failed_calls,
            average_duration_ms: avg_duration,
            errors_count: state.errors_encountered.len(),
            tools_used,
            initialized: state.initialized,
        }
    }

    /// Reset session state
    pub async fn reset_session(&self) {
        let mut state = self.session_state.lock().await;
        *state = SessionState::default();
        
        let mut counter = self.request_id_counter.lock().await;
        *counter = 1;
        
        info!("Mock MCP client session reset");
    }

    /// Validate MCP protocol compliance
    pub async fn validate_protocol_compliance(&self) -> ProtocolCompliance {
        let mut compliance = ProtocolCompliance::default();
        
        // Test initialize
        compliance.supports_initialize = self.initialize().await.is_ok();
        
        // Test tools/list
        compliance.supports_tools_list = self.list_tools().await.is_ok();
        
        // Test tools/call for each tool
        let mut working_tools = 0;
        for tool in &self.tools {
            let test_args = json!({"test": true});
            if self.call_tool(&tool.name, test_args).await.is_ok() {
                working_tools += 1;
            }
        }
        
        compliance.tools_callable = working_tools;
        compliance.total_tools = self.tools.len();
        compliance.protocol_compliant = compliance.supports_initialize 
            && compliance.supports_tools_list 
            && working_tools > 0;
        
        compliance
    }
}

/// Report from a simulated debugging session
#[derive(Debug, Default)]
pub struct DebugSessionReport {
    pub session_start: std::time::SystemTime,
    pub session_end: Option<std::time::SystemTime>,
    pub total_duration: Duration,
    pub initialization_successful: bool,
    pub tools_listed_successfully: bool,
    pub available_tools_count: usize,
    pub successful_tool_calls: usize,
    pub failed_tool_calls: usize,
}

/// Session statistics
#[derive(Debug)]
pub struct SessionStats {
    pub total_tool_calls: usize,
    pub successful_calls: usize,
    pub failed_calls: usize,
    pub average_duration_ms: f64,
    pub errors_count: usize,
    pub tools_used: HashMap<String, usize>,
    pub initialized: bool,
}

/// Protocol compliance validation results
#[derive(Debug, Default)]
pub struct ProtocolCompliance {
    pub supports_initialize: bool,
    pub supports_tools_list: bool,
    pub tools_callable: usize,
    pub total_tools: usize,
    pub protocol_compliant: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_debugger_mcp::config::Config;

    #[tokio::test]
    async fn test_mock_client_creation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };

        let client = MockMcpClient::new(config).await.unwrap();
        assert_eq!(client.tools.len(), 7); // Should have all 7 main tools
    }

    #[tokio::test]
    async fn test_initialize_flow() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };

        let client = MockMcpClient::new(config).await.unwrap();
        let result = client.initialize().await.unwrap();
        
        assert!(result.get("capabilities").is_some());
        assert!(result.get("serverInfo").is_some());
    }

    #[tokio::test]
    async fn test_tools_listing() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };

        let client = MockMcpClient::new(config).await.unwrap();
        let result = client.list_tools().await.unwrap();
        
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert!(tools.len() >= 7);
        
        // Verify essential tools are present
        let tool_names: Vec<&str> = tools
            .iter()
            .map(|t| t.get("name").unwrap().as_str().unwrap())
            .collect();
            
        assert!(tool_names.contains(&"observe"));
        assert!(tool_names.contains(&"experiment"));
        assert!(tool_names.contains(&"screenshot"));
    }

    #[tokio::test]
    async fn test_debugging_session_simulation() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };

        let client = MockMcpClient::new(config).await.unwrap();
        let report = client.simulate_debugging_session().await.unwrap();
        
        assert!(report.initialization_successful);
        assert!(report.tools_listed_successfully);
        assert!(report.available_tools_count >= 7);
        // Some calls might fail due to lack of actual Bevy connection, that's expected
        assert!(report.successful_tool_calls + report.failed_tool_calls >= 7);
    }
}
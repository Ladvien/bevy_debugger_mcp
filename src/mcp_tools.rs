/*
 * Bevy Debugger MCP Server - Centralized Tool Definitions
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use rmcp::{model::*, tool, tool_router, tool_handler, handler::server::{ServerHandler, router::tool::ToolRouter, tool::Parameters}, schemars, Error as McpError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::Arc, future::Future};
use tokio::sync::RwLock;
use tracing::{error, info, debug};
use schemars::JsonSchema;

use crate::brp_client::BrpClient;
use crate::tools::{observe, experiment, hypothesis, anomaly, stress, replay};

// Parameter structures for tools
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ObserveRequest {
    pub query: String,
    #[serde(default)]
    pub diff: bool,
    #[serde(default)]
    pub detailed: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExperimentRequest {
    #[serde(rename = "type")]
    pub experiment_type: String,
    #[serde(default)]
    pub params: Value,
    pub duration: Option<f32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HypothesisRequest {
    pub hypothesis: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub context: Option<Value>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnomalyRequest {
    #[serde(rename = "type")]
    pub detection_type: String,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
    pub window: Option<f32>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct StressTestRequest {
    #[serde(rename = "type")]
    pub test_type: String,
    #[serde(default = "default_intensity")]
    pub intensity: u8,
    #[serde(default = "default_duration")]
    pub duration: f32,
    #[serde(default)]
    pub detailed_metrics: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayRequest {
    pub action: String,
    pub checkpoint_id: Option<String>,
    #[serde(default = "default_speed")]
    pub speed: f32,
}

// Default value functions
fn default_confidence() -> f32 { 0.8 }
fn default_sensitivity() -> f32 { 0.7 }
fn default_intensity() -> u8 { 5 }
fn default_duration() -> f32 { 10.0 }
fn default_speed() -> f32 { 1.0 }

/// Centralized tool schema definitions for better discoverability
#[derive(Clone)]
pub struct BevyDebuggerTools {
    brp_client: Arc<RwLock<BrpClient>>,
    tool_router: ToolRouter<Self>,
}

impl BevyDebuggerTools {
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self { 
            brp_client,
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router]
impl BevyDebuggerTools {
    /// Observe and query Bevy game state
    #[tool(description = "Observe and query Bevy game state in real-time. Use this to inspect entities, components, resources, and game state. Perfect for debugging entity spawning, component updates, and understanding your ECS architecture.")]
    pub async fn observe(&self, Parameters(req): Parameters<ObserveRequest>) -> Result<CallToolResult, McpError> {
        debug!("Executing observe query: {}", req.query);
        
        let arguments = serde_json::json!({
            "query": req.query,
            "diff": req.diff,
            "detailed": req.detailed,
        });
        
        match observe::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Observe tool error: {}", e);
                Err(McpError::internal_error(format!("Observe tool error: {}", e), None))
            }
        }
    }

    /// Run controlled experiments on game state
    #[tool(description = "Run controlled experiments on your Bevy game to test behavior and performance. Useful for reproducing bugs, testing edge cases, and validating fixes.")]
    pub async fn experiment(&self, Parameters(req): Parameters<ExperimentRequest>) -> Result<CallToolResult, McpError> {
        debug!("Running experiment: {}", req.experiment_type);
        
        let arguments = serde_json::json!({
            "type": req.experiment_type,
            "params": req.params,
            "duration": req.duration,
        });
        
        match experiment::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Experiment tool error: {}", e);
                Err(McpError::internal_error(format!("Experiment tool error: {}", e), None))
            }
        }
    }

    /// Test hypotheses about game behavior
    #[tool(description = "Test hypotheses about game behavior and state. Helps validate assumptions and understand why certain behaviors occur.")]
    pub async fn hypothesis(&self, Parameters(req): Parameters<HypothesisRequest>) -> Result<CallToolResult, McpError> {
        debug!("Testing hypothesis: {}", req.hypothesis);
        
        let arguments = serde_json::json!({
            "hypothesis": req.hypothesis,
            "confidence": req.confidence,
            "context": req.context,
        });
        
        match hypothesis::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Hypothesis tool error: {}", e);
                Err(McpError::internal_error(format!("Hypothesis tool error: {}", e), None))
            }
        }
    }

    /// Detect anomalies in game behavior
    #[tool(description = "Detect anomalies in game behavior, performance, and state. Automatically identifies issues like memory leaks, performance drops, and inconsistent state.")]
    pub async fn detect_anomaly(&self, Parameters(req): Parameters<AnomalyRequest>) -> Result<CallToolResult, McpError> {
        debug!("Running anomaly detection: {}", req.detection_type);
        
        let arguments = serde_json::json!({
            "type": req.detection_type,
            "sensitivity": req.sensitivity,
            "window": req.window,
        });
        
        match anomaly::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Anomaly detection error: {}", e);
                Err(McpError::internal_error(format!("Anomaly detection error: {}", e), None))
            }
        }
    }

    /// Run stress tests
    #[tool(description = "Run stress tests to find performance limits and bottlenecks. Helps identify when and why your game starts to lag or consume excessive resources.")]
    pub async fn stress_test(&self, Parameters(req): Parameters<StressTestRequest>) -> Result<CallToolResult, McpError> {
        info!("Starting stress test: {} at intensity {}", req.test_type, req.intensity);
        
        let arguments = serde_json::json!({
            "type": req.test_type,
            "intensity": req.intensity,
            "duration": req.duration,
            "detailed_metrics": req.detailed_metrics,
        });
        
        match stress::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Stress test error: {}", e);
                Err(McpError::internal_error(format!("Stress test error: {}", e), None))
            }
        }
    }

    /// Record and replay game state
    #[tool(description = "Record and replay game state for time-travel debugging. Capture game state at specific points and replay to understand how bugs occur.")]
    pub async fn replay(&self, Parameters(req): Parameters<ReplayRequest>) -> Result<CallToolResult, McpError> {
        info!("Replay action: {}", req.action);
        
        let arguments = serde_json::json!({
            "action": req.action,
            "checkpoint_id": req.checkpoint_id,
            "speed": req.speed,
        });
        
        match replay::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Replay tool error: {}", e);
                Err(McpError::internal_error(format!("Replay tool error: {}", e), None))
            }
        }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for BevyDebuggerTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "bevy-debugger-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("AI-assisted debugging tools for Bevy games through Claude Code using Model Context Protocol".to_string()),
        }
    }
}
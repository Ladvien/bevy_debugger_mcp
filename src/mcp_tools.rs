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

use rmcp::{Error as McpError, model::*, tool, tool_router, handler::server::tool::ToolRouter};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::future::Future;
use tokio::sync::RwLock;
use tracing::{error, info, debug};

use crate::brp_client::BrpClient;
use crate::error::Result;
use crate::tools::{observe, experiment, hypothesis, anomaly, stress, replay};

/// Centralized tool schema definitions for better discoverability
#[derive(Clone)]
pub struct BevyDebuggerTools {
    brp_client: Arc<RwLock<BrpClient>>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl BevyDebuggerTools {
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        Self { 
            brp_client,
            tool_router: Self::tool_router(),
        }
    }
    
    pub fn router(&self) -> ToolRouter<Self> {
        self.tool_router.clone()
    }

    /// Observe and query Bevy game state
    #[tool(description = "Observe and query Bevy game state in real-time. Use this to inspect entities, components, resources, and game state. Perfect for debugging entity spawning, component updates, and understanding your ECS architecture.")]
    #[tracing::instrument(skip(self))]
    pub async fn observe(&self, query: String, diff: Option<bool>, detailed: Option<bool>) -> Result<CallToolResult, McpError> {
        debug!("Executing observe query: {}", query);
        
        let arguments = serde_json::json!({
            "query": query,
            "diff": diff.unwrap_or(false),
            "detailed": detailed.unwrap_or(false),
        });
        
        match observe::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string()
            )])),
            Err(e) => {
                error!("Observe tool error: {}", e);
                Err(McpError::InvalidRequest(e.to_string()))
            }
        }
    }

    /// Run controlled experiments on game state
    #[tool(description = "Run controlled experiments on your Bevy game to test behavior and performance. Useful for reproducing bugs, testing edge cases, and validating fixes.")]
    #[tracing::instrument(skip(self))]
    pub async fn experiment(&self, experiment_type: String, params: Option<Value>, duration: Option<f32>) -> Result<CallToolResult, McpError> {
        debug!("Running experiment: {}", experiment_type);
        
        let arguments = serde_json::json!({
            "type": experiment_type,
            "params": params.unwrap_or(serde_json::json!({})),
            "duration": duration,
        });
        
        match experiment::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string()
            )])),
            Err(e) => {
                error!("Experiment tool error: {}", e);
                Err(McpError::InvalidRequest(e.to_string()))
            }
        }
    }

    /// Test hypotheses about game behavior
    #[tool(description = "Test hypotheses about game behavior and state. Helps validate assumptions and understand why certain behaviors occur.")]
    #[tracing::instrument(skip(self))]
    pub async fn hypothesis(&self, hypothesis: String, confidence: Option<f32>, context: Option<Value>) -> Result<CallToolResult, McpError> {
        debug!("Testing hypothesis: {}", hypothesis);
        
        let arguments = serde_json::json!({
            "hypothesis": hypothesis,
            "confidence": confidence.unwrap_or(0.8),
            "context": context,
        });
        
        match hypothesis::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string()
            )])),
            Err(e) => {
                error!("Hypothesis tool error: {}", e);
                Err(McpError::InvalidRequest(e.to_string()))
            }
        }
    }

    /// Detect anomalies in game behavior
    #[tool(description = "Detect anomalies in game behavior, performance, and state. Automatically identifies issues like memory leaks, performance drops, and inconsistent state.")]
    #[tracing::instrument(skip(self))]
    pub async fn detect_anomaly(&self, detection_type: String, sensitivity: Option<f32>, window: Option<f32>) -> Result<CallToolResult, McpError> {
        debug!("Running anomaly detection: {}", detection_type);
        
        let arguments = serde_json::json!({
            "type": detection_type,
            "sensitivity": sensitivity.unwrap_or(0.7),
            "window": window,
        });
        
        match anomaly::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string()
            )])),
            Err(e) => {
                error!("Anomaly detection error: {}", e);
                Err(McpError::InvalidRequest(e.to_string()))
            }
        }
    }

    /// Run stress tests
    #[tool(description = "Run stress tests to find performance limits and bottlenecks. Helps identify when and why your game starts to lag or consume excessive resources.")]
    #[tracing::instrument(skip(self))]
    pub async fn stress_test(&self, test_type: String, intensity: Option<u8>, duration: Option<f32>, detailed_metrics: Option<bool>) -> Result<CallToolResult, McpError> {
        info!("Starting stress test: {} at intensity {}", test_type, intensity.unwrap_or(5));
        
        let arguments = serde_json::json!({
            "type": test_type,
            "intensity": intensity.unwrap_or(5),
            "duration": duration.unwrap_or(10.0),
            "detailed_metrics": detailed_metrics.unwrap_or(false),
        });
        
        match stress::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string()
            )])),
            Err(e) => {
                error!("Stress test error: {}", e);
                Err(McpError::InvalidRequest(e.to_string()))
            }
        }
    }

    /// Record and replay game state
    #[tool(description = "Record and replay game state for time-travel debugging. Capture game state at specific points and replay to understand how bugs occur.")]
    #[tracing::instrument(skip(self))]
    pub async fn replay(&self, action: String, checkpoint_id: Option<String>, speed: Option<f32>) -> Result<CallToolResult, McpError> {
        info!("Replay action: {}", action);
        
        let arguments = serde_json::json!({
            "action": action,
            "checkpoint_id": checkpoint_id,
            "speed": speed.unwrap_or(1.0),
        });
        
        match replay::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(
                result.to_string()
            )])),
            Err(e) => {
                error!("Replay tool error: {}", e);
                Err(McpError::InvalidRequest(e.to_string()))
            }
        }
    }
}
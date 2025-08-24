/*
 * Bevy Debugger MCP Server - Security-Enhanced Tool Handler
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
use tracing::{error, info, debug, warn};
use schemars::JsonSchema;

use crate::brp_client::BrpClient;
use crate::tools::{observe, experiment, hypothesis, anomaly, stress, replay};
use crate::security::{SecurityManager, SecurityMiddleware, Role, Claims, SecurityAudit};
use crate::error::{Error, Result};

// Re-export parameter structures from the original tools
pub use crate::mcp_tools::{
    ObserveRequest, ExperimentRequest, HypothesisRequest, 
    AnomalyRequest, StressTestRequest, ReplayRequest
};

// Additional parameter structures for security operations
#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub role: String, // "viewer", "developer", or "admin"
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteUserRequest {
    pub username: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditLogRequest {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub role: String,
    pub expires_in: u64,
}

/// Security-enhanced MCP tools with authentication and authorization
#[derive(Clone)]
pub struct SecureMcpTools {
    brp_client: Arc<RwLock<BrpClient>>,
    security_manager: Arc<SecurityManager>,
    security_middleware: SecurityMiddleware,
    security_audit: SecurityAudit,
    tool_router: ToolRouter<Self>,
}

impl SecureMcpTools {
    pub fn new(
        brp_client: Arc<RwLock<BrpClient>>, 
        security_manager: Arc<SecurityManager>
    ) -> Self {
        let security_middleware = SecurityMiddleware::new(security_manager.clone());
        let security_audit = SecurityAudit::new(security_manager.clone());
        
        Self { 
            brp_client,
            security_manager: security_manager.clone(),
            security_middleware,
            security_audit,
            tool_router: Self::tool_router(),
        }
    }

    /// Extract JWT token from request headers or parameters
    fn extract_token_from_request(params: &Value) -> Option<String> {
        // Check if token is provided in parameters
        if let Some(token) = params.get("auth_token").and_then(|t| t.as_str()) {
            return Some(token.to_string());
        }
        
        // Check if token is in authorization parameter
        if let Some(auth) = params.get("authorization").and_then(|a| a.as_str()) {
            if let Some(token) = auth.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
        
        None
    }

    /// Validate and authorize a tool call
    async fn authorize_tool_call(&self, operation: &str, params: &Value) -> Result<Claims> {
        let token = Self::extract_token_from_request(params)
            .ok_or_else(|| Error::SecurityError("Authentication required".to_string()))?;
        
        self.security_middleware.authorize_tool_call(Some(&token), operation).await
    }

    /// Log a successful tool operation
    async fn log_tool_success(&self, claims: &Claims, operation: &str, resource: Option<&str>) {
        // This would typically be handled by the security manager's audit logging
        debug!("Tool operation successful: {} by user {}", operation, claims.sub);
    }

    /// Log a failed tool operation
    async fn log_tool_failure(&self, operation: &str, error: &str) {
        warn!("Tool operation failed: {} - {}", operation, error);
    }
}

#[tool_router]
impl SecureMcpTools {
    /// Authenticate user and return JWT token
    #[tool(description = "Authenticate with username and password to get a JWT token for accessing debugging tools. Returns a token that must be included in subsequent requests.")]
    pub async fn authenticate(&self, Parameters(req): Parameters<AuthRequest>) -> std::result::Result<CallToolResult, McpError> {
        info!("Authentication attempt for user: {}", req.username);
        
        match self.security_manager.authenticate(
            &req.username, 
            &req.password,
            None, // IP address - could be extracted from request context
            None, // User agent - could be extracted from request context
        ).await {
            Ok(token) => {
                let response = AuthResponse {
                    token: token.clone(),
                    role: "authenticated".to_string(), // Could decode role from token
                    expires_in: 24 * 3600, // Default 24 hours
                };
                
                Ok(CallToolResult::success(vec![
                    Content::text(serde_json::to_string(&response).unwrap())
                ]))
            }
            Err(e) => {
                error!("Authentication failed for {}: {}", req.username, e);
                Err(McpError::invalid_params(format!("Authentication failed: {}", e), None))
            }
        }
    }

    /// Revoke JWT token (logout)
    #[tool(description = "Revoke your JWT token to log out. This will invalidate the token and end your session.")]
    pub async fn logout(&self, Parameters(params): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let token = Self::extract_token_from_request(&params)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;
        
        match self.security_manager.revoke_token(&token).await {
            Ok(_) => {
                info!("Token revoked successfully");
                Ok(CallToolResult::success(vec![Content::text("Logged out successfully".to_string())]))
            }
            Err(e) => {
                error!("Token revocation failed: {}", e);
                Err(McpError::internal_error(format!("Logout failed: {}", e), None))
            }
        }
    }

    /// Observe and query Bevy game state (requires Viewer role or higher)
    #[tool(description = "Observe and query Bevy game state in real-time with optional reflection-based component inspection. Requires authentication token and Viewer role or higher.")]
    pub async fn observe(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("observe", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("observe", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        // Remove auth parameters before passing to the actual tool
        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let observe_req: ObserveRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid observe parameters: {}", e), None))?;

        debug!("User {} executing observe query: {}", claims.sub, observe_req.query);
        
        let arguments = serde_json::json!({
            "query": observe_req.query,
            "diff": observe_req.diff,
            "detailed": observe_req.detailed,
            "reflection": observe_req.reflection,
        });
        
        match observe::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "observe", Some(&observe_req.query)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Observe tool error for user {}: {}", claims.sub, e);
                self.log_tool_failure("observe", &e.to_string()).await;
                Err(McpError::internal_error(format!("Observe tool error: {}", e), None))
            }
        }
    }

    /// Run controlled experiments on game state (requires Developer role or higher)
    #[tool(description = "Run controlled experiments on your Bevy game to test behavior and performance. Requires authentication token and Developer role or higher.")]
    pub async fn experiment(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("experiment", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("experiment", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        // Remove auth parameters
        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let exp_req: ExperimentRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid experiment parameters: {}", e), None))?;

        debug!("User {} running experiment: {}", claims.sub, exp_req.experiment_type);
        
        let arguments = serde_json::json!({
            "type": exp_req.experiment_type,
            "params": exp_req.params,
            "duration": exp_req.duration,
        });
        
        match experiment::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "experiment", Some(&exp_req.experiment_type)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Experiment tool error for user {}: {}", claims.sub, e);
                self.log_tool_failure("experiment", &e.to_string()).await;
                Err(McpError::internal_error(format!("Experiment tool error: {}", e), None))
            }
        }
    }

    /// Test hypotheses about game behavior (requires Viewer role or higher)
    #[tool(description = "Test hypotheses about game behavior and state. Requires authentication token and Viewer role or higher.")]
    pub async fn hypothesis(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("hypothesis", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("hypothesis", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let hyp_req: HypothesisRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid hypothesis parameters: {}", e), None))?;

        debug!("User {} testing hypothesis: {}", claims.sub, hyp_req.hypothesis);
        
        let arguments = serde_json::json!({
            "hypothesis": hyp_req.hypothesis,
            "confidence": hyp_req.confidence,
            "context": hyp_req.context,
        });
        
        match hypothesis::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "hypothesis", Some(&hyp_req.hypothesis)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Hypothesis tool error for user {}: {}", claims.sub, e);
                self.log_tool_failure("hypothesis", &e.to_string()).await;
                Err(McpError::internal_error(format!("Hypothesis tool error: {}", e), None))
            }
        }
    }

    /// Detect anomalies in game behavior (requires Viewer role or higher)
    #[tool(description = "Detect anomalies in game behavior, performance, and state. Requires authentication token and Viewer role or higher.")]
    pub async fn detect_anomaly(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("detect_anomaly", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("detect_anomaly", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let anom_req: AnomalyRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid anomaly detection parameters: {}", e), None))?;

        debug!("User {} running anomaly detection: {}", claims.sub, anom_req.detection_type);
        
        let arguments = serde_json::json!({
            "type": anom_req.detection_type,
            "sensitivity": anom_req.sensitivity,
            "window": anom_req.window,
        });
        
        match anomaly::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "detect_anomaly", Some(&anom_req.detection_type)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Anomaly detection error for user {}: {}", claims.sub, e);
                self.log_tool_failure("detect_anomaly", &e.to_string()).await;
                Err(McpError::internal_error(format!("Anomaly detection error: {}", e), None))
            }
        }
    }

    /// Run stress tests (requires Developer role or higher)
    #[tool(description = "Run stress tests to find performance limits and bottlenecks. Requires authentication token and Developer role or higher.")]
    pub async fn stress_test(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("stress_test", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("stress_test", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let stress_req: StressTestRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid stress test parameters: {}", e), None))?;

        info!("User {} starting stress test: {} at intensity {}", claims.sub, stress_req.test_type, stress_req.intensity);
        
        let arguments = serde_json::json!({
            "type": stress_req.test_type,
            "intensity": stress_req.intensity,
            "duration": stress_req.duration,
            "detailed_metrics": stress_req.detailed_metrics,
        });
        
        match stress::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "stress_test", Some(&stress_req.test_type)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Stress test error for user {}: {}", claims.sub, e);
                self.log_tool_failure("stress_test", &e.to_string()).await;
                Err(McpError::internal_error(format!("Stress test error: {}", e), None))
            }
        }
    }

    /// Replay and time travel (requires Developer role or higher)
    #[tool(description = "Replay game states and perform time travel debugging. Requires authentication token and Developer role or higher.")]
    pub async fn time_travel_replay(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("time_travel_replay", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("time_travel_replay", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let replay_req: ReplayRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid replay parameters: {}", e), None))?;

        info!("User {} executing time travel replay: {}", claims.sub, replay_req.action);
        
        let arguments = serde_json::json!({
            "action": replay_req.action,
            "checkpoint_id": replay_req.checkpoint_id,
            "speed": replay_req.speed,
        });
        
        match replay::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "time_travel_replay", Some(&replay_req.action)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Replay tool error for user {}: {}", claims.sub, e);
                self.log_tool_failure("time_travel_replay", &e.to_string()).await;
                Err(McpError::internal_error(format!("Replay tool error: {}", e), None))
            }
        }
    }

    /// Create a new user (requires Admin role)
    #[tool(description = "Create a new user with specified role. Requires Admin role. Roles: viewer (read-only), developer (full debugging), admin (user management).")]
    pub async fn create_user(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("user_management", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("create_user", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        // Extract token first, then remove auth parameters
        let token = Self::extract_token_from_request(&req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let create_req: CreateUserRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid create user parameters: {}", e), None))?;

        // Parse role
        let role = match create_req.role.to_lowercase().as_str() {
            "viewer" => Role::Viewer,
            "developer" => Role::Developer,
            "admin" => Role::Admin,
            _ => return Err(McpError::invalid_params("Invalid role. Use: viewer, developer, or admin".to_string(), None)),
        };

        info!("Admin {} creating user: {} with role: {:?}", claims.sub, create_req.username, role);
        
        match self.security_manager.create_user(&token, &create_req.username, &create_req.password, role).await {
            Ok(_) => {
                Ok(CallToolResult::success(vec![
                    Content::text(format!("User {} created successfully with role {}", create_req.username, create_req.role))
                ]))
            }
            Err(e) => {
                error!("User creation failed: {}", e);
                self.log_tool_failure("create_user", &e.to_string()).await;
                Err(McpError::internal_error(format!("User creation failed: {}", e), None))
            }
        }
    }

    /// Delete a user (requires Admin role)
    #[tool(description = "Delete an existing user. Requires Admin role. Cannot delete your own account.")]
    pub async fn delete_user(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("user_management", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("delete_user", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let delete_req: DeleteUserRequest = serde_json::from_value(req)
            .map_err(|e| McpError::invalid_params(format!("Invalid delete user parameters: {}", e), None))?;

        info!("Admin {} deleting user: {}", claims.sub, delete_req.username);
        
        match self.security_manager.delete_user(&token, &delete_req.username).await {
            Ok(_) => {
                Ok(CallToolResult::success(vec![
                    Content::text(format!("User {} deleted successfully", delete_req.username))
                ]))
            }
            Err(e) => {
                error!("User deletion failed: {}", e);
                self.log_tool_failure("delete_user", &e.to_string()).await;
                Err(McpError::internal_error(format!("User deletion failed: {}", e), None))
            }
        }
    }

    /// List all users (requires Admin role)
    #[tool(description = "List all users in the system. Requires Admin role.")]
    pub async fn list_users(&self, Parameters(req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("user_management", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("list_users", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

        info!("Admin {} listing users", claims.sub);
        
        match self.security_manager.list_users(&token).await {
            Ok(users) => {
                let user_list = users.into_iter()
                    .map(|u| serde_json::json!({
                        "id": u.id,
                        "username": u.username,
                        "role": u.role,
                        "created_at": u.created_at,
                        "last_login": u.last_login,
                        "active": u.active
                    }))
                    .collect::<Vec<_>>();
                
                Ok(CallToolResult::success(vec![
                    Content::text(serde_json::to_string_pretty(&user_list).unwrap())
                ]))
            }
            Err(e) => {
                error!("List users failed: {}", e);
                self.log_tool_failure("list_users", &e.to_string()).await;
                Err(McpError::internal_error(format!("List users failed: {}", e), None))
            }
        }
    }

    /// Get audit log (requires Admin role)
    #[tool(description = "Get security audit log entries. Requires Admin role. Supports pagination with limit and offset.")]
    pub async fn get_audit_log(&self, Parameters(mut req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("audit_log_access", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("get_audit_log", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

        req.as_object_mut().map(|obj| {
            obj.remove("auth_token");
            obj.remove("authorization");
        });

        let audit_req: AuditLogRequest = serde_json::from_value(req).unwrap_or(AuditLogRequest {
            limit: Some(100),
            offset: Some(0),
        });

        info!("Admin {} accessing audit log", claims.sub);
        
        match self.security_manager.get_audit_log(&token, audit_req.limit, audit_req.offset).await {
            Ok(entries) => {
                Ok(CallToolResult::success(vec![
                    Content::text(serde_json::to_string_pretty(&entries).unwrap())
                ]))
            }
            Err(e) => {
                error!("Audit log access failed: {}", e);
                self.log_tool_failure("get_audit_log", &e.to_string()).await;
                Err(McpError::internal_error(format!("Audit log access failed: {}", e), None))
            }
        }
    }

    /// Run security vulnerability scan (requires Admin role)
    #[tool(description = "Run a comprehensive security vulnerability scan. Requires Admin role. Identifies security issues and provides remediation recommendations.")]
    pub async fn security_scan(&self, Parameters(req): Parameters<Value>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("security_scan", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("security_scan", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

        info!("Admin {} initiating security scan", claims.sub);
        
        match self.security_audit.run_security_scan(&token).await {
            Ok(report) => {
                Ok(CallToolResult::success(vec![
                    Content::text(serde_json::to_string_pretty(&report).unwrap())
                ]))
            }
            Err(e) => {
                error!("Security scan failed: {}", e);
                self.log_tool_failure("security_scan", &e.to_string()).await;
                Err(McpError::internal_error(format!("Security scan failed: {}", e), None))
            }
        }
    }
}

// Implement ServerHandler for the secure tools
impl ServerHandler for SecureMcpTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "bevy-debugger-mcp-secure".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            instructions: Some("Security-enhanced AI-assisted debugging tools for Bevy games. All operations require JWT authentication with role-based permissions.".to_string()),
        }
    }
}


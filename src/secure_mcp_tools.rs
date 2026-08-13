/*
 * Bevy Debugger MCP Server - Security-Enhanced Tool Handler
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use rmcp::{model::*, tool, tool_router, tool_handler, handler::server::{ServerHandler, router::tool::ToolRouter, tool::Parameters}, schemars, Error as McpError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{sync::Arc, future::Future};
use tokio::sync::RwLock;
use tracing::{error, info, debug, warn};
use schemars::JsonSchema;

use crate::brp_client::BrpClient;
use crate::tools::{observe, experiment, guide, hypothesis, anomaly, stress, replay};
use crate::security::{SecurityManager, SecurityMiddleware, Role, Claims, SecurityAudit};
// `Result` is NOT imported from `crate::error` here, deliberately. That alias takes one generic
// argument, and `#[tool_handler]` expands to code naming `Result<CallToolResult, ErrorData>` — so
// importing it shadows `std::result::Result` and the macro fails to compile in this module with a
// baffling "type alias takes 1 generic argument but 2 were supplied". `mcp_tools.rs` never imported
// it, which is the only reason the non-secure handler escaped this.
use crate::error::Error;
use crate::error::Result as DebugResult;

// Re-export parameter structures from the original tools
pub use crate::mcp_tools::{
    ObserveRequest, ExperimentRequest, GuideRequest, HypothesisRequest, 
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
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteUserRequest {
    pub username: String,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AuditLogRequest {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

/// Parameters for the tools that need nothing but a token: `logout`, `list_users`, `security_scan`.
///
/// It exists so those three can name a concrete type in their `#[tool]` signature. Naming
/// `Parameters<Value>` there is what produced an untyped `AnyValue` schema and made the client
/// discard the entire tool list.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TokenOnlyRequest {
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

/// The JWT carried by a tool request.
///
/// `authorize_tool_call` used to read these two fields out of a `serde_json::Value`. Now that every
/// tool names a real struct, the fields are typed, and this trait is what lets one authorization
/// helper serve all twelve of them.
pub trait AuthedRequest {
    fn auth_token(&self) -> Option<&str>;
    fn authorization(&self) -> Option<&str>;

    /// The bearer token, from either field. `auth_token` wins, matching the previous precedence.
    fn bearer(&self) -> Option<String> {
        if let Some(token) = self.auth_token() {
            return Some(token.to_string());
        }
        self.authorization()
            .and_then(|auth| auth.strip_prefix("Bearer "))
            .map(|token| token.to_string())
    }
}

macro_rules! impl_authed_request {
    ($($ty:ty),+ $(,)?) => {
        $(impl AuthedRequest for $ty {
            fn auth_token(&self) -> Option<&str> { self.auth_token.as_deref() }
            fn authorization(&self) -> Option<&str> { self.authorization.as_deref() }
        })+
    };
}

impl_authed_request!(
    TokenOnlyRequest,
    GuideRequest,
    CreateUserRequest,
    DeleteUserRequest,
    AuditLogRequest,
    ObserveRequest,
    ExperimentRequest,
    HypothesisRequest,
    AnomalyRequest,
    StressTestRequest,
    ReplayRequest,
);

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

    /// Extract JWT token from a typed request.
    ///
    /// This read the two fields out of an untyped `serde_json::Value` until every tool started
    /// naming a real parameter struct; the precedence (`auth_token`, then a `Bearer` prefix on
    /// `authorization`) is unchanged and now lives in [`AuthedRequest::bearer`].
    fn extract_token_from_request(params: &impl AuthedRequest) -> Option<String> {
        params.bearer()
    }

    /// Validate and authorize a tool call
    async fn authorize_tool_call(&self, operation: &str, params: &impl AuthedRequest) -> DebugResult<Claims> {
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
    pub async fn logout(&self, Parameters(params): Parameters<TokenOnlyRequest>) -> std::result::Result<CallToolResult, McpError> {
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
    pub async fn observe(&self, Parameters(observe_req): Parameters<ObserveRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("observe", &observe_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("observe", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        // The auth fields no longer need stripping before the call below: they are named fields on
        // `ObserveRequest`, and the `arguments` object is rebuilt field by field rather than forwarded.

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

    /// Guide the human through the running app (requires Developer role or higher)
    #[tool(description = "Guide the HUMAN through something in the running app, and get back what actually happened. This is the only tool here that talks to the person rather than the game: you post a short script, the app draws ONE step at a time over their own window, and it advances itself when a named condition arrives. Use it to walk somebody through reproducing a bug, to hand over an acceptance test, or any time you would otherwise type a numbered list into the chat and hope they map it onto an interface you cannot see.\n\nHOW TO COLLABORATE (not optional -- getting it wrong strands them):\n1. Post steps, then TELL THEM IN CHAT that the guide is up and to look at the app window. Nothing pops up to get their attention.\n2. Poll with {read:true}. When it answers waiting_on_a_person:true, that step has NO machine check and will never advance on its own. STOP AND ASK THEM IN CHAT, by name, and wait for a real answer before calling {skip:true}. The card tells them 'tell the agent' -- you are the agent, and that promise is yours to keep.\n3. When they say a step made no sense or did not work, that is the FINDING, not a detour. Most people follow a confusing instruction rather than report it, so the reports you do get are worth more than a clean run.\n4. Never name a checkpoint the app has not registered: it is refused by name, but the person is already standing in front of it.\n\nWrite steps as you would brief a colleague: WHY before what, two to four short imperatives, and always fill in recovery. What comes back is k/n per step -- passes over attempts -- never a boolean, because 'it worked' is exactly the judgement people get wrong. Requires authentication token and Developer role or higher.")]
    pub async fn guide(&self, Parameters(req): Parameters<GuideRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("guide", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("guide", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        // Rebuilt field by field rather than forwarded, so the auth fields cannot reach the app --
        // the same reason `observe` does it, and the reason these are named fields rather than a
        // flattened `Value`.
        let mut arguments = serde_json::Map::new();
        if let Some(steps) = req.steps.clone() {
            arguments.insert("steps".to_string(), serde_json::Value::Array(steps));
        }
        for (key, on) in [("read", req.read), ("skip", req.skip), ("clear", req.clear)] {
            if on {
                arguments.insert(key.to_string(), serde_json::Value::Bool(true));
            }
        }
        if let Some(visible) = req.visible {
            arguments.insert("visible".to_string(), serde_json::Value::Bool(visible));
        }
        let what = if req.steps.is_some() { "post" } else if req.skip { "skip" } else { "read" };
        debug!("User {} guide: {what}", claims.sub);

        match guide::handle(serde_json::Value::Object(arguments), self.brp_client.clone()).await {
            Ok(result) => {
                self.log_tool_success(&claims, "guide", Some(what)).await;
                Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
            }
            Err(e) => {
                error!("Guide tool error for user {}: {}", claims.sub, e);
                self.log_tool_failure("guide", &e.to_string()).await;
                Err(McpError::internal_error(format!("Guide tool error: {}", e), None))
            }
        }
    }

    /// Run controlled experiments on game state (requires Developer role or higher)
    #[tool(description = "Run controlled experiments on your Bevy game to test behavior and performance. Requires authentication token and Developer role or higher.")]
    pub async fn experiment(&self, Parameters(exp_req): Parameters<ExperimentRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("experiment", &exp_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("experiment", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

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
    pub async fn hypothesis(&self, Parameters(hyp_req): Parameters<HypothesisRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("hypothesis", &hyp_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("hypothesis", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

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
    pub async fn detect_anomaly(&self, Parameters(anom_req): Parameters<AnomalyRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("detect_anomaly", &anom_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("detect_anomaly", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

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
    pub async fn stress_test(&self, Parameters(stress_req): Parameters<StressTestRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("stress_test", &stress_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("stress_test", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

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
    pub async fn time_travel_replay(&self, Parameters(replay_req): Parameters<ReplayRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("time_travel_replay", &replay_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("time_travel_replay", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

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
    pub async fn create_user(&self, Parameters(create_req): Parameters<CreateUserRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("user_management", &create_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("create_user", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&create_req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

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
    pub async fn delete_user(&self, Parameters(delete_req): Parameters<DeleteUserRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("user_management", &delete_req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("delete_user", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&delete_req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

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
    pub async fn list_users(&self, Parameters(req): Parameters<TokenOnlyRequest>) -> std::result::Result<CallToolResult, McpError> {
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
    pub async fn get_audit_log(&self, Parameters(req): Parameters<AuditLogRequest>) -> std::result::Result<CallToolResult, McpError> {
        let claims = match self.authorize_tool_call("audit_log_access", &req).await {
            Ok(claims) => claims,
            Err(e) => {
                self.log_tool_failure("get_audit_log", &e.to_string()).await;
                return Err(McpError::invalid_params(format!("Authorization failed: {}", e), None));
            }
        };

        let token = Self::extract_token_from_request(&req)
            .ok_or_else(|| McpError::invalid_params("Authentication token required".to_string(), None))?;

        // The auth fields used to be stripped out of a `Value` by hand and the remainder re-parsed
        // into this struct, with a silent `unwrap_or` default when that failed — so a typo in `limit`
        // quietly returned the first 100 entries instead of complaining. Deserializing once removes
        // both the dance and the silence.
        let (limit, offset) = (req.limit, req.offset);

        info!("Admin {} accessing audit log", claims.sub);
        
        match self.security_manager.get_audit_log(&token, limit, offset).await {
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
    pub async fn security_scan(&self, Parameters(req): Parameters<TokenOnlyRequest>) -> std::result::Result<CallToolResult, McpError> {
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
// **Without this the tool router is built and never consulted.** `#[tool_router]` on the impl block
// above collects every `#[tool]` into `self.tool_router`, but it is `#[tool_handler]` that generates
// the `list_tools`/`call_tool` implementations which read it. A hand-written `ServerHandler` supplying
// only `get_info` inherits the trait's defaults instead — an empty tool list and a call handler that
// knows nothing — so the server advertised `tools` capability and then answered `tools/list` with
// zero tools. `mcp_tools.rs` has carried this attribute all along; the secure variant lost it.
#[tool_handler(router = self.tool_router)]
impl ServerHandler for SecureMcpTools {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "bevy-debugger-mcp-secure".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            // **The collaboration protocol lives here, because this is the one string every agent
            // reads before it touches a tool.** It said what the server is, which a tool list
            // already says. The half worth spending words on is the half agents get wrong.
            //
            // `guide` is the only tool here that involves a person, and the failure it invites is
            // silence: post a script, go quiet, and somebody is left in front of a card that will
            // never move. That happened during this tool's own development, to the person it was
            // built for, who asked the reasonable question -- "how am I supposed to tell you whether
            // it worked? Am I supposed to come back to the chat?" The answer is yes, and nothing
            // anywhere said so.
            // **The shared protocol, with this variant's one extra fact in front of it.**
            // `crate::mcp_tools::COLLABORATION_PROTOCOL` is the single copy; duplicating it here is
            // how the two server variants would drift into giving agents different rules.
            instructions: Some(format!(
                "All operations on this server require JWT authentication (`authenticate`) with \
                 role-based permissions.\n\n{}",
                crate::mcp_tools::COLLABORATION_PROTOCOL
            )),
        }
    }
}


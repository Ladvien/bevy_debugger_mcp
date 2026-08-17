/*
 * Bevy Debugger MCP Server - Centralized Tool Definitions
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
use tracing::{error, info, debug};
use schemars::JsonSchema;

use crate::brp_client::BrpClient;
use crate::tools::{observe, experiment, guide, hypothesis, anomaly, stress, replay};

// Parameter structures for tools
//
// **Every one of these must be named in a `#[tool]` signature, not deserialized from
// `Parameters<Value>` inside the body.** `schemars` renders `serde_json::Value` as
// `{"$schema": …, "title": "AnyValue"}` — a schema with no `type` and no `properties` — and an MCP
// client rejects the whole tool list with `expected "object" (at tools.N.inputSchema.type)`. The
// tools were unreachable in Claude Code for exactly that reason: `authenticate` named `AuthRequest`
// and advertised a correct object schema, and all twelve others named `Value` and did not.
//
// The `auth_token`/`authorization` pair is declared field-by-field rather than as a shared
// `#[serde(flatten)]` wrapper on purpose: `schemars` 0.8 lowers a flattened generic to `allOf`,
// which reintroduces exactly the "top level is not a plain object" shape the client rejects.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ObserveRequest {
    pub query: String,
    #[serde(default)]
    pub diff: bool,
    #[serde(default)]
    pub detailed: bool,
    #[serde(default)]
    pub reflection: bool,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

/// What `guide` accepts. One tool, four verbs, because they are four things you do to one script
/// and splitting them would make an agent choose a tool before it knows what it wants.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct GuideRequest {
    /// The steps to post, replacing whatever is up. Each step is an object:
    /// `label` (required), `goal`, `do` (array of 2-4 short imperatives), `checkpoint`
    /// (a condition name the APP registered, or null when only a person can judge it),
    /// and `recovery` (what to do when it does not work).
    #[serde(default)]
    pub steps: Option<Vec<Value>>,
    /// Return the transcript and where the person is, changing nothing. Safe to call at any time,
    /// and the way to find out whether the current step is waiting on them.
    #[serde(default)]
    pub read: bool,
    /// Move past the current step WITHOUT its condition passing. This is how a step only a person
    /// can judge gets advanced -- after they have told you, never before.
    #[serde(default)]
    pub skip: bool,
    /// Take the script down. The transcript survives, because the transcript is the result.
    #[serde(default)]
    pub clear: bool,
    /// Show or hide the card without discarding the script.
    #[serde(default)]
    pub visible: Option<bool>,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ExperimentRequest {
    #[serde(rename = "type")]
    pub experiment_type: String,
    #[serde(default)]
    pub params: Value,
    pub duration: Option<f32>,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HypothesisRequest {
    pub hypothesis: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    pub context: Option<Value>,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AnomalyRequest {
    #[serde(rename = "type")]
    pub detection_type: String,
    #[serde(default = "default_sensitivity")]
    pub sensitivity: f32,
    pub window: Option<f32>,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
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
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReplayRequest {
    pub action: String,
    pub checkpoint_id: Option<String>,
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// JWT from `authenticate`.
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Alternative to `auth_token`, as `"Bearer <jwt>"`.
    #[serde(default)]
    pub authorization: Option<String>,
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

/// **How an agent is expected to work with a human through this server.**
///
/// One string, used by both server variants, because it is a rule and a rule with two copies is two
/// rules. `SecureMcpTools` prepends its own line about authentication and then says this.
///
/// It is here rather than in a doc comment because it is the one thing every agent reads before it
/// touches a tool, and the half worth spending words on is the half agents get wrong. `guide` is the
/// only tool in this server that involves a person, and the failure it invites is silence: post a
/// script, go quiet, and somebody is left in front of a card that will never move. That happened
/// during the tool's own development, to the person it was built for, who asked the reasonable
/// question -- "how am I supposed to tell you whether it worked? Am I supposed to come back to the
/// chat?" The answer is yes, and nothing anywhere said so.
pub const COLLABORATION_PROTOCOL: &str =
                "Debugging tools for a RUNNING Bevy game, over the Bevy Remote Protocol. \
                 `observe`/`hypothesis`/`detect_anomaly` inspect it, `experiment`/`stress_test` \
                 poke it, `time_travel_replay` re-runs it.\n\n\
                 WORKING WITH THE PERSON AT THE KEYBOARD:\n\
                 `guide` is different from the rest -- it talks to the human, not the game. It \
                 draws one instruction at a time on their own window and watches for the state it \
                 asked for. Reach for it when you would otherwise type a numbered list into the \
                 chat and hope they map it onto an interface you cannot see: reproducing a bug, \
                 walking an acceptance test, or checking something only a person can look at.\n\n\
                 Three rules, all learned the hard way:\n\
                 - AFTER POSTING A SCRIPT, SAY SO IN CHAT. Nothing in the app gets their attention. \
                 An agent that posts and goes quiet has hidden the instructions rather than \
                 delivered them.\n\
                 - A step with `checkpoint: null` NEVER ADVANCES ON ITS OWN. It is the question a \
                 machine cannot answer, so ask it in the conversation, by name, and wait for a real \
                 answer before calling `skip`. The card promises them 'tell the agent'; you are the \
                 agent.\n\
                 - WHEN THEY SAY A STEP MADE NO SENSE, THAT IS THE FINDING. Do not work around it. \
                 Most people follow a confusing instruction rather than report it, so the ones you \
                 do get are worth more than a passing run.\n\n\
                 What comes back is k/n per step -- times passed over times attempted -- never a \
                 boolean, because 'it worked' is precisely the judgement people get wrong.\n\n\
                 SEEING THE APP: `bevy_debugger/screenshot` captures OFFSCREEN and therefore cannot \
                 see any UI. If you need to know what a panel says, ask the person, or have them \
                 run a whole-frame capture. Do not raise their window.";

#[tool_router]
impl BevyDebuggerTools {
    /// Observe and query Bevy game state
    #[tool(description = "Observe and query Bevy game state in real-time with optional reflection-based component inspection. Use this to inspect entities, components, resources, and game state. Enable 'reflection' parameter for deep component analysis including field inspection, type information, and custom inspectors for complex types like Option<T>, Vec<T>, HashMap<K,V>. Perfect for debugging entity spawning, component updates, and understanding your ECS architecture.")]
    pub async fn observe(&self, Parameters(req): Parameters<ObserveRequest>) -> Result<CallToolResult, McpError> {
        debug!("Executing observe query: {}", req.query);
        
        let arguments = serde_json::json!({
            "query": req.query,
            "diff": req.diff,
            "detailed": req.detailed,
            "reflection": req.reflection,
        });
        
        match observe::handle(arguments, self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Observe tool error: {}", e);
                Err(McpError::internal_error(format!("Observe tool error: {}", e), None))
            }
        }
    }

    /// Guide a human through the running app
    #[tool(description = "Guide the HUMAN through something in the running app, and get back what actually happened. This is the only tool here that talks to the person rather than the game: you post a short script, the app draws ONE step at a time over their own window, and it advances itself when a named condition arrives. Use it to walk somebody through reproducing a bug, to hand over an acceptance test, or any time you would otherwise type a numbered list into the chat and hope they map it onto an interface you cannot see.\n\nHOW TO COLLABORATE (this part is not optional -- getting it wrong strands them):\n1. Post steps, then TELL THEM IN CHAT that the guide is up and to look at the app window. They will not know otherwise; nothing pops up to get their attention.\n2. Poll with {read:true}. When it answers waiting_on_a_person:true, that step has NO machine check and will never advance on its own. STOP AND ASK THEM IN CHAT, in words, naming the step. The card tells them 'tell the agent' -- that promise is yours to keep. Only after they answer, call {skip:true}.\n3. When they say a step made no sense or did not work, that is DATA, not a detour. Record it and fix the step or the app. The transcript is k/n per step -- passes over attempts -- because a boolean 'it worked' is the thing people get wrong.\n4. Never post a step whose checkpoint the app has not registered. The app refuses it by name and lists what would have worked, but the person is already standing there by then.\n\nWrite steps the way you would brief a colleague: say WHY before what (people ignore a suggestion whose reason they cannot see), two to four short imperatives, and always fill in recovery -- what to do when the step does not work is the field that makes this better than a list in a chat window.")]
    pub async fn guide(&self, Parameters(req): Parameters<GuideRequest>) -> Result<CallToolResult, McpError> {
        let mut arguments = serde_json::Map::new();
        if let Some(steps) = req.steps {
            arguments.insert("steps".to_string(), Value::Array(steps));
        }
        if req.read {
            arguments.insert("read".to_string(), Value::Bool(true));
        }
        if req.skip {
            arguments.insert("skip".to_string(), Value::Bool(true));
        }
        if req.clear {
            arguments.insert("clear".to_string(), Value::Bool(true));
        }
        if let Some(visible) = req.visible {
            arguments.insert("visible".to_string(), Value::Bool(visible));
        }
        debug!("Guide request: {} key(s)", arguments.len());

        match guide::handle(Value::Object(arguments), self.brp_client.clone()).await {
            Ok(result) => Ok(CallToolResult::success(vec![Content::text(result.to_string())])),
            Err(e) => {
                error!("Guide tool error: {}", e);
                Err(McpError::internal_error(format!("Guide tool error: {}", e), None))
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
            // **The collaboration protocol lives here, because this is the one string every agent
            // reads before it touches a tool.** It was one sentence describing the server, which is
            // the least useful thing it could say: a tool list already describes the server.
            //
            // The half worth spending words on is the half agents get wrong. `guide` is the only
            // tool here that involves a person, and the failure it invites is silence -- post a
            // script, go quiet, and somebody is left in front of a card that will never move. That
            // happened during the tool's own development, to the person it was built for, who asked
            // the reasonable question: "how am I supposed to tell you whether it worked? Am I
            // supposed to come back to the chat?" The answer is yes, and nothing said so.
            instructions: Some(COLLABORATION_PROTOCOL.to_string()),
        }
    }
}
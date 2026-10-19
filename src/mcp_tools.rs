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
use serde::Deserialize;
use serde_json::Value;
use std::{sync::Arc, future::Future};
use tokio::sync::RwLock;
use tracing::{error, info, debug};
use schemars::JsonSchema;

use crate::brp_client::BrpClient;
use crate::tools::{observe, experiment, guide, hypothesis, anomaly, stress, replay};
use crate::brp::{BrpPayload, BrpRequest};
use base64::Engine;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Parameter structures for tools
//
// **Every one of these must be named in a `#[tool]` signature, not deserialized from
// `Parameters<Value>` inside the body.** `schemars` renders `serde_json::Value` as
// `{"$schema": …, "title": "AnyValue"}` — a schema with no `type` and no `properties` — and an MCP
// client rejects the whole tool list with `expected "object" (at tools.N.inputSchema.type)`. The
// tools were unreachable in Claude Code for exactly that reason.

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ObserveRequest {
    pub query: String,
    #[serde(default)]
    pub diff: bool,
    #[serde(default)]
    pub detailed: bool,
    #[serde(default)]
    pub reflection: bool,
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

/// A tool that takes no arguments. It is a named, empty struct for the reason given at the top of this file.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoArgs {}

/// `run`: run one command the game registered. `commands` lists them with their parameters.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RunRequest {
    /// The command name, `<game>/<verb>`, exactly as `commands` lists it.
    pub command: String,
    /// The command's parameters as a JSON object. Omit it for a command that takes none.
    #[serde(default)]
    pub params: Option<Value>,
}

/// `brp`: a raw Bevy Remote Protocol call, for any method the game serves.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BrpCallRequest {
    /// The BRP method, for example `world.query` or `registry.schema`.
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

/// A rectangle of the captured image, in its pixels.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// `screenshot`: capture the game to a PNG on this machine.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotRequest {
    /// File to write on this machine. Default: `$BEVY_DEBUGGER_CAPTURE_DIR/capture-<unix_ms>.png`,
    /// or `./debug_captures/` when that variable is unset.
    #[serde(default)]
    pub save_to: Option<String>,
    /// `offscreen` (the default) renders the game's own image and leaves out UI. `window` captures
    /// the game window, UI included.
    #[serde(default)]
    pub source: Option<String>,
    /// Crop to this rectangle before saving.
    #[serde(default)]
    pub region: Option<CaptureRegion>,
    /// Magnify the captured image by this factor. Default 1.
    #[serde(default)]
    pub zoom: Option<f32>,
    /// Also return the PNG inline, so the agent sees the image as well as the saved file.
    #[serde(default)]
    pub inline: Option<bool>,
}

/// What `scene` does with the commands recorded in this game session.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SceneAction {
    /// Write the recorded commands to `path` as a replayable scene file.
    Save,
    /// Return the recorded commands.
    Show,
    /// Forget the recorded commands.
    Clear,
}

/// `scene`: save, show or clear the recorded commands.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SceneRequest {
    pub action: SceneAction,
    /// Required for `save`: the file to write, on this machine.
    #[serde(default)]
    pub path: Option<String>,
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

/// One entry of `bevy_debugger/commands`, as the game lists it.
#[derive(Debug, Deserialize)]
struct CommandListing {
    commands: Vec<CommandEntry>,
}

#[derive(Debug, Deserialize)]
struct CommandEntry {
    name: String,
    description: String,
    #[serde(default)]
    params: String,
    #[serde(default)]
    example: Value,
}

const STATUS_METHOD: &str = "bevy_debugger/status";
const COMMANDS_METHOD: &str = "bevy_debugger/commands";
const TIME_COMMAND: &str = "bevy_debugger/time";
const SCREENSHOT_METHOD: &str = "bevy_debugger/screenshot";
const CAPTURE_METHOD: &str = "bevy_debugger/capture";
const SCENE_METHOD: &str = "bevy_debugger/scene";
const QUIT_METHOD: &str = "bevy_debugger/quit";

/// The text an agent reads when nothing answers. Kept exact, because agents match on it.
fn no_game_text(url: &str) -> String {
    format!("bevy-debugger: no game answering at {url} — start the game with DebuggerPlugin and RemoteHttpPlugin, or set BEVY_BRP_HOST/BEVY_BRP_PORT")
}

fn tool_ok(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::success(vec![Content::text(text.into())]))
}

fn tool_err(text: impl Into<String>) -> Result<CallToolResult, McpError> {
    Ok(CallToolResult::error(vec![Content::text(text.into())]))
}

fn command_block(entry: &CommandEntry) -> String {
    let params = if entry.params.is_empty() { "(none)" } else { entry.params.as_str() };
    format!("{} — {}\n  params: {}\n  example: {}", entry.name, entry.description, params, entry.example)
}

/// Name the closest registered command, and list them all, when `asked` is not one of them.
fn unknown_command_text(asked: &str, entries: &[CommandEntry]) -> String {
    let known: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    let closest = known
        .iter()
        .copied()
        .max_by(|a, b| strsim::jaro_winkler(asked, a).total_cmp(&strsim::jaro_winkler(asked, b)));
    match closest {
        Some(closest) => format!("unknown command `{asked}`; did you mean `{closest}`? known: {}", known.join(", ")),
        None => format!("unknown command `{asked}`; the game has registered no commands"),
    }
}

fn write_file_creating_dirs(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)
}

/// `$BEVY_DEBUGGER_CAPTURE_DIR/capture-<unix_ms>.png`, or under `./debug_captures` when unset.
fn default_capture_path() -> PathBuf {
    let dir = std::env::var_os("BEVY_DEBUGGER_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("debug_captures"));
    let unix_ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or_default();
    dir.join(format!("capture-{unix_ms}.png"))
}

impl BevyDebuggerTools {
    /// One BRP call to the game. The `Err` is the text an agent reads: the no-game message when
    /// nothing answers, or the game's own error message verbatim.
    async fn call_game(&self, method: &str, params: Option<Value>) -> Result<Value, String> {
        let request = BrpRequest { method: method.to_string(), id: Some(Value::from(1)), params };
        let mut client = self.brp_client.write().await;
        match client.send_request(&request).await {
            Ok(response) => match response.payload {
                BrpPayload::Result(value) => Ok(value),
                BrpPayload::Error(err) => Err(err.message),
            },
            Err(e) => {
                debug!("BRP call {method} did not reach the game: {e}");
                Err(no_game_text(client.url()))
            }
        }
    }

    async fn game_url(&self) -> String {
        self.brp_client.read().await.url().to_string()
    }

    /// Poll `bevy_debugger/status` until the game reports ready, for at most 120 s.
    async fn wait_until_ready(&self) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let status = self.call_game(STATUS_METHOD, None).await?;
            if status.get("ready").and_then(Value::as_bool) == Some(true) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err("game not ready after 120 s".to_string());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// Every command the game registered, plus `bevy_debugger/time`, with the built-ins last.
    async fn command_list(&self) -> Result<Vec<CommandEntry>, String> {
        let value = self.call_game(COMMANDS_METHOD, None).await?;
        let listing: CommandListing = serde_json::from_value(value)
            .map_err(|e| format!("{COMMANDS_METHOD} returned an unexpected reply: {e}"))?;
        let mut entries = listing.commands;
        if !entries.iter().any(|entry| entry.name == TIME_COMMAND) {
            entries.push(CommandEntry {
                name: TIME_COMMAND.to_string(),
                description: "Pause or slow game time (Time<Virtual>); returns the paused flag and the speed.".to_string(),
                params: "pause: bool (optional); speed: f32 (optional, must be > 0)".to_string(),
                example: serde_json::json!({"pause": true}),
            });
        }
        entries.sort_by(|a, b| {
            (a.name.starts_with("bevy_debugger/"), &a.name).cmp(&(b.name.starts_with("bevy_debugger/"), &b.name))
        });
        Ok(entries)
    }
}

/// **How an agent is expected to work with a human through this server.**
///
/// One string, served by this server, because it is a rule and a rule with two copies is two
/// rules.
///
/// It is here rather than in a doc comment because it is the one thing every agent reads before it
/// touches a tool, and the half worth spending words on is the half agents get wrong. `guide` is the
/// only tool in this server that involves a person, and the failure it invites is silence: post a
/// script, go quiet, and somebody is left in front of a card that will never move. That happened
/// during the tool's own development, to the person it was built for, who asked the reasonable
/// question -- "how am I supposed to tell you whether it worked? Am I supposed to come back to the
/// chat?" The answer is yes, and nothing anywhere said so.
pub const COLLABORATION_PROTOCOL: &str =
                "Debugging tools for a RUNNING Bevy game, over the Bevy Remote Protocol.\n\n\
                 DRIVING A GAME THAT REGISTERS ITS OWN COMMANDS: call `status` first (is the game up and \
                 ready?), then `commands` (what can be run, with each one's parameters and an example), then \
                 `run` for each step. `screenshot` saves a PNG on this machine, `scene save` writes the \
                 commands you ran to a replayable file, `brp` calls any Bevy Remote Protocol method, and \
                 `quit` closes the game.\n\n\
                 INSPECTING THE GAME: `observe`/`hypothesis`/`detect_anomaly` inspect it, `experiment`/\
                 `stress_test` poke it, `replay` re-runs it.\n\n\
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
                 CAPTURES: the default offscreen capture renders the game's own image and leaves out UI. \
                 `screenshot` with source \"window\" captures the game window with its UI included, without \
                 raising or focusing it. A covered or minimised window can give a stale frame, so use it only \
                 when the UI is what you need to see.";

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

    /// Report whether the game is answering and ready for commands
    #[tool(description = "Report whether the game is answering and ready for commands: its frame, how many commands it registered, where captures come from, and replay progress. The reply also carries the URL this server talks to. Call this first.")]
    pub async fn status(&self, Parameters(_req): Parameters<NoArgs>) -> Result<CallToolResult, McpError> {
        let url = self.game_url().await;
        match self.call_game(STATUS_METHOD, None).await {
            Ok(mut value) => {
                if let Some(fields) = value.as_object_mut() {
                    fields.insert("url".to_string(), Value::from(url));
                }
                tool_ok(value.to_string())
            }
            Err(text) => tool_err(text),
        }
    }

    /// List the commands the game registered
    #[tool(description = "List the commands the game registered, each with its parameters and a working example. Call this before `run`. The built-in bevy_debugger/time is listed last.")]
    pub async fn commands(&self, Parameters(_req): Parameters<NoArgs>) -> Result<CallToolResult, McpError> {
        match self.command_list().await {
            Ok(entries) => {
                let blocks: Vec<String> = entries.iter().map(command_block).collect();
                tool_ok(blocks.join("\n\n"))
            }
            Err(text) => tool_err(text),
        }
    }

    /// Run one command the game registered
    #[tool(description = "Run one command the game registered, by name, with its parameters. Waits up to 120 s for the game to be ready. An unknown name is refused with the closest known one. Every command that runs is recorded, so `scene save` can write the run out as a replayable file.")]
    pub async fn run(&self, Parameters(req): Parameters<RunRequest>) -> Result<CallToolResult, McpError> {
        if let Err(text) = self.wait_until_ready().await {
            return tool_err(text);
        }
        let entries = match self.command_list().await {
            Ok(entries) => entries,
            Err(text) => return tool_err(text),
        };
        if !entries.iter().any(|entry| entry.name == req.command) {
            return tool_err(unknown_command_text(&req.command, &entries));
        }
        match self.call_game(&req.command, req.params).await {
            Ok(value) => tool_ok(value.to_string()),
            Err(text) => tool_err(text),
        }
    }

    /// Call any Bevy Remote Protocol method the game serves
    #[tool(description = "Call any Bevy Remote Protocol method the game serves, for example world.query or registry.schema, and return its result. Use `run` for the game's own commands.")]
    pub async fn brp(&self, Parameters(req): Parameters<BrpCallRequest>) -> Result<CallToolResult, McpError> {
        match self.call_game(&req.method, req.params).await {
            Ok(value) => tool_ok(value.to_string()),
            Err(text) => tool_err(text),
        }
    }

    /// Capture the game to a PNG file
    #[tool(description = "Capture the game to a PNG file on this machine and return its path and size. The default offscreen capture leaves out UI; source 'window' captures the game window with its UI included. Pass inline true to also get the image in the reply.")]
    pub async fn screenshot(&self, Parameters(req): Parameters<ScreenshotRequest>) -> Result<CallToolResult, McpError> {
        let mut params = serde_json::Map::new();
        if let Some(source) = req.source {
            params.insert("source".to_string(), Value::from(source));
        }
        if let Some(region) = req.region {
            params.insert(
                "region".to_string(),
                serde_json::json!({"x": region.x, "y": region.y, "width": region.width, "height": region.height}),
            );
        }
        if let Some(zoom) = req.zoom {
            params.insert("zoom".to_string(), Value::from(zoom));
        }
        let started = match self.call_game(SCREENSHOT_METHOD, Some(Value::Object(params))).await {
            Ok(value) => value,
            Err(text) => return tool_err(text),
        };
        let Some(id) = started.get("id").and_then(Value::as_u64) else {
            return tool_err(format!("{SCREENSHOT_METHOD} did not return a capture id: {started}"));
        };
        let deadline = Instant::now() + Duration::from_secs(15);
        let captured = loop {
            let state = match self.call_game(CAPTURE_METHOD, Some(serde_json::json!({"id": id}))).await {
                Ok(value) => value,
                Err(text) => return tool_err(text),
            };
            let status = state.get("status").and_then(Value::as_str).map(str::to_owned);
            match status.as_deref() {
                Some("done") => break state,
                Some("failed") => {
                    let why = state.get("error").and_then(Value::as_str).unwrap_or("the capture failed");
                    return tool_err(why);
                }
                Some("pending") => {}
                _ => return tool_err(format!("{CAPTURE_METHOD} returned an unexpected reply: {state}")),
            }
            if Instant::now() >= deadline {
                return tool_err(format!("capture {id} still pending after 15 s"));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        };
        let width = captured.get("width").and_then(Value::as_u64).unwrap_or_default();
        let height = captured.get("height").and_then(Value::as_u64).unwrap_or_default();
        let Some(png_base64) = captured.get("png_base64").and_then(Value::as_str).map(str::to_owned) else {
            return tool_err(format!("{CAPTURE_METHOD} returned no png_base64"));
        };
        let png = match base64::engine::general_purpose::STANDARD.decode(&png_base64) {
            Ok(bytes) => bytes,
            Err(e) => return tool_err(format!("the capture was not valid base64: {e}")),
        };
        let path = req.save_to.map(PathBuf::from).unwrap_or_else(default_capture_path);
        if let Err(e) = write_file_creating_dirs(&path, &png) {
            return tool_err(format!("could not write {}: {e}", path.display()));
        }
        let absolute = std::path::absolute(&path).unwrap_or_else(|_| path.clone());
        let mut content = vec![Content::text(format!("saved {} ({width}x{height})", absolute.display()))];
        if req.inline.unwrap_or(false) {
            content.push(Content::image(png_base64.to_string(), "image/png"));
        }
        Ok(CallToolResult::success(content))
    }

    /// Save, show or clear the recorded commands
    #[tool(description = "Work with the commands recorded in this game session. save writes them to a replayable scene file on this machine (path required); show returns them; clear forgets them.")]
    pub async fn scene(&self, Parameters(req): Parameters<SceneRequest>) -> Result<CallToolResult, McpError> {
        match req.action {
            SceneAction::Save => {
                let Some(path) = req.path else {
                    return tool_err("scene save needs a path");
                };
                let scene = match self.call_game(SCENE_METHOD, Some(serde_json::json!({"action": "get"}))).await {
                    Ok(value) => value,
                    Err(text) => return tool_err(text),
                };
                let steps = scene.get("steps").and_then(Value::as_array).map_or(0, Vec::len);
                let text = match serde_json::to_string_pretty(&scene) {
                    Ok(text) => text,
                    Err(e) => return tool_err(format!("could not format the scene: {e}")),
                };
                let path = PathBuf::from(path);
                if let Err(e) = write_file_creating_dirs(&path, text.as_bytes()) {
                    return tool_err(format!("could not write {}: {e}", path.display()));
                }
                let absolute = std::path::absolute(&path).unwrap_or_else(|_| path.clone());
                tool_ok(format!("saved {} ({steps} steps)", absolute.display()))
            }
            SceneAction::Show => {
                let scene = match self.call_game(SCENE_METHOD, Some(serde_json::json!({"action": "get"}))).await {
                    Ok(value) => value,
                    Err(text) => return tool_err(text),
                };
                let steps = scene.get("steps").cloned().unwrap_or_else(|| Value::Array(Vec::new()));
                tool_ok(steps.to_string())
            }
            SceneAction::Clear => match self.call_game(SCENE_METHOD, Some(serde_json::json!({"action": "clear"}))).await {
                Ok(value) => tool_ok(value.to_string()),
                Err(text) => tool_err(text),
            },
        }
    }

    /// Ask the game to quit
    #[tool(description = "Ask the game to quit cleanly.")]
    pub async fn quit(&self, Parameters(_req): Parameters<NoArgs>) -> Result<CallToolResult, McpError> {
        match self.call_game(QUIT_METHOD, None).await {
            Ok(value) => tool_ok(value.to_string()),
            Err(text) => tool_err(text),
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
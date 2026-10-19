//! Game-registered debug commands: the game names its own console verbs, and an agent discovers and
//! calls them over BRP.
//!
//! # Why this exists
//!
//! Game engines expose game-specific console commands and a debug camera and pause so that a
//! developer can put the game into a state worth looking at (Gregory, *Game Engine Architecture*,
//! 3rd ed., §10.4–10.6). A generic BRP call can only reach what Bevy reflects, and a game's own
//! components are rarely reflected. So the game registers typed verbs instead, with
//! [`DebugCommandsAppExt::add_debug_command`], and the server discovers them at run time through
//! `bevy_debugger/commands` rather than from a list baked into the server. That is the
//! "stable, well-specified tool schema with exemplars" pattern (Xu, *AI Agent Systems*, 2026,
//! DOI 10.48550/arXiv.2601.01743): the schema travels with the command.
//!
//! # Why every command carries an example
//!
//! Each command ships a `description`, a `params` explanation and an `example`. The example is
//! parsed into the command's parameter type when the command is registered, so a stale example
//! stops the game at startup instead of misleading an agent later. The same documentation-first
//! reasoning as the Rust crypto API study (Mindermann, Keck & Wagner, QRS 2018,
//! DOI 10.1109/QRS.2018.00028): an API whose parameters are not explained is used wrongly.
//!
//! # Why the wrapper records
//!
//! A command that succeeds and is marked `records` is appended to [`SceneLog`](crate::SceneLog) with
//! the params it was called with, so the session can be saved as a replayable scene. A command that
//! only reads (see [`DebugCommandInfo::read_only`]) is never recorded: replaying it would change
//! nothing.

use bevy::prelude::*;
use bevy::remote::{error_codes, BrpResult, RemoteMethodSystemId, RemoteMethods};
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use bevy::ecs::system::SystemId;

use crate::ready::DebugReady;
use crate::scene::{SceneLog, SceneStep};

/// What a game command is, as `bevy_debugger/commands` reports it.
#[derive(Debug, Clone)]
pub struct DebugCommandInfo {
    /// The BRP method name, `<game>/<verb>`, for example `sanctuary/spawn`.
    pub name: String,
    /// One sentence on what the command does.
    pub description: String,
    /// A plain-language explanation of each parameter. Empty when the command takes none.
    pub params: String,
    /// A params value that the command must accept. Checked at registration.
    pub example: Value,
    /// Whether a successful call is written to the scene log. `false` for a query.
    pub records: bool,
}

impl DebugCommandInfo {
    /// A recorded command with no parameters and an empty example.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            params: String::new(),
            example: json!({}),
            records: true,
        }
    }

    /// Explain the parameters in plain language.
    pub fn params(mut self, doc: impl Into<String>) -> Self {
        self.params = doc.into();
        self
    }

    /// The params value a caller would send. It must parse as the command's parameter type.
    pub fn example(mut self, example: Value) -> Self {
        self.example = example;
        self
    }

    /// A command that only reads. It is never written to a scene.
    pub fn read_only(mut self) -> Self {
        self.records = false;
        self
    }
}

/// Every debug command the game registered, kept sorted by name.
#[derive(Resource, Default)]
pub struct DebugCommands(Vec<DebugCommandInfo>);

impl DebugCommands {
    /// The registered commands, in name order.
    pub fn iter(&self) -> impl Iterator<Item = &DebugCommandInfo> {
        self.0.iter()
    }

    /// Whether a command of this name is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|c| c.name == name)
    }
}

/// Registers a typed command on an [`App`].
pub trait DebugCommandsAppExt {
    /// Register `handler` as the BRP method `info.name`.
    ///
    /// The handler takes the command's params as `In<P>` and returns `Ok(result)` or `Err(message)`.
    /// Its params are deserialised from the caller's JSON, so a call with the wrong shape is refused
    /// before the handler runs.
    ///
    /// # Panics
    ///
    /// Panics at startup, not at call time, when the registration is wrong: `DebuggerPlugin` was not
    /// added first, the name is empty or reserved (`bevy_debugger/`, `world.`, `registry.`, `rpc.`,
    /// `schedule.`), the name is already registered, or `info.example` does not parse as `P`. These
    /// are programmer errors, the same policy as [`DebuggerPlugin::finish`](crate::DebuggerPlugin).
    fn add_debug_command<P, M>(
        &mut self,
        info: DebugCommandInfo,
        handler: impl IntoSystem<In<P>, Result<Value, String>, M> + 'static,
    ) -> &mut Self
    where
        P: DeserializeOwned + Send + Sync + 'static;
}

impl DebugCommandsAppExt for App {
    fn add_debug_command<P, M>(
        &mut self,
        info: DebugCommandInfo,
        handler: impl IntoSystem<In<P>, Result<Value, String>, M> + 'static,
    ) -> &mut Self
    where
        P: DeserializeOwned + Send + Sync + 'static,
    {
        register(self, info, handler, false);
        self
    }
}

/// Registers a command the crate itself provides (`bevy_debugger/time`). Skips the reserved-name
/// check, which exists to keep game commands out of the crate's own namespace.
pub(crate) fn add_builtin_command<P, M>(
    app: &mut App,
    info: DebugCommandInfo,
    handler: impl IntoSystem<In<P>, Result<Value, String>, M> + 'static,
) where
    P: DeserializeOwned + Send + Sync + 'static,
{
    register(app, info, handler, true);
}

/// Prefixes a game may not use: the crate's own namespace and the methods Bevy registers itself.
const RESERVED_PREFIXES: [&str; 5] = ["bevy_debugger/", "world.", "registry.", "rpc.", "schedule."];

fn is_reserved(name: &str) -> bool {
    name.is_empty() || RESERVED_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

fn register<P, M>(
    app: &mut App,
    info: DebugCommandInfo,
    handler: impl IntoSystem<In<P>, Result<Value, String>, M> + 'static,
    builtin: bool,
) where
    P: DeserializeOwned + Send + Sync + 'static,
{
    let name = info.name.clone();
    if !app.world().contains_resource::<RemoteMethods>() {
        panic!("add DebuggerPlugin before add_debug_command");
    }
    if !builtin && is_reserved(&name) {
        panic!("debug command `{name}` is reserved or empty; use `<game>/<verb>`");
    }
    if app.world().resource::<RemoteMethods>().get(&name).is_some() {
        panic!("debug command `{name}` registered twice");
    }
    if let Err(err) = serde_json::from_value::<P>(info.example.clone()) {
        panic!("debug command `{name}`: its example does not parse as its params: {err}");
    }

    let example = info.example.clone();
    let records = info.records;
    let method = name.clone();
    let world = app.world_mut();
    let handler_id = world.register_system(handler);
    let wrapper = move |In(raw): In<Option<Value>>, world: &mut World| -> BrpResult {
        run_command::<P>(world, &method, handler_id, &example, records, raw)
    };
    let wrapper_id = world.register_system(wrapper);
    world
        .resource_mut::<RemoteMethods>()
        .insert(name, RemoteMethodSystemId::Instant(wrapper_id));

    let mut registry = world.get_resource_or_init::<DebugCommands>();
    let at = registry.0.partition_point(|c| c.name < info.name);
    registry.0.insert(at, info);
}

/// The body every game command runs through: readiness, params, the handler, then the scene log.
fn run_command<P>(
    world: &mut World,
    name: &str,
    handler: SystemId<In<P>, Result<Value, String>>,
    example: &Value,
    records: bool,
    raw: Option<Value>,
) -> BrpResult
where
    P: DeserializeOwned + Send + Sync + 'static,
{
    if !world.get_resource::<DebugReady>().is_some_and(|r| r.0) {
        return Err(crate::brp_error(
            error_codes::INTERNAL_ERROR,
            "game not ready yet (bevy_debugger/status says ready: false); retry shortly",
        ));
    }
    let raw = raw.unwrap_or_else(|| json!({}));
    let params: P = serde_json::from_value(raw.clone()).map_err(|e| {
        crate::brp_error(
            error_codes::INVALID_PARAMS,
            format!("{name}: {e}; example: {example}"),
        )
    })?;
    let outcome = world
        .run_system_with(handler, params)
        .map_err(|e| crate::brp_error(error_codes::INTERNAL_ERROR, format!("{name}: {e:?}")))?;
    let value = outcome.map_err(|message| crate::brp_error(error_codes::INTERNAL_ERROR, message))?;
    if records {
        world.get_resource_or_init::<SceneLog>().0.push(SceneStep {
            method: name.to_string(),
            params: raw,
        });
    }
    Ok(value)
}

/// BRP handler: `bevy_debugger/commands`. Lists every registered game command. Not ready-gated, so an
/// agent can read the vocabulary while the game is still loading.
pub(crate) fn handle_commands(In(_): In<Option<Value>>, registry: Res<DebugCommands>) -> BrpResult {
    let commands: Vec<Value> = registry
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "description": c.description,
                "params": c.params,
                "example": c.example,
            })
        })
        .collect();
    Ok(json!({ "commands": commands }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_names_are_the_crate_and_bevy_namespaces() {
        assert!(is_reserved(""));
        assert!(is_reserved("bevy_debugger/time"));
        assert!(is_reserved("world.query"));
        assert!(is_reserved("registry.schema"));
        assert!(is_reserved("rpc.discover"));
        assert!(is_reserved("schedule.list"));
        assert!(!is_reserved("sanctuary/spawn"));
        assert!(!is_reserved("worldly/spawn"));
    }

    #[test]
    fn info_builders_set_their_fields() {
        let info = DebugCommandInfo::new("demo/x", "does x")
            .params("n: how many")
            .example(json!({"n": 2}));
        assert_eq!(info.params, "n: how many");
        assert_eq!(info.example, json!({"n": 2}));
        assert!(info.records);
        assert!(!info.read_only().records);
    }
}

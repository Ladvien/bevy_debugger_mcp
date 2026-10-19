# bevy_debugger_bevy

The Bevy side of [`bevy_debugger_mcp`](https://github.com/Ladvien/bevy_debugger_mcp). It runs inside your game and exposes BRP methods that let a coding agent register commands, capture the screen, drive input, and record and replay scenes. The MCP server talks to these methods; you can also call them directly with JSON-RPC.

## Quick start

Add the crate, and enable Bevy's `bevy_remote` feature plus the HTTP transport so an agent can reach the game:

```toml
[dependencies]
bevy_debugger_bevy = "0.2"
bevy = "0.20"
```

Then add the plugin and one command to your app:

```rust,ignore
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_debugger_bevy::{DebugCommandInfo, DebugCommandsAppExt, DebuggerPlugin};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnCube {
    at: [f32; 3],
}

fn spawn_cube(
    In(params): In<SpawnCube>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result<Value, String> {
    let [x, y, z] = params.at;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial::from_color(Color::srgb(0.8, 0.3, 0.2)))),
        Transform::from_xyz(x, y, z),
    ));
    Ok(json!({ "spawned_at": [x, y, z] }))
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DebuggerPlugin::default(), RemoteHttpPlugin::default()))
        .add_debug_command(
            DebugCommandInfo::new("demo/spawn_cube", "Spawn a red cube at a world position in metres")
                .params("at: [x, y, z], the cube's centre in metres")
                .example(json!({ "at": [0.0, 0.5, 0.0] })),
            spawn_cube,
        )
        .run();
}
```

A complete, runnable version with an offscreen capture target is in `examples/agent_scene.rs`:

```text
cargo run -p bevy_debugger_bevy --example agent_scene
```

Then, from another shell:

```text
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":1,"method":"bevy_debugger/status"}'
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":2,"method":"bevy_debugger/commands"}'
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":3,"method":"demo/spawn_cube","params":{"at":[0.0,0.5,0.0]}}'
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":4,"method":"bevy_debugger/screenshot"}'
curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":5,"method":"bevy_debugger/capture","params":{"id":1}}'
```

## What the plugin adds

Built-in BRP methods, all registered on `RemotePlugin`:

| Method | Purpose |
|---|---|
| `bevy_debugger/status` | `ready`, frame count, command count, capture kind, replay progress |
| `bevy_debugger/commands` | Every registered command with its description, params doc and example |
| `bevy_debugger/input` | Injected keys, mouse buttons, cursor, and typed text, as Bevy input messages |
| `bevy_debugger/time` | Pause or resume virtual time, and set its relative speed |
| `bevy_debugger/scene` | Return the recorded steps as a scene file, or clear them |
| `bevy_debugger/quit` | Exit the app cleanly |
| `bevy_debugger/screenshot` | Start a capture; returns an id, and writes a file only if `path` is given |
| `bevy_debugger/capture` | Fetch a capture by id as base64 PNG, or its pending or failed state |
| `bevy_debugger/guide`, `guide+watch` | Show a step on the guide overlay (feature `guide`) |

Game commands are named `<game>/<verb>`. Names starting with `bevy_debugger/`, `world.`, `registry.`, `rpc.` or `schedule.` are reserved. Registering a duplicate, a reserved name, or an example that does not parse as the command's params panics at startup, because that is a programmer error.

Commands that change the scene are recorded as steps, and a recorded list can be saved and replayed. Read-only queries are registered with `DebugCommandInfo::read_only()` and never recorded.

## Readiness

`DebuggerPlugin::default()` reports ready from the first frame. A host that loads assets over several frames should call `DebuggerPlugin::default().host_signals_ready()`, then set `DebugReady(true)` once its scene is up. Commands called before that return "game not ready yet", so an agent retries instead of acting on a half-built scene.

## Captures

Captures come back over BRP as base64 PNG. Nothing needs a shared filesystem, so the game can run on another machine.

- `source: "offscreen"` (the default when `DebugCaptureTarget` is present) reads the image the game renders into. It excludes UI, and it does not depend on the window being visible.
- `source: "window"` reads the window's swapchain, UI included. It does not raise or focus the window. A covered or minimised window can return a stale frame.

The last 8 captures are kept. Alpha is dropped before encoding.

## Scenes

A scene is a JSON file of recorded command steps:

```json
{"format": "bevy_debugger_scene", "version": 1, "steps": [{"method": "demo/spawn_cube", "params": {"at": [0.0, 0.5, 0.0]}}]}
```

Set `BEVY_DEBUGGER_SCENE=/path/to/scene.json` before launching the game and the plugin replays the steps, one per frame, once the game is ready. A bad file is logged with `error!` and nothing is replayed. `status.replay` reports progress.

## Cargo features

- `screenshot` (default): capture through `bevy_debugger/screenshot` and `bevy_debugger/capture`. Needs `bevy_render`, PNG encoding and `image`.
- `guide` (default): the guide overlay and guided steps. It spawns a UI node. A host that keeps its UI behind its own surfaces turns it off with `default-features = false`.

Input, commands, time, scenes and quit are always on.

## Why it looks like this

- Gregory, *Game Engine Architecture* (3rd ed.), §10.4–10.6: game-registered console commands, a debug camera and pause are what make in-engine debugging tools useful. This is the basis for `add_debug_command` and `bevy_debugger/time`.
- Ostrowski and Aroudj, "Automated Regression Testing within Video Game Development", GSTF Journal on Computing 3:10 (2013), DOI 10.7603/s40601-013-0010-4: the game under test processes calls over a network connection, possibly on another machine. This is the basis for BRP over HTTP and for captures returned over the wire.
- Yang et al., "SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering" (2024), DOI 10.48550/arXiv.2405.15793: a small set of simple actions, concise feedback and guardrails that catch mistakes early. This is the basis for the small command surface, the `deny_unknown_fields` errors and the unknown-command suggestions.
- Mindermann, Keck and Wagner, "How Usable Are Rust Cryptography APIs?", QRS 2018, DOI 10.1109/QRS.2018.00028: explain every parameter and ship maintained example code. This is the basis for the required `example` on every command, validated at registration.
- Matthews, *Idiomatic Rust* (2024), ch. 6: do one thing well, keep dependencies and feature flags few, and follow semver. This is the basis for the two cargo features and for not forcing Bevy's default features on host games.
- Xu, "AI Agent Systems" (2026), DOI 10.48550/arXiv.2601.01743: stable, well-specified tool schemas with exemplars let agents learn new tools in context. This is the basis for game commands being discovered at run time through `bevy_debugger/commands`.

## License

MIT OR Apache-2.0.

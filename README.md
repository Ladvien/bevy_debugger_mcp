# bevy-fairy

> ⚠️ **Vibe Coded** — written by an AI agent working from a human's direction. It is used against a shipping game and covered by tests, but it has had no line-by-line human audit. Read it before you trust it.

Lets an agent inspect and drive a **running** Bevy game: query entities, components and resources live, capture frames from an offscreen render target, inject keyboard and mouse input — including **cursor position**, so a click-drag is expressible, and **typed text**, so a text field can be filled and committed — and **post a walkthrough the app renders one step at a time**, watching a named condition and recording what actually happened. None of it touches your desktop, your window manager, or the OS input stack.

> **This repo is the source of truth** for the crate, on Bevy 0.20. It began as a subtree of [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop) and was mirrored out of it until 2026-10-19; that monorepo's copy is now a frozen Bevy 0.19 snapshot, and nothing is pulled back from it. Issues and PRs belong here.

> **The one part that is not an ordinary change is the OS boundary below.** Neither half may reach the machine the game runs on: no OS screen capture of other windows, no OS input synthesis.

[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.95+-orange.svg)](https://www.rust-lang.org)
[![Bevy](https://img.shields.io/badge/bevy-0.20-purple.svg)](https://bevyengine.org)

## Two halves, one crate

| | What it is | Where it runs |
|---|---|---|
| The **server** (`server` feature, the default) | The MCP server binary, `bevy-fairy`. Speaks MCP to an agent and BRP to the game. No login; it starts without a game and finds one when it appears. | Its own process |
| The **plugin** (`plugin` feature) | The Bevy plugin. Registers `bevy_fairy/status`, `commands`, `screenshot`, `capture`, `scene`, `time`, `quit`, `input`, `guide` and `guide+watch`, and lets a game register its own typed commands (`add_debug_command`). | Inside the game |

They are one published crate with two features, and they are independent of each other. Cargo
resolves dependencies per feature, so a game that takes the plugin compiles the plugin and none of
the server's:

```toml
[dependencies]
bevy_fairy = { version = "0.2", default-features = false, features = ["plugin"] }
```

An agent installs the server:

```sh
cargo install bevy_fairy --no-default-features --features server
```

The plugin is what a game links; the server is what an agent talks to. The server talks to any
game that exposes BRP, including one that never heard of this crate.

## Stage a scene and look at it

The workflow the server's tools are built for: `status` (is the game ready?) → `commands` (what can this game do, with an example of each) → `run <command> <params>` … → `screenshot` → `scene save <path>` → `quit`. The game decides what its commands are, so the same seven tools drive any game that registers some; a mistyped command name is answered with the nearest real one. Every command that runs is recorded, and `scene save` writes the run to a file the game replays at startup (`BEVY_FAIRY_SCENE=<file>`), one step a frame, once it says it is ready. A capture comes back over BRP as a PNG, from the offscreen image the game renders to or from its window (`source: "window"`), so the game may run on another machine. `brp` is a raw passthrough for any BRP method.

Wire a game in with the example in `examples/plugin_agent_scene.rs`, point the server at it with `BEVY_BRP_HOST` and `BEVY_BRP_PORT` (default `127.0.0.1:15702`), and set `BEVY_FAIRY_CAPTURE_DIR` for where captures are saved.

### Why it looks like this

- Gregory, *Game Engine Architecture* (3rd ed.), §10.4–10.6: game-registered console commands, a debug camera and pause are what make in-engine debugging tools useful (`add_debug_command`, `bevy_fairy/time`).
- Ostrowski and Aroudj, "Automated Regression Testing within Video Game Development", GSTF Journal on Computing 3:10 (2013), DOI 10.7603/s40601-013-0010-4: the game under test processes calls over a network connection, possibly on another machine (BRP over HTTP, captures returned over the wire).
- Yang et al., "SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering" (2024), DOI 10.48550/arXiv.2405.15793: a small set of simple actions, concise feedback and guardrails that catch mistakes early (the seven tools, `deny_unknown_fields` errors, unknown-command suggestions).
- Mindermann, Keck and Wagner, "How Usable Are Rust Cryptography APIs?", QRS 2018, DOI 10.1109/QRS.2018.00028: explain every parameter and ship maintained example code (the required `example` on every command, validated at registration).
- Matthews, *Idiomatic Rust* (2024), ch. 6: do one thing well, keep dependencies and feature flags few, follow semver (two cargo features, no forced Bevy default features).
- Xu, "AI Agent Systems" (2026), DOI 10.48550/arXiv.2601.01743: stable, well-specified tool schemas with exemplars let agents learn new tools in context (commands discovered at run time through `bevy_fairy/commands`).

## Three directions

| | Method | |
|---|---|---|
| app → agent | `bevy_fairy/screenshot`, `bevy_fairy/capture` | a frame from the offscreen target or the window, with a region and a zoom, returned over BRP |
| agent → app | `bevy_fairy/input`, the game's own commands | keys, text, mouse, cursor position; typed game verbs |
| **agent → person** | `bevy_fairy/guide`, `…/guide+watch` | a walkthrough the app renders one step at a time |

The third is the newest and the least obvious. An agent could already drive an app and look at it; what it could not do was **say a sentence to the person at the keyboard**, so every instruction went to a terminal they had to look away from their work to read, and every answer came back as prose the agent then had to guess its way from.

## Examples

The three that demonstrate what this crate actually does — no window, no GPU, no mouse; all print to the terminal:

```sh
cargo run --features plugin --example plugin_injected_input_lands
cargo run --features plugin --example plugin_cursor_drag_lands
cargo run --features plugin-guide --example plugin_guided_steps_land
```

The first shows the property the plugin exists to provide: an injected key is visible to `just_pressed` for exactly one frame and `just_released` on the next, the same shape a physical key produces.

The second shows a **click-drag** — `move, press, move, move, release` queued in one batch — and the property that makes it mean anything: **the press is read where it was aimed**. Apply the queued move and the queued press in the same frame and the game reads the press at the destination, so the drag starts wherever it ended and selects nothing. It earned its keep immediately: it caught a real ordering bug where a release overtook two still-pending moves and committed the box at the wrong corner.

The third posts a three-step script and drives it to completion against a stand-in author, printing the transcript an agent would receive.

## Guiding a person through an exercise

Post a script; the app shows **one step**, never the list. Each step is a guided-exploration card — hints, a **checkpoint**, and **recovery**:

```jsonc
{"steps": [{
  "label": "drop a floor and a wall",
  "goal": "a tile needs two pieces before the solver has anything to match on",
  "do": ["walk the library with up and down", "press Enter to bring the selected row in"],
  "checkpoint": "tile has two members",
  "recovery": "if nothing lands, the library filter is hiding every row: press Backspace"
}]}
```

Then `curl -N` on `bevy_fairy/guide+watch` and get a frame each time something happens — nothing at all while the condition is unmet.

**Checkpoints are the host's words.** This plugin cannot know what a tile is and must not learn, so a host registers one-shot systems answering `bool`:

```rust
let has_two = app.register_system(|tile: Res<Tile>| tile.members >= 2);
app.world_mut().resource_mut::<Checkpoints>().register("tile has two members", has_two);
```

A step may have **no** checkpoint — *"does this look right?"* is not a machine question — and that is a supported state, not a gap: the stream says `waiting_on_a_person` and holds until an explicit `{"skip": true}`.

What comes back is **`k/n` per step**, never a boolean, plus how long each took. A step nobody can complete shows up as a stall rather than a silence.

The design is grounded rather than guessed, and two of the findings are negative: an on-demand help button *costs* completion, and restricting input to force the step buys nothing at all. See the [design notes](src/plugin/guide.rs) for the citations.

## Injecting a cursor

`bevy_fairy/input` takes `kind: "Cursor"` with `x`/`y` in **logical window pixels**, or `clear: true` to hand the pointer back to the real mouse. The position is state, not an edge — it stays where it was put until moved again, so there is no per-frame stream to keep up with.

```jsonc
{"kind": "Cursor", "x": 640, "y": 360}   // aim
{"kind": "Mouse", "button": "Left", "action": "Press"}
{"kind": "Cursor", "x": 720, "y": 400}   // drag
{"kind": "Mouse", "button": "Left", "action": "Release"}
{"kind": "Cursor", "clear": true}        // give it back
```

**A host has to read the pointer through `bevy_fairy::cursor_position(&window, &debug_cursor)`.** A host that calls `Window::cursor_position` directly cannot be driven, and that is not a bug in the plugin — the window's own cursor is deliberately never written, because Bevy's windowing backend turns a change to it into a request to move the *physical* pointer. That would drag the mouse out from under whoever is at the machine, which is the one thing this crate exists to prevent.

Two older examples ship with the server, and it is worth being precise about them, because neither
demonstrates `DebuggerPlugin`:

| Example | What it actually does |
|---|---|
| `basic_setup` | A plain Bevy app with `RemotePlugin` + `RemoteHttpPlugin` — a *target* to debug, not a use of this crate. It does **not** add `DebuggerPlugin`. |
| `screenshot_setup` | Uses Bevy's built-in `Screenshot::primary_window()`. That is **window-surface capture — the opposite of this crate's path**, which reads an offscreen `Image` precisely so it never needs the window raised. Read it as background on Bevy's own screenshot API, not as guidance for this crate. |

> [!WARNING]
> ## ⚠️ 100% VIBE CODED ⚠️
> 
> **This entire project was developed through "vibe coding" with AI assistance.**
> 
> This means:
> - The code was written iteratively through natural language descriptions
> - No traditional software design process was followed
> - Testing may be incomplete or absent in some areas
> - There might be unconventional patterns or unexpected behaviors
> - **USE AT YOUR OWN RISK**
> 
> While functional, this codebase should be treated as experimental. We recommend:
> - Thoroughly testing in a development environment first
> - Not using in production without extensive review
> - Being prepared for potential edge cases or unusual behavior
> - Contributing fixes if you find issues!
> 
> *That said, it's pretty cool and mostly works!* 🚀

An MCP server that lets a coding agent debug a running Bevy game: read its entities, components and resources, run the commands it registers, capture what it shows, and test claims about its state. It cannot see frame time, CPU or memory — BRP does not report them.

## Table of Contents

- [Installing the server](#installing-the-server)
- [Adding the plugin to a game](#adding-the-plugin-to-a-game)
- [Registering the server with an MCP client](#registering-the-server-with-an-mcp-client)
- [The tools](#the-tools)
- [Configuration](#configuration)
- [Troubleshooting](#troubleshooting)
- [Contributing](#contributing)
- [Changelog](#changelog)
- [License](#license)

## Installing the server

```sh
cargo install bevy_fairy --no-default-features --features server
bevy-fairy --version
```

Or from a checkout:

```sh
git clone https://github.com/Ladvien/bevy_fairy.git
cd bevy_fairy
cargo install --path . --no-default-features --features server
```

Needs Rust 1.95 or newer. The server runs over stdio (`bevy-fairy`, or `bevy-fairy --stdio`); an MCP client starts it, you do not run it by hand. It does not need a game to be running when it starts: it finds one when it appears.

> `bevy-fairy --tcp` exists but currently drops every connection, so use stdio.

## Adding the plugin to a game

```toml
# Cargo.toml
[dependencies]
bevy = "0.20"
bevy_fairy = { version = "0.2", default-features = false, features = ["plugin"] }
```

`plugin` is the leaf. Add `plugin-screenshot` for `bevy_fairy/screenshot` and `bevy_fairy/capture` (it turns on Bevy's renderer), and `plugin-guide` for the guide channel (it spawns a UI overlay). Leave `default-features` off, or the server's whole dependency tree is compiled into your game.

`DebuggerPlugin` owns the BRP `RemotePlugin`, so **never add a second `RemotePlugin` next to it** — Bevy rejects a duplicate plugin by name, and the game panics on start. Add a transport instead, such as `RemoteHttpPlugin`.

```rust
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_fairy::DebuggerPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Owns the BRP RemotePlugin. Never add a second RemotePlugin next to it.
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .run();
}
```

`DebuggerPlugin` needs `bevy::input::InputPlugin` in the app; it asserts so when the app is built. `DefaultPlugins` includes it; `MinimalPlugins` does not.

The plugin registers its `bevy_fairy/*` BRP methods itself (the list is in [Two halves, one crate](#two-halves-one-crate)). Do not register them by hand.

To let an agent drive the game, register your own commands with `add_debug_command` — the game tells the agent what they are, with their parameters and an example. Add `DebuggerPlugin` before the first `add_debug_command`.

```rust
use bevy_fairy::{DebugCommandInfo, DebugCommandsAppExt, DebuggerPlugin};

#[derive(serde::Deserialize)]
struct SpawnCube { at: [f32; 3] }

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .add_debug_command(
            DebugCommandInfo::new("demo/spawn_cube", "Spawn a unit cube at a point.")
                .params("at: [x, y, z] in metres")
                .example(serde_json::json!({"at": [0.0, 0.0, 0.0]})),
            |In(p): In<SpawnCube>, mut commands: Commands| -> Result<serde_json::Value, String> {
                commands.spawn(Transform::from_translation(Vec3::from(p.at)));
                Ok(serde_json::json!({"spawned": true}))
            },
        )
        .run();
}
```

Runnable examples are in [`examples/`](https://github.com/Ladvien/bevy_fairy/tree/main/examples); `fixture_game.rs` is a headless game whose state is known from its own constants, which the tests and the agent evals check the tools against.

## Registering the server with an MCP client

Claude Code, from your project directory:

```sh
claude mcp add bevy-fairy -- bevy-fairy --stdio
```

or, in a project's `.mcp.json`, or the `mcpServers` object of Claude Desktop's `claude_desktop_config.json` (macOS: `~/Library/Application Support/Claude/`, Windows: `%APPDATA%\Claude\`):

```json
{
  "mcpServers": {
    "bevy-fairy": {
      "command": "bevy-fairy",
      "args": ["--stdio"],
      "env": {
        "BEVY_BRP_HOST": "localhost",
        "BEVY_BRP_PORT": "15702"
      }
    }
  }
}
```

If the client cannot find `bevy-fairy` on its `PATH`, give `command` as the full path (`~/.cargo/bin/bevy-fairy`, written out). Restart the client after changing its configuration, start your game, then ask the agent to call `status`.

## The tools

All 13, as `tools/list` returns them (sorted by name):

| Tool | What it does |
|------|--------------|
| `brp` | Calls any Bevy Remote Protocol method the game serves (`world.query`, `registry.schema`, ...) |
| `commands` | Lists the commands the game registered, each with its parameters and a working example |
| `detect_anomaly` | Looks for anomalies in the game's state |
| `experiment` | Spawns, modifies or deletes entities and reads back what changed |
| `guide` | Shows the person at the keyboard one instruction at a time and watches for the state it asked for |
| `hypothesis` | Tests a claim (`description` plus a `success_condition`) against the running game |
| `observe` | Reads entities, components and resources |
| `quit` | Asks the game to quit cleanly |
| `replay` | Records and replays the game's entity state |
| `run` | Runs one command the game registered, by name |
| `scene` | Saves, shows or clears the commands recorded this session |
| `screenshot` | Captures the game to a PNG on this machine (needs the `plugin-screenshot` feature) |
| `status` | Reports whether the game is answering and ready, and its frame |

The server cannot see frame time, CPU or memory: BRP does not report them, and a tool that would need them says so instead of answering with a number.

## Configuration

The server reads environment variables; there is no config file.

| Variable | Default | Meaning |
|---|---|---|
| `BEVY_BRP_HOST` | `localhost` | Host the game's BRP endpoint is on |
| `BEVY_BRP_PORT` | `15702` | Port the game's BRP endpoint is on |
| `BEVY_FAIRY_CAPTURE_DIR` | `./debug_captures` | Where `screenshot` saves PNGs on the server's machine |
| `RUST_LOG` | `info` | Log level. In stdio mode stdout is reserved for MCP, so logs go to stderr |

In the game process, `BEVY_FAIRY_SCENE=<file>` makes the plugin replay a saved scene at startup, one step a frame.

## Troubleshooting

- **The server cannot connect to the game.** The game must be running with `DebuggerPlugin` and a transport such as `RemoteHttpPlugin`, and `BEVY_BRP_HOST` / `BEVY_BRP_PORT` must point at it. Ask the agent to call `status`.
- **The game panics on start.** Either a second `RemotePlugin` was added next to `DebuggerPlugin`, or `InputPlugin` is missing (`MinimalPlugins` does not include it).
- **`screenshot` fails.** The game must be built with the `plugin-screenshot` feature.
- **Injected input does nothing in a host's own pointer code.** The host has to read the pointer through `bevy_fairy::cursor_position`, not `Window::cursor_position`; see [Injecting a cursor](#injecting-a-cursor).
- **The client reports no tools.** Run `bevy-fairy --version` in a terminal to check the binary is installed and on `PATH`, then restart the client.

## Contributing

This repository is the source of truth; issues and PRs belong here.

```sh
git clone https://github.com/Ladvien/bevy_fairy.git
cd bevy_fairy
cargo test --workspace                    # the server, and the plugin's tests
cargo test --no-default-features --features plugin --test plugin_leaf
                                          # the OS-boundary ratchet, on its own
cargo fmt --all
```

`tests/plugin_leaf.rs` is the enforcement of the OS boundary above: it checks the plugin's resolved dependency closure and fails if anything in it can synthesise an operating-system event. Widening it is a design decision, not a side effect.

To drive the fixture game by hand:

```sh
cargo run --example fixture_game --features plugin    # BRP on port 15702, 60 fps
```

### Agent evals

`evals/` asks a fresh headless `omp` session, with only this server's tools, to answer
debugging questions about `examples/fixture_game.rs`, and scores the answers against the
fixture's constants: 6 dev tasks and 3 held-out ones. See [`evals/README.md`](https://github.com/Ladvien/bevy_fairy/blob/main/evals/README.md).

```bash
python3 evals/run.py --server-bin target/debug/bevy-fairy
```

## Changelog

The full changelog is [`CHANGELOG.md`](https://github.com/Ladvien/bevy_fairy/blob/main/CHANGELOG.md), which covers every release. The short
version:

### v0.2.0 - the plugin, and the tools that stopped lying

- The crate is renamed `bevy_fairy` and its two halves, the server and the Bevy plugin, are merged
  into one crate with two independent features. The plugin owns the BRP `RemotePlugin`, so a game
  adds `DebuggerPlugin` and never a second `RemotePlugin`.
- Game-registered commands, capture from an offscreen target or the window, injectable input
  that really arrives, a guide channel, and replayable scenes.
- The `experiment`, `hypothesis` and `detect_anomaly` tools take the arguments their handlers
  read. They used to advertise one set of names and dispatch on another, so every request was
  dropped and a default ran instead.
- `hypothesis` measures the entity and component counts from the game instead of reporting
  constants, and `replay` records the game's entities instead of returning an empty map.
- `stress_test` is removed: every operation it sent failed against a real game while it
  reported success, and its frame-time percentiles were random numbers.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

It was GPL-3.0 until it was adopted into [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop), and relicensed on the way in to match the other `bevy_*` crates there: a GPL crate in the Bevy ecosystem cannot be adopted, and being adoptable is the entire reason these are published.

---

**Questions?** Open an [issue](https://github.com/Ladvien/bevy_fairy/issues).

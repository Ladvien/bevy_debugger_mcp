# Bevy Debugger MCP Server

> ⚠️ **Vibe Coded** — written by an AI agent working from a human's direction. It is used against a shipping game and covered by tests, but it has had no line-by-line human audit. Read it before you trust it.

Lets an agent inspect and drive a **running** Bevy game: query entities, components and resources live, capture frames from an offscreen render target, inject keyboard and mouse input — including **cursor position**, so a click-drag is expressible, and **typed text**, so a text field can be filled and committed — and **post a walkthrough the app renders one step at a time**, watching a named condition and recording what actually happened. None of it touches your desktop, your window manager, or the OS input stack.

> **This repo is the source of truth** for both crates, on Bevy 0.20. It began as a subtree of [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop) and was mirrored out of it until 2026-10-19; that monorepo's copy is now a frozen Bevy 0.19 snapshot, and nothing is pulled back from it. Issues and PRs belong here.

> **The one part that is not an ordinary change is the OS boundary below.** Neither crate may reach the machine the game runs on: no OS screen capture of other windows, no OS input synthesis.

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/ladvien/bevy_fairy)
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
cargo install bevy_fairy
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

A powerful Model Context Protocol (MCP) server that enables AI-assisted debugging of Bevy games through Claude Code. Debug your game state, analyze performance, and test hypotheses with natural language commands.

## 📚 Table of Contents

- [Quick Start](#-quick-start)
- [Features](#-features)
- [Installation](#installation)
  - [Method 1: Homebrew](#method-1-homebrew-macos---recommended)
  - [Method 2: Cargo](#method-2-cargo-install)
  - [Method 3: From Source](#method-3-from-source)
  - [Post-Installation Setup](#post-installation-setup)
- [Configuration](#configuration)
- [Usage Examples](#-usage-examples)
- [Architecture](#-architecture)
- [Troubleshooting](#-troubleshooting)
- [Contributing](#-contributing)

## 🎯 Quick Start

```bash
# 1. Install the debugger (pick your preferred method)
cargo install bevy_fairy

# 2. Run the setup script (IMPORTANT!)
curl -sSL https://raw.githubusercontent.com/ladvien/bevy_fairy/main/setup-claude.sh | bash

# 3. Add the debugger plugin to your Bevy game
# In your game's Cargo.toml:
#   bevy_fairy = "0.2"
# In your game's main.rs:
#   .add_plugins(DebuggerPlugin::default())
#   .add_plugins(RemoteHttpPlugin::default())
# The plugin owns the BRP RemotePlugin. Never add a second RemotePlugin next
# to it: Bevy rejects a duplicate plugin by name, so the game panics on start.

# 4. Restart Claude Code and start debugging!
cargo run  # In your game directory
# Then open Claude Code and say: "Help me debug my Bevy game"
```

## ✨ Features

- **🔍 Real-time Observation**: Monitor entities, components, and resources as your game runs
- **🧪 Smart Experimentation**: Test game behavior changes with automatic rollback
- **📊 Performance Analysis**: Identify bottlenecks and optimize game performance  
- **🚨 Anomaly Detection**: Automatically spot unusual patterns in game behavior
- **📹 Session Recording**: Record and replay debugging sessions for analysis
- **📸 Screenshot Capture**: Take window-specific screenshots of your game for visual debugging
- **🛡️ Error Recovery**: Robust error handling with automatic diagnostics
- **🤖 ML-Powered Suggestions**: Learn from debugging patterns to provide better recommendations
- **📈 Performance Budgets**: Set and monitor performance targets with automatic alerts
- **🔄 Workflow Automation**: Automate common debugging tasks with safety checkpoints

## 🏗️ How It Works

The Bevy Debugger MCP creates a bridge between Claude Code and your Bevy game:

```
┌─────────────┐     MCP Protocol      ┌──────────────────┐     BRP Protocol    ┌─────────────┐
│ Claude Code │ ◄──────────────────► │ bevy-fairy │ ◄─────────────────► │ Your Bevy   │
│   (AI)      │    stdio/TCP          │    (Server)       │    WebSocket       │    Game     │
└─────────────┘                       └──────────────────┘                      └─────────────┘
     │                                         │                                       │
     │ "Find memory leaks"                    │                                       │
     └────────────────────►                   │                                       │
                                              │ Query entities & components           │
                                              └──────────────────────────────────────►│
                                              │                                       │
                                              │◄──────────────────────────────────────┤
                                              │     Entity data & metrics             │
     │◄────────────────────                   │                                       │
     │ "Found 500 orphaned                    │                                       │
     │  bullet entities"                      │                                       │
```

### Architecture Components

1. **Claude Code (AI Agent)**: Natural language interface for debugging commands
2. **MCP Server**: Translates AI requests into game debugging operations
3. **BRP Client**: Communicates with your Bevy game via WebSocket
4. **DebuggerPlugin**: Bevy plugin that exposes game internals for debugging
5. **Debug Tools**: 11 specialized tools for different debugging tasks

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ and Cargo
- Claude Code CLI
- A Bevy game with the `DebuggerPlugin` enabled

### Installation

#### Method 1: Homebrew (macOS) - Recommended
```bash
brew tap ladvien/bevy-fairy
brew install bevy-fairy

# Run setup for Claude integration
bevy-fairy-setup
```

#### Method 2: Cargo Install
```bash
cargo install bevy_fairy

# Run setup script for Claude integration
curl -sSL https://raw.githubusercontent.com/ladvien/bevy_fairy/main/setup-claude.sh | bash
```

#### Method 3: From Source
```bash
git clone https://github.com/ladvien/bevy_fairy.git
cd bevy_fairy
./install.sh  # Handles everything automatically
```

#### Method 4: Claude Desktop Extension
Coming soon - Will be available in the Claude Desktop extension marketplace.

### Post-Installation Setup

**Important:** After installation via ANY method, run the setup script:
```bash
# If installed via Homebrew
bevy-fairy-setup

# If installed via other methods
~/.cargo/bin/bevy-fairy-setup || ./setup-claude.sh
```

This setup script will:
1. Find your bevy-fairy installation
2. Create necessary symlinks for Claude Code compatibility
3. Generate the configuration you need to add to Claude

### Why the Setup Script?

Claude Code may look for binaries in different locations depending on your system:
- `~/.local/bin/` (Claude Code default)
- `~/.cargo/bin/` (Cargo installation)
- `/usr/local/bin/` (System-wide)
- `/opt/homebrew/bin/` (Homebrew on Apple Silicon)

The setup script ensures Claude can find the binary regardless of installation method.

### Server Management with `bevy-fairy-control`

The `bevy-fairy-control` script is automatically installed with the package and provides complete lifecycle management for the MCP server. This solves the common issue of the server hanging when run directly.

#### Basic Commands

```bash
# Start the server in the background
bevy-fairy-control start

# Stop the server gracefully
bevy-fairy-control stop

# Restart the server (useful after configuration changes)
bevy-fairy-control restart

# Check if the server is running and view details
bevy-fairy-control status

# View server logs
bevy-fairy-control logs

# Follow logs in real-time (like tail -f)
bevy-fairy-control logs -f

# Clean up old log files
bevy-fairy-control clean

# Show help and all available commands
bevy-fairy-control help
```

#### Advanced Usage

```bash
# Start server on a different port
BEVY_FAIRY_PORT=3002 bevy-fairy-control start

# Start with custom Bevy host/port
BEVY_BRP_HOST=192.168.1.100 BEVY_BRP_PORT=15703 bevy-fairy-control start

# Clean all logs including current
bevy-fairy-control clean --all

# Check server status with process details
bevy-fairy-control status
# Output shows:
# - PID of running process
# - CPU and memory usage
# - Port binding status
# - Recent log entries
```

#### File Locations

The control script manages the following files:
- **Logs**: `~/.bevy-fairy/bevy-fairy.log`
- **PID file**: `~/.bevy-fairy/bevy-fairy.pid`
- **Rotated logs**: `~/.bevy-fairy/bevy-fairy.log.*`

#### Troubleshooting

If the server fails to start:
```bash
# Check the logs for errors
bevy-fairy-control logs

# Ensure no other instance is running
bevy-fairy-control stop
bevy-fairy-control start

# Verify the binary is installed
which bevy-fairy

# Check if port is already in use
lsof -i :3001  # or your configured port
```

### Setup Your Bevy Game

Add the `DebuggerPlugin` to your Bevy app. It owns the BRP `RemotePlugin`, so **never add a
second `RemotePlugin` next to it** — Bevy rejects a duplicate plugin by name, and the game panics
on start. Add a transport instead, such as `RemoteHttpPlugin`.

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

`DebuggerPlugin` needs `bevy::input::InputPlugin` in the app. `DefaultPlugins` includes it;
`MinimalPlugins` does not.

```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.20", features = ["default"] }
bevy_fairy = "0.2"
```

The plugin registers the BRP methods itself: `bevy_fairy/status`, `bevy_fairy/commands`,
`bevy_fairy/screenshot`, `bevy_fairy/capture`, `bevy_fairy/input`, `bevy_fairy/scene`,
`bevy_fairy/time` and `bevy_fairy/quit`. Do not register them by hand.

To let an agent drive the game, register your own commands with `add_debug_command` — the game
tells the agent what they are, with their parameters and an example:

```rust
use bevy_fairy::{DebugCommandInfo, DebugCommandsAppExt, DebuggerPlugin};

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

### Start Debugging

1. **Run your Bevy game**: `cargo run`
2. **Open Claude Code** in your project directory
3. **Start debugging**: Try commands like:
   - "Show me all entities in the game"
   - "Monitor the player's health component"
   - "Test what happens when I spawn 100 enemies"
   - "Take a screenshot of the current game state"
   - "Record this gameplay session for analysis"

## 🤖 Claude Code Integration Guide

### Setting Up Claude Code

1. **Install the MCP Server** (v0.2.0 or later):
```bash
cargo install bevy_fairy
```

2. **Configure Claude Code** - Add to your Claude Code settings:

**macOS/Linux**: `~/.config/claude/claude_code_config.json`
**Windows**: `%APPDATA%\claude\claude_code_config.json`

```json
{
  "mcpServers": {
    "bevy-fairy": {
      "command": "bevy-fairy",
      "args": ["--stdio"],
      "type": "stdio",
      "env": {
        "BEVY_BRP_HOST": "localhost",
        "BEVY_BRP_PORT": "15702",
        "RUST_LOG": "info"
      }
    }
  }
}
```

3. **Verify Installation**:
```bash
# Check version
bevy-fairy --help

# Test the MCP server responds correctly
echo '{"jsonrpc": "2.0", "method": "initialize", "params": {"capabilities": {}}, "id": 1}' | bevy-fairy --stdio
# Should return: {"id":1,"jsonrpc":"2.0","result":{"capabilities":...}}

# Verify tools are available
echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 2}' | bevy-fairy --stdio
# Should list: observe, experiment, stress, anomaly, replay, hypothesis, screenshot
```

### How Claude Uses the MCP Tools

When you ask Claude to debug your Bevy game, it has access to powerful MCP tools that communicate with your running game:

#### 🔍 **Observation Tools**
Claude can monitor your game state in real-time:
```markdown
You: "Show me all enemies in the game"
Claude: [Uses observe tool to query entities with Enemy component]
        "I found 5 enemies. Here are their positions and health values..."

You: "Track the player's velocity over time"
Claude: [Uses observe tool with continuous monitoring]
        "The player's velocity spikes to 500 units when jumping, which seems abnormal..."
```

#### 🧪 **Experimentation Tools**
Claude can test hypotheses by modifying game state:
```markdown
You: "Test what happens if we spawn 100 enemies at once"
Claude: [Uses experiment tool to spawn entities and measure performance]
        "Spawning 100 enemies causes frame rate to drop to 15 FPS. The bottleneck is..."

You: "Try doubling the player's speed and see if collision still works"
Claude: [Uses experiment tool to modify components with automatic rollback]
        "At 2x speed, the player clips through thin walls. The physics system needs..."
```

#### 🚨 **Anomaly Detection**
Claude can identify unusual patterns:
```markdown
You: "Something is causing random lag spikes"
Claude: [Uses anomaly tool to monitor performance metrics]
        "I detected memory allocations spike every 5 seconds, coinciding with..."

You: "Find any entities behaving strangely"
Claude: [Uses anomaly tool with pattern matching]
        "Entity #42 is teleporting between positions, likely due to..."
```

#### 📊 **Performance Analysis**
Claude can profile and optimize:
```markdown
You: "Why is my game running slowly?"
Claude: [Uses stress tool to identify bottlenecks]
        "The render system is taking 45ms per frame. The main issue is..."

You: "Optimize the enemy AI system"
Claude: [Uses performance profiling tools]
        "The pathfinding is recalculating every frame. We can cache results..."
```

### Real-World Debugging Sessions

#### Example 1: Physics Bug
```markdown
Human: My character falls through the floor randomly

Claude: I'll investigate this physics issue. Let me start by observing the player's Transform and Collider components.

[Uses observe tool to monitor player entity]
I'm tracking the player's Y position... I notice it suddenly changes from 0.0 to -100.0 without intermediate values.

[Uses experiment tool to test collision]
Let me test the collision system by spawning a test entity at the player's position...
The collision detection is failing when the player's velocity exceeds 1000 units/frame.

Solution: Add a velocity cap or use continuous collision detection for high-speed entities.
```

#### Example 2: Performance Investigation
```markdown
Human: The game freezes every few seconds

Claude: I'll analyze the performance patterns to identify the cause of these freezes.

[Uses anomaly detection on frame times]
I've detected periodic spikes in frame time every 3.2 seconds, lasting about 200ms.

[Uses observe tool on system metrics]
Monitoring system activity... The "EnemySpawnSystem" is creating 50 entities at once every 3 seconds.

[Uses experiment tool to test optimization]
Let me test spreading the spawning over multiple frames...
Spawning 5 entities per frame over 10 frames eliminates the freezes.

Solution: Implement batched spawning to distribute the load across frames.
```

#### Example 3: Visual Debugging
```markdown
Human: Can you show me what the AI sees?

Claude: I'll visualize the AI's perception system for you.

[Uses visual overlay tools]
I'm enabling debug overlays for:
- AI vision cones (showing field of view)
- Pathfinding waypoints (showing planned routes)  
- Detection ranges (showing awareness zones)

[Takes screenshot]
Here's a screenshot showing the AI's perception. The red cones are vision, blue lines are paths.

The AI at position (100, 50) can't see the player because there's an obstacle blocking line of sight.
```

### Advanced Usage Patterns

#### Continuous Monitoring
```markdown
Human: Monitor the game for any issues while I playtest

Claude: I'll set up continuous monitoring for common issues.

[Sets up multiple observe and anomaly detection tasks]
Monitoring:
- Memory usage (alerting if >2GB)
- Frame time (alerting if >16ms)
- Entity count (alerting if >10000)
- Error patterns in components

[After 5 minutes]
Alert: Memory usage increased by 500MB in the last minute.
Investigating... Found 5000 orphaned bullet entities not being destroyed.
```

#### Automated Testing
```markdown
Human: Test if my save system works correctly

Claude: I'll test the save/load system comprehensively.

[Uses orchestration tool to chain operations]
1. Capturing current game state...
2. Modifying player position, inventory, and score...
3. Triggering save...
4. Resetting game...
5. Triggering load...
6. Comparing states...

Results: Save system works correctly for position and score, but inventory items are duplicated on load.
```

### Troubleshooting Claude Code Connection

If Claude can't connect to your game:

```bash
# 1. Ensure your Bevy game is running with the debugger plugin
# Check if Bevy is listening on the correct port:
lsof -i :15702  # Should show your Bevy game process

# 2. Test the MCP server standalone:
bevy-fairy --help  # Should show version 0.2.0
bevy-fairy --stdio  # Should wait for input (Ctrl+C to exit)

# 3. Verify Claude Code configuration:
cat ~/.config/claude/claude_code_config.json
# Should contain the bevy-fairy configuration

# 4. Test manual MCP connection:
echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 2}' | bevy-fairy --stdio
# Should return list of available tools

# 5. Check direct Bevy connection:
curl -X POST http://localhost:15702/query \
  -H "Content-Type: application/json" \
  -d '{"method": "bevy/list", "params": {}}'
# Should return Bevy data

# 6. Enable debug logging:
RUST_LOG=debug bevy-fairy --stdio

# 7. Common issues:
# - Port 15702 blocked by firewall
# - Bevy game not compiled with bevy_remote feature
# - Multiple MCP servers running on same port
# - Claude Code needs restart after config changes
```

### Performance Characteristics

The debugger is designed for minimal impact on your game:

| Metric | Target | Actual | Notes |
|--------|--------|--------|-------|
| **Idle Overhead** | <5% | <3% | When connected but not actively debugging |
| **Active Overhead** | <10% | <7% | During active debugging operations |
| **Memory Usage** | <50MB | ~30MB | Includes caching and session data |
| **Startup Time** | <1s | ~500ms | With lazy initialization |
| **Command Latency** | <200ms | <50ms | For simple queries |
| **Complex Query** | <1s | ~200ms | For queries returning 1000+ entities |

### Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| **"Failed to connect to BRP"** | Ensure your Bevy game is running with `DebuggerPlugin` enabled |
| **"No tools available"** | Update to v0.2.0: `cargo install bevy_fairy --force` |
| **High CPU usage** | Reduce monitoring frequency or use `--tcp` mode instead of stdio |
| **Screenshot not working** | Add screenshot handler to your Bevy game (see setup example) |
| **Memory leak detection false positives** | Adjust detection thresholds in anomaly tool |
| **Claude not responding** | Restart Claude Code after configuration changes |

## 🛠️ Configuration

The server uses environment variables for configuration:

```bash
export BEVY_BRP_HOST=localhost    # Bevy Remote Protocol host
export BEVY_BRP_PORT=15702        # Bevy Remote Protocol port  
export MCP_PORT=3000              # MCP server port (not used in stdio mode)
export RUST_LOG=info              # Logging level
```

## 📁 Project Structure

```
bevy_fairy/
├── src/
│   ├── main.rs              # Entry point with stdio/TCP transport
│   ├── mcp_server.rs        # MCP protocol implementation
│   ├── brp_client.rs        # Bevy Remote Protocol client
│   ├── tools/               # Debugging tool implementations
│   │   ├── observe.rs       # Entity/component observation
│   │   ├── experiment.rs    # Game state experimentation
│   │   ├── stress.rs        # Performance stress testing
│   │   └── ...
│   └── ...
├── scripts/                 # Installation and management scripts
├── docs/                    # Documentation
├── tests/                   # Integration tests
└── README.md
```

## 📚 Complete Developer Workflow

### Step 1: Prepare Your Bevy Game

```rust
// In your main.rs or lib.rs
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_fairy::DebuggerPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // The plugin owns the BRP RemotePlugin. Never add a second one next to
        // it: Bevy rejects a duplicate plugin by name, so the game panics.
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .add_systems(Update, your_game_systems)
        .run();
}
```

### Step 2: Install and Configure

```bash
# Install the debugger
cargo install bevy_fairy

# Add to Claude Code config (~/.config/claude/claude_code_config.json)
{
  "mcpServers": {
    "bevy-fairy": {
      "command": "bevy-fairy",
      "args": []
    }
  }
}
```

### Step 3: Start Debugging Session

1. **Run your Bevy game**: `cargo run`
2. **Open Claude Code** in your project
3. **Start debugging** with natural language:
   - "What entities are in my game?"
   - "Why is performance dropping?"
   - "Monitor the player's health"
   - "Test spawning 1000 enemies"

## 🧪 Complete Debugging Toolkit (v0.2.0)

### Core MCP Tools

| Tool | Description | Example Usage |
|------|-------------|---------------|
| `observe` | Monitor game entities, components, and resources in real-time | "Show me all entities with Health component" |
| `experiment` | Test changes to game state with automatic rollback | "Set player speed to 2x and test collision" |
| `stress` | Performance testing and bottleneck identification | "Stress test the physics system with 500 objects" |
| `anomaly` | Detect unusual patterns in game behavior | "Find any entities with abnormal velocities" |
| `replay` | Record and replay debugging sessions | "Record the next 30 seconds of gameplay" |
| `hypothesis` | Test specific assumptions about game behavior | "Test if framerate drops when spawning > 100 enemies" |
| `screenshot` | Capture game window visuals with timing control | "Take a screenshot after 2 seconds warmup" |

### Internal Debug Commands (11 Integrated Tools)

The debugger provides 11 specialized debugging tools accessible through the MCP protocol:

| Tool | Purpose | Key Features |
|------|---------|--------------|
| **EntityInspector** | Deep entity analysis | • Component inspection<br>• Relationship tracking<br>• Change detection |
| **SystemProfiler** | System performance analysis | • Microsecond precision<br>• Dependency tracking<br>• <3% overhead |
| **VisualDebugOverlay** | In-game debug visualization | • Entity highlights<br>• Collider visualization<br>• Performance metrics |
| **QueryBuilder** | Type-safe ECS queries | • Natural language queries<br>• Query validation<br>• Result caching |
| **MemoryProfiler** | Memory usage tracking | • Allocation tracking<br>• Leak detection<br>• Usage patterns |
| **SessionManager** | Debug session management | • Session recording<br>• Checkpoint creation<br>• State comparison |
| **IssueDetector** | Automated issue detection | • 17 detection patterns<br>• Real-time monitoring<br>• Auto-diagnostics |
| **PerformanceBudgetMonitor** | Performance budget enforcement | • Frame time budgets<br>• Memory limits<br>• Violation tracking |
| **PatternLearningSystem** | ML-based pattern recognition | • Privacy-preserving (k=5)<br>• Pattern mining<br>• Suggestion generation |
| **SuggestionEngine** | Context-aware suggestions | • Based on learned patterns<br>• Confidence scoring<br>• Action recommendations |
| **WorkflowAutomation** | Automated debug workflows | • Common task automation<br>• Safety checkpoints<br>• Rollback support |

## 🖥️ Platform Support

| Platform | Installation | Status |
|----------|--------------|--------|
| **macOS** | `./scripts/install.sh` | ✅ Full support with LaunchAgent service |
| **Linux** | `./scripts/install.sh` | ✅ Full support |
| **Windows** | Manual build | ⚠️ Basic support (help wanted) |

### macOS Service Management

On macOS, the debugger can run as a background service:

```bash
# Service management
./scripts/service.sh start      # Start background service
./scripts/service.sh stop       # Stop service
./scripts/service.sh status     # Check status
./scripts/service.sh logs       # View logs
```

## 🤝 Contributing

We welcome contributions! Please see our [contribution guidelines](CONTRIBUTING.md).

```bash
# Development setup
git clone https://github.com/ladvien/bevy_fairy.git
cd bevy_fairy
cargo test                      # Run basic tests
cargo test --ignored           # Run full integration tests
cargo test screenshot_integration_wrapper::test_screenshot_ci_suite  # Fast screenshot tests
cargo fmt                       # Format code
cargo clippy                    # Lint code
```

### Running Screenshot Tests

The screenshot functionality has comprehensive test coverage:

```bash
# Fast screenshot tests (suitable for CI/development)
cargo test screenshot_integration_wrapper::test_screenshot_ci_suite

# Full screenshot integration suite  
cargo test screenshot_integration_wrapper::test_screenshot_integration_suite -- --ignored

# Individual test categories
cargo test screenshot_integration_wrapper::test_screenshot_utilities
cargo test screenshot_integration_wrapper::test_screenshot_basic_functionality
cargo test screenshot_integration_wrapper::test_screenshot_parameter_validation
cargo test screenshot_integration_wrapper::test_screenshot_timing_controls

# Performance testing
cargo test screenshot_integration_wrapper::test_screenshot_performance
```

## 📚 Documentation

- **[Usage Guide](docs/USAGE_GUIDE.md)** - Detailed feature documentation
- **[Claude Prompts](docs/CLAUDE_SUBAGENT_GUIDE.md)** - Effective prompting strategies
- **[macOS Service Setup](docs/MACOS_SERVICE.md)** - Background service configuration

## 🔒 Security & Privacy

- All communication happens locally between your game and Claude Code
- No game data is transmitted externally
- Sensitive information is automatically redacted from logs
- Debug recordings are stored locally and encrypted

## 📦 Changelog

The full changelog is [`CHANGELOG.md`](CHANGELOG.md), which covers every release. The short
version:

### v0.2.0 - the plugin, and the tools that stopped lying

- The companion Bevy plugin crate, `bevy_fairy`, published for the first time. It owns
  the BRP `RemotePlugin`, so a game adds `DebuggerPlugin` and never a second `RemotePlugin`.
- Game-registered commands, capture from an offscreen target or the window, injectable input
  that really arrives, a guide channel, and replayable scenes.
- The `experiment`, `hypothesis`, `detect_anomaly` and `stress_test` tools take the arguments
  their handlers read. They used to advertise one set of names and dispatch on another, so
  every request was dropped and a default ran instead.
- `hypothesis` measures the entity and component counts from the game instead of reporting
  constants, and `replay` records the game's entities instead of returning an empty map.

### v0.1.6 - Production Ready
## 📄 License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option.

It was GPL-3.0 until it was adopted into [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop), and relicensed on the way in to match the other `bevy_*` crates there: a GPL crate in the Bevy ecosystem cannot be adopted, and being adoptable is the entire reason these are published.

## 🙏 Acknowledgments

- Built for the [Bevy Engine](https://bevyengine.org/) community
- Powered by [Anthropic's MCP](https://modelcontextprotocol.io/)
- Inspired by the need for better game debugging tools

---

**Questions?** Open an [issue](https://github.com/ladvien/bevy_fairy/issues) or join the discussion in [Bevy's Discord](https://discord.gg/bevy).
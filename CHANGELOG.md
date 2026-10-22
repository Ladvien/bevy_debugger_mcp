# Changelog

## 0.2.0 — 2026-10-21

A minor bump, and a breaking one: the crate is renamed and merged, the tool schemas
changed, and the BRP method names changed.

The gap from 0.1.8 (2025-08-23) to here is large — a year of work, most of it in a
Bevy plugin that was not published at all until now.

### Breaking

- **The crate is renamed and the two halves are merged.** `bevy_debugger_mcp` and
  `bevy_debugger_bevy` are now one crate, `bevy_fairy`. The names stuttered, and a game
  that wanted the plugin had to depend on a crate whose name said it was about the server.

  The two halves are two *features* of the one crate, and they stay independent: Cargo
  resolves dependencies per feature, so a game that takes the plugin compiles the plugin's
  closure and none of the server's.

  | | before | after |
  |---|---|---|
  | a game | `bevy_debugger_bevy = "0.2"` | `bevy_fairy = { version = "0.2", default-features = false, features = ["plugin"] }` |
  | an agent | `cargo install bevy_debugger_mcp` | `cargo install bevy_fairy --no-default-features --features server` |
  | the binary | `bevy-debugger-mcp` | `bevy-fairy` |
  | the BRP methods | `bevy_debugger/*` | `bevy_fairy/*` |
  | the environment | `BEVY_DEBUGGER_*` | `BEVY_FAIRY_*` |
  | the control script | `bevy-debugger-control` | `bevy-fairy-control` |

  The plugin's public items are re-exported at the crate root, so a game writes
  `use bevy_fairy::DebuggerPlugin` — the same as it did when the plugin was its own crate.

  `bevy_debugger_mcp` 0.1.8 stays published and is not yanked. It will not receive further
  releases; this is the one that replaces it.

- **The `experiment`, `hypothesis`, `detect_anomaly` and `stress_test` tools take different
  arguments.** Each tool's parameters are now generated from the same Rust type its handler
  parses, so the advertised schema and the handler cannot drift apart again. The old schemas
  advertised `experiment_type`, `hypothesis`, `detection_type` and `test_type`; the handlers
  dispatched on `action`, so every one of those requests was dropped and a default ran
  instead. The new schema names the field the handler reads.

  | tool | before | after |
  |---|---|---|
  | `experiment` | `experiment_type`, `params`, `duration` | `action`, `actions` |
  | `hypothesis` | `hypothesis`, `confidence`, `context` | `action`, `description`, `success_condition`, `actions`, `variation_type`, `numeric_min`, `numeric_max`, `string_pool`, `parameter_name`, `parameter_values`, `iterations`, `timeout`, `seed`, `threshold` |
  | `detect_anomaly` | `detection_type`, `sensitivity`, `window` | `action`, `window_size`, `z_score_threshold`, `iqr_multiplier`, `min_samples`, `performance_threshold`, `entity_growth_threshold`, `min_severity` |
  | `stress_test` | `test_type`, `intensity`, `duration` | `action`, `test_type`, `intensity`, `duration`, `ramp_up`, `entity_type`, `spawn_rate`, `max_entities`, `change_rate`, `component_types`, `target_entities`, `target_memory_mb`, `complexity` |

  An agent that learned the old names has to learn the new ones. Nothing that worked stops
  working: the old names were never read.

- **`bevy` is now 0.20.** The plugin is a Bevy 0.20 plugin.

### Added

- **A Bevy plugin**, behind the `plugin` feature, published for the first time. It
  owns the BRP `RemotePlugin`, so a host adds `DebuggerPlugin` plus a transport and never a
  second `RemotePlugin` — adding one twice panics the game on start.
- **The guarantee that the plugin cannot reach the operating system is now checked against
  the resolved dependency graph** rather than against a list. `tests/plugin_leaf.rs` runs
  `cargo tree` for the plugin's feature set and fails if anything in it can synthesise an
  OS event, which is stronger than the manifest check it replaces: the graph cannot be
  quietly widened by adding a dependency to a feature.
- **Game-registered commands.** A host registers a typed handler under a `<game>/<verb>` name
  with `add_debug_command`; the agent discovers the vocabulary from the game itself through
  `bevy_fairy/commands`, and each command ships a checked example.
- **Capture from an offscreen target or the window.** `bevy_fairy/screenshot` asks for
  `offscreen` when a `DebugCaptureTarget` is present, and falls back to the window when it is
  not. Neither path raises, focuses or captures the screen the way a window manager would.
- **Injectable input that really arrives.** Input is written into Bevy's own input message
  stream, inside the game process, ahead of the input systems — so it behaves like a key
  somebody pressed, and an agent can type into a text field. The cursor is injectable too, and
  a host reads it through `bevy_fairy::cursor_position`. The window's own cursor is
  deliberately never written: Bevy's windowing backend would turn that into a request to move
  the *physical* pointer, dragging the mouse out from under whoever is at the machine.
- **A guide channel.** `bevy_fairy/guide` posts a script and the host renders one step;
  `bevy_fairy/guide+watch` waits on a condition and records `k/n`. The plugin never learns
  the host's vocabulary: checkpoints are one-shot systems the host registers by name.
- **Replayable scenes.** Every recorded command call is logged, and `bevy_fairy/scene`
  returns that log as a file. `BEVY_FAIRY_SCENE` replays one step per frame at startup.
- **A stdio transport that Claude Desktop can use.** Logging goes to stderr, never stdout, in
  stdio mode.

### Fixed

- **The server kept its tools in a router nothing read**, and quit right after `initialize`.
- **Injected input was written as the fold rather than the source**, so `just_pressed`-based
  actions could never fire and no text field could be typed into.
- **Four deadlocks and three correctness bugs** in the plugin's own internals.
- **The four tools above now do what they are asked**, and `hypothesis` measures the entity and
  component counts from the game instead of reporting constants. The three metrics BRP cannot
  report — frame time, memory, CPU — are refused with a reason rather than answered with a
  number.
- **`replay` records the game's entities** instead of returning an empty map. Events are not
  observable over BRP, so a frame says so rather than showing an empty list that reads as
  "nothing happened".
- **The docs match the code.** `CLAUDE.md`, the README and `book/SCREENSHOT_SETUP.md` no
  longer tell a reader to hand-register a method the plugin already registers, and the
  capture description says what the code does: the fallback is to window capture, which is
  weaker than offscreen but is not a focus-stealing path.

### Migration

An agent that calls the four tools above with the old field names will get a structured error
naming the bad value, listing the valid ones, and giving one corrected call. Read the tool's
schema from `tools/list` rather than from memory.

A host that linked the plugin by git now depends on
`bevy_fairy = { version = "0.2", default-features = false, features = ["plugin"] }`. A host
that hand-registered `bevy_debugger/screenshot` must delete that registration: the plugin
owns it, and registering it twice panics the game on start.

A host that registered a game command under a `bevy_debugger/` name must rename it to
`bevy_fairy/`: the BRP method names changed with the crate.

## 0.1.8 — 2025-08-23

The last release before this one. See the git history for its contents.

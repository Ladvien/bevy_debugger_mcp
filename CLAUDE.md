# bevy_debugger_mcp — notes for agents

An MCP server plus a companion Bevy plugin that let an agent inspect and drive a **running** Bevy game: live entity/component/resource queries, offscreen frame capture, and keyboard/mouse injection.

## Source of truth

The source of truth is [`Ladvien/foundation_vs_slop`](https://github.com/Ladvien/foundation_vs_slop) at `crates/bevy_debugger_mcp/`. If you are reading this in a standalone `Ladvien/bevy_debugger_mcp` checkout, that is a read-only `git subtree split` mirror — **changes made here cannot be pulled back**. Make them upstream.

This crate was vendored in with `git subtree add`, history intact, *because* it used to be a pinned git dependency and that pin made it unfixable: a bug found while driving the game could only be repaired by editing another repo, cutting a rev, and bumping. It is now an ordinary edit.

## Two independent halves

- **`src/`** — the MCP server binary `bevy-debugger-mcp`. A separate process: speaks MCP over stdio to an agent, BRP over JSON-RPC/HTTP (:15702) to the game. ~52k lines, 56 direct dependencies.
- **`crates/bevy_debugger_bevy/`** — the Bevy plugin the game links. ~400 lines, five dependencies.

The server does **not** depend on the plugin. Do not introduce a dependency between them: an agent can use Bevy's built-in BRP methods against a game that never linked the plugin, and a game can link the plugin without anyone running the server.

## The non-negotiable: nothing here may touch the OS

The whole reason this exists is that capturing a window requires raising it (stealing focus, possibly switching workspaces) and driving the OS keyboard sends keystrokes to whatever actually has focus. Measured on the game this was built for: **7,188 distinct colours** captured with the window focused, **1** with another app in front.

So:

- **Capture reads an `Image` a camera renders to, never the window surface.** `bevy_debugger/screenshot` fails loudly when `DebugCaptureTarget` is absent rather than falling back to window capture — the fallback *is* the focus-stealing path.
- **Input writes into Bevy's own `ButtonInput` resources**, inside the game process.

`crates/bevy_debugger_bevy/tests/leaf.rs` is the ratchet: it pins the plugin's dependency list and fails on any source mentioning `enigo`, `xdotool`, `CGEvent`, `SendInput`, `winit` or `unsafe`. That test is the enforcement of the paragraph above — widening it is a design decision and should cost a deliberate edit.

## BRP handlers run in `Last`, and that has bitten input

`DebuggerPlugin::build` registers its methods with `RemotePlugin::with_method_main(..)`, so handlers execute in Bevy's `Last` schedule. Bevy's `keyboard_input_system` clears `just_pressed`/`just_released` in the **next** frame's `PreUpdate`.

A press written in `Last` therefore has its `just_pressed` flag cleared before any `Update` system reads it, so **`just_pressed`-based actions could never be triggered by injected input** — the method returned `success: true` and the game did not move. Held `pressed` state survives the clear, which is why some actions appeared to work and others did not. See `input.rs` for how this is handled now; if you change the schedule or the action shape, re-verify against a real game rather than a unit test.

## `DebuggerPlugin` owns `RemotePlugin`

`DebuggerPlugin::build` adds `RemotePlugin::default().with_method_main(..)` itself. Bevy rejects a duplicate plugin by name, so a host app that also adds `RemotePlugin` panics the moment the feature is switched on. A host should add `DebuggerPlugin` plus **only** a transport (e.g. `RemoteHttpPlugin`).

## Profiles

There is deliberately no `[profile.*]` in `Cargo.toml`. As a workspace member its profiles would be ignored anyway, with a warning on every cargo command; the monorepo root already sets `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`. The manifest comment records what was removed.

## Build and test

From the monorepo, both halves are workspace members, so `cargo test --workspace` covers them. Individually:

```sh
cargo test -p bevy_debugger_bevy      # the plugin, including the dependency ratchet
cargo test -p bevy_debugger_mcp       # the server
cargo install --path .                # the bevy-debugger-mcp binary
```

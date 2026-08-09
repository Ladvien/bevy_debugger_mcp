# bevy_debugger_mcp — notes for agents

> **This is our code. Edit it here.**
>
> It is not a third-party dependency, not a vendored upstream, and not something to work around. If
> the debugger is missing a capability you need, **add the capability** — that is an ordinary change
> in this repo, reviewed like any other. The one thing that is *not* ordinary is widening the OS
> boundary below; that costs a deliberate argument.
>
> The word "vendored" appears in the history because this arrived by `git subtree add` rather than
> being written here. It says where the code came from, not who may change it. Every previous agent
> that treated a missing feature here as a fixed constraint was wrong: the cursor-position gap in
> `bevy_debugger/input` sat unfixed through two whole steps of work for exactly that reason, and it
> was a forty-line change.

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

## The cursor is a resource, not the window's

`bevy_debugger/input` accepts `kind: "Cursor"` with `x`/`y` in logical window pixels, or `clear: true`. It writes `DebugCursor`, **not** `Window::set_cursor_position`.

That looks like the obvious call and it is the wrong one, read out of the pinned engine rather than guessed: `Window::set_cursor_position` writes an internal field, and Bevy's windowing backend diffs that field against a per-window cache every frame and asks the platform to **move the physical pointer** (`bevy_winit-0.19.0/src/system.rs:433`). The cache is `pub(crate)`, so a plugin cannot suppress the diff. Writing the window would drag the mouse out from under whoever is at the machine — the precise class of thing this crate exists to avoid, and it would pass `tests/leaf.rs` while defeating its whole purpose.

**A host has to read the pointer through `bevy_debugger_bevy::cursor_position(&window, &debug_cursor)`** or the injected half never reaches it. A host that calls `Window::cursor_position` directly is undrivable by an agent, and that is not a bug in the plugin.

Ordering is load-bearing and is handled in `apply_pending_input`: a move *after* a button, or a second move in one frame, is deferred to the next frame. Applying `press, move` together makes the game read the press at the new position — the click never happens where it was aimed — and two moves in one frame collapse a drag's path to its endpoint.

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

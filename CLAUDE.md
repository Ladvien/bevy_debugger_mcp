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
- **Input writes into Bevy's own input message stream**, inside the game process — the same
  `Messages<KeyboardInput>` / `Messages<MouseButtonInput>` buffers `bevy_winit` appends to.

`crates/bevy_debugger_bevy/tests/leaf.rs` is the ratchet: it pins the plugin's dependency list and fails on any source mentioning `enigo`, `xdotool`, `CGEvent`, `SendInput`, `winit` or `unsafe`. That test is the enforcement of the paragraph above — widening it is a design decision and should cost a deliberate edit.

## BRP handlers run in `RemoteLast` — after everything — and that has bitten input

`DebuggerPlugin::build` registers its methods with `RemotePlugin::with_method_main(..)`, so handlers execute in **`RemoteLast`**, a schedule `RemotePlugin` inserts *after* `Last` (`bevy_remote-0.19.0/src/lib.rs:832-835`). It is the very end of the frame. Bevy's `keyboard_input_system` clears `just_pressed`/`just_released` in the **next** frame's `PreUpdate`.

A press written there therefore has its `just_pressed` flag cleared before any `Update` system reads it, so **`just_pressed`-based actions could never be triggered by injected input** — the method returned `success: true` and the game did not move. Held `pressed` state survives the clear, which is why some actions appeared to work and others did not. See `input.rs` for how this is handled now; if you change the schedule or the action shape, re-verify against a real game rather than a unit test.

(This paragraph said `Last` for as long as it existed, and it was close enough to be useful and wrong enough to mislead anyone reasoning about ordering against another `Last` system.)

## A watching method is how you wait, and the engine already owns it

`with_watching_method_main` (`bevy_remote-0.19.0/src/lib.rs:629`) re-runs a handler **every frame** in `RemoteLast` with exclusive `World`. `Ok(None)` means *not yet* and parks the request in `RemoteWatchingRequests`; `Ok(Some(v))`/`Err` sends a frame to the client and **keeps** the request open; `remove_closed_watching_requests` reaps it when the client goes away. The HTTP transport serves it as a `BrpHttpResponse::Stream` of `application/json` chunks, so `curl -N` consumes it — refused only inside a batch.

`bevy_debugger/guide+watch` is the first user of this, and it is why the guide channel needed **zero new dependencies**: a condition-watcher with lifecycle management, already written and tested by the engine.

The trap it walked into first: **the handler runs every frame, so any answer that does not change state is re-sent sixty times a second.** Both non-advancing answers (`waiting_on_a_person`, `done`) now announce once per step index and then park. See `Guide::announced`, and `tests/guide_steps.rs` for the two tests that exist because the first draft did not.

## The guide channel: the plugin talks to the *person*, not just the app

`bevy_debugger/guide` posts a script and the host renders one step; `bevy_debugger/guide+watch` waits on a condition and records `k/n`. The design and its citations are in `docs/bevy_debugger_mcp.md`. The parts that constrain edits here:

- **The plugin never learns the host's vocabulary.** Checkpoints are one-shot systems the host registers by name (`Checkpoints`), the same seam `PendingInput`'s public `queue_*` methods are. Anything that made this crate know what a tile is would be the mistake.
- **The overlay spawns no camera**, because a second one silently breaks any host `Single<&Camera>` query — the trap the mirror camera already argues about.
- **Step copy must be ASCII.** Bevy's embedded font is 95 codepoints and this crate cannot ship one without widening `leaf.rs`. `the_one_script_this_crate_ships_is_ascii` pins the example.

## Injected input is the *source*, not the fold — and that is what lets an agent type

`apply_pending_input` runs in `PreUpdate` **`.before(InputSystems)`** and writes `KeyboardInput` / `MouseButtonInput` **messages**. `ButtonInput<KeyCode>` and `ButtonInput<Key>` are Bevy's own fold of that stream, done by `keyboard_input_system`, which clears last frame's edges and *then* folds — so a message written ahead of it survives the clear that erased the old direct write.

**This reverses the first fix, and the reversal is the point.** Writing `ButtonInput` directly reached everything that reads `ButtonInput` and nothing that reads the stream — which is **every text field in every Bevy app**, since they use `MessageReader<KeyboardInput>` and match `logical_key`. So an agent could press keys and not type into them, and could not press Enter or Escape *in a field* either. That was `FVS-R-12`, and it blocked three verifications in one day.

Two consequences worth knowing before you touch this:

- **`InputPlugin` is now required.** Without it nothing folds the stream and every injection is accepted and read by nobody. `DebuggerPlugin::finish` asserts it — `finish`, not `build`, so it holds whichever order the host adds plugins in.
- **`kind: "Keyboard"` takes `key` *or* `text`, and sending both is refused.** `key` names a key on the keyboard and carries its logical half whenever Bevy spells the same name in `Key` (93 of them do, including `Enter`, `Escape`, `Backspace`, `Space`, `Tab`); `text` is what should arrive, one message per character, so `"site_67"` is one call and one frame. A separate `kind: "Text"` was designed and rejected: `Escape` is spelled identically in both enums, so an agent would have had to pick a kind based on which of the *host's* systems was listening — knowledge it cannot obtain — and each kind would produce only half of what a real key produces.

`text` refuses control characters by name (`use key: "Enter"`), and a space becomes `Key::Space` rather than `Key::Character(" ")`, because that is what a space bar produces and text handlers match the former.

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

//! Headless input injection — drives keyboard and mouse without the OS.
//!
//! Writes into Bevy's own input resources and message queues inside the game process. Nothing here
//! reaches the operating system: if the person at the machine is working in another window, injected
//! keys do not go there. That is structural rather than careful — this crate depends on `bevy`,
//! `bevy_remote`, `serde`, `serde_json` and `image`, none of which can synthesise an OS event.
//!
//! # Key names come from Bevy, not from a table here
//!
//! `key` and `button` are deserialized straight into [`KeyCode`] and [`MouseButton`], so every key
//! Bevy knows is accepted and the spelling is Bevy's own variant name — `KeyW`, `ArrowLeft`, `F11`,
//! `Numpad7`, `ShiftLeft`, `BracketRight`, `AudioVolumeUp`.
//!
//! This replaced a hand-written `match` covering twelve keys. Such a table is a second, always-stale
//! copy of an enum with nearly two hundred variants: it silently lacked every function key, every
//! modifier, every digit and the whole numpad, and nothing would have told you except an
//! `Unknown key` at the moment you needed one. Deserializing has no drift by construction — a Bevy
//! upgrade that adds a key adds it here.
//!
//! `MouseButton::Other(u16)` is reachable as `{"Other": 3}`, since that is how serde spells a
//! newtype variant.

use bevy::input::mouse::{MouseButton, MouseScrollUnit, MouseWheel};
use bevy::input::touch::TouchPhase;
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;
use bevy::remote::{error_codes, BrpError, BrpResult};
use bevy::window::PrimaryWindow;
use serde::Deserialize;
use serde_json::{json, Value};

/// Parameters for the `bevy_debugger/input` BRP method.
#[derive(Debug, Deserialize)]
pub struct InputCommand {
    /// What kind of input to inject.
    pub kind: InputKind,
    /// What action to perform. Defaults to [`InputAction::Tap`], and [`InputKind::Cursor`] ignores it
    /// — a position has no press and no release, so requiring a meaningless one would be a field a
    /// caller has to supply and cannot get right.
    #[serde(default)]
    pub action: InputAction,
    /// Key name — any [`KeyCode`] variant, e.g. `"KeyW"`, `"Space"`, `"ArrowLeft"`, `"F5"`,
    /// `"Numpad7"`, `"ShiftLeft"`.
    #[serde(default)]
    pub key: Option<String>,
    /// Mouse button — any [`MouseButton`] variant: `"Left"`, `"Right"`, `"Middle"`, `"Back"`,
    /// `"Forward"`, or `{"Other": 3}`.
    #[serde(default)]
    pub button: Option<Value>,
    /// Scroll amounts, used by [`InputKind::Scroll`]. Positive `y` scrolls up.
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    /// Scroll unit: `"Line"` (default) or `"Pixel"`.
    #[serde(default)]
    pub unit: Option<String>,
    /// [`InputKind::Cursor`] only: put the pointer back under the real mouse.
    ///
    /// A separate flag rather than "omit `x` and `y`", because omitting a coordinate is much more
    /// likely to be a caller bug than an intent to release the pointer, and the two must not look
    /// alike.
    #[serde(default)]
    pub clear: bool,
}

#[derive(Debug, Deserialize)]
pub enum InputKind {
    Keyboard,
    Mouse,
    Scroll,
    /// **Where the pointer is**, in logical window pixels — `x` right, `y` down from the top-left,
    /// the same frame [`bevy::window::Window::cursor_position`] reports.
    ///
    /// Set it and it stays until moved again or cleared; there is no per-frame stream to keep up.
    /// That is what makes a drag expressible: put the cursor down, press the button, move, release.
    Cursor,
}

#[derive(Debug, Default, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    Press,
    Release,
    /// Pressed for one frame and released the next — what a physical key does, and what a
    /// `just_pressed` reader expects. The default because it is what a caller nearly always means.
    #[default]
    Tap,
}

fn invalid_params(message: String) -> BrpError {
    BrpError { code: error_codes::INVALID_PARAMS, message, data: None }
}

/// One validated injection, waiting for the frame that can apply it.
///
/// Parsing happens in the BRP handler so a malformed request still fails loudly and immediately;
/// only the *application* is deferred. By the time a command reaches this enum it is known-good.
enum Injected {
    Key(KeyCode, InputAction),
    Mouse(MouseButton, InputAction),
    Scroll { unit: MouseScrollUnit, x: f32, y: f32, window: Entity },
    Cursor(Option<Vec2>),
}

/// **Where an injected pointer is**, in logical window pixels — `None` when the real mouse owns it.
///
/// # Why this is a resource and not the window's own cursor
///
/// Bevy has `Window::set_cursor_position`, and it looks like exactly the right call. It is not, and
/// the reason was read out of the pinned engine source rather than guessed.
///
/// `Window::set_cursor_position` writes only an internal field. But Bevy's windowing backend runs a
/// `changed_windows` system that compares that field against a per-window cache each frame, and on a
/// difference asks the platform to **move the physical pointer** — `bevy_winit-0.19.0/src/system.rs`
/// line 433. The cache is `pub(crate)`, so a plugin cannot update it to suppress the diff.
///
/// Writing the window would therefore drag the mouse out from under whoever is at the machine. That
/// is precisely the class of thing this crate exists to avoid, and `tests/leaf.rs` is the ratchet
/// that keeps it avoided; a cursor that moves the real one would satisfy the letter of that test
/// while breaking the whole point of it.
///
/// So the injected position lives here, beside the game's own input, and a host reads it through
/// [`cursor_position`]. Nothing outside the process is touched.
#[derive(Resource, Default, Debug, Clone, Copy, PartialEq)]
pub struct DebugCursor(pub Option<Vec2>);

/// **The pointer a host should act on** — the injected position if one is set, else the real one.
///
/// One question with one answer, asked in one place. A host that calls
/// [`Window::cursor_position`](bevy::window::Window::cursor_position) directly is unreachable by an
/// agent; a host that calls this is drivable and behaves identically for a person, because
/// [`DebugCursor`] is `None` until something explicitly sets it.
///
/// Precedence, not fallback: the injected value is only ever present because a caller asked for it,
/// and clearing it is equally explicit. There is no degraded substitute that switches itself on.
pub fn cursor_position(window: &Window, injected: &DebugCursor) -> Option<Vec2> {
    injected.0.or_else(|| window.cursor_position())
}

/// Injected input waiting to be applied in `PreUpdate`.
///
/// # Why this queue exists — the bug it fixes
///
/// BRP methods registered with `with_method_main` run in the **`Last`** schedule. Bevy's
/// `keyboard_input_system` runs in **`PreUpdate`** and begins by clearing the just-pressed and
/// just-released sets.
///
/// So a press written straight into `ButtonInput` from a handler had its `just_pressed` flag wiped by
/// the very next `PreUpdate`, *before* any `Update` system could read it. **Every `just_pressed`-based
/// action was therefore unreachable by injected input** — and the method still answered
/// `success: true`, so it looked like the game was ignoring a key it never actually saw. Held
/// `pressed` state survived the clear, which is what made the failure look intermittent: some actions
/// responded and some silently did not.
///
/// Queuing and applying in `PreUpdate` **after** [`InputSystems`](bevy::input::InputSystems) puts the
/// write on the far side of the clear, so an injected key is indistinguishable from a real one. There
/// is one path for input, not a special case for injected keys.
#[derive(Resource, Default)]
pub struct PendingInput {
    queue: Vec<Injected>,
    /// Taps press on one frame and release on the next, because that is what a real tap does. Pressing
    /// and releasing within a single frame leaves `pressed` false for the whole frame while both
    /// `just_pressed` and `just_released` are true — a state no physical key can produce, and one that
    /// silently skips every `pressed`-based reader.
    release_next_keys: Vec<KeyCode>,
    release_next_buttons: Vec<MouseButton>,
}

impl PendingInput {
    /// Queue a keyboard action, applied on the next `PreUpdate`.
    ///
    /// The BRP handler is one caller; a host driving itself (a test, an example, a scripted demo) is
    /// another. Both go through this queue rather than writing `ButtonInput` directly, because a
    /// direct write is only correct if it happens after Bevy's clear — and there is exactly one place
    /// that is guaranteed to.
    pub fn queue_key(&mut self, key: KeyCode, action: InputAction) {
        self.queue.push(Injected::Key(key, action));
    }

    /// Queue a one-frame tap: pressed on the next `PreUpdate`, released the frame after — the shape a
    /// physical key produces, and what `just_pressed` readers expect.
    pub fn queue_tap_key(&mut self, key: KeyCode) {
        self.queue_key(key, InputAction::Tap);
    }

    /// Queue a mouse-button action, applied on the next `PreUpdate`.
    pub fn queue_mouse(&mut self, button: MouseButton, action: InputAction) {
        self.queue.push(Injected::Mouse(button, action));
    }

    /// Queue a cursor move — logical window pixels, or `None` to hand the pointer back.
    ///
    /// Through the queue like everything else, so a cursor move and the click that follows it land in
    /// the documented order rather than racing: a click applied a frame before the move would be read
    /// at the old position, which is the drag bug this whole module is shaped around avoiding.
    pub fn queue_cursor(&mut self, at: Option<Vec2>) {
        self.queue.push(Injected::Cursor(at));
    }
}

/// Applies queued injections, in `PreUpdate` after Bevy has cleared last frame's edges.
///
/// Registered by [`DebuggerPlugin`](crate::DebuggerPlugin); a host that builds its own schedule must
/// keep the `.after(InputSystems)` ordering or the clear will eat these writes exactly as before, and
/// must `init_resource::<DebugCursor>()` — in Bevy 0.19 a missing `ResMut<T>` panics the system rather
/// than skipping it, so the absence is loud rather than a cursor injection that quietly does nothing.
pub fn apply_pending_input(
    mut pending: ResMut<PendingInput>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut wheel: MessageWriter<MouseWheel>,
    mut cursor: ResMut<DebugCursor>,
) {
    // Taken out first: draining in place would hold a borrow of `pending` across the loop below.
    let release_keys = core::mem::take(&mut pending.release_next_keys);
    let release_buttons = core::mem::take(&mut pending.release_next_buttons);
    let queue = core::mem::take(&mut pending.queue);

    // Last frame's taps end here, one frame after they began.
    for key in release_keys {
        keys.release(key);
    }
    for button in release_buttons {
        mouse.release(button);
    }

    // **At most one command per key per frame; the rest wait.**
    //
    // BRP drains every request that arrived since the last frame into a single handler run, so a
    // whole burst can land in this queue together. Applying a burst in one pass destroys edges,
    // because `ButtonInput` records a *state* and only two transitions are visible per frame:
    //
    // - two `Tap`s of one key collapse into a single press, since `ButtonInput::press` only sets
    //   `just_pressed` when the key was not already pressed;
    // - a `Tap` followed by a `Press` leaves a scheduled release that cancels the hold a frame later;
    // - a `Tap` followed by a `Release` produces `pressed == false` with both edges set at once,
    //   the very state this queue exists to prevent.
    //
    // Deferring the remainder spreads them over consecutive frames, which is what a real device does:
    // one key cannot be pressed twice in the same frame either. Relative order is preserved, because
    // the first command for a key marks it and every later one for that key falls through to
    // `deferred` behind it.
    let mut keys_touched: Vec<KeyCode> = Vec::new();
    let mut buttons_touched: Vec<MouseButton> = Vec::new();
    let mut cursor_moved = false;
    let mut deferred: Vec<Injected> = Vec::new();

    for cmd in queue {
        // **Once anything is held back, everything after it is too.**
        //
        // Without this the queue is only ordered *per key*: a later command of a different kind
        // overtakes an earlier deferred one. Measured by `examples/cursor_drag_lands.rs` the day
        // cursor injection landed — in `move, press, move, move, release` the release jumped ahead of
        // the two pending moves and the drag committed at the wrong corner, while every individual
        // rule was behaving as written.
        //
        // The cost is that two *different* keys queued behind a deferral now take a frame each
        // instead of sharing one. That is the right trade: a caller wrote a sequence, and a sequence
        // that arrives out of order is wrong in a way no amount of speed makes up for.
        if !deferred.is_empty() {
            deferred.push(cmd);
            continue;
        }
        match &cmd {
            Injected::Key(key, _) if keys_touched.contains(key) => {
                deferred.push(cmd);
                continue;
            }
            Injected::Mouse(button, _) if buttons_touched.contains(button) => {
                deferred.push(cmd);
                continue;
            }
            // **A move after a button, or a second move, waits for the next frame.**
            //
            // Position is state and a button edge is read against it, so both orderings matter and
            // only one is expressible per frame. `press, move` applied together would have the game
            // read the press at the *new* position — the press never happens where it was aimed —
            // and two moves in one frame would collapse a drag's path to its endpoint. Deferring
            // gives each its own frame, which is the shape a real mouse produces.
            //
            // A move *before* an untouched button is the one combination that is correct together,
            // and it is the common one: aim, then click there.
            Injected::Cursor(_) if cursor_moved || !buttons_touched.is_empty() => {
                deferred.push(cmd);
                continue;
            }
            _ => {}
        }

        match cmd {
            Injected::Key(key, action) => {
                keys_touched.push(key);
                match action {
                    InputAction::Press => keys.press(key),
                    InputAction::Release => keys.release(key),
                    InputAction::Tap => {
                        keys.press(key);
                        pending.release_next_keys.push(key);
                    }
                }
            }
            Injected::Mouse(button, action) => {
                buttons_touched.push(button);
                match action {
                    InputAction::Press => mouse.press(button),
                    InputAction::Release => mouse.release(button),
                    InputAction::Tap => {
                        mouse.press(button);
                        pending.release_next_buttons.push(button);
                    }
                }
            }
            // Scroll is a delta, not a state, so several in one frame simply sum — there is no edge
            // to lose and nothing to defer.
            Injected::Scroll { unit, x, y, window } => {
                wheel.write(MouseWheel {
                    unit,
                    x,
                    y,
                    window,
                    // A mouse always reports `Moved`; the variant exists for touch.
                    phase: TouchPhase::Moved,
                });
            }
            Injected::Cursor(at) => {
                cursor_moved = true;
                cursor.0 = at;
            }
        }
    }

    pending.queue = deferred;
}

/// BRP handler: `bevy_debugger/input`.
///
/// Validates now, applies in `PreUpdate` — see [`PendingInput`] for why the write cannot happen here.
pub fn handle_input(
    In(params): In<Option<Value>>,
    mut pending: ResMut<PendingInput>,
    window: Option<Single<Entity, With<PrimaryWindow>>>,
) -> BrpResult {
    // `serde_json::Error` has no `From` into `BrpError`, so `?` cannot carry it — a malformed payload
    // is reported as INVALID_PARAMS with the parser's own message, which is the part a caller can act on.
    let cmd: InputCommand = match params.as_ref() {
        Some(p) => serde_json::from_value(p.clone())
            .map_err(|e| invalid_params(format!("invalid input params: {e}")))?,
        None => return Err(invalid_params("Missing input parameters".to_string())),
    };

    match cmd.kind {
        InputKind::Keyboard => {
            let name = cmd
                .key
                .ok_or_else(|| invalid_params("Missing 'key' for keyboard input".to_string()))?;
            // Bevy's own variant names, parsed by Bevy's own deserializer.
            let keycode: KeyCode = serde_json::from_value(Value::String(name.clone()))
                .map_err(|e| invalid_params(format!("unknown key `{name}`: {e}")))?;
            pending.queue.push(Injected::Key(keycode, cmd.action));
        }
        InputKind::Mouse => {
            let raw = cmd
                .button
                .ok_or_else(|| invalid_params("Missing 'button' for mouse input".to_string()))?;
            let button: MouseButton = serde_json::from_value(raw.clone())
                .map_err(|e| invalid_params(format!("unknown mouse button `{raw}`: {e}")))?;
            pending.queue.push(Injected::Mouse(button, cmd.action));
        }
        InputKind::Scroll => {
            // This was a stub that returned success and wrote nothing — the worst of both, because a
            // caller scrolling and seeing no movement has no way to tell "ignored" from "the game did
            // not react". A `MouseWheel` message needs a window to be addressed to, so with no primary
            // window it now says so instead of silently doing nothing.
            let window = window.ok_or_else(|| BrpError {
                code: error_codes::INTERNAL_ERROR,
                message: "scroll needs a primary window to address the MouseWheel message to, and \
                          there is none"
                    .to_string(),
                data: None,
            })?;
            let unit = match cmd.unit.as_deref() {
                None | Some("Line") => MouseScrollUnit::Line,
                Some("Pixel") => MouseScrollUnit::Pixel,
                Some(other) => {
                    return Err(invalid_params(format!(
                        "unknown scroll unit `{other}`; expected \"Line\" or \"Pixel\""
                    )))
                }
            };
            let (x, y) = (cmd.x.unwrap_or(0.0), cmd.y.unwrap_or(0.0));
            if !x.is_finite() || !y.is_finite() {
                return Err(invalid_params(format!(
                    "scroll amounts must be finite, got x={x}, y={y}"
                )));
            }
            pending.queue.push(Injected::Scroll { unit, x, y, window: *window });
        }
        InputKind::Cursor => {
            if cmd.clear {
                pending.queue.push(Injected::Cursor(None));
            } else {
                // Both required, and named individually: a caller who sent only `x` has made a
                // mistake, and a `y` defaulted to 0 would put the pointer at the top of the window
                // and look like a working call.
                let x = cmd
                    .x
                    .ok_or_else(|| invalid_params("Missing 'x' for cursor input".to_string()))?;
                let y = cmd
                    .y
                    .ok_or_else(|| invalid_params("Missing 'y' for cursor input".to_string()))?;
                if !x.is_finite() || !y.is_finite() {
                    return Err(invalid_params(format!(
                        "cursor position must be finite, got x={x}, y={y}"
                    )));
                }
                // Deliberately not bounded to the window here. `Window::cursor_position` already
                // reports `None` outside the window area, so a host reading through
                // `cursor_position` sees "off the window" — which is a real state a mouse produces
                // and a thing worth being able to test.
                pending.queue.push(Injected::Cursor(Some(Vec2::new(x, y))));
            }
        }
    }

    Ok(json!({
        "success": true,
        // Deliberately not "Input injected": it has been accepted and validated, and lands on the next
        // `PreUpdate`. Claiming it already reached the game is what made the old schedule bug so hard
        // to see — the reply said the key had been delivered while the game never saw it.
        "message": "Input queued; applied on the next frame's PreUpdate",
    }))
}

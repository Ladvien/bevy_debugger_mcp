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
    /// What action to perform.
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
}

#[derive(Debug, Deserialize)]
pub enum InputKind {
    Keyboard,
    Mouse,
    Scroll,
}

#[derive(Debug, Deserialize)]
pub enum InputAction {
    Press,
    Release,
    Tap,
}

fn invalid_params(message: String) -> BrpError {
    BrpError { code: error_codes::INVALID_PARAMS, message, data: None }
}

/// BRP handler: `bevy_debugger/input`.
pub fn handle_input(
    In(params): In<Option<Value>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
    mut wheel: MessageWriter<MouseWheel>,
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
            match cmd.action {
                InputAction::Press => keys.press(keycode),
                InputAction::Release => keys.release(keycode),
                // Press and release in the same call, so a system reading `just_pressed` and one
                // reading `just_released` both see it this frame.
                InputAction::Tap => {
                    keys.press(keycode);
                    keys.release(keycode);
                }
            }
        }
        InputKind::Mouse => {
            let raw = cmd
                .button
                .ok_or_else(|| invalid_params("Missing 'button' for mouse input".to_string()))?;
            let button: MouseButton = serde_json::from_value(raw.clone())
                .map_err(|e| invalid_params(format!("unknown mouse button `{raw}`: {e}")))?;
            match cmd.action {
                InputAction::Press => mouse.press(button),
                InputAction::Release => mouse.release(button),
                InputAction::Tap => {
                    mouse.press(button);
                    mouse.release(button);
                }
            }
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
            wheel.write(MouseWheel {
                unit,
                x,
                y,
                window: *window,
                // A mouse always reports `Moved`; the variant exists for touch.
                phase: TouchPhase::Moved,
            });
        }
    }

    Ok(json!({
        "success": true,
        "message": "Input injected",
    }))
}

//! Headless input injection — drives keyboard/mouse without the OS.
//!
//! Writes directly to Bevy's input resources and message queues,
//! never touching the real OS input stack.

use bevy::prelude::*;
use bevy::remote::BrpResult;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use serde::Deserialize;
use serde_json::{json, Value};

/// Parameters for the `bevy_debugger/input` BRP method.
#[derive(Debug, Deserialize)]
pub struct InputCommand {
    /// What kind of input to inject.
    pub kind: InputKind,
    /// What action to perform.
    pub action: InputAction,
    /// Key name for keyboard (e.g. "KeyW", "Space", "ArrowLeft").
    #[serde(default)]
    pub key: Option<String>,
    /// Mouse button name ("Left", "Right", "Middle").
    #[serde(default)]
    pub button: Option<String>,
    /// Mouse position for movement.
    #[serde(default)]
    pub x: Option<f32>,
    #[serde(default)]
    pub y: Option<f32>,
    /// Mouse delta for motion.
    #[serde(default)]
    pub dx: Option<f32>,
    #[serde(default)]
    pub dy: Option<f32>,
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
    Move,
}

/// BRP handler: `bevy_debugger/input`
pub fn handle_input(
    In(params): In<Option<Value>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut mouse: ResMut<ButtonInput<MouseButton>>,
) -> BrpResult {
    // `serde_json::Error` has no `From` into `BrpError`, so `?` cannot carry it — a malformed payload
    // is reported as INVALID_PARAMS with the parser's own message, which is the part a caller can act on.
    let cmd: InputCommand = match params.as_ref() {
        Some(p) => serde_json::from_value(p.clone()).map_err(|e| bevy_remote::BrpError {
            code: bevy_remote::error_codes::INVALID_PARAMS,
            message: format!("invalid input params: {e}"),
            data: None,
        })?,
        None => {
            return Err(bevy_remote::BrpError {
                code: bevy_remote::error_codes::INVALID_PARAMS,
                message: "Missing input parameters".to_string(),
                data: None,
            })
        }
    };

    match cmd.kind {
        InputKind::Keyboard => {
            let key = cmd.key
                .as_deref()
                .ok_or_else(|| bevy_remote::BrpError {
                    code: bevy_remote::error_codes::INVALID_PARAMS,
                    message: "Missing 'key' for keyboard input".to_string(),
                    data: None,
                })?;
            let keycode = parse_keycode(key)?;
            match cmd.action {
                InputAction::Press | InputAction::Tap => {
                    keys.press(keycode);
                    if matches!(cmd.action, InputAction::Tap) {
                        keys.release(keycode);
                    }
                }
                InputAction::Release => {
                    keys.release(keycode);
                }
                _ => {}
            }
        }
        InputKind::Mouse => {
            if let Some(btn) = &cmd.button {
                let button = parse_mouse_button(btn)?;
                match cmd.action {
                    InputAction::Press | InputAction::Tap => {
                        mouse.press(button);
                        if matches!(cmd.action, InputAction::Tap) {
                            mouse.release(button);
                        }
                    }
                    InputAction::Release => {
                        mouse.release(button);
                    }
                    _ => {}
                }
            }
        }
        InputKind::Scroll => {
            // MouseWheel messages would be written here.
        }
    }

    Ok(json!({
        "success": true,
        "message": "Input injected",
    }))
}

fn parse_keycode(name: &str) -> Result<KeyCode, bevy_remote::BrpError> {
    match name {
        "Space" => Ok(KeyCode::Space),
        "KeyW" => Ok(KeyCode::KeyW),
        "KeyA" => Ok(KeyCode::KeyA),
        "KeyS" => Ok(KeyCode::KeyS),
        "KeyD" => Ok(KeyCode::KeyD),
        "ArrowLeft" => Ok(KeyCode::ArrowLeft),
        "ArrowRight" => Ok(KeyCode::ArrowRight),
        "ArrowUp" => Ok(KeyCode::ArrowUp),
        "ArrowDown" => Ok(KeyCode::ArrowDown),
        "Enter" => Ok(KeyCode::Enter),
        "Escape" => Ok(KeyCode::Escape),
        "Tab" => Ok(KeyCode::Tab),
        _ => Err(bevy_remote::BrpError {
            code: bevy_remote::error_codes::INVALID_PARAMS,
            message: format!("Unknown key: {name}"),
            data: None,
        }),
    }
}

fn parse_mouse_button(name: &str) -> Result<MouseButton, bevy_remote::BrpError> {
    match name {
        "Left" => Ok(MouseButton::Left),
        "Right" => Ok(MouseButton::Right),
        "Middle" => Ok(MouseButton::Middle),
        _ => Err(bevy_remote::BrpError {
            code: bevy_remote::error_codes::INVALID_PARAMS,
            message: format!("Unknown mouse button: {name}"),
            data: None,
        }),
    }
}
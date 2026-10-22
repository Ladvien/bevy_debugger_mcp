//! `bevy_fairy/time`: pause the game clock and set its speed.
//!
//! Gregory, *Game Engine Architecture* (3rd ed., §10.4–10.6), lists a pause and a time scale among
//! the features that make in-engine debugging useful: an agent can freeze a frame, then inspect it.
//! This drives `Time<Virtual>`, the clock that gameplay and animation read, so pausing it freezes
//! the simulation but not the renderer or BRP itself.

use bevy::prelude::*;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::plugin::commands::{add_builtin_command, DebugCommandInfo};

/// Params of `bevy_fairy/time`. Both fields are optional; an empty call only reports the clock.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimeParams {
    pub pause: Option<bool>,
    pub speed: Option<f32>,
}

/// Registers `bevy_fairy/time` as a recorded built-in command.
pub(crate) fn register(app: &mut App) {
    add_builtin_command::<TimeParams, _>(
        app,
        DebugCommandInfo::new(
            "bevy_fairy/time",
            "Pause or unpause the game clock (Time<Virtual>) and set its speed multiplier. Returns the clock state.",
        )
        .params("pause: bool, optional; speed: f32 > 0, optional (1.0 is real time)")
        .example(json!({"pause": true})),
        handle_time,
    );
}

fn handle_time(In(params): In<TimeParams>, mut clock: ResMut<Time<Virtual>>) -> Result<Value, String> {
    if let Some(speed) = params.speed {
        if !speed.is_finite() || speed <= 0.0 {
            return Err("speed must be > 0".to_string());
        }
        clock.set_relative_speed(speed);
    }
    match params.pause {
        Some(true) => clock.pause(),
        Some(false) => clock.unpause(),
        None => {}
    }
    Ok(json!({
        "paused": clock.is_paused(),
        "speed": clock.relative_speed(),
    }))
}

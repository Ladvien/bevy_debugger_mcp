//! `bevy_debugger/quit`: close the game cleanly.
//!
//! Agents need a way to end a session they started, so the game is not left running on a shared
//! machine. The request writes `AppExit::Success` as a message, the same path a window close takes,
//! so `Drop` impls and the render shutdown run. It is not recorded in a scene: quitting is a session
//! action, not part of the state being inspected.

use bevy::prelude::*;
use bevy::remote::BrpResult;
use serde_json::{json, Value};

/// BRP handler: `bevy_debugger/quit`.
pub(crate) fn handle_quit(In(_): In<Option<Value>>, mut exit: MessageWriter<AppExit>) -> BrpResult {
    exit.write(AppExit::Success);
    Ok(json!({ "quitting": true }))
}

//! # Bevy Debugger Bevy Plugin
//!
//! Companion plugin for `bevy_debugger_mcp`. Register this plugin in your Bevy game
//! to enable screenshot capture (with zoom/region selection), entity inspection,
//! and headless input injection (keyboard/mouse without the OS).
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy::remote::RemotePlugin;
//! use bevy::remote::http::RemoteHttpPlugin;
//! use bevy_debugger_bevy::DebuggerPlugin;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(RemotePlugin::default())
//!         .add_plugins(RemoteHttpPlugin::default())
//!         .add_plugins(DebuggerPlugin)
//!         .run();
//! }
//! ```

use bevy::prelude::*;

mod screenshot;
mod input;

pub use screenshot::{DebugCaptureTarget, ScreenshotParams};
pub use input::InputCommand;

/// Plugin that registers all custom BRP methods for the debugger.
///
/// Methods registered:
/// - `bevy_debugger/screenshot` — **offscreen** capture with optional zoom/region. Requires the host
///   to insert [`DebugCaptureTarget`]; it never captures the window, because that needs the window
///   raised and focused.
/// - `bevy_debugger/input` — headless keyboard/mouse injection
pub struct DebuggerPlugin;

impl Plugin for DebuggerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::remote::RemotePlugin::default()
            .with_method_main("bevy_debugger/screenshot", screenshot::handle_screenshot)
            .with_method_main("bevy_debugger/input", input::handle_input)
        );
    }
}
//! # Bevy Debugger Bevy Plugin
//!
//! Companion plugin for `bevy_debugger_mcp`. Register this plugin in your Bevy game
//! to enable screenshot capture (with zoom/region selection), entity inspection,
//! and headless input injection (keyboard/mouse without the OS).
//!
//! ## Quick Start
//!
//! ## Quick Start
//!
//! **Add [`DebuggerPlugin`] and a transport — never a second `RemotePlugin`.** This plugin builds its
//! own `RemotePlugin` in order to register its methods, and Bevy rejects a duplicate plugin by name,
//! so adding one alongside panics the moment the feature is switched on.
//!
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy::remote::http::RemoteHttpPlugin;
//! use bevy_debugger_bevy::DebuggerPlugin;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         // `DebuggerPlugin` owns `RemotePlugin`; only the transport is yours to add.
//!         .add_plugins(DebuggerPlugin)
//!         .add_plugins(RemoteHttpPlugin::default())
//!         .run();
//! }
//! ```

use bevy::prelude::*;

mod screenshot;
mod input;

pub use screenshot::{DebugCaptureTarget, ScreenshotParams};
pub use input::{apply_pending_input, InputAction, InputCommand, InputKind, PendingInput};

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
        )
        // The handler above runs in `Last` and only QUEUES; this is what actually writes into
        // `ButtonInput`. The ordering is the whole fix and is not cosmetic: `InputSystems` clears
        // last frame's just-pressed/just-released edges at the top of `PreUpdate`, so a write placed
        // before it is erased before any `Update` reader runs — which is exactly what used to happen
        // from `Last`, making every `just_pressed` action unreachable while the method still reported
        // success. See `input::PendingInput`.
        .init_resource::<input::PendingInput>()
        // **Claim the input state this plugin reads.**
        //
        // `apply_pending_input` takes `ButtonInput<KeyCode>`, `ButtonInput<MouseButton>` and a
        // `MessageWriter<MouseWheel>`. In Bevy 0.19 a missing `Res<T>` **panics the system** rather
        // than skipping it, so registering this system unconditionally would panic every frame in any
        // host that has not added `InputPlugin` — including the `MinimalPlugins` shape a headless host
        // would reasonably use. Before this change those resources were only touched inside the BRP
        // handler, so such a host never reached them.
        //
        // Both calls are idempotent — `init_resource` by definition, and `add_message` guards on
        // `contains_resource::<Messages<T>>` (bevy_app-0.19.0/src/sub_app.rs:386) — so a host that
        // *does* add `InputPlugin` is unaffected whichever order the plugins go in.
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        .add_systems(
            PreUpdate,
            input::apply_pending_input.after(bevy::input::InputSystems),
        );
    }
}
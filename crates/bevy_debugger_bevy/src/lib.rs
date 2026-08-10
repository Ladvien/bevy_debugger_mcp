//! # Bevy Debugger Bevy Plugin
//!
//! Companion plugin for `bevy_debugger_mcp`. Register this plugin in your Bevy game
//! to enable screenshot capture (with zoom/region selection), entity inspection,
//! and headless input injection — keyboard, mouse, cursor position and **typed text**, none of it
//! touching the OS.
//!
//! ## Quick Start
//!
//! **Add [`DebuggerPlugin`] and a transport — never a second `RemotePlugin`.** This plugin builds its
//! own `RemotePlugin` in order to register its methods, and Bevy rejects a duplicate plugin by name,
//! so adding one alongside panics the moment the feature is switched on.
//!
//! **`InputPlugin` must be present** (`DefaultPlugins` includes it; `MinimalPlugins` does not).
//! Injected input is written as `KeyboardInput`/`MouseButtonInput` messages and `InputPlugin` owns the
//! systems that fold those into `ButtonInput`; without it every injection is accepted and read by
//! nobody. [`DebuggerPlugin::finish`] asserts this rather than leaving it to be discovered.
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
pub use input::{
    apply_pending_input, cursor_position, logical_for, typed, DebugCursor, InputAction, InputCommand,
    InputKind, PendingInput,
};

/// Plugin that registers all custom BRP methods for the debugger.
///
/// Methods registered:
/// - `bevy_debugger/screenshot` — **offscreen** capture with optional zoom/region. Requires the host
///   to insert [`DebugCaptureTarget`]; it never captures the window, because that needs the window
///   raised and focused.
/// - `bevy_debugger/input` — headless keyboard/mouse injection, including **cursor position**, so a
///   drag is expressible: move, press, move, release. A host must read the pointer through
///   [`cursor_position`] rather than [`Window::cursor_position`](bevy::window::Window::cursor_position)
///   for that half to reach it — see [`DebugCursor`] for why the window's own cursor is not written.
pub struct DebuggerPlugin;

impl Plugin for DebuggerPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bevy::remote::RemotePlugin::default()
            .with_method_main("bevy_debugger/screenshot", screenshot::handle_screenshot)
            .with_method_main("bevy_debugger/input", input::handle_input)
        )
        // The handler above runs in `Last` and only QUEUES; this is what actually writes the input
        // messages. See `input::PendingInput`.
        .init_resource::<input::PendingInput>()
        // **Claim the message queues this plugin writes.**
        //
        // `apply_pending_input` takes `MessageWriter`s for `KeyboardInput`, `MouseButtonInput` and
        // `MouseWheel`, and in Bevy 0.19 a missing `Res<T>` **panics the system** rather than skipping
        // it. `add_message` guards on `contains_resource::<Messages<T>>`
        // (bevy_app-0.19.0/src/sub_app.rs:386), so a host that also adds `InputPlugin` is unaffected
        // whichever order the plugins go in.
        //
        // The two `ButtonInput` resources that used to be claimed here are gone with the direct write:
        // this plugin no longer touches them, because Bevy derives them from the stream above.
        .add_message::<bevy::input::keyboard::KeyboardInput>()
        .add_message::<bevy::input::mouse::MouseButtonInput>()
        .add_message::<bevy::input::mouse::MouseWheel>()
        // **The injected pointer.** Not the window's own cursor, and that is not a shortcut — writing
        // `Window::set_cursor_position` makes Bevy's windowing backend move the *physical* mouse, so
        // it would drag the pointer out from under whoever is at the machine. `input::DebugCursor`
        // carries the full argument and the line number it was read from.
        .init_resource::<input::DebugCursor>()
        // **Before, not after, and the reversal is the fix rather than a tidy-up.**
        //
        // `keyboard_input_system` clears last frame's edges and *then* folds the `KeyboardInput`
        // stream into `ButtonInput` (bevy_input-0.19.0/src/keyboard.rs). Writing messages ahead of it
        // means Bevy produces the edge itself, from the same stream a real key travels on — so both
        // `ButtonInput` readers and the ~ten `MessageReader<KeyboardInput>` text fields in a typical
        // app see one injected key.
        //
        // The previous ordering was `.after(..)`, writing `ButtonInput` directly on the far side of
        // the clear. That was correct for what it did and reached only half of the readers; the
        // superseded test `ordering_after_input_systems_costs_a_frame` still pins the fact it rested
        // on, from the other side.
        .add_systems(
            PreUpdate,
            input::apply_pending_input.before(bevy::input::InputSystems),
        );
    }

    /// **Injected input is inert without `InputPlugin`, so say so at startup.**
    ///
    /// `ButtonInput` is Bevy's fold of the message stream this plugin writes, and the system that
    /// folds it belongs to `InputPlugin`. Without it, every injection is accepted, queued, written —
    /// and read by nobody. A method that reports success while the game never moves is the precise
    /// failure this crate exists to make impossible, and it is worth a panic at startup rather than a
    /// silence at the moment somebody is depending on it.
    ///
    /// **`finish`, not `build`**, so it holds whichever order the host adds its plugins in
    /// (bevy_app-0.19.0/src/plugin.rs:70). And it asserts rather than adding `InputPlugin` itself:
    /// this plugin already owns `RemotePlugin`, and a second silently-added plugin is a
    /// duplicate-name panic waiting on somebody else's add order.
    fn finish(&self, app: &mut App) {
        assert!(
            app.is_plugin_added::<bevy::input::InputPlugin>(),
            "DebuggerPlugin needs bevy::input::InputPlugin: injected keys and clicks are written as \
             KeyboardInput/MouseButtonInput messages, and InputPlugin owns the systems that fold \
             those into ButtonInput. Without it every injection is accepted and read by nobody. Add \
             InputPlugin (DefaultPlugins includes it; MinimalPlugins does not)."
        );
    }
}
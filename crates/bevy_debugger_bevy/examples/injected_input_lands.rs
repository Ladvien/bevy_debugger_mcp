//! **Injected input behaves exactly like a real key press — shown in a terminal, no GPU.**
//!
//! This is the crate's least obvious guarantee, and the one that was broken. `bevy_debugger/input`
//! does not write into `ButtonInput` when the BRP request arrives: BRP handlers run in `Last`, and
//! Bevy clears the just-pressed/just-released edges at the top of the next `PreUpdate`. A write from
//! `Last` is therefore erased before any `Update` system can see it, so every `just_pressed`-based
//! action was unreachable while the method still answered `success: true`.
//!
//! Instead the handler queues, and `apply_pending_input` writes in `PreUpdate` *after*
//! `InputSystems`. This example drives that path directly — no MCP server, no network, no window —
//! and prints what an ordinary `Update` system observes on each frame.
//!
//! Run it:
//!
//! ```sh
//! cargo run -p bevy_debugger_bevy --example injected_input_lands
//! ```
//!
//! Expect a `Tap` to be visible to `just_pressed` on exactly one frame and to `just_released` on the
//! next — the same shape a physical key produces.

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy_debugger_bevy::PendingInput;

/// What an ordinary gameplay system saw, recorded per frame so the run can be printed as a table.
#[derive(Resource, Default)]
struct Observed(Vec<String>);

fn main() {
    let mut app = App::new();
    // `MinimalPlugins` gives a schedule and nothing else — no window, no renderer, no audio. Bevy's
    // input plugin is added on its own because that is what owns the clear this example is about.
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .init_resource::<PendingInput>()
        .init_resource::<Observed>()
        .add_systems(
            PreUpdate,
            // The ordering IS the fix. Drop the `.after` and this example prints "nothing" forever.
            bevy_debugger_bevy::apply_pending_input.after(InputSystems),
        )
        .add_systems(Update, observe);

    // Frame 1: nothing queued — the baseline.
    app.update();

    // Queue a tap the way the BRP handler does, then run the frame that applies it.
    app.world_mut()
        .resource_mut::<PendingInput>()
        .queue_tap_key(KeyCode::KeyQ);
    app.update(); // frame 2 — press lands
    app.update(); // frame 3 — release lands
    app.update(); // frame 4 — back to rest

    println!("\n  frame  what an Update system saw");
    println!("  -----  --------------------------");
    for (n, line) in app.world().resource::<Observed>().0.iter().enumerate() {
        println!("  {:>5}  {line}", n + 1);
    }
    println!(
        "\n  A tap is visible to `just_pressed` on one frame and `just_released` on the next.\n  \
         Pressing and releasing in the SAME frame — which is what the handler used to do — would\n  \
         leave `pressed` false for the whole frame while both edges were true, a state no physical\n  \
         key can produce, and every `pressed`-based reader would silently skip it.\n"
    );
}

fn observe(keys: Res<ButtonInput<KeyCode>>, mut seen: ResMut<Observed>) {
    let mut parts = Vec::new();
    if keys.just_pressed(KeyCode::KeyQ) {
        parts.push("just_pressed");
    }
    if keys.pressed(KeyCode::KeyQ) {
        parts.push("pressed");
    }
    if keys.just_released(KeyCode::KeyQ) {
        parts.push("just_released");
    }
    seen.0.push(if parts.is_empty() {
        "nothing".to_string()
    } else {
        parts.join(" + ")
    });
}

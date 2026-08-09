//! **A click-drag, injected — no window, no GPU, no mouse.**
//!
//! ```sh
//! cargo run -p bevy_debugger_bevy --example cursor_drag_lands
//! ```
//!
//! Keyboard injection was never enough to drive an editor. A box-select, a lasso, a seating
//! surface — all of them are *press here, move there, release* — and until the cursor could be
//! placed, an agent could reach every keyboard verb of a tool and none of its mouse ones.
//!
//! The property this prints is the one that makes a drag mean anything: **the press is read where
//! it was aimed**. A naive implementation applies the queued move and the queued press in the same
//! frame, and the game reads the press at the *destination* — so the drag starts wherever it ended
//! and selects nothing. `apply_pending_input` defers a move that follows a button to the next frame,
//! which is what a real mouse does anyway.
//!
//! # It cannot reach your desktop
//!
//! The pointer here is [`DebugCursor`], a resource beside the game's own input — **not** the window's
//! cursor. Bevy's windowing backend turns a change to `Window`'s cursor into a request to move the
//! *physical* pointer, which would drag the mouse out from under whoever is at the machine. See
//! `DebugCursor`'s own docs for the measurement and the line it was read from.

use bevy::prelude::*;
use bevy_debugger_bevy::{apply_pending_input, DebugCursor, InputAction, PendingInput};
use bevy::input::InputSystems;

/// A toy "box select": where the drag started, where it is now, and whether it has finished.
#[derive(Resource, Default)]
struct Marquee {
    from: Option<Vec2>,
    to: Option<Vec2>,
    committed: Option<(Vec2, Vec2)>,
}

/// The kind of system an editor actually has — it reads the pointer and a button, nothing else.
fn box_select(
    cursor: Res<DebugCursor>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut marquee: ResMut<Marquee>,
) {
    let Some(at) = cursor.0 else { return };
    if mouse.just_pressed(MouseButton::Left) {
        marquee.from = Some(at);
        marquee.to = Some(at);
        println!("  press   at {at:?}   <- the frame the drag begins");
    } else if mouse.pressed(MouseButton::Left) {
        marquee.to = Some(at);
        println!("  drag    to {at:?}");
    } else if mouse.just_released(MouseButton::Left) {
        if let (Some(from), Some(_)) = (marquee.from, marquee.to) {
            marquee.committed = Some((from, at));
            println!("  release at {at:?}   <- the box is {from:?} .. {at:?}");
        }
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .init_resource::<PendingInput>()
        // `DebuggerPlugin` inits this; an app that registers the system by hand has to as well. In
        // Bevy 0.19 a missing `ResMut<T>` panics the system rather than skipping it, so forgetting is
        // loud rather than a cursor injection that quietly does nothing.
        .init_resource::<DebugCursor>()
        .init_resource::<Marquee>()
        // **After `InputSystems`, and that is the whole fix.** Bevy clears last frame's
        // just-pressed/just-released edges at the top of `PreUpdate`; a write placed before it is
        // erased before any `Update` reader runs.
        .add_systems(PreUpdate, apply_pending_input.after(InputSystems))
        .add_systems(Update, box_select);

    println!("\nQueuing a whole drag in one batch — move, press, move, move, release:\n");
    {
        let mut pending = app.world_mut().resource_mut::<PendingInput>();
        pending.queue_cursor(Some(Vec2::new(100.0, 100.0)));
        pending.queue_mouse(MouseButton::Left, InputAction::Press);
        pending.queue_cursor(Some(Vec2::new(140.0, 130.0)));
        pending.queue_cursor(Some(Vec2::new(180.0, 160.0)));
        pending.queue_mouse(MouseButton::Left, InputAction::Release);
    }

    // Enough frames for the queue to drain: one command per key/button per frame, one move per
    // frame, which is what spreads the drag over a path instead of collapsing it to its endpoint.
    for frame in 0..7 {
        println!("frame {frame}:");
        app.update();
    }

    let marquee = app.world().resource::<Marquee>();
    match marquee.committed {
        Some((from, to)) => {
            println!("\nboxed {from:?} .. {to:?}");
            assert_eq!(
                from,
                Vec2::new(100.0, 100.0),
                "the press has to be read where it was aimed, not where the drag ended"
            );
            assert_eq!(to, Vec2::new(180.0, 160.0));
            println!(
                "the press landed at its aim and the release at the far corner — a real box, from \
                 injected input alone, with the machine's own mouse untouched.\n"
            );
        }
        None => panic!("the drag never completed — the queue did not drain in the frames given"),
    }
}

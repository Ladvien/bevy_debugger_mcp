//! **The injected-input contract, pinned.**
//!
//! Injected input has one job: be indistinguishable from a real key. That is entirely a question of
//! *when* the write happens and *how many* writes land in one frame, so none of it is visible to a
//! test that only calls `ButtonInput::press` directly — the bug this file guards against was invisible
//! to exactly that kind of test for as long as it existed.
//!
//! Two properties are pinned here:
//!
//! 1. **Ordering.** `apply_pending_input` must run after [`InputSystems`], which clears last frame's
//!    edges at the top of `PreUpdate`. Written before it, every `just_pressed` is erased before any
//!    `Update` system runs — the original bug, where the BRP method answered `success: true` and the
//!    game never moved.
//! 2. **One command per key per frame.** BRP drains a whole burst of requests into one handler run, and
//!    `ButtonInput` can only show two transitions per frame, so applying a burst at once silently
//!    destroys edges.

use bevy::input::InputSystems;
use bevy::prelude::*;
use bevy_debugger_bevy::{apply_pending_input, DebugCursor, InputAction, PendingInput};

/// What an ordinary `Update` system saw, one entry per frame.
#[derive(Resource, Default)]
struct Seen(Vec<Edges>);

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
struct Edges {
    just_pressed: bool,
    pressed: bool,
    just_released: bool,
}

fn observe(keys: Res<ButtonInput<KeyCode>>, mut seen: ResMut<Seen>) {
    seen.0.push(Edges {
        just_pressed: keys.just_pressed(KeyCode::KeyQ),
        pressed: keys.pressed(KeyCode::KeyQ),
        just_released: keys.just_released(KeyCode::KeyQ),
    });
}

/// An app wired the way [`DebuggerPlugin`](bevy_debugger_bevy::DebuggerPlugin) wires it.
fn app_correctly_ordered() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .init_resource::<PendingInput>()
        // `apply_pending_input` writes the injected pointer here, and in Bevy 0.19 a missing
        // `ResMut<T>` panics the system rather than skipping it. `DebuggerPlugin` inits this; a host
        // that registers the system by hand — as these tests do — has to init it too.
        .init_resource::<bevy_debugger_bevy::DebugCursor>()
        .init_resource::<Seen>()
        .add_systems(PreUpdate, apply_pending_input.after(InputSystems))
        .add_systems(Update, observe);
    app
}

fn queue_key(app: &mut App, key: KeyCode, action: InputAction) {
    app.world_mut()
        .resource_mut::<PendingInput>()
        .queue_key(key, action);
}

fn seen(app: &App) -> Vec<Edges> {
    app.world().resource::<Seen>().0.clone()
}

#[test]
fn a_tap_presses_for_one_frame_and_releases_the_next() {
    let mut app = app_correctly_ordered();
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Tap);
    app.update();
    app.update();
    app.update();

    let frames = seen(&app);
    assert!(
        frames[0].just_pressed && frames[0].pressed,
        "the tap must be visible to `just_pressed` on the frame it lands: {frames:?}"
    );
    assert!(
        frames[1].just_released && !frames[1].pressed,
        "and released on the very next frame, the way a physical key behaves: {frames:?}"
    );
    assert_eq!(
        frames[2],
        Edges::default(),
        "and leave nothing behind afterwards: {frames:?}"
    );
}

#[test]
fn ordering_before_input_systems_erases_the_press_entirely() {
    // The same app with the one ordering constraint inverted. This is the original bug, and it is
    // the reason `.after(InputSystems)` is not a stylistic choice: `keyboard_input_system` clears the
    // just-pressed set at the top of `PreUpdate`, so a write placed ahead of it is gone before any
    // `Update` system can see it.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .init_resource::<PendingInput>()
        // `apply_pending_input` writes the injected pointer here, and in Bevy 0.19 a missing
        // `ResMut<T>` panics the system rather than skipping it. `DebuggerPlugin` inits this; a host
        // that registers the system by hand — as these tests do — has to init it too.
        .init_resource::<bevy_debugger_bevy::DebugCursor>()
        .init_resource::<Seen>()
        .add_systems(PreUpdate, apply_pending_input.before(InputSystems))
        .add_systems(Update, observe);

    queue_key(&mut app, KeyCode::KeyQ, InputAction::Tap);
    app.update();

    let frames = seen(&app);
    assert!(
        !frames[0].just_pressed,
        "misordered, the press edge must be lost — if this ever starts passing, the clear has moved \
         and the ordering guarantee needs rechecking: {frames:?}"
    );
}

#[test]
fn two_taps_in_one_batch_produce_two_separate_press_edges() {
    // Both requests arrive before a single frame boundary, which is the normal case when an agent
    // sends two calls in quick succession. Applied in one pass they would collapse into one press,
    // because `ButtonInput::press` only sets `just_pressed` when the key was not already pressed.
    let mut app = app_correctly_ordered();
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Tap);
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Tap);
    for _ in 0..4 {
        app.update();
    }

    let presses = seen(&app).iter().filter(|e| e.just_pressed).count();
    assert_eq!(
        presses,
        2,
        "two taps must produce two press edges, not one: {:?}",
        seen(&app)
    );
}

#[test]
fn a_tap_followed_by_a_press_leaves_the_key_held() {
    // "Step once, then keep walking." The tap's deferred release must not cancel the hold.
    let mut app = app_correctly_ordered();
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Tap);
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Press);
    for _ in 0..4 {
        app.update();
    }

    let frames = seen(&app);
    let last = frames.last().copied().unwrap_or_default();
    assert!(
        last.pressed,
        "the key must still be held after the tap's release has run: {frames:?}"
    );
}

#[test]
fn a_tap_and_a_release_never_show_both_edges_with_the_key_unpressed() {
    // Press and release inside one frame yields `pressed == false` while both edges are set — a state
    // no physical key can produce, and one every `pressed`-based reader silently skips.
    let mut app = app_correctly_ordered();
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Tap);
    queue_key(&mut app, KeyCode::KeyQ, InputAction::Release);
    for _ in 0..4 {
        app.update();
    }

    for (n, frame) in seen(&app).iter().enumerate() {
        assert!(
            !(frame.just_pressed && frame.just_released && !frame.pressed),
            "frame {n} shows a press and a release at once with the key unpressed: {frame:?}"
        );
    }
}

// ------------------------------------------------------------------------------------------------
// Cursor position — the half that makes a drag expressible
// ------------------------------------------------------------------------------------------------

/// A move lands, stays put without being re-sent, and clearing hands the pointer back.
#[test]
fn a_cursor_move_lands_and_clearing_releases_it() {
    let mut app = app_correctly_ordered();
    app.world_mut()
        .resource_mut::<PendingInput>()
        .queue_cursor(Some(Vec2::new(120.0, 64.0)));
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(120.0, 64.0)));

    // No per-frame stream to keep up: a position is state, not an edge.
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(120.0, 64.0)));

    app.world_mut().resource_mut::<PendingInput>().queue_cursor(None);
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, None);
}

/// **Aim, then click, in one frame** — the one ordering that is correct together, and the common one.
#[test]
fn a_move_before_an_untouched_button_applies_in_the_same_frame() {
    let mut app = app_correctly_ordered();
    {
        let mut pending = app.world_mut().resource_mut::<PendingInput>();
        pending.queue_cursor(Some(Vec2::new(10.0, 20.0)));
        pending.queue_mouse(MouseButton::Left, InputAction::Press);
    }
    app.update();
    assert_eq!(
        app.world().resource::<DebugCursor>().0,
        Some(Vec2::new(10.0, 20.0)),
        "the aim has to land on the frame the press is read"
    );
    assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));
}

/// **A press must be read where it was aimed**, so a move queued after one waits a frame.
///
/// Applied together, the game would see the press at the *new* position and the click would never
/// happen where it was pointed.
#[test]
fn a_move_after_a_button_waits_for_the_next_frame() {
    let mut app = app_correctly_ordered();
    {
        let mut pending = app.world_mut().resource_mut::<PendingInput>();
        pending.queue_cursor(Some(Vec2::new(10.0, 10.0)));
        pending.queue_mouse(MouseButton::Left, InputAction::Press);
        pending.queue_cursor(Some(Vec2::new(90.0, 90.0)));
    }
    app.update();
    assert_eq!(
        app.world().resource::<DebugCursor>().0,
        Some(Vec2::new(10.0, 10.0)),
        "the press frame must still read the position it was aimed at"
    );
    app.update();
    assert_eq!(
        app.world().resource::<DebugCursor>().0,
        Some(Vec2::new(90.0, 90.0)),
        "and the drag continues on the next frame"
    );
}

/// Two moves in one batch are two frames, so a drag has a path and not just an endpoint.
#[test]
fn two_moves_in_one_batch_are_spread_over_two_frames() {
    let mut app = app_correctly_ordered();
    {
        let mut pending = app.world_mut().resource_mut::<PendingInput>();
        pending.queue_cursor(Some(Vec2::new(1.0, 1.0)));
        pending.queue_cursor(Some(Vec2::new(2.0, 2.0)));
    }
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(1.0, 1.0)));
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(2.0, 2.0)));
}

/// **The injected pointer wins while it is set, and the window's own is read otherwise.**
///
/// One question with one answer. A host calling `Window::cursor_position` directly is undrivable by
/// an agent; a host calling this behaves identically for a person, because `DebugCursor` is `None`
/// until something explicitly sets it.
#[test]
fn the_injected_pointer_takes_precedence_and_hands_back_when_cleared() {
    use bevy::window::{Window, WindowResolution};
    let window = Window {
        resolution: WindowResolution::new(800, 600),
        ..Default::default()
    };
    assert_eq!(
        bevy_debugger_bevy::cursor_position(&window, &DebugCursor(Some(Vec2::new(5.0, 6.0)))),
        Some(Vec2::new(5.0, 6.0))
    );
    // A bare `Window` has no real cursor, so clearing gives what the window gives: nothing.
    assert_eq!(
        bevy_debugger_bevy::cursor_position(&window, &DebugCursor(None)),
        window.cursor_position()
    );
}

/// **A later command must never overtake an earlier deferred one.**
///
/// The regression this pins was found by `examples/cursor_drag_lands.rs` on the day cursor injection
/// landed, and every individual rule was behaving as written: the queue was ordered *per key*, so in
/// `move, press, move, move, release` the release — a button nothing had touched yet that frame —
/// jumped ahead of the two still-pending moves and the drag committed at the wrong corner.
#[test]
fn a_release_does_not_overtake_the_moves_queued_before_it() {
    let mut app = app_correctly_ordered();
    {
        let mut pending = app.world_mut().resource_mut::<PendingInput>();
        pending.queue_cursor(Some(Vec2::new(100.0, 100.0)));
        pending.queue_mouse(MouseButton::Left, InputAction::Press);
        pending.queue_cursor(Some(Vec2::new(140.0, 130.0)));
        pending.queue_cursor(Some(Vec2::new(180.0, 160.0)));
        pending.queue_mouse(MouseButton::Left, InputAction::Release);
    }
    // Frame 0: aim and press together — the one pairing that is correct in one frame.
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(100.0, 100.0)));
    assert!(app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left));

    // Frame 1: the drag moves, and the button is still held.
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(140.0, 130.0)));
    assert!(
        app.world().resource::<ButtonInput<MouseButton>>().pressed(MouseButton::Left),
        "the release must not have jumped the queue"
    );

    // Frame 2: the last move, and only then the release — at the far corner, where it was aimed.
    app.update();
    assert_eq!(app.world().resource::<DebugCursor>().0, Some(Vec2::new(180.0, 160.0)));
    assert!(app.world().resource::<ButtonInput<MouseButton>>().just_released(MouseButton::Left));
}

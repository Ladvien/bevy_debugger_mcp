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
use bevy_debugger_bevy::{apply_pending_input, InputAction, PendingInput};

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

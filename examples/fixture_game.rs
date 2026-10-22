//! The fixture game: a Bevy app whose state is known from its own code.
//!
//! Passing unit tests are not evidence that a tool tells the truth -- this crate once had 463 of
//! them sitting beside tools that fabricated their answers. So the server's tools are checked
//! against this game instead, and every value a test expects comes from a `pub const` below,
//! never from the tool being tested.
//!
//! It is headless: `MinimalPlugins`, Bevy's `InputPlugin`, the debugger plugin and the BRP HTTP
//! transport. No window, no renderer, so it runs on a CI box or a busy shared host. The parts of
//! a fixture that need a window -- a UI button, a pickable object, a text field -- are not here
//! yet, because nothing headless can click them honestly.
//!
//! ```sh
//! cargo run --example fixture_game --features plugin            # port 15702, 60 fps
//! FIXTURE_PORT=15999 FIXTURE_FPS=30 cargo run --example fixture_game --features plugin
//! ```
//!
//! Tests include this file with `#[path]` and call [`build_app`], so the game they check is the
//! one an agent would drive.

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::input::InputPlugin;
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_fairy::DebuggerPlugin;

/// The weight of every [`Crate`] the game spawns, in spawn order. The last one is the
/// deliberately odd value: a crate cannot weigh less than nothing.
pub const CRATE_WEIGHTS: [f32; 6] = [1.0, 2.0, 3.0, 4.0, 5.0, -7.5];

/// The value of the [`Score`] resource. Nothing changes it.
pub const SCORE_POINTS: u32 = 42;

/// How fast the [`Mover`] travels along +x, in world units per second, from x = 0.
pub const MOVER_SPEED: f32 = 2.0;

/// The frame on which the [`Spinner`] stops turning. This is the planted bug: the spinner is
/// supposed to turn forever, and `spin` quietly stops after this many frames.
pub const SPINNER_STOPS_AFTER_FRAMES: u32 = 120;

/// How far the [`Spinner`] turns each frame while it still turns, in radians.
pub const SPINNER_STEP: f32 = 0.05;

/// The frame cap when `FIXTURE_FPS` is not set.
pub const DEFAULT_FPS: f64 = 60.0;

/// The BRP port when `FIXTURE_PORT` is not set: Bevy's own default.
pub const DEFAULT_PORT: u16 = 15702;

/// A box with a weight. There are [`CRATE_WEIGHTS`]`.len()` of them.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
#[type_path = "fixture_game"]
pub struct Crate {
    pub weight: f32,
}

/// The one score the game keeps.
#[derive(Resource, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Resource)]
#[type_path = "fixture_game"]
pub struct Score {
    pub points: u32,
}

/// The entity that moves along +x at [`MOVER_SPEED`].
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
#[type_path = "fixture_game"]
pub struct Mover;

/// The entity that is supposed to turn forever, and does not.
#[derive(Component, Reflect, Debug, Clone, Copy, PartialEq)]
#[reflect(Component)]
#[type_path = "fixture_game"]
pub struct Spinner {
    /// The current angle, in radians.
    pub angle: f32,
    /// How many frames `spin` has run on this spinner.
    pub frames: u32,
}

/// The fixture game, listening for BRP on `port` and capped at `fps` frames a second.
pub fn build_app(port: u16, fps: f64) -> App {
    let mut app = App::new();
    app.add_plugins(
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / fps))),
    )
    .add_plugins(InputPlugin)
    // Owns the BRP RemotePlugin. Never add a second RemotePlugin next to it.
    .add_plugins(DebuggerPlugin::default())
    .add_plugins(RemoteHttpPlugin::default().with_port(port))
    .register_type::<Crate>()
    .register_type::<Score>()
    .register_type::<Mover>()
    .register_type::<Spinner>()
    .insert_resource(Score {
        points: SCORE_POINTS,
    })
    .add_systems(Startup, spawn_world)
    .add_systems(Update, (move_mover, spin));
    app
}

fn spawn_world(mut commands: Commands) {
    for weight in CRATE_WEIGHTS {
        commands.spawn((Name::new("crate"), Crate { weight }));
    }
    commands.spawn((Name::new("mover"), Mover, Transform::default()));
    commands.spawn((
        Name::new("spinner"),
        Spinner {
            angle: 0.0,
            frames: 0,
        },
    ));
}

fn move_mover(time: Res<Time>, mut movers: Query<&mut Transform, With<Mover>>) {
    for mut transform in &mut movers {
        transform.translation.x += MOVER_SPEED * time.delta_secs();
    }
}

/// The planted bug. It counts frames and turns the spinner, but stops turning once the count
/// passes [`SPINNER_STOPS_AFTER_FRAMES`] -- while still counting, so the frame counter keeps
/// going and only the angle freezes.
fn spin(mut spinners: Query<&mut Spinner>) {
    for mut spinner in &mut spinners {
        spinner.frames += 1;
        if spinner.frames <= SPINNER_STOPS_AFTER_FRAMES {
            spinner.angle += SPINNER_STEP;
        }
    }
}

fn env_or<T: std::str::FromStr>(name: &str, default: T) -> T {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

// Tests include this file with `#[path]` and call `build_app` directly, so in a test binary
// `main` is never called.
#[allow(dead_code)]
fn main() {
    let port = env_or("FIXTURE_PORT", DEFAULT_PORT);
    let fps = env_or("FIXTURE_FPS", DEFAULT_FPS);
    build_app(port, fps).run();
}

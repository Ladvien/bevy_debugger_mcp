//! A minimal host for `bevy_fairy`: one game-registered command, an offscreen capture target,
//! and the BRP HTTP endpoint an agent talks to. Run it with
//!
//! ```text
//! cargo run --features plugin-screenshot --example plugin_agent_scene
//! ```
//!
//! then, from another shell, drive it with plain JSON-RPC over HTTP on port 15702:
//!
//! ```text
//! curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":1,"method":"bevy_fairy/status"}'
//! curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":2,"method":"bevy_fairy/commands"}'
//! curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":3,"method":"demo/spawn_cube","params":{"at":[0.0,0.5,0.0]}}'
//! curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":4,"method":"bevy_fairy/screenshot"}'
//! curl -s localhost:15702 -d '{"jsonrpc":"2.0","id":5,"method":"bevy_fairy/capture","params":{"id":1}}'
//! ```
//! `status` reports `ready: true` from the first frame, because this host does not call
//! `host_signals_ready`. `commands` lists `demo/spawn_cube` with its description and
//! example. The capture call returns the PNG as base64; the `id` comes from the earlier screenshot call.
//! The `bevy_fairy` server wraps exactly these calls as its `run`, `screenshot` and `status` tools.

use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy::render::render_resource::TextureFormat;
use bevy::camera::RenderTarget;
use bevy_fairy::{DebugCaptureTarget, DebugCommandInfo, DebugCommandsAppExt, DebuggerPlugin};
use serde::Deserialize;
use serde_json::{json, Value};

/// The params of `demo/spawn_cube`. `deny_unknown_fields` makes a typo a named error, not a silent
/// default, so the agent learns the right shape from the error and the example.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpawnCube {
    at: [f32; 3],
}

fn spawn_cube(
    In(params): In<SpawnCube>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) -> Result<Value, String> {
    let [x, y, z] = params.at;
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::default())),
        MeshMaterial3d(materials.add(StandardMaterial::from_color(Color::srgb(0.8, 0.3, 0.2)))),
        Transform::from_xyz(x, y, z),
    ));
    Ok(json!({ "spawned_at": [x, y, z] }))
}

fn setup(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    // The camera renders into an image, not the window, so an offscreen capture never depends on the
    // window being visible. The same handle is shared with the companion as its capture target.
    let target = images.add(Image::new_target_texture(
        640,
        360,
        TextureFormat::Bgra8UnormSrgb,
        None,
    ));
    commands.spawn((
        Camera3d::default(),
        RenderTarget::from(target.clone()),
        Transform::from_xyz(0.0, 4.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(DebugCaptureTarget { image: target });
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, DebuggerPlugin::default(), RemoteHttpPlugin::default()))
        .add_debug_command(
            DebugCommandInfo::new("demo/spawn_cube", "Spawn a red cube at a world position in metres")
                .params("at: [x, y, z], the cube's centre in metres")
                .example(json!({ "at": [0.0, 0.5, 0.0] })),
            spawn_cube,
        )
        .add_systems(Startup, setup)
        .run();
}

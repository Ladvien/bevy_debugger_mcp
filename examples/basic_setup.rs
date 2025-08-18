use bevy::prelude::*;
use bevy::remote::RemotePlugin;

/// Basic Bevy game setup with RemotePlugin for debugging
///
/// This example shows the minimal setup required to enable
/// debugging with Bevy Debugger MCP.
///
/// Run with: cargo run --example basic_setup
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default()) // Enable remote debugging
        .add_systems(Startup, setup)
        .add_systems(Update, (move_player, rotate_cube))
        .run();
}

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Cube;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Spawn a player entity
    commands.spawn((
        Transform::from_xyz(0.0, 0.5, 0.0),
        Player,
        Name::new("Player"),
    ));

    // Spawn a cube to observe
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Cube,
        Name::new("Rotating Cube"),
    ));

    // Light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

fn move_player(
    keyboard_input: Res<ButtonInput<KeyCode>>,
    mut query: Query<&mut Transform, (With<Player>, Without<Cube>)>,
    time: Res<Time>,
) {
    for mut transform in &mut query {
        let mut direction = Vec3::ZERO;

        if keyboard_input.pressed(KeyCode::ArrowLeft) {
            direction.x -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowRight) {
            direction.x += 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowUp) {
            direction.z -= 1.0;
        }
        if keyboard_input.pressed(KeyCode::ArrowDown) {
            direction.z += 1.0;
        }

        if direction.length() > 0.0 {
            direction = direction.normalize();
            transform.translation += direction * 5.0 * time.delta_secs();
        }
    }
}

fn rotate_cube(mut query: Query<&mut Transform, (With<Cube>, Without<Player>)>, time: Res<Time>) {
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * 0.5);
    }
}

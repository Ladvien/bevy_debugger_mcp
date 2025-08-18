use bevy::{
    prelude::*,
    remote::{RemotePlugin, BrpResult},
    render::view::screenshot::{save_to_disk, Screenshot},
};
use serde_json::Value;

/// Example Bevy game setup with screenshot support for the Bevy Debugger MCP
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Enable remote debugging with custom screenshot method
        .add_plugins(
            RemotePlugin::default()
                .with_method("bevy_debugger/screenshot", screenshot_handler)
        )
        .add_systems(Startup, setup)
        .add_systems(Update, (rotate_cube, screenshot_on_spacebar))
        .run();
}

/// Set up a simple scene with a rotating cube
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Add a camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0),
    ));

    // Add a cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        Transform::default(),
        Cube, // Add marker component for easier querying
    ));

    // Add a light
    commands.spawn((
        PointLight {
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
}

/// Marker component for cube entities
#[derive(Component)]
struct Cube;

/// Rotate the cube each frame
fn rotate_cube(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Cube>>,
) {
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * 0.5);
    }
}

/// Take screenshot when spacebar is pressed (for testing)
fn screenshot_on_spacebar(
    mut commands: Commands,
    input: Res<ButtonInput<KeyCode>>,
    mut counter: Local<u32>,
) {
    if input.just_pressed(KeyCode::Space) {
        let path = format!("./local-screenshot-{}.png", *counter);
        *counter += 1;
        println!("Taking screenshot: {}", path);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

/// Custom BRP handler for screenshot requests from the debugger
fn screenshot_handler(
    In(params): In<Option<Value>>, 
    mut commands: Commands,
) -> BrpResult {
    // Parse parameters from MCP request
    let path = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("./screenshot.png")
        .to_string();

    let description = params
        .as_ref()
        .and_then(|p| p.get("description"))
        .and_then(|d| d.as_str())
        .unwrap_or("Screenshot from Bevy game");

    // Note: timing controls (warmup_duration, capture_delay) are handled by the MCP server
    // before the BRP request reaches this handler
    
    println!("Screenshot requested via BRP: {} -> {}", description, path);
    
    // Use Bevy's built-in screenshot system
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    
    // Return success response
    Ok(serde_json::json!({
        "path": path,
        "success": true,
        "description": description,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }))
}
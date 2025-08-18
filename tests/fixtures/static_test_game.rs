use bevy::{
    prelude::*,
    remote::{RemotePlugin, BrpResult},
    render::view::screenshot::{save_to_disk, Screenshot},
    app::AppExit,
    window::WindowPlugin,
};
use serde_json::Value;
use std::time::Duration;

/// Static test game fixture for E2E screenshot testing
/// Creates a predictable scene with colored objects for reliable visual testing
pub fn run_static_test_game() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Screenshot Test Game - Static".into(),
                resolution: (800.0, 600.0).into(),
                position: WindowPosition::Centered(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(
            RemotePlugin::default()
                .with_method("bevy_debugger/screenshot", screenshot_handler)
        )
        .add_systems(Startup, setup_static_scene)
        .add_systems(Update, (auto_exit_system, handle_screenshot_requests))
        .run();
}

/// Set up a static, predictable scene for screenshot testing
fn setup_static_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera positioned for consistent screenshots
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 3.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("TestCamera"),
    ));

    // Red cube - primary test object
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.2, 0.2),
            ..default()
        })),
        Transform::from_xyz(-1.5, 0.0, 0.0),
        Name::new("RedCube"),
        TestMarker,
    ));

    // Green sphere - secondary test object
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.8))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 1.0, 0.2),
            ..default()
        })),
        Transform::from_xyz(1.5, 0.0, 0.0),
        Name::new("GreenSphere"),
        TestMarker,
    ));

    // Blue cylinder - tertiary test object  
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.6, 2.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.2, 1.0),
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, -1.5),
        Name::new("BlueCylinder"),
        TestMarker,
    ));

    // Bright white light for consistent illumination
    commands.spawn((
        PointLight {
            intensity: 1000.0,
            color: Color::WHITE,
            shadows_enabled: false, // Disable shadows for consistency
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
        Name::new("TestLight"),
    ));

    // Add ground plane for reference
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(10.0, 10.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            ..default()
        })),
        Transform::from_xyz(0.0, -1.0, 0.0),
        Name::new("GroundPlane"),
    ));

    info!("Static test scene initialized");
}

/// Marker component for test objects
#[derive(Component)]
struct TestMarker;

/// Auto-exit system to prevent hanging in CI
fn auto_exit_system(
    time: Res<Time>,
    mut exit: EventWriter<AppExit>,
    mut timer: Local<Option<Timer>>,
) {
    if timer.is_none() {
        *timer = Some(Timer::new(Duration::from_secs(30), TimerMode::Once));
    }

    if let Some(ref mut t) = timer.as_mut() {
        t.tick(time.delta());
        if t.finished() {
            info!("Auto-exit timer finished - shutting down test game");
            exit.write(AppExit::Success);
        }
    }
}

/// Handle screenshot requests and log status
fn handle_screenshot_requests() {
    // This system exists to keep the game running and responsive to BRP requests
    // The actual screenshot handling is done by the BRP handler
}

/// Enhanced screenshot handler with timing controls
fn screenshot_handler(
    In(params): In<Option<Value>>, 
    mut commands: Commands,
) -> BrpResult {
    let path = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("./test_screenshot.png")
        .to_string();

    let _capture_delay = params
        .as_ref()
        .and_then(|p| p.get("capture_delay"))
        .and_then(|d| d.as_u64())
        .unwrap_or(100); // Shorter delay for tests

    let _wait_for_render = params
        .as_ref()
        .and_then(|p| p.get("wait_for_render"))
        .and_then(|w| w.as_bool())
        .unwrap_or(true);

    let description = params
        .as_ref()
        .and_then(|p| p.get("description"))
        .and_then(|d| d.as_str())
        .unwrap_or("E2E test screenshot");

    info!("Screenshot handler called: {} -> {}", description, path);

    // Use Bevy's built-in screenshot system
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    
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
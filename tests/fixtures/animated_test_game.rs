use bevy::{
    prelude::*,
    remote::{RemotePlugin, BrpResult},
    render::view::screenshot::{save_to_disk, Screenshot},
    app::AppExit,
    window::WindowPlugin,
};
use serde_json::Value;
use std::time::Duration;

/// Animated test game fixture for E2E screenshot testing
/// Creates rotating and moving objects to test timing controls
pub fn run_animated_test_game() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Screenshot Test Game - Animated".into(),
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
        .add_systems(Startup, setup_animated_scene)
        .add_systems(Update, (
            rotate_cubes,
            orbit_sphere,
            pulse_light,
            auto_exit_system,
        ))
        .run();
}

/// Set up an animated scene with predictable motion patterns
fn setup_animated_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 3.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("AnimatedTestCamera"),
    ));

    // Rotating red cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.5, 1.5, 1.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.2, 0.2),
            ..default()
        })),
        Transform::from_xyz(-2.0, 0.0, 0.0),
        Name::new("RotatingCube"),
        RotatingObject { speed: 1.0 },
    ));

    // Counter-rotating blue cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.2, 1.2, 1.2))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.2, 1.0),
            ..default()
        })),
        Transform::from_xyz(2.0, 0.0, 0.0),
        Name::new("CounterRotatingCube"),
        RotatingObject { speed: -0.7 },
    ));

    // Orbiting green sphere
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.6))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 1.0, 0.2),
            ..default()
        })),
        Transform::from_xyz(0.0, 2.0, 0.0),
        Name::new("OrbitingSphere"),
        OrbitingObject {
            radius: 3.0,
            speed: 0.5,
            angle: 0.0,
        },
    ));

    // Pulsing point light
    commands.spawn((
        PointLight {
            intensity: 1000.0,
            color: Color::WHITE,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 0.0),
        Name::new("PulsingLight"),
        PulsingLight {
            base_intensity: 1000.0,
            pulse_speed: 2.0,
        },
    ));

    // Static ground for reference
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(8.0, 8.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.6, 0.6),
            ..default()
        })),
        Transform::from_xyz(0.0, -2.0, 0.0),
        Name::new("Ground"),
    ));

    info!("Animated test scene initialized");
}

/// Component for objects that rotate
#[derive(Component)]
struct RotatingObject {
    speed: f32,
}

/// Component for objects that orbit around a center point
#[derive(Component)]
struct OrbitingObject {
    radius: f32,
    speed: f32,
    angle: f32,
}

/// Component for pulsing lights
#[derive(Component)]
struct PulsingLight {
    base_intensity: f32,
    pulse_speed: f32,
}

/// System to rotate objects
fn rotate_cubes(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &RotatingObject)>,
) {
    for (mut transform, rotating) in &mut query {
        transform.rotate_y(time.delta_secs() * rotating.speed);
        transform.rotate_x(time.delta_secs() * rotating.speed * 0.7);
    }
}

/// System to orbit objects
fn orbit_sphere(
    time: Res<Time>,
    mut query: Query<(&mut Transform, &mut OrbitingObject)>,
) {
    for (mut transform, mut orbiting) in &mut query {
        orbiting.angle += time.delta_secs() * orbiting.speed;
        let x = orbiting.radius * orbiting.angle.cos();
        let z = orbiting.radius * orbiting.angle.sin();
        transform.translation = Vec3::new(x, transform.translation.y, z);
    }
}

/// System to pulse light intensity
fn pulse_light(
    time: Res<Time>,
    mut query: Query<(&mut PointLight, &PulsingLight)>,
) {
    for (mut light, pulsing) in &mut query {
        let pulse = (time.elapsed_secs() * pulsing.pulse_speed).sin();
        light.intensity = pulsing.base_intensity * (1.0 + pulse * 0.3);
    }
}

/// Auto-exit system to prevent hanging in CI
fn auto_exit_system(
    time: Res<Time>,
    mut exit: EventWriter<AppExit>,
    mut timer: Local<Option<Timer>>,
) {
    if timer.is_none() {
        *timer = Some(Timer::new(Duration::from_secs(60), TimerMode::Once));
    }

    if let Some(ref mut t) = timer.as_mut() {
        t.tick(time.delta());
        if t.finished() {
            info!("Auto-exit timer finished - shutting down animated test game");
            exit.write(AppExit::Success);
        }
    }
}

/// Screenshot handler with enhanced timing support for animated scenes
fn screenshot_handler(
    In(params): In<Option<Value>>, 
    mut commands: Commands,
    time: Res<Time>,
) -> BrpResult {
    let path = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("./animated_test_screenshot.png")
        .to_string();

    let description = params
        .as_ref()
        .and_then(|p| p.get("description"))
        .and_then(|d| d.as_str())
        .unwrap_or("Animated E2E test screenshot");

    // Log timing information for animated captures
    let elapsed_time = time.elapsed_secs();
    info!(
        "Animated screenshot handler called at T+{:.2}s: {} -> {}", 
        elapsed_time, description, path
    );

    // Use Bevy's built-in screenshot system
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    
    Ok(serde_json::json!({
        "path": path,
        "success": true,
        "description": description,
        "game_time": elapsed_time,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }))
}
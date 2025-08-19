/// Performance Test Game Fixture
/// 
/// A specialized Bevy game designed to stress-test performance optimizations
/// with realistic ECS patterns and entity management scenarios.

use bevy::{
    prelude::*,
    remote::{RemotePlugin, BrpResult},
    app::AppExit,
    window::WindowPlugin,
    time::common_conditions::on_timer,
    ecs::schedule::ScheduleLabel,
    diagnostic::{FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin},
};
use serde_json::Value;
use std::time::Duration;

/// Run the performance test game
pub fn run_performance_test_game() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Performance Test Game".into(),
                resolution: (1024.0, 768.0).into(),
                position: WindowPosition::Centered(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .add_plugins((
            FrameTimeDiagnosticsPlugin,
            SystemInformationDiagnosticsPlugin,
        ))
        .add_plugins(
            RemotePlugin::default()
                .with_method("bevy_debugger/get_entity_count", get_entity_count)
                .with_method("bevy_debugger/spawn_entities", spawn_entities_handler)
                .with_method("bevy_debugger/despawn_entities", despawn_entities_handler)
                .with_method("bevy_debugger/get_performance_metrics", get_performance_metrics)
                .with_method("bevy_debugger/stress_test", stress_test_handler)
        )
        .init_resource::<EntitySpawnConfig>()
        .init_resource::<PerformanceMetrics>()
        .add_systems(Startup, setup_performance_test_scene)
        .add_systems(Update, (
            update_performance_metrics,
            entity_lifecycle_system,
            movement_system,
            collision_system,
            cleanup_despawned_entities,
            auto_exit_system,
        ))
        .add_systems(Update, (
            spawn_entities_periodically
                .run_if(on_timer(Duration::from_secs(2))),
            update_entity_stats
                .run_if(on_timer(Duration::from_millis(100))),
        ))
        .run();
}

/// Configuration for entity spawning
#[derive(Resource)]
struct EntitySpawnConfig {
    pub entities_per_wave: usize,
    pub max_entities: usize,
    pub spawn_enabled: bool,
    pub entity_types: Vec<EntityType>,
}

impl Default for EntitySpawnConfig {
    fn default() -> Self {
        Self {
            entities_per_wave: 10,
            max_entities: 1000,
            spawn_enabled: true,
            entity_types: vec![
                EntityType::SimpleEntity,
                EntityType::MovingEntity,
                EntityType::ComplexEntity,
            ],
        }
    }
}

/// Performance metrics tracking
#[derive(Resource, Default)]
struct PerformanceMetrics {
    pub frame_times: Vec<f32>,
    pub entity_counts: Vec<usize>,
    pub memory_usage: Vec<usize>,
    pub system_times: std::collections::HashMap<String, f32>,
}

/// Types of entities for testing different performance characteristics
#[derive(Clone)]
enum EntityType {
    SimpleEntity,    // Minimal components for baseline testing
    MovingEntity,    // Entities with movement for system stress testing
    ComplexEntity,   // Entities with many components for memory testing
}

/// Marker component for test entities
#[derive(Component)]
struct TestEntity {
    entity_type: String,
    created_at: f32,
    lifetime: Option<f32>,
}

/// Movement component for dynamic entities
#[derive(Component)]
struct Movement {
    velocity: Vec3,
    acceleration: Vec3,
    max_speed: f32,
}

/// Health component for complex entities
#[derive(Component)]
struct Health {
    current: f32,
    maximum: f32,
}

/// Combat component for complex entities
#[derive(Component)]
struct Combat {
    damage: f32,
    range: f32,
    cooldown: f32,
    last_attack: f32,
}

/// Inventory component for complex entities
#[derive(Component)]
struct Inventory {
    items: Vec<String>,
    capacity: usize,
}

/// Marker for entities that should be despawned
#[derive(Component)]
struct ScheduledForDespawn;

/// Set up the performance test scene
fn setup_performance_test_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    info!("Setting up performance test scene");

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(10.0, 10.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("PerformanceTestCamera"),
    ));

    // Lighting
    commands.spawn((
        PointLight {
            intensity: 1000.0,
            color: Color::WHITE,
            ..default()
        },
        Transform::from_xyz(5.0, 10.0, 5.0),
        Name::new("TestLight"),
    ));

    // Ground plane for reference
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.5, 0.5, 0.5),
            ..default()
        })),
        Transform::from_xyz(0.0, -1.0, 0.0),
        Name::new("GroundPlane"),
    ));

    // Spawn initial entities for baseline testing
    spawn_initial_entities(&mut commands, &mut meshes, &mut materials);

    info!("Performance test scene initialized");
}

/// Spawn initial test entities
fn spawn_initial_entities(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    // Spawn a variety of entity types for initial testing
    for i in 0..100 {
        let entity_type = match i % 3 {
            0 => EntityType::SimpleEntity,
            1 => EntityType::MovingEntity,
            _ => EntityType::ComplexEntity,
        };

        spawn_entity_of_type(commands, meshes, materials, entity_type, i as f32);
    }
}

/// Spawn a specific type of entity
fn spawn_entity_of_type(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    entity_type: EntityType,
    index: f32,
) {
    let position = Vec3::new(
        (index * 0.5) % 20.0 - 10.0,
        0.0,
        ((index * 0.3) as i32 / 20) as f32 * 2.0 - 10.0,
    );

    match entity_type {
        EntityType::SimpleEntity => {
            commands.spawn((
                Mesh3d(meshes.add(Sphere::new(0.2))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.8, 0.2),
                    ..default()
                })),
                Transform::from_translation(position),
                TestEntity {
                    entity_type: "simple".to_string(),
                    created_at: 0.0, // Will be updated by system
                    lifetime: Some(30.0), // 30 second lifetime
                },
                Name::new(format!("SimpleEntity_{}", index)),
            ));
        }
        EntityType::MovingEntity => {
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.4, 0.4, 0.4))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.8, 0.2, 0.2),
                    ..default()
                })),
                Transform::from_translation(position),
                TestEntity {
                    entity_type: "moving".to_string(),
                    created_at: 0.0,
                    lifetime: Some(45.0),
                },
                Movement {
                    velocity: Vec3::new(
                        (index * 0.1).sin() * 2.0,
                        0.0,
                        (index * 0.1).cos() * 2.0,
                    ),
                    acceleration: Vec3::ZERO,
                    max_speed: 5.0,
                },
                Name::new(format!("MovingEntity_{}", index)),
            ));
        }
        EntityType::ComplexEntity => {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.3, 0.8))),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(0.2, 0.2, 0.8),
                    ..default()
                })),
                Transform::from_translation(position),
                TestEntity {
                    entity_type: "complex".to_string(),
                    created_at: 0.0,
                    lifetime: Some(60.0),
                },
                Movement {
                    velocity: Vec3::new(
                        (index * 0.05).sin() * 1.0,
                        0.0,
                        (index * 0.05).cos() * 1.0,
                    ),
                    acceleration: Vec3::ZERO,
                    max_speed: 2.0,
                },
                Health {
                    current: 100.0,
                    maximum: 100.0,
                },
                Combat {
                    damage: 25.0,
                    range: 5.0,
                    cooldown: 1.0,
                    last_attack: 0.0,
                },
                Inventory {
                    items: vec!["sword".to_string(), "potion".to_string()],
                    capacity: 10,
                },
                Name::new(format!("ComplexEntity_{}", index)),
            ));
        }
    }
}

/// System to periodically spawn entities
fn spawn_entities_periodically(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    config: Res<EntitySpawnConfig>,
    query: Query<&TestEntity>,
    time: Res<Time>,
) {
    if !config.spawn_enabled {
        return;
    }

    let current_count = query.iter().count();
    if current_count >= config.max_entities {
        return;
    }

    let entities_to_spawn = (config.entities_per_wave).min(config.max_entities - current_count);
    
    for i in 0..entities_to_spawn {
        let entity_type = config.entity_types[i % config.entity_types.len()].clone();
        spawn_entity_of_type(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity_type,
            time.elapsed_secs() + i as f32,
        );
    }

    if entities_to_spawn > 0 {
        debug!("Spawned {} entities, total: {}", entities_to_spawn, current_count + entities_to_spawn);
    }
}

/// System to handle entity lifecycle (aging and despawning)
fn entity_lifecycle_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut TestEntity)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs();
    
    for (entity, mut test_entity) in query.iter_mut() {
        if test_entity.created_at == 0.0 {
            test_entity.created_at = current_time;
        }

        if let Some(lifetime) = test_entity.lifetime {
            let age = current_time - test_entity.created_at;
            if age > lifetime {
                commands.entity(entity).insert(ScheduledForDespawn);
            }
        }
    }
}

/// Movement system for moving entities
fn movement_system(
    mut query: Query<(&mut Transform, &mut Movement)>,
    time: Res<Time>,
) {
    for (mut transform, mut movement) in query.iter_mut() {
        // Apply velocity
        transform.translation += movement.velocity * time.delta_secs();

        // Apply acceleration
        movement.velocity += movement.acceleration * time.delta_secs();

        // Clamp to max speed
        if movement.velocity.length() > movement.max_speed {
            movement.velocity = movement.velocity.normalize() * movement.max_speed;
        }

        // Bounce off boundaries
        if transform.translation.x.abs() > 20.0 {
            movement.velocity.x *= -1.0;
            transform.translation.x = transform.translation.x.clamp(-20.0, 20.0);
        }
        if transform.translation.z.abs() > 20.0 {
            movement.velocity.z *= -1.0;
            transform.translation.z = transform.translation.z.clamp(-20.0, 20.0);
        }
    }
}

/// Simple collision system for performance testing
fn collision_system(
    mut query: Query<(&Transform, &mut Health), (With<TestEntity>, With<Health>)>,
) {
    let mut combinations = query.iter_combinations_mut();
    
    while let Some([(transform_a, mut health_a), (transform_b, mut health_b)]) = combinations.fetch_next() {
        let distance = transform_a.translation.distance(transform_b.translation);
        
        if distance < 1.0 {
            // Simple collision - reduce health
            health_a.current -= 0.1;
            health_b.current -= 0.1;
            
            if health_a.current <= 0.0 {
                health_a.current = 0.0;
            }
            if health_b.current <= 0.0 {
                health_b.current = 0.0;
            }
        }
    }
}

/// System to clean up despawned entities
fn cleanup_despawned_entities(
    mut commands: Commands,
    query: Query<Entity, With<ScheduledForDespawn>>,
) {
    let despawn_count = query.iter().count();
    
    for entity in query.iter() {
        commands.entity(entity).despawn_recursive();
    }
    
    if despawn_count > 0 {
        debug!("Despawned {} entities", despawn_count);
    }
}

/// System to update performance metrics
fn update_performance_metrics(
    mut metrics: ResMut<PerformanceMetrics>,
    time: Res<Time>,
    query: Query<&TestEntity>,
) {
    // Track frame time
    metrics.frame_times.push(time.delta_secs());
    if metrics.frame_times.len() > 1000 {
        metrics.frame_times.remove(0);
    }

    // Track entity count
    metrics.entity_counts.push(query.iter().count());
    if metrics.entity_counts.len() > 1000 {
        metrics.entity_counts.remove(0);
    }
}

/// System to update entity statistics
fn update_entity_stats(
    simple_query: Query<&TestEntity, (With<TestEntity>, Without<Movement>)>,
    moving_query: Query<&TestEntity, (With<Movement>, Without<Health>)>,
    complex_query: Query<&TestEntity, (With<Health>, With<Combat>)>,
) {
    let simple_count = simple_query.iter().count();
    let moving_count = moving_query.iter().count();
    let complex_count = complex_query.iter().count();
    
    debug!(
        "Entity stats - Simple: {}, Moving: {}, Complex: {}, Total: {}",
        simple_count,
        moving_count,
        complex_count,
        simple_count + moving_count + complex_count
    );
}

/// Auto-exit system for CI testing
fn auto_exit_system(
    time: Res<Time>,
    mut exit: EventWriter<AppExit>,
    mut timer: Local<Option<Timer>>,
) {
    if timer.is_none() {
        *timer = Some(Timer::new(Duration::from_secs(120), TimerMode::Once)); // 2 minute timeout
    }

    if let Some(ref mut t) = timer.as_mut() {
        t.tick(time.delta());
        if t.finished() {
            info!("Performance test auto-exit timeout reached");
            exit.send(AppExit::Success);
        }
    }
}

// BRP Remote Method Handlers

/// Get current entity count
fn get_entity_count(
    In(params): In<Option<Value>>,
    query: Query<&TestEntity>,
) -> BrpResult {
    let total_count = query.iter().count();
    
    let simple_count = query.iter().filter(|e| e.entity_type == "simple").count();
    let moving_count = query.iter().filter(|e| e.entity_type == "moving").count();
    let complex_count = query.iter().filter(|e| e.entity_type == "complex").count();

    Ok(serde_json::json!({
        "total": total_count,
        "simple": simple_count,
        "moving": moving_count,
        "complex": complex_count,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }))
}

/// Spawn entities on demand
fn spawn_entities_handler(
    In(params): In<Option<Value>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
) -> BrpResult {
    let count = params
        .as_ref()
        .and_then(|p| p.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(10) as usize;

    let entity_type_str = params
        .as_ref()
        .and_then(|p| p.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("simple");

    let entity_type = match entity_type_str {
        "simple" => EntityType::SimpleEntity,
        "moving" => EntityType::MovingEntity,
        "complex" => EntityType::ComplexEntity,
        _ => EntityType::SimpleEntity,
    };

    for i in 0..count {
        spawn_entity_of_type(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity_type.clone(),
            time.elapsed_secs() + i as f32,
        );
    }

    info!("Spawned {} {} entities", count, entity_type_str);

    Ok(serde_json::json!({
        "spawned": count,
        "type": entity_type_str,
        "success": true
    }))
}

/// Despawn entities on demand
fn despawn_entities_handler(
    In(params): In<Option<Value>>,
    mut commands: Commands,
    query: Query<(Entity, &TestEntity)>,
) -> BrpResult {
    let count = params
        .as_ref()
        .and_then(|p| p.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(10) as usize;

    let entity_type_filter = params
        .as_ref()
        .and_then(|p| p.get("type"))
        .and_then(|t| t.as_str());

    let mut despawned = 0;
    for (entity, test_entity) in query.iter().take(count) {
        if let Some(filter) = entity_type_filter {
            if test_entity.entity_type != filter {
                continue;
            }
        }
        
        commands.entity(entity).insert(ScheduledForDespawn);
        despawned += 1;
        
        if despawned >= count {
            break;
        }
    }

    info!("Scheduled {} entities for despawn", despawned);

    Ok(serde_json::json!({
        "scheduled_for_despawn": despawned,
        "success": true
    }))
}

/// Get performance metrics
fn get_performance_metrics(
    In(_params): In<Option<Value>>,
    metrics: Res<PerformanceMetrics>,
    time: Res<Time>,
) -> BrpResult {
    let avg_frame_time = if !metrics.frame_times.is_empty() {
        metrics.frame_times.iter().sum::<f32>() / metrics.frame_times.len() as f32
    } else {
        0.0
    };

    let current_entity_count = metrics.entity_counts.last().copied().unwrap_or(0);
    
    Ok(serde_json::json!({
        "average_frame_time": avg_frame_time,
        "fps": if avg_frame_time > 0.0 { 1.0 / avg_frame_time } else { 0.0 },
        "entity_count": current_entity_count,
        "frame_samples": metrics.frame_times.len(),
        "uptime": time.elapsed_secs(),
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }))
}

/// Stress test handler
fn stress_test_handler(
    In(params): In<Option<Value>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut spawn_config: ResMut<EntitySpawnConfig>,
    time: Res<Time>,
) -> BrpResult {
    let test_type = params
        .as_ref()
        .and_then(|p| p.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("entity_spawn");

    let intensity = params
        .as_ref()
        .and_then(|p| p.get("intensity"))
        .and_then(|i| i.as_u64())
        .unwrap_or(1) as usize;

    match test_type {
        "entity_spawn" => {
            let spawn_count = intensity * 50;
            for i in 0..spawn_count {
                let entity_type = match i % 3 {
                    0 => EntityType::SimpleEntity,
                    1 => EntityType::MovingEntity,
                    _ => EntityType::ComplexEntity,
                };
                spawn_entity_of_type(
                    &mut commands,
                    &mut meshes,
                    &mut materials,
                    entity_type,
                    time.elapsed_secs() + i as f32,
                );
            }
            
            Ok(serde_json::json!({
                "stress_test": "entity_spawn",
                "entities_spawned": spawn_count,
                "intensity": intensity,
                "success": true
            }))
        }
        "continuous_spawn" => {
            spawn_config.entities_per_wave = intensity * 10;
            spawn_config.max_entities = intensity * 1000;
            spawn_config.spawn_enabled = true;
            
            Ok(serde_json::json!({
                "stress_test": "continuous_spawn",
                "entities_per_wave": spawn_config.entities_per_wave,
                "max_entities": spawn_config.max_entities,
                "intensity": intensity,
                "success": true
            }))
        }
        _ => {
            Ok(serde_json::json!({
                "error": "Unknown stress test type",
                "available_types": ["entity_spawn", "continuous_spawn"],
                "success": false
            }))
        }
    }
}
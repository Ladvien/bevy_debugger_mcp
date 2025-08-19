/// Complex ECS Game Fixture
/// 
/// A sophisticated Bevy game with advanced ECS patterns, complex systems,
/// and realistic game mechanics for comprehensive debugging and optimization testing.

use bevy::{
    prelude::*,
    remote::{RemotePlugin, BrpResult},
    app::AppExit,
    window::WindowPlugin,
    time::common_conditions::on_timer,
    diagnostic::{FrameTimeDiagnosticsPlugin, SystemInformationDiagnosticsPlugin},
    ecs::system::SystemParam,
};
use serde_json::Value;
use std::time::Duration;
use std::collections::HashMap;

/// Run the complex ECS test game
pub fn run_complex_ecs_game() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Complex ECS Test Game".into(),
                resolution: (1280.0, 720.0).into(),
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
                .with_method("bevy_debugger/query_entities", query_entities_handler)
                .with_method("bevy_debugger/get_system_info", get_system_info)
                .with_method("bevy_debugger/spawn_complex_entity", spawn_complex_entity_handler)
                .with_method("bevy_debugger/run_simulation", run_simulation_handler)
                .with_method("bevy_debugger/get_world_state", get_world_state_handler)
        )
        .init_resource::<GameWorld>()
        .init_resource::<EconomyResource>()
        .init_resource::<CombatStats>()
        .init_resource::<SimulationConfig>()
        .add_event::<CombatEvent>()
        .add_event::<TradeEvent>()
        .add_event::<MovementEvent>()
        .add_systems(Startup, setup_complex_world)
        .add_systems(Update, (
            // Core gameplay systems
            ai_decision_system,
            movement_system,
            physics_system,
            combat_system,
            health_system,
            economy_system,
            relationship_system,
            
            // Management systems
            lifecycle_system,
            resource_management_system,
            event_processing_system,
            statistics_system,
            
            // Utility systems
            auto_exit_system,
        ))
        .add_systems(Update, (
            // Periodic systems
            spawn_random_entities
                .run_if(on_timer(Duration::from_secs(5))),
            economy_update
                .run_if(on_timer(Duration::from_secs(10))),
            world_events_system
                .run_if(on_timer(Duration::from_secs(15))),
        ))
        .run();
}

// Resources

#[derive(Resource)]
struct GameWorld {
    pub entities_spawned: usize,
    pub total_entities_created: usize,
    pub simulation_time: f32,
    pub world_events: Vec<String>,
}

impl Default for GameWorld {
    fn default() -> Self {
        Self {
            entities_spawned: 0,
            total_entities_created: 0,
            simulation_time: 0.0,
            world_events: Vec::new(),
        }
    }
}

#[derive(Resource, Default)]
struct EconomyResource {
    pub global_resources: HashMap<String, f32>,
    pub market_prices: HashMap<String, f32>,
    pub trade_volume: HashMap<String, f32>,
}

#[derive(Resource, Default)]
struct CombatStats {
    pub total_battles: usize,
    pub damage_dealt: f32,
    pub entities_defeated: usize,
}

#[derive(Resource)]
struct SimulationConfig {
    pub max_entities: usize,
    pub ai_complexity: f32,
    pub economy_enabled: bool,
    pub combat_enabled: bool,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            max_entities: 500,
            ai_complexity: 1.0,
            economy_enabled: true,
            combat_enabled: true,
        }
    }
}

// Events

#[derive(Event)]
struct CombatEvent {
    pub attacker: Entity,
    pub defender: Entity,
    pub damage: f32,
}

#[derive(Event)]
struct TradeEvent {
    pub trader: Entity,
    pub resource: String,
    pub amount: f32,
    pub price: f32,
}

#[derive(Event)]
struct MovementEvent {
    pub entity: Entity,
    pub from: Vec3,
    pub to: Vec3,
    pub reason: String,
}

// Components

#[derive(Component)]
struct ComplexEntity {
    pub entity_id: u64,
    pub entity_class: EntityClass,
    pub creation_time: f32,
    pub last_update: f32,
}

#[derive(Clone)]
enum EntityClass {
    Warrior,
    Merchant,
    Villager,
    Monster,
    Resource,
}

#[derive(Component)]
struct AIBehavior {
    pub behavior_type: BehaviorType,
    pub decision_weight: f32,
    pub last_decision_time: f32,
    pub target: Option<Entity>,
    pub goals: Vec<String>,
}

#[derive(Clone)]
enum BehaviorType {
    Aggressive,
    Defensive,
    Neutral,
    Trader,
    Explorer,
}

#[derive(Component)]
struct Position {
    pub current: Vec3,
    pub previous: Vec3,
    pub target: Option<Vec3>,
}

#[derive(Component)]
struct Velocity {
    pub linear: Vec3,
    pub angular: f32,
    pub max_speed: f32,
}

#[derive(Component)]
struct Health {
    pub current: f32,
    pub maximum: f32,
    pub regeneration_rate: f32,
    pub last_damage_time: f32,
}

#[derive(Component)]
struct Combat {
    pub attack_power: f32,
    pub defense: f32,
    pub range: f32,
    pub cooldown: f32,
    pub last_attack: f32,
    pub weapon_type: String,
}

#[derive(Component)]
struct Inventory {
    pub items: HashMap<String, f32>,
    pub capacity: f32,
    pub current_weight: f32,
    pub value: f32,
}

#[derive(Component)]
struct Economy {
    pub wealth: f32,
    pub trade_skill: f32,
    pub preferred_goods: Vec<String>,
    pub trade_history: Vec<TradeRecord>,
}

#[derive(Clone)]
struct TradeRecord {
    pub resource: String,
    pub amount: f32,
    pub price: f32,
    pub timestamp: f32,
}

#[derive(Component)]
struct Relationships {
    pub relations: HashMap<Entity, RelationshipData>,
    pub faction: String,
    pub reputation: f32,
}

#[derive(Clone)]
struct RelationshipData {
    pub trust: f32,
    pub hostility: f32,
    pub trade_affinity: f32,
    pub last_interaction: f32,
}

#[derive(Component)]
struct Lifecycle {
    pub age: f32,
    pub max_age: Option<f32>,
    pub energy: f32,
    pub needs: HashMap<String, f32>,
}

#[derive(Component)]
struct ResourceNode {
    pub resource_type: String,
    pub amount: f32,
    pub max_amount: f32,
    pub regeneration_rate: f32,
    pub extraction_difficulty: f32,
}

// Systems

/// Set up the complex world with various entity types
fn setup_complex_world(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<GameWorld>,
    mut economy: ResMut<EconomyResource>,
) {
    info!("Setting up complex ECS world");

    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(15.0, 15.0, 15.0).looking_at(Vec3::ZERO, Vec3::Y),
        Name::new("ComplexWorldCamera"),
    ));

    // Lighting setup
    commands.spawn((
        DirectionalLight {
            illuminance: 3000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.5, -0.5, 0.0)),
        Name::new("Sun"),
    ));

    // Initialize economy
    economy.global_resources.insert("gold".to_string(), 10000.0);
    economy.global_resources.insert("food".to_string(), 5000.0);
    economy.global_resources.insert("wood".to_string(), 8000.0);
    economy.global_resources.insert("stone".to_string(), 3000.0);
    
    economy.market_prices.insert("gold".to_string(), 1.0);
    economy.market_prices.insert("food".to_string(), 2.5);
    economy.market_prices.insert("wood".to_string(), 1.8);
    economy.market_prices.insert("stone".to_string(), 3.2);

    // Spawn initial entities
    spawn_entity_population(&mut commands, &mut meshes, &mut materials, &mut world);

    // Create resource nodes
    spawn_resource_nodes(&mut commands, &mut meshes, &mut materials);

    info!("Complex ECS world initialized with {} entities", world.entities_spawned);
}

/// Spawn a diverse population of entities
fn spawn_entity_population(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    world: &mut ResMut<GameWorld>,
) {
    let entity_counts = [
        (EntityClass::Warrior, 30),
        (EntityClass::Merchant, 20),
        (EntityClass::Villager, 40),
        (EntityClass::Monster, 15),
    ];

    for (entity_class, count) in entity_counts {
        for i in 0..count {
            spawn_complex_entity_of_class(
                commands,
                meshes,
                materials,
                entity_class.clone(),
                i,
                world,
            );
        }
    }
}

/// Spawn resource nodes around the world
fn spawn_resource_nodes(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
) {
    let resource_types = [
        ("gold_mine", Color::srgb(1.0, 1.0, 0.0), 1000.0),
        ("forest", Color::srgb(0.0, 0.8, 0.0), 2000.0),
        ("quarry", Color::srgb(0.6, 0.6, 0.6), 1500.0),
        ("farm", Color::srgb(0.8, 0.4, 0.0), 3000.0),
    ];

    for (i, (resource_type, color, amount)) in resource_types.iter().enumerate() {
        let position = Vec3::new(
            (i as f32 * 8.0 - 12.0),
            0.0,
            (i as f32 * 6.0 - 9.0),
        );

        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(1.5, 0.5))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: *color,
                ..default()
            })),
            Transform::from_translation(position),
            ResourceNode {
                resource_type: resource_type.to_string(),
                amount: *amount,
                max_amount: *amount,
                regeneration_rate: amount * 0.01, // 1% per update
                extraction_difficulty: 1.0,
            },
            Name::new(format!("ResourceNode_{}", resource_type)),
        ));
    }
}

/// Spawn a complex entity of a specific class
fn spawn_complex_entity_of_class(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    entity_class: EntityClass,
    index: usize,
    world: &mut ResMut<GameWorld>,
) {
    let position = Vec3::new(
        (index as f32 * 2.0) % 20.0 - 10.0,
        0.0,
        (index / 10) as f32 * 3.0 - 6.0,
    );

    let (mesh, color, behavior, combat_stats, inventory_items, faction) = match entity_class {
        EntityClass::Warrior => (
            meshes.add(Cuboid::new(0.8, 1.6, 0.8)),
            Color::srgb(0.8, 0.2, 0.2),
            BehaviorType::Aggressive,
            (50.0, 30.0), // attack, defense
            vec![("sword".to_string(), 1.0), ("shield".to_string(), 1.0)],
            "warriors",
        ),
        EntityClass::Merchant => (
            meshes.add(Sphere::new(0.6)),
            Color::srgb(0.2, 0.8, 0.2),
            BehaviorType::Trader,
            (10.0, 5.0),
            vec![("gold".to_string(), 100.0), ("goods".to_string(), 50.0)],
            "merchants",
        ),
        EntityClass::Villager => (
            meshes.add(Capsule3d::new(0.4, 1.2)),
            Color::srgb(0.6, 0.4, 0.2),
            BehaviorType::Neutral,
            (15.0, 10.0),
            vec![("food".to_string(), 10.0), ("tools".to_string(), 2.0)],
            "villagers",
        ),
        EntityClass::Monster => (
            meshes.add(Sphere::new(0.8)),
            Color::srgb(0.8, 0.0, 0.8),
            BehaviorType::Aggressive,
            (40.0, 20.0),
            vec![("claws".to_string(), 1.0)],
            "monsters",
        ),
        EntityClass::Resource => (
            meshes.add(Cylinder::new(0.3, 0.6)),
            Color::srgb(0.5, 0.5, 0.5),
            BehaviorType::Neutral,
            (0.0, 100.0),
            vec![],
            "resources",
        ),
    };

    let mut inventory = Inventory {
        items: HashMap::new(),
        capacity: 100.0,
        current_weight: 0.0,
        value: 0.0,
    };

    for (item, amount) in inventory_items {
        inventory.items.insert(item, amount);
        inventory.current_weight += amount;
        inventory.value += amount * 10.0; // Simple value calculation
    }

    let entity_id = world.total_entities_created as u64;
    world.total_entities_created += 1;
    world.entities_spawned += 1;

    commands.spawn((
        Mesh3d(mesh),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: color,
            ..default()
        })),
        Transform::from_translation(position),
        ComplexEntity {
            entity_id,
            entity_class: entity_class.clone(),
            creation_time: 0.0, // Will be set by system
            last_update: 0.0,
        },
        AIBehavior {
            behavior_type: behavior,
            decision_weight: rand::random::<f32>(),
            last_decision_time: 0.0,
            target: None,
            goals: generate_goals_for_class(&entity_class),
        },
        Position {
            current: position,
            previous: position,
            target: None,
        },
        Velocity {
            linear: Vec3::ZERO,
            angular: 0.0,
            max_speed: 5.0,
        },
        Health {
            current: 100.0,
            maximum: 100.0,
            regeneration_rate: 1.0,
            last_damage_time: 0.0,
        },
        Combat {
            attack_power: combat_stats.0,
            defense: combat_stats.1,
            range: 2.0,
            cooldown: 1.0,
            last_attack: 0.0,
            weapon_type: inventory.items.keys().next().unwrap_or(&"fists".to_string()).clone(),
        },
        inventory,
        Economy {
            wealth: rand::random::<f32>() * 100.0,
            trade_skill: rand::random::<f32>(),
            preferred_goods: vec!["food".to_string(), "gold".to_string()],
            trade_history: Vec::new(),
        },
        Relationships {
            relations: HashMap::new(),
            faction: faction.to_string(),
            reputation: 0.0,
        },
        Lifecycle {
            age: 0.0,
            max_age: Some(rand::random::<f32>() * 300.0 + 100.0), // 100-400 seconds
            energy: 100.0,
            needs: HashMap::from([
                ("hunger".to_string(), rand::random::<f32>() * 50.0),
                ("rest".to_string(), rand::random::<f32>() * 30.0),
            ]),
        },
        Name::new(format!("{:?}_{}", entity_class, entity_id)),
    ));
}

/// Generate goals based on entity class
fn generate_goals_for_class(entity_class: &EntityClass) -> Vec<String> {
    match entity_class {
        EntityClass::Warrior => vec![
            "find enemies".to_string(),
            "protect allies".to_string(),
            "improve combat skills".to_string(),
        ],
        EntityClass::Merchant => vec![
            "find trade opportunities".to_string(),
            "maximize profit".to_string(),
            "build trade networks".to_string(),
        ],
        EntityClass::Villager => vec![
            "gather resources".to_string(),
            "maintain health".to_string(),
            "build relationships".to_string(),
        ],
        EntityClass::Monster => vec![
            "hunt prey".to_string(),
            "defend territory".to_string(),
            "survive".to_string(),
        ],
        EntityClass::Resource => vec![],
    }
}

// Complex Systems

/// AI decision-making system
fn ai_decision_system(
    mut query: Query<(
        Entity,
        &mut AIBehavior,
        &mut Position,
        &Health,
        &Inventory,
        &Economy,
        &Relationships,
    )>,
    time: Res<Time>,
    config: Res<SimulationConfig>,
) {
    let current_time = time.elapsed_secs();
    let decision_interval = 2.0 / config.ai_complexity; // More complex AI decides more frequently

    for (entity, mut ai, mut position, health, inventory, economy, relationships) in query.iter_mut() {
        if current_time - ai.last_decision_time < decision_interval {
            continue;
        }

        ai.last_decision_time = current_time;

        // Simple AI decision tree
        match ai.behavior_type {
            BehaviorType::Aggressive => {
                // Look for targets or move to combat
                if health.current > health.maximum * 0.3 {
                    // Find a new target position
                    position.target = Some(Vec3::new(
                        rand::random::<f32>() * 20.0 - 10.0,
                        0.0,
                        rand::random::<f32>() * 20.0 - 10.0,
                    ));
                }
            }
            BehaviorType::Trader => {
                // Look for trade opportunities
                if economy.wealth < 50.0 {
                    // Move towards resource areas
                    position.target = Some(Vec3::new(
                        rand::random::<f32>() * 16.0 - 8.0,
                        0.0,
                        rand::random::<f32>() * 12.0 - 6.0,
                    ));
                }
            }
            BehaviorType::Neutral | BehaviorType::Defensive => {
                // Wander or move to safety
                if health.current < health.maximum * 0.5 {
                    // Move away from center (safety)
                    let away_direction = position.current.normalize() * 2.0;
                    position.target = Some(position.current + away_direction);
                }
            }
            BehaviorType::Explorer => {
                // Constantly move to new areas
                position.target = Some(Vec3::new(
                    rand::random::<f32>() * 30.0 - 15.0,
                    0.0,
                    rand::random::<f32>() * 30.0 - 15.0,
                ));
            }
        }
    }
}

/// Movement system with pathfinding
fn movement_system(
    mut query: Query<(&mut Transform, &mut Position, &mut Velocity)>,
    time: Res<Time>,
) {
    for (mut transform, mut position, mut velocity) in query.iter_mut() {
        position.previous = position.current;
        position.current = transform.translation;

        if let Some(target) = position.target {
            let direction = (target - position.current).normalize();
            let distance = position.current.distance(target);

            if distance > 0.5 {
                // Move towards target
                velocity.linear = direction * velocity.max_speed;
                transform.translation += velocity.linear * time.delta_secs();
                position.current = transform.translation;
            } else {
                // Reached target
                position.target = None;
                velocity.linear *= 0.8; // Decelerate
            }
        } else {
            // No target, gradually stop
            velocity.linear *= 0.95;
            transform.translation += velocity.linear * time.delta_secs();
            position.current = transform.translation;
        }

        // Keep entities in bounds
        if transform.translation.x.abs() > 25.0 {
            transform.translation.x = transform.translation.x.clamp(-25.0, 25.0);
            velocity.linear.x *= -0.5;
        }
        if transform.translation.z.abs() > 25.0 {
            transform.translation.z = transform.translation.z.clamp(-25.0, 25.0);
            velocity.linear.z *= -0.5;
        }
    }
}

/// Physics system for realistic interactions
fn physics_system(
    mut query: Query<(&Transform, &mut Velocity), (With<ComplexEntity>, Changed<Transform>)>,
) {
    // Simple physics: apply friction and gravity effects
    for (_transform, mut velocity) in query.iter_mut() {
        // Apply friction
        velocity.linear *= 0.98;
        velocity.angular *= 0.95;

        // Limit velocities
        if velocity.linear.length() > velocity.max_speed {
            velocity.linear = velocity.linear.normalize() * velocity.max_speed;
        }
    }
}

/// Combat system with complex interactions
fn combat_system(
    mut query: Query<(Entity, &Transform, &mut Health, &mut Combat, &AIBehavior)>,
    mut combat_events: EventWriter<CombatEvent>,
    mut combat_stats: ResMut<CombatStats>,
    time: Res<Time>,
    config: Res<SimulationConfig>,
) {
    if !config.combat_enabled {
        return;
    }

    let current_time = time.elapsed_secs();
    let mut combatants: Vec<_> = query.iter_mut().collect();

    for i in 0..combatants.len() {
        let (attacker_entity, attacker_transform, _, mut attacker_combat, attacker_ai) = &mut combatants[i];
        
        if current_time - attacker_combat.last_attack < attacker_combat.cooldown {
            continue;
        }

        // Find targets in range
        for j in 0..combatants.len() {
            if i == j {
                continue;
            }

            let (defender_entity, defender_transform, mut defender_health, _, defender_ai) = &mut combatants[j];
            let distance = attacker_transform.translation.distance(defender_transform.translation);

            if distance <= attacker_combat.range {
                // Check if should attack based on AI behavior and relationships
                let should_attack = match (&attacker_ai.behavior_type, &defender_ai.behavior_type) {
                    (BehaviorType::Aggressive, _) => true,
                    (_, BehaviorType::Aggressive) if matches!(attacker_ai.behavior_type, BehaviorType::Defensive) => true,
                    (BehaviorType::Trader, BehaviorType::Trader) => false, // Traders don't fight each other
                    _ => rand::random::<f32>() < 0.1, // 10% chance of random conflict
                };

                if should_attack && defender_health.current > 0.0 {
                    // Calculate damage
                    let base_damage = attacker_combat.attack_power;
                    let damage_reduction = defender_health.current.min(attacker_combat.attack_power * 0.1);
                    let final_damage = (base_damage - damage_reduction).max(1.0);

                    // Apply damage
                    defender_health.current = (defender_health.current - final_damage).max(0.0);
                    defender_health.last_damage_time = current_time;
                    attacker_combat.last_attack = current_time;

                    // Record combat event
                    combat_events.send(CombatEvent {
                        attacker: *attacker_entity,
                        defender: *defender_entity,
                        damage: final_damage,
                    });

                    combat_stats.total_battles += 1;
                    combat_stats.damage_dealt += final_damage;

                    if defender_health.current <= 0.0 {
                        combat_stats.entities_defeated += 1;
                    }

                    break; // Only attack one target per turn
                }
            }
        }
    }
}

/// Health system with regeneration and death handling
fn health_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Health, &mut Lifecycle)>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs();

    for (entity, mut health, mut lifecycle) in query.iter_mut() {
        // Health regeneration
        if current_time - health.last_damage_time > 5.0 && health.current > 0.0 {
            health.current = (health.current + health.regeneration_rate * time.delta_secs())
                .min(health.maximum);
        }

        // Handle death
        if health.current <= 0.0 {
            lifecycle.energy = 0.0;
            // Mark for despawn (will be handled by lifecycle system)
        }
    }
}

/// Economy system with trading and resource management
fn economy_system(
    mut query: Query<(Entity, &Transform, &mut Economy, &mut Inventory, &AIBehavior)>,
    mut trade_events: EventWriter<TradeEvent>,
    mut economy_resource: ResMut<EconomyResource>,
    time: Res<Time>,
    config: Res<SimulationConfig>,
) {
    if !config.economy_enabled {
        return;
    }

    let traders: Vec<_> = query.iter_mut().collect();
    
    for i in 0..traders.len() {
        let (trader_entity, trader_transform, mut trader_economy, mut trader_inventory, trader_ai) = &traders[i];
        
        if !matches!(trader_ai.behavior_type, BehaviorType::Trader) {
            continue;
        }

        // Look for trade partners nearby
        for j in 0..traders.len() {
            if i == j {
                continue;
            }

            let (partner_entity, partner_transform, mut partner_economy, mut partner_inventory, _) = &traders[j];
            let distance = trader_transform.translation.distance(partner_transform.translation);

            if distance <= 3.0 && rand::random::<f32>() < 0.1 {
                // Attempt trade
                if let Some(trade_good) = trader_economy.preferred_goods.first().cloned() {
                    if let Some(partner_amount) = partner_inventory.items.get(&trade_good).copied() {
                        if partner_amount > 0.0 && trader_economy.wealth > 10.0 {
                            let trade_amount = (partner_amount * 0.1).min(10.0);
                            let price = economy_resource.market_prices
                                .get(&trade_good)
                                .copied()
                                .unwrap_or(1.0);
                            let total_cost = trade_amount * price;

                            if trader_economy.wealth >= total_cost {
                                // Execute trade
                                trader_economy.wealth -= total_cost;
                                // partner_economy.wealth += total_cost; // Would need mutable access
                                
                                // Update inventories
                                trader_inventory.items.entry(trade_good.clone())
                                    .and_modify(|amount| *amount += trade_amount)
                                    .or_insert(trade_amount);
                                
                                // Record trade
                                trade_events.send(TradeEvent {
                                    trader: *trader_entity,
                                    resource: trade_good.clone(),
                                    amount: trade_amount,
                                    price,
                                });

                                trader_economy.trade_history.push(TradeRecord {
                                    resource: trade_good,
                                    amount: trade_amount,
                                    price,
                                    timestamp: time.elapsed_secs(),
                                });

                                break; // Only one trade per update
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Relationship system for social interactions
fn relationship_system(
    mut query: Query<(Entity, &Transform, &mut Relationships, &AIBehavior)>,
    combat_events: EventReader<CombatEvent>,
    trade_events: EventReader<TradeEvent>,
) {
    // Update relationships based on recent events
    // This would be more complex in a real game, but demonstrates the concept
    for (entity, _transform, mut relationships, _ai) in query.iter_mut() {
        // Decay relationships over time
        for (_other_entity, relationship_data) in relationships.relations.iter_mut() {
            relationship_data.trust *= 0.999;
            relationship_data.hostility *= 0.998;
        }

        // Update reputation based on recent actions
        relationships.reputation *= 0.999; // Slow reputation decay
    }
}

/// Lifecycle system for aging and death
fn lifecycle_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Lifecycle, &ComplexEntity)>,
    mut world: ResMut<GameWorld>,
    time: Res<Time>,
) {
    let current_time = time.elapsed_secs();
    world.simulation_time = current_time;

    for (entity, mut lifecycle, complex_entity) in query.iter_mut() {
        lifecycle.age = current_time - complex_entity.creation_time;

        // Age-based energy reduction
        lifecycle.energy -= time.delta_secs() * 0.5;

        // Handle needs
        for (_need_name, need_value) in lifecycle.needs.iter_mut() {
            *need_value += time.delta_secs() * rand::random::<f32>(); // Needs increase over time
        }

        // Check for death conditions
        let should_die = lifecycle.energy <= 0.0 
            || lifecycle.max_age.map_or(false, |max_age| lifecycle.age >= max_age)
            || lifecycle.needs.values().any(|&need| need > 100.0);

        if should_die {
            commands.entity(entity).despawn_recursive();
            world.entities_spawned = world.entities_spawned.saturating_sub(1);
        }
    }
}

/// Resource management system
fn resource_management_system(
    mut resource_query: Query<&mut ResourceNode>,
    time: Res<Time>,
) {
    for mut resource in resource_query.iter_mut() {
        // Regenerate resources
        if resource.amount < resource.max_amount {
            resource.amount = (resource.amount + resource.regeneration_rate * time.delta_secs())
                .min(resource.max_amount);
        }
    }
}

/// Event processing system
fn event_processing_system(
    mut combat_events: EventReader<CombatEvent>,
    mut trade_events: EventReader<TradeEvent>,
    mut movement_events: EventReader<MovementEvent>,
    mut world: ResMut<GameWorld>,
) {
    // Process combat events
    for event in combat_events.read() {
        world.world_events.push(format!(
            "Combat: {:?} dealt {:.1} damage to {:?}",
            event.attacker, event.damage, event.defender
        ));
    }

    // Process trade events
    for event in trade_events.read() {
        world.world_events.push(format!(
            "Trade: {:?} traded {:.1} {} for {:.2} gold",
            event.trader, event.amount, event.resource, event.price
        ));
    }

    // Process movement events
    for event in movement_events.read() {
        world.world_events.push(format!(
            "Movement: {:?} moved from {:?} to {:?} ({})",
            event.entity, event.from, event.to, event.reason
        ));
    }

    // Keep only recent events
    if world.world_events.len() > 100 {
        world.world_events.drain(0..world.world_events.len() - 100);
    }
}

/// Statistics system for performance monitoring
fn statistics_system(
    entity_query: Query<&ComplexEntity>,
    health_query: Query<&Health>,
    combat_query: Query<&Combat>,
    economy_query: Query<&Economy>,
    mut world: ResMut<GameWorld>,
) {
    // Update entity count
    world.entities_spawned = entity_query.iter().count();

    // Could collect more detailed statistics here
    // This demonstrates how complex queries can impact performance
    let healthy_entities = health_query.iter()
        .filter(|h| h.current > h.maximum * 0.5)
        .count();
    
    let wealthy_entities = economy_query.iter()
        .filter(|e| e.wealth > 50.0)
        .count();

    if world.entities_spawned > 0 {
        debug!(
            "World stats - Total: {}, Healthy: {}, Wealthy: {}",
            world.entities_spawned, healthy_entities, wealthy_entities
        );
    }
}

/// Spawn random entities periodically
fn spawn_random_entities(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<GameWorld>,
    config: Res<SimulationConfig>,
) {
    if world.entities_spawned >= config.max_entities {
        return;
    }

    let spawn_count = rand::random::<usize>() % 3 + 1; // 1-3 entities
    let entity_classes = [
        EntityClass::Warrior,
        EntityClass::Merchant,
        EntityClass::Villager,
        EntityClass::Monster,
    ];

    for i in 0..spawn_count {
        if world.entities_spawned >= config.max_entities {
            break;
        }

        let entity_class = entity_classes[rand::random::<usize>() % entity_classes.len()].clone();
        spawn_complex_entity_of_class(
            &mut commands,
            &mut meshes,
            &mut materials,
            entity_class,
            world.total_entities_created,
            &mut world,
        );
    }
}

/// Economy update system
fn economy_update(
    mut economy: ResMut<EconomyResource>,
) {
    // Update market prices based on supply and demand
    for (_resource, price) in economy.market_prices.iter_mut() {
        *price *= 1.0 + (rand::random::<f32>() - 0.5) * 0.1; // ±5% price fluctuation
        *price = price.max(0.1); // Minimum price
    }

    // Reset trade volumes
    economy.trade_volume.clear();
}

/// World events system
fn world_events_system(
    mut world: ResMut<GameWorld>,
    query: Query<&ComplexEntity>,
) {
    // Generate random world events
    let event_types = [
        "A merchant caravan arrived",
        "Strange weather patterns observed",
        "Resource deposits discovered",
        "Peaceful negotiations between factions",
        "Festival celebrations begin",
    ];

    if rand::random::<f32>() < 0.3 {
        let event = event_types[rand::random::<usize>() % event_types.len()];
        world.world_events.push(format!("World Event: {}", event));
    }
}

/// Auto-exit system for testing
fn auto_exit_system(
    time: Res<Time>,
    mut exit: EventWriter<AppExit>,
    mut timer: Local<Option<Timer>>,
) {
    if timer.is_none() {
        *timer = Some(Timer::new(Duration::from_secs(300), TimerMode::Once)); // 5 minute timeout
    }

    if let Some(ref mut t) = timer.as_mut() {
        t.tick(time.delta());
        if t.finished() {
            info!("Complex ECS game auto-exit timeout reached");
            exit.send(AppExit::Success);
        }
    }
}

// BRP Remote Method Handlers

/// Query entities with complex filters
fn query_entities_handler(
    In(params): In<Option<Value>>,
    entity_query: Query<(Entity, &ComplexEntity, &Health, &Economy, &Transform)>,
) -> BrpResult {
    let entity_type = params
        .as_ref()
        .and_then(|p| p.get("type"))
        .and_then(|t| t.as_str());

    let min_health = params
        .as_ref()
        .and_then(|p| p.get("min_health"))
        .and_then(|h| h.as_f64())
        .unwrap_or(0.0) as f32;

    let min_wealth = params
        .as_ref()
        .and_then(|p| p.get("min_wealth"))
        .and_then(|w| w.as_f64())
        .unwrap_or(0.0) as f32;

    let mut results = Vec::new();

    for (entity, complex_entity, health, economy, transform) in entity_query.iter() {
        // Apply filters
        if let Some(filter_type) = entity_type {
            let entity_type_str = format!("{:?}", complex_entity.entity_class).to_lowercase();
            if !entity_type_str.contains(filter_type) {
                continue;
            }
        }

        if health.current < min_health || economy.wealth < min_wealth {
            continue;
        }

        results.push(serde_json::json!({
            "entity_id": format!("{:?}", entity),
            "class": format!("{:?}", complex_entity.entity_class),
            "health": {
                "current": health.current,
                "maximum": health.maximum
            },
            "wealth": economy.wealth,
            "position": {
                "x": transform.translation.x,
                "y": transform.translation.y,
                "z": transform.translation.z
            },
            "age": complex_entity.last_update
        }));
    }

    Ok(serde_json::json!({
        "entities": results,
        "total_found": results.len(),
        "filters_applied": {
            "entity_type": entity_type,
            "min_health": min_health,
            "min_wealth": min_wealth
        }
    }))
}

/// Get system performance information
fn get_system_info(
    In(_params): In<Option<Value>>,
    world: Res<GameWorld>,
    combat_stats: Res<CombatStats>,
    economy: Res<EconomyResource>,
    config: Res<SimulationConfig>,
) -> BrpResult {
    Ok(serde_json::json!({
        "world": {
            "entities_spawned": world.entities_spawned,
            "total_entities_created": world.total_entities_created,
            "simulation_time": world.simulation_time,
            "recent_events": world.world_events.len()
        },
        "combat": {
            "total_battles": combat_stats.total_battles,
            "damage_dealt": combat_stats.damage_dealt,
            "entities_defeated": combat_stats.entities_defeated
        },
        "economy": {
            "global_resources": economy.global_resources,
            "market_prices": economy.market_prices
        },
        "config": {
            "max_entities": config.max_entities,
            "ai_complexity": config.ai_complexity,
            "economy_enabled": config.economy_enabled,
            "combat_enabled": config.combat_enabled
        }
    }))
}

/// Spawn a complex entity on demand
fn spawn_complex_entity_handler(
    In(params): In<Option<Value>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<GameWorld>,
) -> BrpResult {
    let entity_type = params
        .as_ref()
        .and_then(|p| p.get("type"))
        .and_then(|t| t.as_str())
        .unwrap_or("villager");

    let entity_class = match entity_type {
        "warrior" => EntityClass::Warrior,
        "merchant" => EntityClass::Merchant,
        "monster" => EntityClass::Monster,
        _ => EntityClass::Villager,
    };

    spawn_complex_entity_of_class(
        &mut commands,
        &mut meshes,
        &mut materials,
        entity_class,
        world.total_entities_created,
        &mut world,
    );

    Ok(serde_json::json!({
        "spawned": true,
        "entity_type": entity_type,
        "entity_id": world.total_entities_created - 1
    }))
}

/// Run simulation scenarios
fn run_simulation_handler(
    In(params): In<Option<Value>>,
    mut config: ResMut<SimulationConfig>,
) -> BrpResult {
    let scenario = params
        .as_ref()
        .and_then(|p| p.get("scenario"))
        .and_then(|s| s.as_str())
        .unwrap_or("default");

    match scenario {
        "high_activity" => {
            config.ai_complexity = 2.0;
            config.max_entities = 1000;
            config.economy_enabled = true;
            config.combat_enabled = true;
        }
        "peaceful" => {
            config.ai_complexity = 0.5;
            config.combat_enabled = false;
            config.economy_enabled = true;
        }
        "combat_heavy" => {
            config.ai_complexity = 1.5;
            config.combat_enabled = true;
            config.economy_enabled = false;
            config.max_entities = 200;
        }
        _ => {
            config.ai_complexity = 1.0;
            config.max_entities = 500;
            config.economy_enabled = true;
            config.combat_enabled = true;
        }
    }

    Ok(serde_json::json!({
        "simulation_updated": true,
        "scenario": scenario,
        "config": {
            "ai_complexity": config.ai_complexity,
            "max_entities": config.max_entities,
            "economy_enabled": config.economy_enabled,
            "combat_enabled": config.combat_enabled
        }
    }))
}

/// Get comprehensive world state
fn get_world_state_handler(
    In(_params): In<Option<Value>>,
    world: Res<GameWorld>,
    entity_query: Query<&ComplexEntity>,
    health_query: Query<&Health>,
    combat_stats: Res<CombatStats>,
) -> BrpResult {
    let entity_classes: std::collections::HashMap<String, usize> = entity_query
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, entity| {
            let class_name = format!("{:?}", entity.entity_class);
            *acc.entry(class_name).or_insert(0) += 1;
            acc
        });

    let health_stats = health_query.iter().fold(
        (0, 0.0, 0.0, 0.0),
        |(count, total, min, max), health| {
            (
                count + 1,
                total + health.current,
                if count == 0 { health.current } else { min.min(health.current) },
                max.max(health.current),
            )
        },
    );

    Ok(serde_json::json!({
        "world_state": {
            "simulation_time": world.simulation_time,
            "entities_by_class": entity_classes,
            "total_entities": world.entities_spawned,
            "health_statistics": {
                "average": if health_stats.0 > 0 { health_stats.1 / health_stats.0 as f32 } else { 0.0 },
                "minimum": health_stats.2,
                "maximum": health_stats.3,
                "entities_with_health": health_stats.0
            },
            "combat_statistics": {
                "total_battles": combat_stats.total_battles,
                "entities_defeated": combat_stats.entities_defeated,
                "average_damage_per_battle": if combat_stats.total_battles > 0 {
                    combat_stats.damage_dealt / combat_stats.total_battles as f32
                } else {
                    0.0
                }
            },
            "recent_events": world.world_events.iter().rev().take(10).collect::<Vec<_>>()
        }
    }))
}
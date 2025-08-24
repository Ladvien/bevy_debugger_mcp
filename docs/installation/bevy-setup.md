# Bevy Game Setup Guide

*Complete guide to integrating the Bevy Debugger MCP with your Bevy game*

## Overview

This guide walks you through setting up the Bevy Remote Protocol (BRP) in your Bevy game to work with the Bevy Debugger MCP server. The setup is minimal - typically just one line of code - but this guide covers all the details and advanced configuration options.

## Prerequisites

- Bevy 0.14+ game project
- Rust 1.70+
- Basic familiarity with Bevy ECS and plugins

## Basic Setup (2 minutes)

### Step 1: Add RemotePlugin to Your Game

The simplest setup requires just adding one plugin to your Bevy app:

```rust
use bevy::prelude::*;
use bevy::remote::RemotePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default())  // ← Add this line
        .add_systems(Startup, setup)
        .add_systems(Update, game_systems)
        .run();
}

fn setup(mut commands: Commands) {
    // Your game setup code
    commands.spawn(Camera3dBundle::default());
}

fn game_systems(/* your systems */) {
    // Your game logic
}
```

### Step 2: Verify It Works

1. **Run your game**: `cargo run`
2. **Check BRP is active**: You should see a log message like:
   ```
   INFO bevy_remote: Starting BRP server on localhost:15702
   ```
3. **Test the connection**:
   ```bash
   curl -X POST http://localhost:15702/bevy/list_entities
   ```

If you see JSON output with your game's entities, the setup is complete!

## Configuration Options

### Custom Port Configuration

If port 15702 is already in use, you can configure a different port:

```rust
use bevy::remote::{RemotePlugin, BrpConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin {
            config: BrpConfig {
                port: 15703,  // Use different port
                ..default()
            }
        })
        .run();
}
```

Don't forget to update the MCP server configuration:
```bash
export BEVY_BRP_PORT=15703
```

### Network Binding Options

For remote debugging or deployment scenarios:

```rust
use bevy::remote::{RemotePlugin, BrpConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin {
            config: BrpConfig {
                address: "0.0.0.0".to_string(),  // Bind to all interfaces
                port: 15702,
                ..default()
            }
        })
        .run();
}
```

**Security Note**: Binding to `0.0.0.0` allows external connections. Only use this in trusted environments.

### Debug vs Release Configuration

Use different settings for development vs production:

```rust
use bevy::remote::{RemotePlugin, BrpConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins({
            #[cfg(debug_assertions)]
            {
                RemotePlugin {
                    config: BrpConfig {
                        address: "localhost".to_string(),
                        port: 15702,
                        ..default()
                    }
                }
            }
            #[cfg(not(debug_assertions))]
            {
                // Disable in release builds for security
                RemotePlugin::disabled()
            }
        })
        .run();
}
```

### Conditional Plugin Loading

Load the plugin only when debugging is needed:

```rust
use bevy::remote::RemotePlugin;

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    
    // Enable debugging with environment variable
    if std::env::var("BEVY_DEBUG").is_ok() {
        app.add_plugins(RemotePlugin::default());
    }
    
    app.add_systems(Startup, setup)
       .run();
}
```

Then run with: `BEVY_DEBUG=1 cargo run`

## Game-Specific Integration

### Component Setup for Better Debugging

Add debug-friendly components to make debugging easier:

```rust
use bevy::prelude::*;

#[derive(Component, Debug, Clone)]
pub struct Player {
    pub health: f32,
    pub speed: f32,
    pub level: u32,
}

#[derive(Component, Debug)]
pub struct DebugName(pub String);

#[derive(Component, Debug)]
pub struct Performance {
    pub frame_count: u64,
    pub last_frame_time: f32,
}

fn setup(mut commands: Commands) {
    // Spawn player with debug-friendly components
    commands.spawn((
        Transform::from_translation(Vec3::new(0.0, 0.0, 0.0)),
        Player { health: 100.0, speed: 5.0, level: 1 },
        DebugName("Main Player".to_string()),
    ));
    
    // Spawn camera with debug name
    commands.spawn((
        Camera3dBundle::default(),
        DebugName("Main Camera".to_string()),
    ));
}
```

### Debug-Only Systems

Add systems that only run in debug mode to help with debugging:

```rust
use bevy::prelude::*;

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        #[cfg(debug_assertions)]
        {
            app.add_systems(Update, (
                debug_entity_counter,
                debug_performance_monitor,
                debug_player_status,
            ));
        }
    }
}

#[cfg(debug_assertions)]
fn debug_entity_counter(
    mut commands: Commands,
    query: Query<Entity>,
    mut last_count: Local<usize>,
) {
    let current_count = query.iter().count();
    if current_count != *last_count {
        info!("Entity count changed: {} -> {}", *last_count, current_count);
        *last_count = current_count;
    }
}

#[cfg(debug_assertions)]
fn debug_performance_monitor(time: Res<Time>) {
    if time.delta_seconds() > 0.020 {  // > 20ms (< 50 FPS)
        warn!("Slow frame detected: {:.2}ms", time.delta_seconds() * 1000.0);
    }
}

#[cfg(debug_assertions)]
fn debug_player_status(
    query: Query<&Player, Changed<Player>>,
) {
    for player in query.iter() {
        info!("Player status: health={}, speed={}, level={}", 
              player.health, player.speed, player.level);
    }
}
```

### Custom Debug Resources

Create resources to track debug information:

```rust
use bevy::prelude::*;

#[derive(Resource, Debug)]
pub struct DebugStats {
    pub total_spawns: u64,
    pub total_despawns: u64,
    pub peak_entity_count: usize,
    pub frame_count: u64,
}

impl Default for DebugStats {
    fn default() -> Self {
        Self {
            total_spawns: 0,
            total_despawns: 0,
            peak_entity_count: 0,
            frame_count: 0,
        }
    }
}

fn setup_debug_resources(mut commands: Commands) {
    commands.insert_resource(DebugStats::default());
}

fn update_debug_stats(
    mut stats: ResMut<DebugStats>,
    all_entities: Query<Entity>,
) {
    stats.frame_count += 1;
    let current_count = all_entities.iter().count();
    if current_count > stats.peak_entity_count {
        stats.peak_entity_count = current_count;
    }
}
```

## Advanced Configuration

### Custom BRP Endpoints

Add custom endpoints for game-specific debugging:

```rust
use bevy::prelude::*;
use bevy::remote::{RemotePlugin, RemoteHttpPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .add_systems(Startup, setup_custom_debug_endpoints)
        .run();
}

fn setup_custom_debug_endpoints(world: &mut World) {
    // Custom endpoints will be automatically discovered
    // by the BRP reflection system
}
```

### Performance Optimizations

Configure BRP for optimal performance:

```rust
use bevy::remote::{RemotePlugin, BrpConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin {
            config: BrpConfig {
                // Limit concurrent connections for performance
                max_connections: 10,
                // Reduce timeout for responsiveness  
                connection_timeout_secs: 30,
                // Buffer size optimization
                buffer_size: 8192,
                ..default()
            }
        })
        .run();
}
```

## Integration with Game Frameworks

### Integration with bevy_inspector_egui

Combine with the popular inspector plugin:

```toml
# Cargo.toml
[dependencies]
bevy = "0.14"
bevy_inspector_egui = "0.24"
```

```rust
use bevy::prelude::*;
use bevy::remote::RemotePlugin;
use bevy_inspector_egui::quick::WorldInspectorPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default())
        .add_plugins(WorldInspectorPlugin::new())  // Visual debugging
        .run();
}
```

### Integration with bevy_hanabi (Particles)

Special considerations for particle systems:

```rust
use bevy::prelude::*;
use bevy::remote::RemotePlugin;
// use bevy_hanabi::prelude::*;  // If using hanabi

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default())
        // .add_plugins(HanabiPlugin)  // Particle system
        .add_systems(Update, debug_particle_systems)
        .run();
}

fn debug_particle_systems(
    // particle_query: Query<&EffectAsset>,  // Example with hanabi
) {
    // Custom debugging for particle systems
}
```

## Common Patterns

### Entity Lifecycle Tracking

Track entity creation and destruction for debugging:

```rust
use bevy::prelude::*;

fn track_entity_lifecycle(
    mut commands: Commands,
    mut events: EventReader<EntityEvent>,
    mut debug_stats: ResMut<DebugStats>,
) {
    for event in events.read() {
        match event {
            EntityEvent::Spawned(entity) => {
                debug_stats.total_spawns += 1;
                info!("Entity spawned: {:?}", entity);
            },
            EntityEvent::Despawned(entity) => {
                debug_stats.total_despawns += 1;
                info!("Entity despawned: {:?}", entity);
            },
        }
    }
}

// Custom event type for entity lifecycle
#[derive(Event)]
pub enum EntityEvent {
    Spawned(Entity),
    Despawned(Entity),
}
```

### Component Change Tracking

Monitor specific component changes:

```rust
use bevy::prelude::*;

fn track_position_changes(
    query: Query<(Entity, &Transform, &DebugName), Changed<Transform>>,
) {
    for (entity, transform, name) in query.iter() {
        info!("Entity {:?} ({}) moved to {:?}", 
              entity, name.0, transform.translation);
    }
}

fn track_health_changes(
    query: Query<(Entity, &Player, &DebugName), Changed<Player>>,
) {
    for (entity, player, name) in query.iter() {
        if player.health <= 0.0 {
            warn!("Entity {:?} ({}) died!", entity, name.0);
        } else if player.health < 20.0 {
            warn!("Entity {:?} ({}) low health: {}", entity, name.0, player.health);
        }
    }
}
```

## Troubleshooting

### "BRP server failed to start"

**Cause**: Port already in use or permission issues.

**Solution**:
```bash
# Check if port is in use
lsof -i :15702

# Try different port
export BEVY_BRP_PORT=15703
```

### "Connection refused" from debugger

**Cause**: Game not running or RemotePlugin not added.

**Solution**:
1. Verify RemotePlugin is added to your app
2. Check game is running: `ps aux | grep your-game`
3. Test direct connection: `curl http://localhost:15702/bevy/list_entities`

### Performance impact from BRP

**Cause**: BRP processing overhead during intensive debugging.

**Solution**:
```rust
// Limit BRP in performance-critical sections
#[cfg(not(feature = "debug_mode"))]
fn performance_critical_system() {
    // Disable BRP temporarily if needed
}
```

### Entities not visible to debugger

**Cause**: Components not serializable or private.

**Solution**:
```rust
// Make sure components derive necessary traits
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct MyComponent {
    pub value: f32,  // Public fields are accessible
}

// Register with reflection system
fn setup_reflection(mut registry: ResMut<AppTypeRegistry>) {
    registry.register::<MyComponent>();
}
```

## Security Considerations

### Production Deployment

For production builds, disable or secure BRP:

```rust
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    
    #[cfg(debug_assertions)]
    {
        app.add_plugins(RemotePlugin::default());
    }
    
    // Or use feature flags
    #[cfg(feature = "debug")]
    {
        app.add_plugins(RemotePlugin::default());
    }
    
    app.run();
}
```

### Network Security

If enabling remote connections:

```rust
use bevy::remote::{RemotePlugin, BrpConfig};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin {
            config: BrpConfig {
                address: "127.0.0.1".to_string(),  // Localhost only
                // Add authentication if available
                require_auth: true,
                ..default()
            }
        })
        .run();
}
```

## Best Practices

### 1. Use Debug Names

Always add descriptive names to important entities:

```rust
commands.spawn((
    MyComponent::default(),
    DebugName("Player Character".to_string()),
));
```

### 2. Group Related Entities

Use consistent naming patterns:

```rust
// Good naming patterns
DebugName("Enemy_Goblin_001".to_string())
DebugName("Projectile_Arrow_Player".to_string())  
DebugName("UI_HealthBar_Player".to_string())
```

### 3. Add Debug Information

Include debugging metadata in your components:

```rust
#[derive(Component, Debug, Reflect)]
pub struct Player {
    pub health: f32,
    pub speed: f32,
    
    // Debug information
    #[cfg(debug_assertions)]
    pub debug_last_position: Vec3,
    #[cfg(debug_assertions)]
    pub debug_spawn_time: f64,
}
```

### 4. Use Conditional Compilation

Keep debug code separate from release code:

```rust
#[cfg(debug_assertions)]
fn debug_only_system() {
    // Expensive debugging operations
}

fn production_system() {
    // Core game logic
    
    #[cfg(debug_assertions)]
    {
        // Optional debug information
        debug!("Player position: {:?}", position);
    }
}
```

## What's Next?

Now that your Bevy game is set up for debugging:

- **[Quick Start Guide](../quick-start.md)**: Learn basic debugging commands
- **[Tool Reference](../tools/)**: Master all debugging tools
- **[Tutorials](../tutorials/)**: Follow step-by-step debugging scenarios
- **[Troubleshooting](../troubleshooting/)**: Solve common issues

---

*Your Bevy game is now ready for AI-powered debugging! Start debugging with natural language commands through Claude Code.*
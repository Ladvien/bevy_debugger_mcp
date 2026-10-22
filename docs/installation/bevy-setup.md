# Bevy Game Setup Guide

*Complete guide to integrating bevy-fairy with your Bevy game*

## Overview

This guide walks you through adding the debugger to your Bevy game so it can be inspected and
driven over the Bevy Remote Protocol (BRP) by bevy-fairy.

The setup is one plugin. The plugin owns the BRP `RemotePlugin` and registers every
`bevy_fairy/*` method, so there is nothing else to wire up.

## Prerequisites

- Bevy 0.20 game project
- Rust 1.95+
- Basic familiarity with Bevy ECS and plugins

## Basic Setup (2 minutes)

### Step 1: Add the dependency

```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.20", features = ["default"] }
bevy_fairy = "0.2"
```

### Step 2: Add the plugin to your game

```rust
use bevy::prelude::*;
use bevy::remote::http::RemoteHttpPlugin;
use bevy_fairy::DebuggerPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Owns the BRP RemotePlugin. Never add a second RemotePlugin next to it.
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, game_systems)
        .run();
}
```

**Do not add `RemotePlugin` yourself.** `DebuggerPlugin` adds it, and Bevy rejects a duplicate
plugin by name — a game that adds it twice panics on start.

`DebuggerPlugin` needs `bevy::input::InputPlugin` in the app. `DefaultPlugins` includes it;
`MinimalPlugins` does not.

### Step 3: Verify It Works

1. **Run your game**: `cargo run`
2. **Check BRP is active**: You should see a log message like:
   ```
   INFO bevy_remote: Starting BRP server on localhost:15702
   ```
3. **Test the connection**:
   ```bash
   curl -X POST http://localhost:15702/bevy_fairy/status
   ```

If you see JSON output with your game's status, the setup is complete!

## Configuration Options

### Custom Port Configuration

If port 15702 is already in use, configure a different port on the HTTP transport:

```rust
use bevy::remote::http::RemoteHttpPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default().with_port(15703))
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
use bevy::remote::http::RemoteHttpPlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(
            RemoteHttpPlugin::default()
                .with_address("0.0.0.0")
                .with_port(15702),
        )
        .run();
}
```

**Security Note**: Binding to `0.0.0.0` allows external connections. Only use this in trusted
environments.

### Debug vs Release Configuration

Load the plugin only in debug builds:

```rust
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    #[cfg(debug_assertions)]
    app.add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default());

    app.add_systems(Startup, setup)
       .run();
}
```

### Conditional Plugin Loading

Load the plugin only when debugging is needed:

```rust
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    // Enable debugging with environment variable
    if std::env::var("BEVY_DEBUG").is_ok() {
        app.add_plugins(DebuggerPlugin::default())
            .add_plugins(RemoteHttpPlugin::default());
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

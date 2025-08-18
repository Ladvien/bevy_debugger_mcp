# Screenshot Setup Guide

The Bevy Debugger MCP now supports taking window-specific screenshots of your Bevy game, perfect for visual debugging and documentation.

## Why Use Game Window Screenshots?

Instead of capturing the entire screen (which includes your IDE, browser, and other windows), the MCP screenshot tool captures exactly what your Bevy game is rendering. This provides:

- **Precise capture**: Only the game window, no desktop clutter
- **Works when occluded**: Captures even if the window is covered or minimized
- **Consistent results**: Always captures the same content regardless of screen setup

## Setup Instructions

### 1. Update Your Bevy Dependencies

Make sure you're using Bevy 0.16+ with the `bevy_remote` feature:

```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.16", features = ["default", "bevy_remote"] }
```

### 2. Add Screenshot Support to Your Game

Update your Bevy app to include the screenshot handler:

```rust
use bevy::prelude::*;
use bevy::remote::{RemotePlugin, BrpResult};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use serde_json::Value;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(
            RemotePlugin::default()
                // Add the custom screenshot method
                .with_method("bevy_debugger/screenshot", screenshot_handler)
        )
        // Your existing systems...
        .run();
}

/// Custom BRP handler for screenshot requests from the debugger
fn screenshot_handler(
    In(params): In<Option<Value>>, 
    mut commands: Commands,
) -> BrpResult {
    // Parse the path parameter from MCP request
    let path = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("./screenshot.png")
        .to_string();

    println!("Screenshot requested via BRP: {}", path);
    
    // Use Bevy's built-in screenshot system
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    
    // Return success response
    Ok(serde_json::json!({
        "path": path,
        "success": true
    }))
}
```

### 3. Restart Your Game

After adding the screenshot handler, restart your Bevy game for the changes to take effect.

## Usage Examples

### Basic Screenshot

```
Human: Take a screenshot of the current game state

Claude: I'll take a screenshot of your game window now.
```

This saves a screenshot as `./screenshot.png` in your project directory.

### Custom Path Screenshot

```
Human: Take a screenshot and save it as "debug/player-position.png"

Claude: I'll take a screenshot and save it to debug/player-position.png
```

This saves the screenshot to the specified path.

### Screenshot for Bug Reports

```
Human: I'm seeing a visual glitch with the UI. Can you take a screenshot so I can include it in a bug report?

Claude: I'll take a screenshot to help document the visual issue you're seeing.
```

Perfect for capturing bugs, unexpected behavior, or documenting features.

## Technical Details

### How It Works

1. **MCP Request**: When you ask for a screenshot, Claude sends a request to the MCP server
2. **BRP Communication**: The MCP server sends a custom BRP request to your Bevy game
3. **Bevy Screenshot**: Your game uses Bevy's built-in screenshot system to capture the primary window
4. **File Save**: The screenshot is saved to the specified path using Bevy's `save_to_disk` observer
5. **Response**: The MCP server confirms the screenshot was saved successfully

### Supported Formats

The screenshot system saves images in PNG format by default. The filename extension determines the format:

- `.png` - PNG format (default)
- `.jpg` or `.jpeg` - JPEG format  
- `.bmp` - Bitmap format
- `.tga` - Targa format

### Error Handling

If the screenshot fails (e.g., invalid path, permission issues), you'll receive a detailed error message explaining what went wrong.

## Troubleshooting

### "BRP client not connected" Error

This means your Bevy game isn't running or the RemotePlugin isn't properly configured. Make sure:

1. Your game is running with `cargo run`
2. You've added `RemotePlugin` to your app
3. The `bevy_remote` feature is enabled in Cargo.toml

### "Screenshot request failed" Error

This indicates an issue with the Bevy screenshot system. Check:

1. The file path is valid and writable
2. The directory exists (create it if needed)
3. Your game has a primary window

### Permission Errors

If you get permission errors:

1. Check that the target directory is writable
2. Use an absolute path if having issues with relative paths
3. Make sure the directory exists before taking the screenshot

## Example: Complete Setup

Here's a complete example showing how to set up a basic Bevy game with screenshot support:

```rust
use bevy::{
    prelude::*,
    remote::{RemotePlugin, BrpResult},
    render::view::screenshot::{save_to_disk, Screenshot},
};
use serde_json::Value;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // Enable remote debugging with screenshot support
        .add_plugins(
            RemotePlugin::default()
                .with_method("bevy_debugger/screenshot", screenshot_handler)
        )
        .add_systems(Startup, setup)
        .add_systems(Update, rotate_cube)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Camera
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 0.0, 5.0),
    ));

    // Cube
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.8, 0.7, 0.6))),
        Transform::default(),
    ));

    // Light
    commands.spawn((
        PointLight::default(),
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
}

fn rotate_cube(
    time: Res<Time>,
    mut query: Query<&mut Transform, With<Mesh3d>>,
) {
    for mut transform in &mut query {
        transform.rotate_y(time.delta_secs() * 0.5);
    }
}

fn screenshot_handler(
    In(params): In<Option<Value>>, 
    mut commands: Commands,
) -> BrpResult {
    let path = params
        .as_ref()
        .and_then(|p| p.get("path"))
        .and_then(|p| p.as_str())
        .unwrap_or("./screenshot.png")
        .to_string();

    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path.clone()));
    
    Ok(serde_json::json!({
        "path": path,
        "success": true
    }))
}
```

This creates a simple rotating cube that you can screenshot for testing the functionality.
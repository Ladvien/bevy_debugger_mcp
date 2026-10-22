# Screenshot Setup Guide

The bevy-fairy captures what your Bevy game renders, without reaching outside the game.

## What it captures

There are two sources, and the plugin picks between them:

- **offscreen** — an `Image` that one of your cameras renders to. This is the default when you
  insert a `DebugCaptureTarget`, and it is the source to use. It works while the game is behind
  other windows, on another Space, or minimised, and it never touches the screen.
- **window** — the game's own primary window surface. This is the default when there is no
  `DebugCaptureTarget`. It includes the UI drawn to the window, and it does not raise or focus
  the window, but the operating system only keeps a window's surface current while the window is
  on screen. A covered or minimised window can give a stale frame, and on macOS an unfocused
  window has been measured to give a single flat colour.

Neither source is a screen capture. There is no window manager, no focus change, and no
screenshot of your desktop.

## Setup Instructions

### 1. Update your Bevy dependencies

```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.20", features = ["default"] }
bevy_fairy = "0.2"
```

### 2. Add the plugin to your game

`DebuggerPlugin` owns the BRP `RemotePlugin`, so **never add a second `RemotePlugin` next to it** —
Bevy rejects a duplicate plugin by name, and the game panics on start. Add a transport instead.

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
        .run();
}
```

The plugin registers `bevy_fairy/screenshot` and `bevy_fairy/capture` itself. Do not
register them by hand: adding them again adds a second `RemotePlugin`, and the game panics.

### 3. Give the plugin something to capture

Without a `DebugCaptureTarget`, a capture falls back to the window source. To capture the game's
own image, insert the target with the `Image` your camera renders to:

```rust
use bevy::prelude::*;
use bevy::render::view::RenderTarget;
use bevy_fairy::{DebugCaptureTarget, DebuggerPlugin};

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(DebuggerPlugin::default())
        .add_plugins(RemoteHttpPlugin::default())
        .add_systems(Startup, setup)
        .run();
}

fn setup(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    mut cameras: Query<&mut Camera>,
) {
    let size = Extent3d {
        width: 1280,
        height: 720,
        depth_or_array_layers: 1,
    }..;
    let format = TextureFormat::Rgba8UnormSrgb;
    let mut image = Image::new_fill(size, TextureDimension::D2, &[0; 4], format, RenderAssetUsages::default());
    let handle = images.add(image);

    for mut camera in &mut cameras {
        camera.target = RenderTarget::Image(handle.clone());
    }
    commands.insert_resource(DebugCaptureTarget { image: handle });
}
```

A camera that renders to an offscreen image draws no UI, because the UI is drawn to the window.
Use the window source when the UI is what you need to see.

## Using it from an agent

The MCP server exposes this as the `screenshot` tool:

```
screenshot { "save_to": "shot.png", "source": "offscreen" }
```

`source` is `offscreen` or `window`. `region` crops, and `zoom` magnifies. The server polls
`bevy_fairy/capture` until the PNG is ready and writes it to `save_to`.

A capture that goes to the window is not flagged as possibly stale, because the server cannot know
whether the window was on screen at the time. If that matters, insert a `DebugCaptureTarget` and
use the offscreen source.

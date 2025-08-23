# Bevy Debugger MCP - Quick Start Guide

*Get from zero to debugging your Bevy game in under 5 minutes*

## ⚡ 5-Minute Setup

### Prerequisites (30 seconds)
- ✅ Rust 1.70+ installed
- ✅ Claude Code installed
- ✅ A Bevy game project

### Step 1: Install the Debugger (1 minute)

```bash
# Install from crates.io
cargo install bevy_debugger_mcp

# Verify installation
bevy-debugger-mcp --help
```

### Step 2: Enable RemotePlugin (1 minute)

Add this to your Bevy game's `main.rs`:

```rust
use bevy::prelude::*;
use bevy::remote::RemotePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default()) // ← Add this line
        .run();
}
```

### Step 3: Configure Claude Code (1 minute)

Add to your Claude Code configuration:

**Location**: `~/.config/claude/claude_code_config.json` (macOS/Linux)

```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"]
    }
  }
}
```

### Step 4: Start Debugging (30 seconds)

1. **Run your game**: `cargo run`
2. **Open Claude Code** in your project directory
3. **Try your first command**:

```
Show me all entities in my Bevy game
```

### Step 5: Verify It Works (1 minute)

You should see output like:
```
Found 3 entities in your game:
• Entity 0: Transform, Camera
• Entity 1: Transform, Sprite, Player
• Entity 2: Transform, Light
```

## 🎯 First Debugging Commands

Try these to explore your game:

### Basic Observation
```
What components does the player have?
```

### Performance Check
```
Check the current frame rate and memory usage
```

### Take a Screenshot
```
Take a screenshot of my game for documentation
```

### Test Something
```
What happens if I spawn 10 new entities?
```

## 🚨 Quick Troubleshooting

**Nothing happens when I run commands?**
- Check your game is running with `lsof -i :15702`
- Restart Claude Code after config changes

**"BRP connection failed"?**
- Ensure `RemotePlugin::default()` is in your App
- Check port 15702 isn't blocked by firewall

**Tools not available?**
- Update to latest version: `cargo install bevy_debugger_mcp --force`
- Verify installation: `bevy-debugger-mcp --help`

## 🎮 What's Next?

Now that you're connected, explore these advanced features:

- **[Performance Debugging](tutorials/README.md#tutorial-2-performance-debugging)**: Find bottlenecks
- **[Entity Investigation](tutorials/README.md#tutorial-3-entity-investigation)**: Debug specific entities
- **[Visual Debugging](tutorials/README.md#tutorial-4-visual-debugging)**: Screenshots and overlays
- **[Automated Testing](tutorials/README.md#tutorial-5-automated-testing)**: Set up monitoring

## 📚 Full Documentation

- **[Installation Guide](installation/)**
- **[Tool Reference](tools/)**
- **[Configuration](api-reference.md)**
- **[Troubleshooting](troubleshooting.md)**

---

🎉 **Congratulations!** You're now debugging your Bevy game with AI assistance. Happy debugging!
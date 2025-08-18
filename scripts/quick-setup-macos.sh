#!/bin/bash
# Quick setup script for macOS - The easiest way to get started!

set -e

echo "🚀 Bevy Debugger MCP - Quick Setup for macOS"
echo "============================================"
echo ""

# Step 1: Build the project
echo "📦 Building the debugger..."
if ! cargo build --release 2>/dev/null; then
    echo "❌ Build failed. Make sure you have Rust installed:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Step 2: Install binary
echo "📥 Installing binary..."
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="bevy-debugger-mcp"

# Create symlink for easy updates
ln -sf "$(pwd)/target/release/bevy_debugger_mcp" "$INSTALL_DIR/$BINARY_NAME"
echo "✅ Installed to $INSTALL_DIR/$BINARY_NAME"

# Step 3: Configure Claude Desktop
echo "🔧 Configuring Claude Desktop..."
CLAUDE_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"

if [ ! -d "$(dirname "$CLAUDE_CONFIG")" ]; then
    echo "⚠️  Claude Desktop not found. Please install it from:"
    echo "   https://claude.ai/download"
    echo ""
    echo "After installing Claude, run this script again."
else
    # Backup existing config
    if [ -f "$CLAUDE_CONFIG" ]; then
        cp "$CLAUDE_CONFIG" "$CLAUDE_CONFIG.bak"
        echo "📁 Backed up existing Claude config"
    fi
    
    # Create new config
    cat > "$CLAUDE_CONFIG" << EOF
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "$INSTALL_DIR/$BINARY_NAME",
      "args": ["serve"],
      "env": {
        "RUST_LOG": "info"
      }
    }
  }
}
EOF
    echo "✅ Claude Desktop configured"
fi

# Step 4: Create test project (optional)
echo ""
read -p "Would you like to create a test Bevy project? (y/N) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    TEST_PROJECT="bevy-debug-test"
    echo "🎮 Creating test project '$TEST_PROJECT'..."
    
    # Create project directory
    mkdir -p "$TEST_PROJECT"
    cd "$TEST_PROJECT"
    
    # Create Cargo.toml
    cat > Cargo.toml << 'EOF'
[package]
name = "bevy-debug-test"
version = "0.1.0"
edition = "2021"

[dependencies]
bevy = { version = "0.14", features = ["remote"] }
EOF
    
    # Create main.rs
    mkdir -p src
    cat > src/main.rs << 'EOF'
use bevy::prelude::*;
use bevy::remote::RemotePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default()) // Enable debugging!
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    
    // Spawn a test entity
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color: Color::rgb(0.25, 0.75, 0.25),
                custom_size: Some(Vec2::new(100.0, 100.0)),
                ..default()
            },
            ..default()
        },
        Name::new("Test Square"),
    ));
    
    println!("✅ Test game running! BRP listening on ws://localhost:15702");
}
EOF
    
    echo "✅ Test project created at $(pwd)"
    echo ""
    echo "To test the debugger:"
    echo "  1. In Terminal 1: cd $TEST_PROJECT && cargo run"
    echo "  2. Open Claude Desktop (restart it first!)"
    echo "  3. Ask Claude: 'Show me all entities in the game'"
    
    cd ..
fi

# Step 5: Final instructions
echo ""
echo "======================================"
echo "✨ Setup Complete!"
echo "======================================"
echo ""
echo "Quick Test:"
echo "  1. Restart Claude Desktop (Cmd+Q and reopen)"
echo "  2. Run your Bevy game with RemotePlugin"
echo "  3. You should see the 🔌 MCP icon in Claude"
echo ""
echo "Useful Commands:"
echo "  $BINARY_NAME doctor         # Check installation"
echo "  $BINARY_NAME test           # Test connection to game"
echo "  $BINARY_NAME serve          # Start server manually"
echo "  $BINARY_NAME --help         # Show all commands"
echo ""
echo "Documentation:"
echo "  • README.md                 - Installation & setup"
echo "  • CLAUDE_SUBAGENT_GUIDE.md  - How to use with Claude"
echo "  • USAGE_GUIDE.md            - Advanced features"
echo ""
echo "Happy debugging! 🎮🐛"
#!/bin/bash

# Universal setup script for bevy-debugger-mcp with Claude Code/Desktop
# Run this after installing via any method (cargo, brew, manual)

set -e

echo "Setting up bevy-debugger-mcp for Claude..."

# Find the bevy-debugger-mcp binary
BINARY_PATH=""

# Check common installation locations
if command -v bevy-debugger-mcp &> /dev/null; then
    BINARY_PATH=$(which bevy-debugger-mcp)
    echo "✅ Found bevy-debugger-mcp at: $BINARY_PATH"
elif [ -f "$HOME/.cargo/bin/bevy-debugger-mcp" ]; then
    BINARY_PATH="$HOME/.cargo/bin/bevy-debugger-mcp"
    echo "✅ Found bevy-debugger-mcp at: $BINARY_PATH"
elif [ -f "/usr/local/bin/bevy-debugger-mcp" ]; then
    BINARY_PATH="/usr/local/bin/bevy-debugger-mcp"
    echo "✅ Found bevy-debugger-mcp at: $BINARY_PATH"
elif [ -f "/opt/homebrew/bin/bevy-debugger-mcp" ]; then
    BINARY_PATH="/opt/homebrew/bin/bevy-debugger-mcp"
    echo "✅ Found bevy-debugger-mcp at: $BINARY_PATH"
else
    echo "❌ bevy-debugger-mcp not found. Please install it first:"
    echo "  cargo install bevy_debugger_mcp"
    echo "  or"
    echo "  brew install bevy-debugger-mcp"
    exit 1
fi

# Create symlinks for Claude Code compatibility
echo "Creating compatibility symlinks..."
mkdir -p ~/.local/bin
ln -sf "$BINARY_PATH" ~/.local/bin/bevy-debugger-mcp

# Also ensure it's in ~/.cargo/bin for consistency
if [ ! -f "$HOME/.cargo/bin/bevy-debugger-mcp" ] && [ "$BINARY_PATH" != "$HOME/.cargo/bin/bevy-debugger-mcp" ]; then
    mkdir -p ~/.cargo/bin
    ln -sf "$BINARY_PATH" ~/.cargo/bin/bevy-debugger-mcp
fi

# Setup Claude Code configuration
CLAUDE_CODE_CONFIG="$HOME/.claude/mcp_settings.json"
if [ -f "$CLAUDE_CODE_CONFIG" ]; then
    echo "Found Claude Code config at: $CLAUDE_CODE_CONFIG"
    if grep -q "bevy-debugger-mcp" "$CLAUDE_CODE_CONFIG"; then
        echo "⚠️  bevy-debugger-mcp already configured in Claude Code"
    else
        echo "📝 Add this to your $CLAUDE_CODE_CONFIG:"
    fi
else
    echo "📝 Create $CLAUDE_CODE_CONFIG with:"
    mkdir -p ~/.claude
fi

cat << EOF

{
  "mcpServers": {
    "bevy-debugger-mcp": {
      "command": "$BINARY_PATH",
      "args": ["stdio"],
      "env": {
        "RUST_LOG": "info",
        "BEVY_BRP_HOST": "127.0.0.1",
        "BEVY_BRP_PORT": "15702"
      }
    }
  }
}
EOF

# Setup Claude Desktop configuration
CLAUDE_DESKTOP_CONFIG="$HOME/Library/Application Support/Claude/claude_desktop_config.json"
if [ -f "$CLAUDE_DESKTOP_CONFIG" ]; then
    echo ""
    echo "Found Claude Desktop config at: $CLAUDE_DESKTOP_CONFIG"
    if grep -q "bevy-debugger-mcp" "$CLAUDE_DESKTOP_CONFIG"; then
        echo "⚠️  bevy-debugger-mcp already configured in Claude Desktop"
    else
        echo "📝 Add the bevy-debugger-mcp section to your Claude Desktop config"
    fi
fi

echo ""
echo "✅ Setup complete!"
echo ""
echo "Symlinks created:"
echo "  - ~/.local/bin/bevy-debugger-mcp -> $BINARY_PATH"
if [ -L "$HOME/.cargo/bin/bevy-debugger-mcp" ]; then
    echo "  - ~/.cargo/bin/bevy-debugger-mcp -> $BINARY_PATH"
fi
echo ""
echo "Next steps:"
echo "1. Add the configuration above to your Claude config files"
echo "2. Restart Claude Code or Claude Desktop"
echo "3. Start your Bevy game with RemotePlugin enabled"
echo "4. The bevy-debugger-mcp should now connect successfully"
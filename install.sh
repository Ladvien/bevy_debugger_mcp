#!/bin/bash

# Installation script for bevy-fairy
# This script installs the binary and sets up necessary symlinks for Claude Code compatibility

set -e

echo "Installing bevy-fairy..."

# Build the binary in release mode
echo "Building bevy-fairy..."
cargo build --release --bin bevy-fairy

# Install to cargo bin directory
echo "Installing to ~/.cargo/bin..."
cp target/release/bevy-fairy ~/.cargo/bin/
chmod +x ~/.cargo/bin/bevy-fairy

# Create symlink for Claude Code compatibility
# Claude Code sometimes looks for binaries in ~/.local/bin
echo "Creating symlink for Claude Code compatibility..."
mkdir -p ~/.local/bin
ln -sf ~/.cargo/bin/bevy-fairy ~/.local/bin/bevy-fairy

# Verify installation
if command -v bevy-fairy &> /dev/null; then
    echo "✅ bevy-fairy installed successfully!"
    echo ""
    echo "Binary locations:"
    echo "  - Main: ~/.cargo/bin/bevy-fairy"
    echo "  - Symlink: ~/.local/bin/bevy-fairy"
    echo ""
    echo "To use with Claude Code, add to your MCP settings:"
    echo "  ~/.claude/mcp_settings.json (Claude Code CLI)"
    echo "  ~/Library/Application Support/Claude/claude_desktop_config.json (Claude Desktop)"
    echo ""
    echo "Example configuration:"
    echo '  "bevy-fairy": {'
    echo '    "command": "/Users/'$USER'/.cargo/bin/bevy-fairy",'
    echo '    "args": ["--stdio"],'
    echo '    "env": {'
    echo '      "RUST_LOG": "info",'
    echo '      "BEVY_BRP_HOST": "127.0.0.1",'
    echo '      "BEVY_BRP_PORT": "15702"'
    echo '    }'
    echo '  }'
else
    echo "❌ Installation failed. Please check the error messages above."
    exit 1
fi
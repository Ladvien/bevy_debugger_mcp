#!/bin/bash

# Installation script for bevy-debugger-mcp
# This script installs the binary and sets up necessary symlinks for Claude Code compatibility

set -e

echo "Installing bevy-debugger-mcp..."

# Build the binary in release mode
echo "Building bevy-debugger-mcp..."
cargo build --release --bin bevy-debugger-mcp

# Install to cargo bin directory
echo "Installing to ~/.cargo/bin..."
cp target/release/bevy-debugger-mcp ~/.cargo/bin/
chmod +x ~/.cargo/bin/bevy-debugger-mcp

# Create symlink for Claude Code compatibility
# Claude Code sometimes looks for binaries in ~/.local/bin
echo "Creating symlink for Claude Code compatibility..."
mkdir -p ~/.local/bin
ln -sf ~/.cargo/bin/bevy-debugger-mcp ~/.local/bin/bevy-debugger-mcp

# Verify installation
if command -v bevy-debugger-mcp &> /dev/null; then
    echo "✅ bevy-debugger-mcp installed successfully!"
    echo ""
    echo "Binary locations:"
    echo "  - Main: ~/.cargo/bin/bevy-debugger-mcp"
    echo "  - Symlink: ~/.local/bin/bevy-debugger-mcp"
    echo ""
    echo "To use with Claude Code, add to your MCP settings:"
    echo "  ~/.claude/mcp_settings.json (Claude Code CLI)"
    echo "  ~/Library/Application Support/Claude/claude_desktop_config.json (Claude Desktop)"
    echo ""
    echo "Example configuration:"
    echo '  "bevy-debugger-mcp": {'
    echo '    "command": "/Users/'$USER'/.cargo/bin/bevy-debugger-mcp",'
    echo '    "args": ["stdio"],'
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
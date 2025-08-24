# Installation Guide for Bevy Debugger MCP

## Prerequisites

- Rust 1.70+ installed
- Claude Desktop application
- A Bevy game with Remote Protocol enabled

## Quick Installation

### 1. Install from Source

```bash
# Clone the repository
git clone https://github.com/Ladvien/bevy_debugger_mcp.git
cd bevy_debugger_mcp

# Build release version
cargo build --release

# Install locally
mkdir -p ~/.local/bin
cp target/release/bevy-debugger-mcp ~/.local/bin/
chmod +x ~/.local/bin/bevy-debugger-mcp
```

### 2. Create Wrapper Script (Required for Claude Desktop)

Due to a logging issue with the rmcp library, a wrapper script is needed:

```bash
cat > ~/.local/bin/bevy-debugger-mcp-stdio << 'EOF'
#!/usr/bin/env python3
"""
Wrapper for bevy-debugger-mcp that filters out log lines from stdout.
This fixes the stdio mode for Claude Desktop integration.
"""
import sys
import subprocess
import json
import threading

def is_json_line(line):
    """Check if a line is valid JSON-RPC."""
    try:
        data = json.loads(line)
        return 'jsonrpc' in data or 'method' in data or 'id' in data
    except:
        return False

def main():
    # Start the subprocess
    proc = subprocess.Popen(
        ['~/.local/bin/bevy-debugger-mcp', '--stdio'],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        bufsize=0
    )
    
    # Forward stdin to the subprocess
    def forward_stdin():
        try:
            while True:
                line = sys.stdin.readline()
                if not line:
                    break
                proc.stdin.write(line)
                proc.stdin.flush()
        except:
            pass
    
    stdin_thread = threading.Thread(target=forward_stdin, daemon=True)
    stdin_thread.start()
    
    # Filter stdout from the subprocess
    try:
        while True:
            line = proc.stdout.readline()
            if not line:
                break
            # Only output lines that look like JSON-RPC
            if line.startswith('{') and is_json_line(line.strip()):
                sys.stdout.write(line)
                sys.stdout.flush()
    except KeyboardInterrupt:
        pass
    finally:
        proc.terminate()

if __name__ == '__main__':
    main()
EOF

chmod +x ~/.local/bin/bevy-debugger-mcp-stdio
```

### 3. Configure Claude Desktop

Edit `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "/Users/YOUR_USERNAME/.local/bin/bevy-debugger-mcp-stdio",
      "args": [],
      "env": {
        "RUST_LOG": "off",
        "BEVY_BRP_HOST": "localhost",
        "BEVY_BRP_PORT": "15702"
      }
    }
  }
}
```

Replace `YOUR_USERNAME` with your actual username.

### 4. Enable Bevy Remote Protocol in Your Game

Add to your `Cargo.toml`:
```toml
[dependencies]
bevy = { version = "0.16", features = ["bevy_remote"] }
```

In your game's main.rs:
```rust
use bevy::prelude::*;
use bevy::remote::RemotePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default()) // Enables BRP on port 15702
        .run();
}
```

### 5. Restart Claude Desktop

Quit and restart Claude Desktop to load the new MCP server configuration.

## Verification

1. Start your Bevy game with Remote Protocol enabled
2. Open Claude Desktop
3. Check if the MCP server is connected (you should see "bevy-debugger" in the tools list)
4. Try a debugging command like "observe the current game state"

## Troubleshooting

### Check MCP Server Logs

```bash
tail -f ~/Library/Logs/Claude/mcp-server-bevy-debugger.log
```

### Test Manually

```bash
# Test the wrapper script
echo '{"jsonrpc":"2.0","method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{}},"id":1}' | \
  ~/.local/bin/bevy-debugger-mcp-stdio
```

### Common Issues

1. **"Unexpected token" errors**: Make sure you're using the wrapper script, not the binary directly
2. **Connection refused**: Ensure your Bevy game is running with RemotePlugin on port 15702
3. **Server not showing in Claude**: Restart Claude Desktop after updating the config

## Alternative: TCP Mode

If stdio mode doesn't work, you can run in TCP mode:

```bash
# Terminal 1: Start the MCP server
~/.local/bin/bevy-debugger-mcp --tcp

# Terminal 2: Configure Claude to connect to TCP port 3001
```

## Uninstallation

```bash
# Remove binaries
rm ~/.local/bin/bevy-debugger-mcp
rm ~/.local/bin/bevy-debugger-mcp-stdio

# Remove from Claude config
# Edit ~/Library/Application Support/Claude/claude_desktop_config.json
# and remove the "bevy-debugger" section
```

## Support

For issues or questions:
- GitHub Issues: https://github.com/Ladvien/bevy_debugger_mcp/issues
- Documentation: https://github.com/Ladvien/bevy_debugger_mcp/docs
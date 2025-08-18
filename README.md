# Bevy Debugger MCP Server

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/ladvien/bevy_debugger_mcp)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Bevy](https://img.shields.io/badge/bevy-0.14+-purple.svg)](https://bevyengine.org)

A powerful Model Context Protocol (MCP) server that enables AI-assisted debugging of Bevy games through Claude Code. Debug your game state, analyze performance, and test hypotheses with natural language commands.

## ✨ Features

- **🔍 Real-time Observation**: Monitor entities, components, and resources as your game runs
- **🧪 Smart Experimentation**: Test game behavior changes with automatic rollback
- **📊 Performance Analysis**: Identify bottlenecks and optimize game performance  
- **🚨 Anomaly Detection**: Automatically spot unusual patterns in game behavior
- **📹 Session Recording**: Record and replay debugging sessions for analysis
- **🛡️ Error Recovery**: Robust error handling with automatic diagnostics

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ and Cargo
- Claude Code CLI
- A Bevy game with [RemotePlugin](https://docs.rs/bevy/latest/bevy/remote/struct.RemotePlugin.html) enabled

### Installation

```bash
# Clone and build
git clone https://github.com/ladvien/bevy_debugger_mcp.git
cd bevy_debugger_mcp
cargo build --release

# Install (macOS/Linux)
./scripts/install.sh

# Setup Claude Code integration
./scripts/setup-claude.sh
```

### Setup Your Bevy Game

Add the RemotePlugin to your Bevy app:

```rust
use bevy::prelude::*;
use bevy::remote::RemotePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default()) // Enable remote debugging
        .run();
}
```

```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.14", features = ["remote"] }
```

### Start Debugging

1. **Run your Bevy game**: `cargo run`
2. **Open Claude Code** in your project directory
3. **Start debugging**: Try commands like:
   - "Show me all entities in the game"
   - "Monitor the player's health component"
   - "Test what happens when I spawn 100 enemies"
   - "Record this gameplay session for analysis"

## 🎮 Example Usage

```markdown
Human: My player is randomly teleporting. Can you help debug this?

Claude: I'll help investigate the teleportation issue. Let me examine the player's Transform component and movement system.

[Uses MCP tools to observe player entity, analyze movement patterns, and identify the bug]

I found the issue! The player's position is being reset every frame due to a conflicting movement system. The `PlayerController` and `PhysicsSystem` are both trying to control the transform simultaneously.
```

## 🛠️ Configuration

The server uses environment variables for configuration:

```bash
export BEVY_BRP_HOST=localhost    # Bevy Remote Protocol host
export BEVY_BRP_PORT=15702        # Bevy Remote Protocol port  
export MCP_PORT=3000              # MCP server port (not used in stdio mode)
export RUST_LOG=info              # Logging level
```

## 📁 Project Structure

```
bevy_debugger_mcp/
├── src/
│   ├── main.rs              # Entry point with stdio/TCP transport
│   ├── mcp_server.rs        # MCP protocol implementation
│   ├── brp_client.rs        # Bevy Remote Protocol client
│   ├── tools/               # Debugging tool implementations
│   │   ├── observe.rs       # Entity/component observation
│   │   ├── experiment.rs    # Game state experimentation
│   │   ├── stress.rs        # Performance stress testing
│   │   └── ...
│   └── ...
├── scripts/                 # Installation and management scripts
├── docs/                    # Documentation
├── tests/                   # Integration tests
└── README.md
```

## 🧪 Available Tools

| Tool | Description |
|------|-------------|
| `observe` | Monitor game entities, components, and resources |
| `experiment` | Test changes to game state with automatic rollback |
| `stress` | Performance testing and bottleneck identification |
| `anomaly` | Detect unusual patterns in game behavior |
| `replay` | Record and replay debugging sessions |
| `orchestrate` | Chain multiple debugging operations |

## 🖥️ Platform Support

| Platform | Installation | Status |
|----------|--------------|--------|
| **macOS** | `./scripts/install.sh` | ✅ Full support with LaunchAgent service |
| **Linux** | `./scripts/install.sh` | ✅ Full support |
| **Windows** | Manual build | ⚠️ Basic support (help wanted) |

### macOS Service Management

On macOS, the debugger can run as a background service:

```bash
# Service management
./scripts/service.sh start      # Start background service
./scripts/service.sh stop       # Stop service
./scripts/service.sh status     # Check status
./scripts/service.sh logs       # View logs
```

## 🤝 Contributing

We welcome contributions! Please see our [contribution guidelines](CONTRIBUTING.md).

```bash
# Development setup
git clone https://github.com/ladvien/bevy_debugger_mcp.git
cd bevy_debugger_mcp
cargo test                      # Run tests
cargo fmt                       # Format code
cargo clippy                    # Lint code
```

## 📚 Documentation

- **[Usage Guide](docs/USAGE_GUIDE.md)** - Detailed feature documentation
- **[Claude Prompts](docs/CLAUDE_SUBAGENT_GUIDE.md)** - Effective prompting strategies
- **[macOS Service Setup](docs/MACOS_SERVICE.md)** - Background service configuration

## 🔒 Security & Privacy

- All communication happens locally between your game and Claude Code
- No game data is transmitted externally
- Sensitive information is automatically redacted from logs
- Debug recordings are stored locally and encrypted

## 📄 License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built for the [Bevy Engine](https://bevyengine.org/) community
- Powered by [Anthropic's MCP](https://modelcontextprotocol.io/)
- Inspired by the need for better game debugging tools

---

**Questions?** Open an [issue](https://github.com/ladvien/bevy_debugger_mcp/issues) or join the discussion in [Bevy's Discord](https://discord.gg/bevy).
# Bevy Debugger MCP Server

[![Build Status](https://img.shields.io/badge/build-passing-brightgreen)](https://github.com/ladvien/bevy_debugger_mcp)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70+-orange.svg)](https://www.rust-lang.org)
[![Bevy](https://img.shields.io/badge/bevy-0.14+-purple.svg)](https://bevyengine.org)

A powerful Model Context Protocol (MCP) server that enables AI-assisted debugging of Bevy games through Claude Code. Debug your game state, analyze performance, and test hypotheses with natural language commands.

## 🎯 Quick Start (5 minutes)

```bash
# 1. Install the debugger
cargo install bevy_debugger_mcp

# 2. Add RemotePlugin to your Bevy game
# In your game's main.rs:
# .add_plugins(RemotePlugin::default())

# 3. Configure Claude Code (add to ~/.config/claude/claude_code_config.json)
echo '{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": []
    }
  }
}' >> ~/.config/claude/claude_code_config.json

# 4. Run your game and start debugging!
cargo run  # In your game directory
# Then open Claude Code and say: "Help me debug my Bevy game"
```

## ✨ Features

- **🔍 Real-time Observation**: Monitor entities, components, and resources as your game runs
- **🧪 Smart Experimentation**: Test game behavior changes with automatic rollback
- **📊 Performance Analysis**: Identify bottlenecks and optimize game performance  
- **🚨 Anomaly Detection**: Automatically spot unusual patterns in game behavior
- **📹 Session Recording**: Record and replay debugging sessions for analysis
- **📸 Screenshot Capture**: Take window-specific screenshots of your game for visual debugging
- **🛡️ Error Recovery**: Robust error handling with automatic diagnostics
- **🤖 ML-Powered Suggestions**: Learn from debugging patterns to provide better recommendations
- **📈 Performance Budgets**: Set and monitor performance targets with automatic alerts
- **🔄 Workflow Automation**: Automate common debugging tasks with safety checkpoints

## 🏗️ How It Works

The Bevy Debugger MCP creates a bridge between Claude Code and your Bevy game:

```
┌─────────────┐     MCP Protocol      ┌──────────────────┐     BRP Protocol    ┌─────────────┐
│ Claude Code │ ◄──────────────────► │ bevy-debugger-mcp │ ◄─────────────────► │ Your Bevy   │
│   (AI)      │    stdio/TCP          │    (Server)       │    WebSocket       │    Game     │
└─────────────┘                       └──────────────────┘                      └─────────────┘
     │                                         │                                       │
     │ "Find memory leaks"                    │                                       │
     └────────────────────►                   │                                       │
                                              │ Query entities & components           │
                                              └──────────────────────────────────────►│
                                              │                                       │
                                              │◄──────────────────────────────────────┤
                                              │     Entity data & metrics             │
     │◄────────────────────                   │                                       │
     │ "Found 500 orphaned                    │                                       │
     │  bullet entities"                      │                                       │
```

### Architecture Components

1. **Claude Code (AI Agent)**: Natural language interface for debugging commands
2. **MCP Server**: Translates AI requests into game debugging operations
3. **BRP Client**: Communicates with your Bevy game via WebSocket
4. **RemotePlugin**: Bevy plugin that exposes game internals for debugging
5. **Debug Tools**: 11 specialized tools for different debugging tasks

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ and Cargo
- Claude Code CLI
- A Bevy game with [RemotePlugin](https://docs.rs/bevy/latest/bevy/remote/struct.RemotePlugin.html) enabled

### Installation

```bash
# Install from crates.io (recommended)
cargo install bevy_debugger_mcp

# Or clone and build from source
git clone https://github.com/ladvien/bevy_debugger_mcp.git
cd bevy_debugger_mcp
cargo build --release

# Install (macOS/Linux)
./scripts/install.sh

# Setup Claude Code integration
./scripts/setup-claude.sh
```

### Server Management with `bevy-debugger-control`

The `bevy-debugger-control` script is automatically installed with the package and provides complete lifecycle management for the MCP server. This solves the common issue of the server hanging when run directly.

#### Basic Commands

```bash
# Start the server in the background
bevy-debugger-control start

# Stop the server gracefully
bevy-debugger-control stop

# Restart the server (useful after configuration changes)
bevy-debugger-control restart

# Check if the server is running and view details
bevy-debugger-control status

# View server logs
bevy-debugger-control logs

# Follow logs in real-time (like tail -f)
bevy-debugger-control logs -f

# Clean up old log files
bevy-debugger-control clean

# Show help and all available commands
bevy-debugger-control help
```

#### Advanced Usage

```bash
# Start server on a different port
BEVY_DEBUGGER_PORT=3002 bevy-debugger-control start

# Start with custom Bevy host/port
BEVY_BRP_HOST=192.168.1.100 BEVY_BRP_PORT=15703 bevy-debugger-control start

# Clean all logs including current
bevy-debugger-control clean --all

# Check server status with process details
bevy-debugger-control status
# Output shows:
# - PID of running process
# - CPU and memory usage
# - Port binding status
# - Recent log entries
```

#### File Locations

The control script manages the following files:
- **Logs**: `~/.bevy-debugger/bevy-debugger.log`
- **PID file**: `~/.bevy-debugger/bevy-debugger.pid`
- **Rotated logs**: `~/.bevy-debugger/bevy-debugger.log.*`

#### Troubleshooting

If the server fails to start:
```bash
# Check the logs for errors
bevy-debugger-control logs

# Ensure no other instance is running
bevy-debugger-control stop
bevy-debugger-control start

# Verify the binary is installed
which bevy-debugger-mcp

# Check if port is already in use
lsof -i :3001  # or your configured port
```

### Setup Your Bevy Game

Add the RemotePlugin to your Bevy app:

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
                .with_method("bevy_debugger/screenshot", screenshot_handler)
        )
        .run();
}

// Enable screenshot functionality
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

```toml
# Cargo.toml
[dependencies]
bevy = { version = "0.16", features = ["default", "bevy_remote"] }
```

### Start Debugging

1. **Run your Bevy game**: `cargo run`
2. **Open Claude Code** in your project directory
3. **Start debugging**: Try commands like:
   - "Show me all entities in the game"
   - "Monitor the player's health component"
   - "Test what happens when I spawn 100 enemies"
   - "Take a screenshot of the current game state"
   - "Record this gameplay session for analysis"

## 🤖 Claude Code Integration Guide

### Setting Up Claude Code

1. **Install the MCP Server** (v0.1.6 or later):
```bash
cargo install bevy_debugger_mcp
```

2. **Configure Claude Code** - Add to your Claude Code settings:

**macOS/Linux**: `~/.config/claude/claude_code_config.json`
**Windows**: `%APPDATA%\claude\claude_code_config.json`

```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "type": "stdio",
      "env": {
        "BEVY_BRP_HOST": "localhost",
        "BEVY_BRP_PORT": "15702",
        "RUST_LOG": "info"
      }
    }
  }
}
```

3. **Verify Installation**:
```bash
# Check version
bevy-debugger-mcp --help

# Test the MCP server responds correctly
echo '{"jsonrpc": "2.0", "method": "initialize", "params": {"capabilities": {}}, "id": 1}' | bevy-debugger-mcp --stdio
# Should return: {"id":1,"jsonrpc":"2.0","result":{"capabilities":...}}

# Verify tools are available
echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 2}' | bevy-debugger-mcp --stdio
# Should list: observe, experiment, stress, anomaly, replay, hypothesis, screenshot
```

### How Claude Uses the MCP Tools

When you ask Claude to debug your Bevy game, it has access to powerful MCP tools that communicate with your running game:

#### 🔍 **Observation Tools**
Claude can monitor your game state in real-time:
```markdown
You: "Show me all enemies in the game"
Claude: [Uses observe tool to query entities with Enemy component]
        "I found 5 enemies. Here are their positions and health values..."

You: "Track the player's velocity over time"
Claude: [Uses observe tool with continuous monitoring]
        "The player's velocity spikes to 500 units when jumping, which seems abnormal..."
```

#### 🧪 **Experimentation Tools**
Claude can test hypotheses by modifying game state:
```markdown
You: "Test what happens if we spawn 100 enemies at once"
Claude: [Uses experiment tool to spawn entities and measure performance]
        "Spawning 100 enemies causes frame rate to drop to 15 FPS. The bottleneck is..."

You: "Try doubling the player's speed and see if collision still works"
Claude: [Uses experiment tool to modify components with automatic rollback]
        "At 2x speed, the player clips through thin walls. The physics system needs..."
```

#### 🚨 **Anomaly Detection**
Claude can identify unusual patterns:
```markdown
You: "Something is causing random lag spikes"
Claude: [Uses anomaly tool to monitor performance metrics]
        "I detected memory allocations spike every 5 seconds, coinciding with..."

You: "Find any entities behaving strangely"
Claude: [Uses anomaly tool with pattern matching]
        "Entity #42 is teleporting between positions, likely due to..."
```

#### 📊 **Performance Analysis**
Claude can profile and optimize:
```markdown
You: "Why is my game running slowly?"
Claude: [Uses stress tool to identify bottlenecks]
        "The render system is taking 45ms per frame. The main issue is..."

You: "Optimize the enemy AI system"
Claude: [Uses performance profiling tools]
        "The pathfinding is recalculating every frame. We can cache results..."
```

### Real-World Debugging Sessions

#### Example 1: Physics Bug
```markdown
Human: My character falls through the floor randomly

Claude: I'll investigate this physics issue. Let me start by observing the player's Transform and Collider components.

[Uses observe tool to monitor player entity]
I'm tracking the player's Y position... I notice it suddenly changes from 0.0 to -100.0 without intermediate values.

[Uses experiment tool to test collision]
Let me test the collision system by spawning a test entity at the player's position...
The collision detection is failing when the player's velocity exceeds 1000 units/frame.

Solution: Add a velocity cap or use continuous collision detection for high-speed entities.
```

#### Example 2: Performance Investigation
```markdown
Human: The game freezes every few seconds

Claude: I'll analyze the performance patterns to identify the cause of these freezes.

[Uses anomaly detection on frame times]
I've detected periodic spikes in frame time every 3.2 seconds, lasting about 200ms.

[Uses observe tool on system metrics]
Monitoring system activity... The "EnemySpawnSystem" is creating 50 entities at once every 3 seconds.

[Uses experiment tool to test optimization]
Let me test spreading the spawning over multiple frames...
Spawning 5 entities per frame over 10 frames eliminates the freezes.

Solution: Implement batched spawning to distribute the load across frames.
```

#### Example 3: Visual Debugging
```markdown
Human: Can you show me what the AI sees?

Claude: I'll visualize the AI's perception system for you.

[Uses visual overlay tools]
I'm enabling debug overlays for:
- AI vision cones (showing field of view)
- Pathfinding waypoints (showing planned routes)  
- Detection ranges (showing awareness zones)

[Takes screenshot]
Here's a screenshot showing the AI's perception. The red cones are vision, blue lines are paths.

The AI at position (100, 50) can't see the player because there's an obstacle blocking line of sight.
```

### Advanced Usage Patterns

#### Continuous Monitoring
```markdown
Human: Monitor the game for any issues while I playtest

Claude: I'll set up continuous monitoring for common issues.

[Sets up multiple observe and anomaly detection tasks]
Monitoring:
- Memory usage (alerting if >2GB)
- Frame time (alerting if >16ms)
- Entity count (alerting if >10000)
- Error patterns in components

[After 5 minutes]
Alert: Memory usage increased by 500MB in the last minute.
Investigating... Found 5000 orphaned bullet entities not being destroyed.
```

#### Automated Testing
```markdown
Human: Test if my save system works correctly

Claude: I'll test the save/load system comprehensively.

[Uses orchestration tool to chain operations]
1. Capturing current game state...
2. Modifying player position, inventory, and score...
3. Triggering save...
4. Resetting game...
5. Triggering load...
6. Comparing states...

Results: Save system works correctly for position and score, but inventory items are duplicated on load.
```

### Troubleshooting Claude Code Connection

If Claude can't connect to your game:

```bash
# 1. Ensure your Bevy game is running with RemotePlugin
# Check if Bevy is listening on the correct port:
lsof -i :15702  # Should show your Bevy game process

# 2. Test the MCP server standalone:
bevy-debugger-mcp --help  # Should show version 0.1.6
bevy-debugger-mcp --stdio  # Should wait for input (Ctrl+C to exit)

# 3. Verify Claude Code configuration:
cat ~/.config/claude/claude_code_config.json
# Should contain the bevy-debugger configuration

# 4. Test manual MCP connection:
echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 2}' | bevy-debugger-mcp --stdio
# Should return list of available tools

# 5. Check direct Bevy connection:
curl -X POST http://localhost:15702/query \
  -H "Content-Type: application/json" \
  -d '{"method": "bevy/list", "params": {}}'
# Should return Bevy data

# 6. Enable debug logging:
RUST_LOG=debug bevy-debugger-mcp --stdio

# 7. Common issues:
# - Port 15702 blocked by firewall
# - Bevy game not compiled with bevy_remote feature
# - Multiple MCP servers running on same port
# - Claude Code needs restart after config changes
```

### Performance Characteristics

The debugger is designed for minimal impact on your game:

| Metric | Target | Actual | Notes |
|--------|--------|--------|-------|
| **Idle Overhead** | <5% | <3% | When connected but not actively debugging |
| **Active Overhead** | <10% | <7% | During active debugging operations |
| **Memory Usage** | <50MB | ~30MB | Includes caching and session data |
| **Startup Time** | <1s | ~500ms | With lazy initialization |
| **Command Latency** | <200ms | <50ms | For simple queries |
| **Complex Query** | <1s | ~200ms | For queries returning 1000+ entities |

### Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| **"Failed to connect to BRP"** | Ensure your Bevy game is running with `RemotePlugin` enabled |
| **"No tools available"** | Update to v0.1.6: `cargo install bevy_debugger_mcp --force` |
| **High CPU usage** | Reduce monitoring frequency or use `--tcp` mode instead of stdio |
| **Screenshot not working** | Add screenshot handler to your Bevy game (see setup example) |
| **Memory leak detection false positives** | Adjust detection thresholds in anomaly tool |
| **Claude not responding** | Restart Claude Code after configuration changes |

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

## 📚 Complete Developer Workflow

### Step 1: Prepare Your Bevy Game

```rust
// In your main.rs or lib.rs
use bevy::prelude::*;
use bevy::remote::RemotePlugin;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(RemotePlugin::default()) // Essential for debugging
        .add_systems(Update, your_game_systems)
        .run();
}
```

### Step 2: Install and Configure

```bash
# Install the debugger
cargo install bevy_debugger_mcp

# Add to Claude Code config (~/.config/claude/claude_code_config.json)
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": []
    }
  }
}
```

### Step 3: Start Debugging Session

1. **Run your Bevy game**: `cargo run`
2. **Open Claude Code** in your project
3. **Start debugging** with natural language:
   - "What entities are in my game?"
   - "Why is performance dropping?"
   - "Monitor the player's health"
   - "Test spawning 1000 enemies"

## 🧪 Complete Debugging Toolkit (v0.1.6)

### Core MCP Tools

| Tool | Description | Example Usage |
|------|-------------|---------------|
| `observe` | Monitor game entities, components, and resources in real-time | "Show me all entities with Health component" |
| `experiment` | Test changes to game state with automatic rollback | "Set player speed to 2x and test collision" |
| `stress` | Performance testing and bottleneck identification | "Stress test the physics system with 500 objects" |
| `anomaly` | Detect unusual patterns in game behavior | "Find any entities with abnormal velocities" |
| `replay` | Record and replay debugging sessions | "Record the next 30 seconds of gameplay" |
| `hypothesis` | Test specific assumptions about game behavior | "Test if framerate drops when spawning > 100 enemies" |
| `screenshot` | Capture game window visuals with timing control | "Take a screenshot after 2 seconds warmup" |

### Internal Debug Commands (11 Integrated Tools)

The debugger provides 11 specialized debugging tools accessible through the MCP protocol:

| Tool | Purpose | Key Features |
|------|---------|--------------|
| **EntityInspector** | Deep entity analysis | • Component inspection<br>• Relationship tracking<br>• Change detection |
| **SystemProfiler** | System performance analysis | • Microsecond precision<br>• Dependency tracking<br>• <3% overhead |
| **VisualDebugOverlay** | In-game debug visualization | • Entity highlights<br>• Collider visualization<br>• Performance metrics |
| **QueryBuilder** | Type-safe ECS queries | • Natural language queries<br>• Query validation<br>• Result caching |
| **MemoryProfiler** | Memory usage tracking | • Allocation tracking<br>• Leak detection<br>• Usage patterns |
| **SessionManager** | Debug session management | • Session recording<br>• Checkpoint creation<br>• State comparison |
| **IssueDetector** | Automated issue detection | • 17 detection patterns<br>• Real-time monitoring<br>• Auto-diagnostics |
| **PerformanceBudgetMonitor** | Performance budget enforcement | • Frame time budgets<br>• Memory limits<br>• Violation tracking |
| **PatternLearningSystem** | ML-based pattern recognition | • Privacy-preserving (k=5)<br>• Pattern mining<br>• Suggestion generation |
| **SuggestionEngine** | Context-aware suggestions | • Based on learned patterns<br>• Confidence scoring<br>• Action recommendations |
| **WorkflowAutomation** | Automated debug workflows | • Common task automation<br>• Safety checkpoints<br>• Rollback support |

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
cargo test                      # Run basic tests
cargo test --ignored           # Run full integration tests
cargo test screenshot_integration_wrapper::test_screenshot_ci_suite  # Fast screenshot tests
cargo fmt                       # Format code
cargo clippy                    # Lint code
```

### Running Screenshot Tests

The screenshot functionality has comprehensive test coverage:

```bash
# Fast screenshot tests (suitable for CI/development)
cargo test screenshot_integration_wrapper::test_screenshot_ci_suite

# Full screenshot integration suite  
cargo test screenshot_integration_wrapper::test_screenshot_integration_suite -- --ignored

# Individual test categories
cargo test screenshot_integration_wrapper::test_screenshot_utilities
cargo test screenshot_integration_wrapper::test_screenshot_basic_functionality
cargo test screenshot_integration_wrapper::test_screenshot_parameter_validation
cargo test screenshot_integration_wrapper::test_screenshot_timing_controls

# Performance testing
cargo test screenshot_integration_wrapper::test_screenshot_performance
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

## 📦 Changelog

### v0.1.6 (Latest) - Production Ready
- ✅ All 11 debugging tools fully integrated and operational
- ✅ Fixed critical async initialization issues
- ✅ Enhanced error handling and sensitive data sanitization
- ✅ Performance optimizations with lazy initialization
- ✅ Comprehensive test coverage (232+ tests)
- ✅ Machine learning pattern recognition with privacy preservation
- ✅ Workflow automation for common debugging tasks
- ✅ Production-ready with <3% performance overhead

### v0.1.5
- Added GPL-3.0 license compliance
- Initial pattern learning system
- Enhanced error context

### v0.1.4
- Improved Claude Code integration
- Added suggestion engine
- Bug fixes

## 📄 License

This project is licensed under the GNU General Public License v3.0 - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Built for the [Bevy Engine](https://bevyengine.org/) community
- Powered by [Anthropic's MCP](https://modelcontextprotocol.io/)
- Inspired by the need for better game debugging tools

---

**Questions?** Open an [issue](https://github.com/ladvien/bevy_debugger_mcp/issues) or join the discussion in [Bevy's Discord](https://discord.gg/bevy).
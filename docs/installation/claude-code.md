# Claude Code Integration Guide

*Complete guide to integrating the Bevy Debugger MCP with Claude Code*

## Overview

Claude Code is Anthropic's official CLI that supports the Model Context Protocol (MCP). This guide covers everything you need to know about setting up and using the Bevy Debugger MCP with Claude Code.

## Prerequisites

- Claude Code CLI installed
- Bevy Debugger MCP server installed (`cargo install bevy_debugger_mcp`)
- Basic familiarity with command line

## Configuration

### 1. Locate Your Claude Code Configuration

The configuration file location varies by platform:

- **macOS/Linux**: `~/.config/claude/claude_code_config.json`
- **Windows**: `%APPDATA%\claude\claude_code_config.json`

### 2. Add MCP Server Configuration

Edit the configuration file and add the Bevy Debugger server:

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

### 3. Configuration Options

#### Basic Configuration (Recommended)
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

#### Advanced Configuration
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
        "MCP_PORT": "3001",
        "RUST_LOG": "debug",
        "BEVY_DEBUGGER_TIMEOUT": "30",
        "BEVY_DEBUGGER_MAX_RETRIES": "3"
      },
      "disabled": false
    }
  }
}
```

#### Remote Game Configuration
```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "env": {
        "BEVY_BRP_HOST": "192.168.1.100",
        "BEVY_BRP_PORT": "15702"
      }
    }
  }
}
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `BEVY_BRP_HOST` | `localhost` | Hostname where your Bevy game is running |
| `BEVY_BRP_PORT` | `15702` | Port for Bevy Remote Protocol |
| `MCP_PORT` | `3001` | Port for MCP server (TCP mode only) |
| `RUST_LOG` | `info` | Logging level (trace, debug, info, warn, error) |
| `BEVY_DEBUGGER_TIMEOUT` | `30` | Connection timeout in seconds |
| `BEVY_DEBUGGER_MAX_RETRIES` | `3` | Maximum connection retry attempts |

## Verification

### 1. Test MCP Server Standalone

```bash
# Test the server responds correctly
bevy-debugger-mcp --help

# Test MCP protocol handshake
echo '{"jsonrpc": "2.0", "method": "initialize", "params": {"capabilities": {}}, "id": 1}' | bevy-debugger-mcp --stdio
```

Expected response:
```json
{"id":1,"jsonrpc":"2.0","result":{"capabilities":{"tools":{"listChanged":false},"resources":{"subscribe":false,"listChanged":false},"prompts":{"listChanged":false},"logging":{}},"serverInfo":{"name":"bevy-debugger","version":"0.1.8"},"protocolVersion":"2024-11-05"}}
```

### 2. Test Tool Discovery

```bash
# List available tools
echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 2}' | bevy-debugger-mcp --stdio
```

Expected tools: `observe`, `experiment`, `stress_test`, `detect_anomaly`, `replay`, `hypothesis`

### 3. Test with Claude Code

1. **Start Claude Code** in your project directory
2. **Run a test command**:
   ```
   Check if the Bevy debugger connection is working
   ```

You should see Claude attempt to connect and report the status.

## Usage Patterns

### Basic Debugging Session

```markdown
You: Show me all entities in my game

Claude: I'll check your game's entities using the observe tool.
[Uses observe tool to query entities]
Found 5 entities in your game:
- Entity 0: Transform, Camera 
- Entity 1: Transform, Sprite, Player
- Entity 2-4: Transform, Enemy
```

### Performance Analysis

```markdown
You: My game is running slowly, can you help?

Claude: I'll analyze your game's performance to identify bottlenecks.
[Uses stress_test and observe tools]
I found the issue: 500 bullet entities are being created but not destroyed...
```

### Hypothesis Testing

```markdown
You: I think my collision detection is broken

Claude: Let me test your collision system systematically.
[Uses hypothesis and experiment tools]
Testing collision detection... The issue occurs when entities move faster than 100 units/frame...
```

## Advanced Features

### Multiple MCP Servers

You can run multiple MCP servers simultaneously:

```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"]
    },
    "other-tool": {
      "command": "other-mcp-server",
      "args": ["--stdio"]
    }
  }
}
```

### Development vs Production Configs

**Development configuration** (`dev` profile):
```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "env": {
        "RUST_LOG": "debug",
        "BEVY_DEBUGGER_TIMEOUT": "60"
      }
    }
  }
}
```

**Production configuration** (`prod` profile):
```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "env": {
        "RUST_LOG": "warn",
        "BEVY_DEBUGGER_TIMEOUT": "10"
      }
    }
  }
}
```

## Troubleshooting

### Claude Code Doesn't See the Tools

**Symptoms**: Claude responds "I don't have access to debugging tools"

**Solutions**:
1. **Restart Claude Code** after configuration changes
2. **Check configuration syntax**:
   ```bash
   cat ~/.config/claude/claude_code_config.json | jq .
   ```
3. **Verify server path**:
   ```bash
   which bevy-debugger-mcp
   ```

### Connection Timeouts

**Symptoms**: "Failed to connect to game" errors

**Solutions**:
1. **Increase timeout**:
   ```json
   "env": {
     "BEVY_DEBUGGER_TIMEOUT": "60"
   }
   ```
2. **Check game is running**:
   ```bash
   lsof -i :15702
   ```

### Permission Errors

**Symptoms**: "Permission denied" when starting server

**Solutions**:
1. **Check executable permissions**:
   ```bash
   ls -la $(which bevy-debugger-mcp)
   chmod +x $(which bevy-debugger-mcp)
   ```
2. **Use absolute path**:
   ```json
   "command": "/full/path/to/bevy-debugger-mcp"
   ```

### Logging and Debugging

Enable detailed logging to diagnose issues:

```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "env": {
        "RUST_LOG": "trace,bevy_debugger_mcp=trace"
      }
    }
  }
}
```

View logs in Claude Code output or capture to file:
```bash
RUST_LOG=debug bevy-debugger-mcp --stdio 2>&1 | tee debug.log
```

## Performance Impact

The MCP server is designed for minimal impact:

| Metric | Impact |
|--------|--------|
| **Claude Code Startup** | +200-500ms |
| **Memory Usage** | +20-30MB |
| **CPU (Idle)** | <1% |
| **CPU (Active)** | <5% |

## Security Considerations

- All communication happens locally via stdio
- No network connections required
- Game data stays on your machine
- Sensitive information is automatically redacted from logs

## Best Practices

### 1. Configuration Management
- Use version control for your Claude Code config
- Create separate configs for different projects
- Document custom environment variables

### 2. Performance Optimization
- Use `info` log level in production
- Increase timeouts for complex games
- Monitor resource usage during long debugging sessions

### 3. Debugging Workflow
- Start with simple observations
- Use hypothesis-driven debugging
- Combine multiple tools for comprehensive analysis
- Document findings in your codebase

## Integration Examples

### CI/CD Pipeline Integration

```yaml
# .github/workflows/debug-test.yml
- name: Test MCP Integration
  run: |
    cargo install bevy_debugger_mcp
    echo '{"jsonrpc": "2.0", "method": "tools/list", "id": 1}' | bevy-debugger-mcp --stdio
```

### Project-Specific Config

Create `.claude/config.json` in your project:
```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "env": {
        "BEVY_BRP_PORT": "15703",
        "RUST_LOG": "debug"
      }
    }
  }
}
```

## What's Next?

- **[Bevy Setup Guide](bevy-setup.md)**: Configure your Bevy game
- **[Tool Usage Examples](../tools/)**: Learn specific debugging techniques
- **[Advanced Configuration](../api-reference.md)**: Full configuration reference

---

*Need help? Check the [troubleshooting guide](../troubleshooting.md) or open an issue.*
# macOS Service Setup Guide

This guide explains how to set up the Bevy Debugger MCP server as a background service on macOS using LaunchAgent.

## 🚀 Quick Start

```bash
# Install as a service
./scripts/install.sh

# Start the service
./scripts/service.sh start

# Check status
./scripts/service.sh status
```

## 📋 Installation

### Automatic Installation

The installation script will:
1. Build the binary if needed
2. Install to `/usr/local/bin/bevy-debugger-mcp`
3. Create configuration files
4. Set up LaunchAgent for background operation
5. Create log directories

```bash
./scripts/install.sh
```

### Manual Installation Steps

If you prefer to understand each step:

1. **Build the binary**:
   ```bash
   cargo build --release
   ```

2. **Install the binary**:
   ```bash
   sudo cp target/release/bevy_debugger_mcp /usr/local/bin/bevy-debugger-mcp
   sudo chmod +x /usr/local/bin/bevy-debugger-mcp
   ```

3. **Create directories**:
   ```bash
   mkdir -p ~/.config/bevy-debugger
   mkdir -p ~/Library/Application\ Support/bevy-debugger
   mkdir -p ~/Library/Logs/bevy-debugger
   ```

4. **Install LaunchAgent**:
   ```bash
   sed "s|HOME_DIR|${HOME}|g" launchd/com.bevy-debugger-mcp.plist > ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   ```

## 🛠️ Service Management

### Basic Commands

```bash
# Start/Stop
./scripts/service.sh start
./scripts/service.sh stop
./scripts/service.sh restart

# Status and Information
./scripts/service.sh status
./scripts/service.sh info
./scripts/service.sh pid

# Logs
./scripts/service.sh logs
```

### Auto-Start Configuration

```bash
# Enable auto-start at login
./scripts/service.sh enable

# Disable auto-start
./scripts/service.sh disable
```

### Testing and Health Checks

```bash
# Test connection to Bevy game
./scripts/service.sh test

# Check MCP server health
./scripts/service.sh health
```

## ⚙️ Configuration

### Configuration File

The service reads configuration from `~/.config/bevy-debugger/config.toml`:

```toml
[connection]
bevy_host = "localhost"    # Bevy game hostname
bevy_port = 15702          # Bevy Remote Protocol port
mcp_port = 3000            # MCP server port

[logging]
level = "info"             # Log level: error, warn, info, debug, trace
debug_mode = false         # Enable verbose debugging

[features]
auto_reconnect = true      # Automatically reconnect to Bevy game
checkpoint_interval = 1000 # Checkpoint frequency (frames)
max_recording_size = "100MB" # Maximum recording file size
```

### Editing Configuration

```bash
# Edit configuration with your default editor
./scripts/service.sh config

# Or edit manually
vim ~/.config/bevy-debugger/config.toml

# Reload configuration
./scripts/service.sh reload
```

### Environment Variables

The LaunchAgent sets these environment variables:

- `RUST_LOG=info`
- `BEVY_BRP_HOST=localhost`
- `BEVY_BRP_PORT=15702`
- `MCP_PORT=3000`

## 📊 Monitoring

### Service Status

```bash
$ ./scripts/service.sh status
INFO: Checking bevy-debugger-mcp status...
STATUS: Service is RUNNING
  LaunchAgent: 12345  0  com.bevy-debugger-mcp
  PID: 12345
  Process: 0.1 0.2 00:05:30 bevy-debugger-mcp serve
STATUS: MCP Server: LISTENING on port 3000
WARNING: Bevy Game: NOT DETECTED on port 15702
```

### Log Files

Logs are stored in `~/Library/Logs/bevy-debugger/`:

- `stdout.log` - Standard output
- `stderr.log` - Errors and warnings

```bash
# Follow logs in real-time
./scripts/service.sh logs

# View specific log files
tail -f ~/Library/Logs/bevy-debugger/stderr.log
```

### Process Information

```bash
# Get process ID
./scripts/service.sh pid

# Detailed service information
./scripts/service.sh info
```

## 🔧 Troubleshooting

### Service Won't Start

1. Check if binary exists:
   ```bash
   ls -la /usr/local/bin/bevy-debugger-mcp
   ```

2. Validate LaunchAgent plist:
   ```bash
   plutil -lint ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   ```

3. Check logs:
   ```bash
   tail -f ~/Library/Logs/bevy-debugger/stderr.log
   ```

### Port Conflicts

If port 3000 is already in use:

1. Find what's using the port:
   ```bash
   lsof -i :3000
   ```

2. Change the port in configuration:
   ```bash
   ./scripts/service.sh config
   # Edit mcp_port = 3001
   ```

3. Restart the service:
   ```bash
   ./scripts/service.sh restart
   ```

### Connection Issues

1. Test Bevy game connection:
   ```bash
   ./scripts/service.sh test
   ```

2. Check if Bevy game has RemotePlugin:
   ```rust
   App::new()
       .add_plugins(DefaultPlugins)
       .add_plugins(RemotePlugin::default()) // ← This line
       .run();
   ```

3. Verify ports are correct:
   ```bash
   netstat -an | grep LISTEN | grep -E "3000|15702"
   ```

### Permission Issues

If you get permission errors:

1. Fix binary permissions:
   ```bash
   sudo chmod +x /usr/local/bin/bevy-debugger-mcp
   ```

2. Fix directory permissions:
   ```bash
   chmod 755 ~/.config/bevy-debugger
   chmod 755 ~/Library/Application\ Support/bevy-debugger
   ```

## 🗑️ Uninstallation

### Automatic Uninstallation

```bash
./scripts/uninstall.sh
```

### Manual Uninstallation

1. Stop and remove service:
   ```bash
   ./scripts/service.sh stop
   launchctl unload ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   rm ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   ```

2. Remove binary:
   ```bash
   sudo rm /usr/local/bin/bevy-debugger-mcp
   ```

3. Remove directories (optional):
   ```bash
   rm -rf ~/.config/bevy-debugger
   rm -rf ~/Library/Application\ Support/bevy-debugger
   rm -rf ~/Library/Logs/bevy-debugger
   ```

## 🏗️ Advanced Usage

### Custom LaunchAgent Configuration

You can modify the LaunchAgent behavior by editing the plist file:

```bash
vim ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
```

Key settings:
- `RunAtLoad`: Start automatically when loaded
- `KeepAlive`: Restart if the process crashes
- `ThrottleInterval`: Delay between restart attempts
- `ProcessType`: Background process type
- `Nice`: Process priority

### Integration with Development Workflow

Add to your game's development script:

```bash
#!/bin/bash
# start-dev.sh

# Start the MCP service
./scripts/service.sh start

# Start your Bevy game
cargo run --features debug

# Optionally stop the service when done
# ./scripts/service.sh stop
```

### Multiple Configurations

You can run multiple configurations by:

1. Creating different config files
2. Using different ports
3. Running separate instances

```bash
# Start with custom config
bevy-debugger-mcp serve --config ~/.config/bevy-debugger/game1.toml
```

## 📚 Additional Resources

- [LaunchAgent Documentation](https://developer.apple.com/library/archive/documentation/MacOSX/Conceptual/BPSystemStartup/Chapters/CreatingLaunchdJobs.html)
- [Bevy Remote Protocol](https://docs.rs/bevy/latest/bevy/remote/index.html)
- [Claude Code MCP Documentation](https://docs.anthropic.com/claude/docs/mcp)

## ⚡ Tips

1. **Use the service for development**: Keep it running in the background while developing games
2. **Monitor logs**: Use `./scripts/service.sh logs` to watch for connection issues
3. **Test connections**: Use `./scripts/service.sh test` to verify your Bevy game setup
4. **Auto-start for convenience**: Use `./scripts/service.sh enable` for automatic startup
5. **Configuration management**: Keep different configs for different projects
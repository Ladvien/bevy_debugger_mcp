# Troubleshooting Guide - Bevy Debugger MCP

This guide helps you diagnose and resolve common issues when using the Bevy Debugger MCP server.

## Table of Contents

1. [Connection Issues](#connection-issues)
2. [Performance Problems](#performance-problems)  
3. [Command Failures](#command-failures)
4. [Screenshot Issues](#screenshot-issues)
5. [Integration Test Failures](#integration-test-failures)
6. [Configuration Problems](#configuration-problems)
7. [Platform-Specific Issues](#platform-specific-issues)
8. [Advanced Diagnostics](#advanced-diagnostics)

---

## Connection Issues

### Issue 1: "BRP client not connected" Error

**Symptoms**: MCP tools fail with "BRP client not connected to Bevy game"

**Causes**:
- Bevy game is not running
- RemotePlugin not enabled in Bevy app
- Incorrect host/port configuration
- Firewall blocking connections

**Solutions**:
1. **Verify Bevy game is running** with RemotePlugin:
   ```rust
   use bevy::prelude::*;
   use bevy::remote::RemotePlugin;
   
   fn main() {
       App::new()
           .add_plugins(DefaultPlugins)
           .add_plugins(RemotePlugin::default()) // This line is crucial
           .run();
   }
   ```

2. **Check configuration**:
   ```bash
   # Check if MCP server is using correct BRP port
   export BEVY_BRP_HOST=localhost
   export BEVY_BRP_PORT=15702
   ```

3. **Test BRP connection directly**:
   ```bash
   curl -X POST http://localhost:15702/bevy/list_entities
   ```

4. **Check firewall settings** on macOS/Linux:
   ```bash
   # macOS
   sudo pfctl -sr | grep 15702
   
   # Linux
   sudo iptables -L | grep 15702
   ```

---

### Issue 2: WebSocket Connection Failures

**Symptoms**: Connection drops, reconnection attempts fail

**Causes**:
- Network instability
- Resource exhaustion
- Port conflicts

**Solutions**:
1. **Check for port conflicts**:
   ```bash
   lsof -i :15702  # BRP port
   lsof -i :3000   # MCP port (default)
   ```

2. **Enable connection logging**:
   ```bash
   RUST_LOG=debug bevy-debugger-mcp --stdio
   ```

3. **Increase reconnection timeout**:
   ```toml
   # config.toml
   [connection]
   timeout_seconds = 30
   retry_attempts = 5
   ```

---

## Performance Problems

### Issue 3: High Latency (>100ms) Tool Calls

**Symptoms**: MCP commands take longer than expected, performance violations reported

**Causes**:
- Complex queries
- Large datasets
- Resource contention
- System overload

**Solutions**:
1. **Check performance budget violations**:
   ```javascript
   await mcpClient.callTool("debug", {
     command: {"GetBudgetStatistics": {}}
   });
   ```

2. **Optimize queries**:
   ```javascript
   // Instead of this (slow)
   await mcpClient.callTool("observe", {
     query: "all entities with any component"
   });
   
   // Use this (fast)  
   await mcpClient.callTool("observe", {
     query: "entities with Transform limit 100"
   });
   ```

3. **Monitor resource usage**:
   ```javascript
   const metrics = await mcpClient.callTool("resource_metrics", {});
   console.log("CPU:", metrics.cpu_percent);
   console.log("Memory:", metrics.memory_bytes);
   ```

---

### Issue 4: Memory Usage Growing Over Time

**Symptoms**: MCP server memory usage increases during long sessions

**Causes**:
- Unbounded history retention
- Memory leaks in processors
- Large screenshot buffers

**Solutions**:
1. **Clear violation history periodically**:
   ```javascript
   await mcpClient.callTool("debug", {
     command: {"ClearBudgetHistory": {}}
   });
   ```

2. **Configure history limits**:
   ```toml
   # config.toml
   [performance_budget]
   max_violation_history = 500
   max_compliance_samples = 5000
   ```

3. **Monitor memory with diagnostic report**:
   ```javascript
   const report = await mcpClient.callTool("diagnostic_report", {
     action: "generate"
   });
   ```

---

## Command Failures

### Issue 5: "Invalid Query" Errors

**Symptoms**: `observe` tool fails with query parsing errors

**Causes**:
- Malformed natural language queries
- Unrecognized component names
- Query too complex

**Solutions**:
1. **Use simple, clear queries**:
   ```javascript
   // Good queries
   "entities with Transform"
   "player entities"
   "entities moving fast"
   "entities near position 0,0,0"
   
   // Problematic queries  
   "all the things that are doing stuff"
   "entities with Transform and Velocity and Sprite and..."
   ```

2. **Check available components**:
   ```javascript
   await mcpClient.callTool("observe", {
     query: "list component types"
   });
   ```

3. **Use debug mode for detailed errors**:
   ```bash
   RUST_LOG=debug,bevy_debugger_mcp=trace bevy-debugger-mcp
   ```

---

### Issue 6: Screenshot Tool Failures

**Symptoms**: Screenshots fail to save or are blank/corrupted

**Causes**:
- Insufficient warmup time
- Invalid file paths
- Permission issues
- Graphics driver problems

**Solutions**:
1. **Increase warmup duration**:
   ```javascript
   await mcpClient.callTool("screenshot", {
     path: "debug/test.png",
     warmup_duration: 3000,  // 3 seconds
     wait_for_render: true
   });
   ```

2. **Check file permissions**:
   ```bash
   # Ensure directory exists and is writable
   mkdir -p debug/
   chmod 755 debug/
   ```

3. **Test with absolute path**:
   ```javascript
   await mcpClient.callTool("screenshot", {
     path: "/tmp/debug_screenshot.png"
   });
   ```

4. **Verify graphics setup** (headless environments):
   ```bash
   # Install virtual display for CI
   sudo apt-get install xvfb
   xvfb-run -a bevy-debugger-mcp
   ```

---

## Integration Test Failures

### Issue 7: "Mock BRP client connection timeout"

**Symptoms**: Integration tests fail with connection timeouts

**Causes**:
- Test environment configuration
- Resource limits in CI
- Race conditions

**Solutions**:
1. **Increase test timeouts**:
   ```rust
   #[tokio::test]
   async fn test_with_longer_timeout() {
       let config = TestConfig {
           timeout_ms: 30000, // 30 seconds
           ..Default::default()
       };
       // ... test code
   }
   ```

2. **Use mock clients for CI**:
   ```rust
   let config = TestConfig {
       enable_mock_brp: true,  // Use mocks in CI
       ..Default::default()
   };
   ```

3. **Add retry logic**:
   ```rust
   let mut attempts = 0;
   while attempts < 3 {
       match harness.execute_tool_call("observe", args).await {
           Ok(result) => return Ok(result),
           Err(_) if attempts < 2 => {
               attempts += 1;
               tokio::time::sleep(Duration::from_millis(1000)).await;
           }
           Err(e) => return Err(e),
       }
   }
   ```

---

### Issue 8: Performance Regression Test Failures

**Symptoms**: CI fails on performance tests that pass locally

**Causes**:
- CI resource constraints
- Different CPU/memory configurations
- Timing variations

**Solutions**:
1. **Adjust performance thresholds for CI**:
   ```rust
   let threshold = if std::env::var("CI").is_ok() {
       Duration::from_millis(200)  // More lenient in CI
   } else {
       Duration::from_millis(100)  // Strict locally
   };
   ```

2. **Use relative performance metrics**:
   ```rust
   // Instead of absolute time limits
   let baseline = measure_baseline_performance().await;
   let actual = measure_actual_performance().await;
   assert!(actual <= baseline * 1.5);  // 50% tolerance
   ```

---

## Configuration Problems

### Issue 9: Environment Variables Not Loading

**Symptoms**: Default values used instead of environment configuration

**Causes**:
- Variable naming mismatches
- Shell environment issues
- Service configuration problems

**Solutions**:
1. **Verify environment variables**:
   ```bash
   env | grep BEVY_
   echo $BEVY_BRP_HOST
   echo $BEVY_BRP_PORT
   echo $MCP_PORT
   ```

2. **Use configuration file**:
   ```toml
   # config/config.toml
   bevy_brp_host = "localhost"
   bevy_brp_port = 15702
   mcp_port = 3000
   
   [debug]
   enable_performance_tracking = true
   ```

3. **Check service configuration** (macOS):
   ```bash
   launchctl list | grep bevy-debugger
   cat ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   ```

---

## Platform-Specific Issues

### Issue 10: macOS Service Integration Problems

**Symptoms**: MCP server doesn't start automatically, Claude Code can't connect

**Solutions**:
1. **Check service status**:
   ```bash
   launchctl list com.bevy-debugger-mcp
   ```

2. **Reload service**:
   ```bash
   launchctl unload ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   launchctl load ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist
   ```

3. **Check service logs**:
   ```bash
   tail -f /tmp/bevy-debugger-mcp.log
   ```

---

### Issue 11: Linux Permission Issues

**Symptoms**: Cannot write screenshots, configuration files not accessible

**Solutions**:
1. **Check file permissions**:
   ```bash
   ls -la ~/.config/bevy-debugger-mcp/
   ```

2. **Fix permissions**:
   ```bash
   chmod 755 ~/.config/bevy-debugger-mcp/
   chmod 644 ~/.config/bevy-debugger-mcp/config.toml
   ```

---

### Issue 12: Windows Path Issues

**Symptoms**: File paths with spaces or special characters cause failures

**Solutions**:
1. **Use forward slashes**:
   ```javascript
   await mcpClient.callTool("screenshot", {
     path: "C:/Debug/Screenshots/test.png"  // Not C:\Debug\Screenshots\test.png
   });
   ```

2. **Escape paths properly**:
   ```javascript
   const path = "C:\\Users\\Name\\Documents\\screenshot.png";
   ```

---

## Advanced Diagnostics

### Issue 13: Intermittent Connection Drops

**Symptoms**: Connection works sometimes but fails unpredictably

**Diagnostic Steps**:
1. **Enable comprehensive logging**:
   ```bash
   RUST_LOG=trace,tokio=debug bevy-debugger-mcp --stdio 2>&1 | tee debug.log
   ```

2. **Monitor network activity**:
   ```bash
   # Linux
   sudo tcpdump -i lo port 15702
   
   # macOS  
   sudo tcpdump -i lo0 port 15702
   ```

3. **Check system resources**:
   ```bash
   # Monitor during issues
   top -p $(pgrep bevy-debugger-mcp)
   iostat 1
   ```

---

### Issue 14: Memory Corruption or Panics

**Symptoms**: Segfaults, memory corruption errors, unexpected panics

**Diagnostic Steps**:
1. **Run with debug symbols**:
   ```bash
   RUST_BACKTRACE=full cargo run --bin bevy-debugger-mcp
   ```

2. **Use memory debugging tools**:
   ```bash
   # Linux
   valgrind --tool=memcheck ./target/debug/bevy-debugger-mcp
   
   # macOS
   leaks -atExit -- ./target/debug/bevy-debugger-mcp
   ```

3. **Enable additional runtime checks**:
   ```bash
   RUSTFLAGS="-Z sanitizer=address" cargo run --bin bevy-debugger-mcp
   ```

---

### Issue 15: Performance Profiling

**When to Profile**:
- Commands consistently exceed latency budgets
- Memory usage grows unexpectedly
- CPU usage is higher than expected

**Profiling Steps**:
1. **CPU profiling with perf** (Linux):
   ```bash
   perf record -g --call-graph=dwarf ./target/release/bevy-debugger-mcp
   perf report
   ```

2. **Memory profiling with heaptrack**:
   ```bash
   heaptrack ./target/release/bevy-debugger-mcp
   heaptrack_gui heaptrack.bevy-debugger-mcp.*.zst
   ```

3. **Built-in performance monitoring**:
   ```javascript
   const report = await mcpClient.callTool("resource_metrics", {});
   const dashboard = await mcpClient.callTool("performance_dashboard", {});
   ```

---

## Emergency Recovery Procedures

### Complete Reset
If nothing else works, perform a complete reset:

```bash
# Stop all services
launchctl unload ~/Library/LaunchAgents/com.bevy-debugger-mcp.plist

# Clear all state
rm -rf ~/.local/share/bevy-debugger-mcp/
rm -rf debug_sessions/

# Reinstall  
cargo install --force bevy-debugger-mcp

# Reconfigure
bevy-debugger-mcp --help
```

### Data Recovery
If you need to recover debugging session data:

```bash
# Session data locations
ls ~/.local/share/bevy-debugger-mcp/sessions/
ls debug_sessions/checkpoints/

# Export session data
cargo run --bin bevy-debugger-mcp -- export-sessions --output sessions_backup.json
```

---

## Getting Additional Help

1. **Enable debug logging** and capture the full output
2. **Create a minimal reproduction case**
3. **Include system information**:
   ```bash
   uname -a
   cargo --version
   rustc --version
   ```
4. **File an issue** at: https://github.com/anthropics/bevy-debugger-mcp/issues

### Useful Commands for Bug Reports

```bash
# System info
echo "OS: $(uname -a)"
echo "Rust: $(rustc --version)"
echo "Cargo: $(cargo --version)"

# Configuration
echo "Config: $(cat ~/.config/bevy-debugger-mcp/config.toml)"

# Service status (macOS)
echo "Service: $(launchctl list com.bevy-debugger-mcp)"

# Process info
echo "Process: $(ps aux | grep bevy-debugger-mcp)"

# Network connections
echo "Network: $(lsof -i :15702 -i :3000)"
```

---

*This troubleshooting guide covers the most common issues. For additional help, consult the API documentation and usage guide.*
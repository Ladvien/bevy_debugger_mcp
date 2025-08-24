# Debugging Tools Reference

Complete reference for all 6 debugging tools available in the Bevy Debugger MCP.

## Tool Overview

| Tool | Purpose | Best For |
|------|---------|----------|
| **[observe](observe.md)** | Query game state in real-time | Entity investigation, component inspection, state verification |
| **[experiment](experiment.md)** | Run controlled tests on game behavior | Bug reproduction, performance testing, feature validation |
| **[hypothesis](hypothesis.md)** | Test theories with statistical analysis | Root cause analysis, performance investigation, A/B testing |
| **[detect_anomaly](detect_anomaly.md)** | Automatically find unusual patterns | Automated monitoring, regression detection, anomaly alerts |
| **[stress_test](stress_test.md)** | Find performance limits and bottlenecks | Scalability testing, optimization validation, breaking point analysis |
| **[replay](replay.md)** | Record and replay game sessions | Time-travel debugging, bug reproduction, comparative analysis |

## Quick Start Examples

### Basic Debugging Session
```
1. observe "all entities in my game"           # See current state
2. experiment "spawn 100 enemies"              # Test behavior
3. observe "entities with low health"          # Check results
4. screenshot "after spawning enemies"        # Document findings
```

### Performance Investigation
```
1. detect_anomaly "frame_time"                 # Monitor for issues
2. stress_test "entity_spawn"                  # Find limits
3. hypothesis "frame drops occur at 1000+ entities"  # Test theory
4. experiment "optimize MovementSystem"       # Validate fixes
```

### Bug Reproduction
```
1. replay "record collision_bug_investigation"  # Start recording
2. observe "player and enemy positions"         # Monitor entities
3. experiment "trigger collision scenarios"     # Reproduce bug
4. replay "stop" and analyze                    # Examine recording
```

## Tool Categories

### 🔍 **Investigation Tools**
- **[observe](observe.md)**: Real-time state inspection
- **[detect_anomaly](detect_anomaly.md)**: Automated pattern detection

### 🧪 **Testing Tools**  
- **[experiment](experiment.md)**: Controlled behavior testing
- **[hypothesis](hypothesis.md)**: Statistical validation
- **[stress_test](stress_test.md)**: Performance limit testing

### 📹 **Recording Tools**
- **[replay](replay.md)**: Session recording and playback

## Common Workflows

### Debug Performance Issues

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ detect_anomaly │ → │ stress_test │ → │ hypothesis  │
│ (find issues)  │   │ (find limits)│   │ (test fixes)│
└─────────────┘    └─────────────┘    └─────────────┘
        │                  │                  │
        ▼                  ▼                  ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   observe   │    │   observe   │    │ experiment  │
│ (understand)│    │ (analyze)   │    │ (validate)  │
└─────────────┘    └─────────────┘    └─────────────┘
```

### Reproduce Bugs

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   replay    │ → │   observe   │ → │ experiment  │
│  (record)   │   │ (examine)   │   │ (reproduce) │
└─────────────┘    └─────────────┘    └─────────────┘
        │                  │                  │
        ▼                  ▼                  ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   replay    │    │ hypothesis  │    │   replay    │
│ (playback)  │    │  (validate) │    │ (verify fix)│
└─────────────┘    └─────────────┘    └─────────────┘
```

### System Optimization

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ stress_test │ → │   observe   │ → │ hypothesis  │
│ (baseline)  │   │ (profile)   │   │ (plan fix)  │
└─────────────┘    └─────────────┘    └─────────────┘
        │                  │                  │
        ▼                  ▼                  ▼
┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│ experiment  │    │ stress_test │    │detect_anomaly│
│ (implement) │    │  (verify)   │    │ (monitor)   │
└─────────────┘    └─────────────┘    └─────────────┘
```

## Tool Selection Guide

### Choose **observe** when you need to:
- See what entities exist in your game
- Check component values on specific entities
- Understand current system state
- Verify game state matches expectations
- Monitor changes over time (with diff mode)

### Choose **experiment** when you need to:
- Test "what if" scenarios safely
- Reproduce specific conditions
- Measure impact of changes
- Validate system behavior under controlled conditions
- Compare different approaches

### Choose **hypothesis** when you need to:
- Test theories about game behavior scientifically
- Get statistical confidence in your conclusions
- Understand cause-and-effect relationships
- Validate optimization claims with data
- Compare performance between different implementations

### Choose **detect_anomaly** when you need to:
- Automatically monitor for issues
- Find problems you might miss manually
- Get alerts about performance regressions
- Detect unusual patterns in game behavior
- Continuously validate system health

### Choose **stress_test** when you need to:
- Find maximum capacity limits
- Identify performance bottlenecks
- Test system stability under load
- Validate scalability improvements
- Understand breaking points

### Choose **replay** when you need to:
- Capture bugs for later analysis
- Step through problems frame-by-frame
- Compare behavior between different versions
- Create reproducible test scenarios
- Analyze complex timing-dependent issues

## Parameter Patterns

### Common Parameters Across Tools

Most tools support these common parameters:

- **Duration Control**: `duration`, `timeout`, `max_time`
- **Quality Settings**: `quality`, `detail_level`, `precision`
- **Safety Limits**: `safety_limits`, `max_resources`, `circuit_breakers`
- **Output Options**: `output_format`, `save_results`, `include_analysis`

### Parameter Examples

**High Detail Analysis**:
```json
{
  "quality": "full",
  "reflection": true,
  "include_analysis": true,
  "save_results": true
}
```

**Quick Check**:
```json
{
  "quality": "minimal",
  "duration": 10,
  "basic_metrics_only": true
}
```

**Production Safe**:
```json
{
  "safety_limits": {"max_cpu": 80, "max_memory": 90},
  "circuit_breakers": true,
  "graceful_degradation": true
}
```

## Error Handling

### Common Error Patterns

All tools return errors in a consistent format:

```json
{
  "success": false,
  "error": {
    "code": "CONNECTION_ERROR",
    "message": "Failed to connect to game",
    "context": {"port": 15702, "host": "localhost"},
    "suggestions": [
      "Ensure game is running with RemotePlugin",
      "Check port 15702 is not blocked"
    ]
  }
}
```

### Error Recovery Strategies

**Connection Issues**:
```
1. Check game is running: observe "connection status"
2. Restart game with RemotePlugin enabled
3. Verify port configuration
```

**Performance Issues**:
```
1. Reduce tool parameters (duration, intensity, detail)
2. Check system resources: stress_test "system_health"
3. Use incremental approach instead of full load
```

**Resource Exhaustion**:
```
1. Enable safety limits in tool parameters
2. Monitor with: detect_anomaly "resource_usage"
3. Use cleanup commands between tests
```

## Performance Guidelines

### Tool Performance Impact

| Tool | CPU Impact | Memory Impact | Recommended Usage |
|------|------------|---------------|-------------------|
| **observe** | Low (1-3%) | Low (10-50MB) | Frequent use OK |
| **experiment** | Medium (5-15%) | Medium (50-200MB) | Moderate use |
| **hypothesis** | High (10-30%) | Medium (100-300MB) | Occasional use |
| **detect_anomaly** | Low (2-5%) | Low (20-100MB) | Continuous OK |
| **stress_test** | Very High (20-80%) | High (200MB-2GB) | Careful use |
| **replay** | Medium (5-20%) | High (100MB-5GB) | Session-based |

### Optimization Tips

**For Frequent Use**:
```json
{
  "quality": "minimal",
  "cache_results": true,
  "batch_operations": true,
  "reduce_precision": true
}
```

**For Deep Analysis**:
```json
{
  "quality": "full", 
  "detailed_logging": true,
  "extended_metrics": true,
  "save_intermediate_results": true
}
```

## Best Practices

### 1. Start Simple, Get Specific
```
❌ observe "everything about my game's performance and state"
✅ observe "entities with Transform components"
✅ observe "player health and position"
```

### 2. Use Tools in Combination
```
✅ observe → experiment → hypothesis → validate
✅ detect_anomaly → stress_test → observe → fix
```

### 3. Document Your Process
```
✅ Take screenshots at key moments
✅ Save experiment parameters that work
✅ Record successful debugging workflows
```

### 4. Monitor Performance Impact
```
✅ Check system resources before heavy operations
✅ Use safety limits on stress tests
✅ Clean up between test runs
```

### 5. Learn from Results
```
✅ Analyze why tools succeed or fail
✅ Adjust parameters based on results
✅ Build up knowledge of your game's behavior
```

## Integration Examples

### With IDEs
```javascript
// VS Code extension integration
const result = await vscode.debug.mcpClient.callTool("observe", {
  query: "entities at breakpoint location"
});
```

### With CI/CD
```yaml
# GitHub Actions integration
- name: Performance Regression Test
  run: |
    bevy-debugger-mcp stress_test entity_spawn --max-entities=2000
    bevy-debugger-mcp hypothesis "performance within baseline"
```

### With Monitoring
```javascript
// Prometheus metrics integration
const anomalies = await mcpClient.callTool("detect_anomaly", {
  detection_type: "performance_summary"
});
prometheus.recordMetrics(anomalies.data);
```

## Troubleshooting

### Tool Won't Connect
1. Check game has RemotePlugin enabled
2. Verify port 15702 is open
3. Test with: `curl -X POST http://localhost:15702/bevy/list_entities`

### Results Don't Make Sense
1. Establish baseline with known-good state
2. Use diff mode to see what changed
3. Cross-reference with other tools

### Performance Too Slow
1. Reduce tool parameters (duration, detail level)
2. Use incremental testing approaches
3. Check system has sufficient resources

### Memory Issues
1. Enable safety limits
2. Clear caches between operations
3. Use minimal quality for routine checks

## What's Next?

- **[Quick Start Guide](../quick-start.md)**: Get up and running in 5 minutes
- **[Installation](../installation/)**: Setup instructions for your platform
- **[Tutorials](../tutorials/)**: Step-by-step debugging scenarios
- **[API Reference](../api/)**: Complete technical documentation

---

*Master these 6 tools and you'll be able to debug any issue in your Bevy game with confidence and precision.*
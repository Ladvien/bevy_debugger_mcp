# Bevy Debugger MCP - API Reference

Auto-generated API documentation for all debugging tools and interfaces.

## Table of Contents

- [MCP Tools](#mcp-tools)
- [Debug Commands](#debug-commands)
- [Core Types](#core-types)
- [Error Handling](#error-handling)
- [Performance Monitoring](#performance-monitoring)

## MCP Tools

The following tools are available through the Model Context Protocol interface:

### observe

**Description**: Observe Bevy game state and entities using natural language queries.

**Parameters**:
- `query` (string, required): Natural language query describing what to observe

**Example**:
```json
{
  "query": "entities with Transform and Velocity components"
}
```

**Example Response**:
```json
{
  "success": true,
  "data": {
    "entities": [
      {
        "id": 123,
        "components": [
          {
            "type_name": "Transform",
            "data": {
              "translation": [0.0, 0.0, 0.0],
              "rotation": [0.0, 0.0, 0.0, 1.0],
              "scale": [1.0, 1.0, 1.0]
            }
          },
          {
            "type_name": "Velocity", 
            "data": {
              "linear": [1.5, 0.0, 0.0],
              "angular": 0.0
            }
          }
        ]
      }
    ],
    "total_count": 1
  }
}
```

**Returns**: JSON object containing matching entities and their component data.

---

### experiment

**Description**: Run controlled experiments on the game to test hypotheses.

**Parameters**:
- `type` (string, required): Type of experiment to run
- `parameters` (object, optional): Experiment-specific parameters

**Example**:
```json
{
  "type": "performance_test",
  "parameters": {
    "duration_seconds": 10,
    "metrics": ["frame_time", "memory"]
  }
}
```

**Returns**: Experiment results with collected data and analysis.

---

### screenshot

**Description**: Capture a screenshot of the Bevy game window for visual debugging.

**Parameters**:
- `path` (string, optional): File path where to save the screenshot. Defaults to `./screenshot.png`
- `warmup_duration` (integer, optional): Time in milliseconds to wait before taking screenshot. Default: 1000ms
- `capture_delay` (integer, optional): Additional delay in milliseconds before capture. Default: 500ms  
- `wait_for_render` (boolean, optional): Whether to wait for frame render. Default: true
- `description` (string, optional): Description of what this screenshot captures

**Example**:
```json
{
  "path": "debug/player-bug.png",
  "warmup_duration": 2000,
  "description": "Player movement bug with UI overlap"
}
```

**Returns**: Screenshot capture result with file path and success status.

---

### hypothesis

**Description**: Test specific hypotheses about game behavior using automated testing.

**Parameters**:
- `hypothesis` (string, required): The hypothesis to test in natural language
- `test_duration` (integer, optional): How long to test in seconds
- `confidence_level` (number, optional): Required confidence level (0.0-1.0)

**Example**:
```json
{
  "hypothesis": "Player movement speed affects frame rate",
  "test_duration": 30,
  "confidence_level": 0.95
}
```

**Returns**: Hypothesis test results with statistical analysis.

---

### stress

**Description**: Run stress tests to evaluate game performance under load.

**Parameters**:
- `type` (string, required): Type of stress test
- `count` (integer, optional): Number of entities/operations to test with
- `duration` (integer, optional): Duration of stress test in seconds

**Example**:
```json
{
  "type": "entity_spawn",
  "count": 1000,
  "duration": 60
}
```

**Returns**: Stress test results with performance metrics.

---

### replay

**Description**: Replay recorded game sessions for debugging.

**Parameters**:
- `session_id` (string, required): ID of the session to replay
- `speed_multiplier` (number, optional): Playback speed multiplier. Default: 1.0
- `start_frame` (integer, optional): Frame to start replay from

**Example**:
```json
{
  "session_id": "debug_session_2024_01_15",
  "speed_multiplier": 0.5,
  "start_frame": 1000
}
```

**Returns**: Replay session status and control information.

---

### anomaly

**Description**: Detect anomalies in game behavior and performance.

**Parameters**:
- `metric` (string, required): Metric to analyze for anomalies
- `sensitivity` (number, optional): Sensitivity threshold (0.0-1.0)
- `window_size` (integer, optional): Analysis window size in samples

**Example**:
```json
{
  "metric": "frame_time",
  "sensitivity": 0.8,
  "window_size": 100
}
```

**Returns**: Detected anomalies with timestamps and severity.

---

### orchestrate

**Description**: Execute complex debugging workflows using multiple tools.

**Parameters**:
- `tool` (string, required): Primary tool to execute
- `arguments` (object, required): Arguments for the primary tool
- `config` (object, optional): Orchestration configuration

**Example**:
```json
{
  "tool": "observe",
  "arguments": {"query": "slow entities"},
  "config": {
    "auto_experiment": true,
    "auto_record": true
  }
}
```

**Returns**: Orchestrated workflow results.

---

## Debug Commands

Debug commands are used internally by processors and can be accessed through the `debug` tool:

### Performance Budget Commands

- `StartBudgetMonitoring`: Start performance budget monitoring
- `StopBudgetMonitoring`: Stop performance budget monitoring  
- `SetPerformanceBudget`: Configure performance budgets
- `GetPerformanceBudget`: Get current budget configuration
- `CheckBudgetViolations`: Check for current budget violations
- `GetBudgetViolationHistory`: Get historical violations
- `ClearBudgetHistory`: Clear violation history
- `GenerateComplianceReport`: Generate compliance report
- `GetBudgetRecommendations`: Get budget adjustment recommendations
- `GetBudgetStatistics`: Get monitoring statistics

### System Profiling Commands

- `StartSystemProfiling`: Start profiling specific systems
- `StopSystemProfiling`: Stop system profiling
- `GetProfilingData`: Get collected profiling data
- `ExportProfilingData`: Export data in various formats

### Entity Inspection Commands

- `InspectEntity`: Get detailed entity information
- `ListEntities`: List entities matching criteria
- `SearchEntities`: Search entities using queries

## Core Types

### EntityData
```rust
pub struct EntityData {
    pub id: u32,
    pub components: Vec<ComponentData>,
    pub archetype: String,
}
```

### ComponentData  
```rust
pub struct ComponentData {
    pub type_name: String,
    pub data: serde_json::Value,
}
```

### PerformanceMetrics
```rust
pub struct PerformanceMetrics {
    pub frame_time_ms: f32,
    pub memory_mb: f32,
    pub cpu_percent: f32,
    pub gpu_time_ms: f32,
    pub entity_count: usize,
    pub draw_calls: usize,
    pub network_bandwidth_kbps: f32,
    pub timestamp: DateTime<Utc>,
}
```

### BudgetViolation
```rust
pub struct BudgetViolation {
    pub id: String,
    pub metric: ViolatedMetric,
    pub actual_value: f32,
    pub budget_value: f32,
    pub violation_percent: f32,
    pub timestamp: DateTime<Utc>,
    pub duration_ms: u64,
    pub severity: ViolationSeverity,
    pub context: HashMap<String, String>,
}
```

## Error Handling

All API calls return results in the following format:

### Success Response
```json
{
  "success": true,
  "data": { /* Response data */ },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Error Response
```json
{
  "success": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human readable error message",
    "context": { /* Additional error context */ }
  },
  "timestamp": "2024-01-15T10:30:00Z"
}
```

### Common Error Codes

- `CONNECTION_ERROR`: BRP connection to game failed
  - **Cause**: Bevy game not running or RemotePlugin not enabled
  - **Solution**: Start game with RemotePlugin and verify port 15702 is open
  
- `VALIDATION_ERROR`: Invalid parameters or request format
  - **Cause**: Missing required parameters or incorrect data types
  - **Solution**: Check API documentation for correct parameter format
  
- `TIMEOUT_ERROR`: Operation timed out
  - **Cause**: Operation took longer than configured timeout
  - **Solution**: Increase timeout or optimize query complexity
  
- `RESOURCE_ERROR`: Insufficient resources to complete operation
  - **Cause**: System memory/CPU limits reached
  - **Solution**: Reduce query scope or wait for resources to free up
  
- `NOT_FOUND`: Requested resource not found
  - **Cause**: Entity/component/session doesn't exist
  - **Solution**: Verify resource exists and hasn't been despawned
  
- `PERMISSION_DENIED`: Operation not permitted in current state
  - **Cause**: Trying to perform operation while system is in wrong state
  - **Solution**: Check system status and ensure prerequisites are met

## Performance Monitoring

### Latency Requirements

All MCP tool calls must complete within performance bounds:

- **observe**: < 50ms for simple queries, < 200ms for complex queries
- **experiment**: < 500ms setup, variable execution time
- **screenshot**: < 2000ms including warmup
- **hypothesis**: < 100ms setup, variable test duration
- **stress**: < 100ms setup, variable execution time  
- **replay**: < 200ms setup, variable playback time
- **anomaly**: < 100ms analysis

### Memory Usage

The debugger maintains bounded memory usage:

- **Violation history**: Limited to 1000 entries
- **Performance samples**: Limited to 10000 entries
- **Entity cache**: LRU cache with 5000 entry limit
- **Screenshot buffer**: Cleared after each capture

### Resource Management

Resources are automatically managed:

- **Connection pooling**: BRP connections are pooled and reused
- **Background cleanup**: Expired data cleaned up automatically
- **Graceful degradation**: Continues working with reduced functionality if resources are limited

## Integration Examples

### Basic Debugging Session

```javascript
// Initialize
const result = await mcpClient.callTool("observe", {
  query: "entities with high velocity"
});

// Take screenshot for documentation
await mcpClient.callTool("screenshot", {
  path: "debug/high_velocity_entities.png",
  description: "Entities moving faster than expected"
});

// Run performance analysis
await mcpClient.callTool("experiment", {
  type: "performance_test",
  parameters: {
    focus: "velocity_systems",
    duration_seconds: 30
  }
});
```

### Advanced Workflow

```javascript
// Orchestrated debugging workflow
const workflow = await mcpClient.callTool("orchestrate", {
  tool: "observe", 
  arguments: {query: "performance bottlenecks"},
  config: {
    auto_experiment: true,
    auto_record: true,
    auto_screenshot: true
  }
});
```

---

*This API reference is automatically generated from the codebase. For implementation details, see the source code documentation.*
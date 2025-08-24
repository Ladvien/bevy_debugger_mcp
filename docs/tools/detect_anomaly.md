# Detect Anomaly Tool

**Description**: Automatically detect anomalies in game behavior, performance, and state using machine learning and statistical analysis.

The `detect_anomaly` tool continuously monitors your game and alerts you to unusual patterns that might indicate bugs, performance issues, or unexpected behavior. It learns what "normal" looks like and flags deviations that require attention.

## When to Use

- **Automated Monitoring**: Continuously watch for performance regressions
- **Bug Detection**: Find issues you might not notice manually
- **Performance Alerts**: Get notified of frame drops or memory spikes
- **State Validation**: Detect inconsistent or corrupted game state
- **Regression Testing**: Ensure changes don't introduce new problems

## Parameters

- `detection_type` (string, required): Type of anomaly detection to perform
- `sensitivity` (number, optional): Detection sensitivity (0.0-1.0). Default: 0.8
- `window_size` (integer, optional): Analysis window size in samples. Default: 100
- `baseline_period` (integer, optional): Time to establish baseline behavior. Default: 60 seconds

## Detection Types

### Performance Anomalies

#### `frame_time`
Detects unusual frame time patterns and performance drops.

**Parameters**:
- `threshold_percentile` (number): Percentile threshold for outlier detection
- `consecutive_frames` (integer): How many consecutive bad frames trigger alert

**Example**:
```json
{
  "detection_type": "frame_time",
  "sensitivity": 0.9,
  "params": {
    "threshold_percentile": 95,
    "consecutive_frames": 3
  }
}
```

#### `memory_usage`
Monitors memory consumption for leaks and spikes.

**Parameters**:
- `growth_rate_threshold` (number): MB/minute growth that triggers alert
- `spike_threshold` (number): Sudden increase percentage that triggers alert

#### `cpu_usage`
Tracks CPU utilization patterns and spikes.

**Parameters**:
- `sustained_threshold` (number): CPU percentage threshold for sustained load
- `spike_duration_ms` (integer): How long spikes must last to trigger alert

### Entity Anomalies

#### `entity_lifecycle`
Monitors entity spawning and destruction patterns.

**Parameters**:
- `spawn_rate_threshold` (number): Entities/second that triggers alert
- `entity_leak_threshold` (integer): Unreferenced entities that trigger alert

**Example**:
```json
{
  "detection_type": "entity_lifecycle", 
  "sensitivity": 0.7,
  "params": {
    "spawn_rate_threshold": 100,
    "entity_leak_threshold": 1000
  }
}
```

#### `component_state`
Detects unusual component value patterns.

**Parameters**:
- `component_types` (array): Component types to monitor
- `value_change_threshold` (number): Change rate that triggers alert

### System Anomalies

#### `system_performance`
Monitors individual system execution times.

**Parameters**:
- `systems` (array): System names to monitor
- `execution_time_threshold` (number): Milliseconds threshold

#### `system_scheduling`
Detects unusual system execution order or timing.

**Parameters**:
- `expected_order` (array): Expected system execution order
- `timing_variance_threshold` (number): Acceptable timing variance

### Network Anomalies

#### `network_latency`
Monitors network performance in multiplayer games.

**Parameters**:
- `latency_threshold_ms` (integer): Latency that triggers alert
- `packet_loss_threshold` (number): Packet loss percentage threshold

#### `synchronization`
Detects entity synchronization issues between clients.

**Parameters**:
- `position_drift_threshold` (number): Maximum position difference
- `state_mismatch_threshold` (number): State difference percentage

## Example Usage

### Example 1: Performance Monitoring

**Goal**: Monitor for frame rate drops during gameplay.

**Command**:
```
Set up anomaly detection for frame time drops and performance issues
```

**Generated Detection**:
```json
{
  "detection_type": "frame_time",
  "sensitivity": 0.8,
  "window_size": 60,
  "params": {
    "fps_threshold": 45,
    "consecutive_frames": 5,
    "alert_on_spike": true
  }
}
```

**Results**:
```json
{
  "anomalies_detected": 3,
  "time_period": "2024-01-15T10:30:00Z to 2024-01-15T11:30:00Z",
  "alerts": [
    {
      "timestamp": "2024-01-15T10:45:23Z",
      "type": "frame_time_spike",
      "severity": "medium",
      "description": "Frame time exceeded 22ms for 8 consecutive frames",
      "values": {
        "max_frame_time": 24.7,
        "avg_frame_time": 22.1,
        "frames_affected": 8
      },
      "possible_causes": [
        "System load spike",
        "Garbage collection event",
        "Resource loading"
      ]
    }
  ]
}
```

### Example 2: Memory Leak Detection

**Goal**: Automatically detect memory leaks over long gaming sessions.

**Command**:
```
Monitor memory usage for potential leaks during extended gameplay
```

**Generated Detection**:
```json
{
  "detection_type": "memory_usage",
  "sensitivity": 0.9,
  "baseline_period": 300,
  "params": {
    "growth_rate_threshold": 5.0,
    "leak_detection_window": 1800,
    "ignore_gc_events": true
  }
}
```

**Results**:
```json
{
  "leak_detected": true,
  "growth_rate": 8.3,
  "confidence": 0.94,
  "analysis": {
    "start_memory": 245.2,
    "current_memory": 394.7,
    "growth_mb": 149.5,
    "duration_minutes": 18,
    "suspected_components": [
      "EntityManager",
      "ResourceCache",
      "TextureAtlas"
    ]
  },
  "recommendation": "Memory leak detected in EntityManager. Consider implementing entity cleanup."
}
```

### Example 3: Entity Spawning Anomalies

**Goal**: Detect unusual entity spawning patterns that might indicate bugs.

**Command**:
```
Watch for unusual entity spawning behavior that might indicate a bug
```

**Generated Detection**:
```json
{
  "detection_type": "entity_lifecycle",
  "sensitivity": 0.75,
  "params": {
    "normal_spawn_rate": 10,
    "spike_multiplier": 5,
    "entity_types": ["enemy", "projectile", "particle"]
  }
}
```

**Results**:
```json
{
  "anomaly_detected": true,
  "type": "excessive_spawning",
  "details": {
    "entity_type": "projectile",
    "normal_rate": 8.5,
    "anomalous_rate": 147.2,
    "duration_seconds": 3.7,
    "total_spawned": 544
  },
  "impact": {
    "performance_drop": "15% frame rate decrease",
    "memory_usage": "+45MB in projectile components"
  },
  "root_cause_analysis": "Rapid-fire weapon not properly rate-limited"
}
```

### Example 4: System Performance Regression

**Goal**: Detect when a system starts performing worse than usual.

**Command**:
```
Monitor MovementSystem performance for regressions compared to baseline
```

**Generated Detection**:
```json
{
  "detection_type": "system_performance",
  "sensitivity": 0.85,
  "params": {
    "systems": ["MovementSystem"],
    "baseline_percentile": 90,
    "regression_threshold": 1.5
  }
}
```

**Results**:
```json
{
  "regression_detected": true,
  "system": "MovementSystem",
  "performance_change": {
    "baseline_avg_ms": 3.2,
    "current_avg_ms": 5.8,
    "degradation_percent": 81.25,
    "confidence": 0.97
  },
  "timeline": {
    "regression_start": "2024-01-15T10:22:15Z",
    "detection_time": "2024-01-15T10:28:33Z",
    "samples_analyzed": 234
  },
  "correlation_analysis": [
    "Entity count increased by 45%",
    "Complex pathfinding queries increased",
    "New collision detection algorithm"
  ]
}
```

## Machine Learning Models

The anomaly detection system uses several ML approaches:

### Statistical Models

**Isolation Forest**: Identifies outliers in multidimensional performance data
- Good for: Complex performance anomalies
- Parameters: `contamination`, `n_estimators`

**Z-Score Analysis**: Detects values beyond standard deviations
- Good for: Simple threshold-based detection  
- Parameters: `z_threshold`, `rolling_window`

**Seasonal Decomposition**: Separates trends from anomalies
- Good for: Time-series data with patterns
- Parameters: `seasonality_period`, `trend_strength`

### Time Series Models

**LSTM Autoencoders**: Learn normal behavior patterns
- Good for: Complex behavioral anomalies
- Parameters: `sequence_length`, `encoding_dims`

**Prophet**: Handles seasonality and trend changes
- Good for: Long-term performance monitoring
- Parameters: `changepoint_prior_scale`, `seasonality_mode`

### Ensemble Methods

**Voting Classifier**: Combines multiple detection methods
- Improves accuracy and reduces false positives
- Automatically weights different models

## Advanced Features

### Multi-Modal Detection

Monitor multiple metrics simultaneously:

```json
{
  "detection_type": "multi_modal",
  "metrics": [
    {"type": "frame_time", "weight": 0.4},
    {"type": "memory_usage", "weight": 0.3}, 
    {"type": "entity_count", "weight": 0.3}
  ],
  "correlation_analysis": true
}
```

### Adaptive Thresholds

Automatically adjust thresholds based on learning:

```json
{
  "detection_type": "adaptive_performance",
  "adaptation_rate": 0.1,
  "confidence_interval": 0.95,
  "update_frequency": 300
}
```

### Custom Anomaly Types

Define domain-specific anomaly detection:

```json
{
  "detection_type": "custom",
  "definition": {
    "name": "player_stuck_detection",
    "metrics": ["player_velocity", "input_activity"],
    "conditions": {
      "velocity_threshold": 0.1,
      "input_present": true,
      "duration_seconds": 5
    }
  }
}
```

## Alert Configuration

### Severity Levels

**Low**: Minor deviations from normal behavior
- Frame time spikes < 25ms
- Memory growth < 10MB/hour
- Entity count variations < 20%

**Medium**: Noticeable impact on game experience  
- Frame time spikes 25-40ms
- Memory growth 10-50MB/hour
- System performance degradation 25-50%

**High**: Significant performance or functionality issues
- Frame time spikes > 40ms
- Memory growth > 50MB/hour
- System failures or crashes

**Critical**: Game-breaking issues requiring immediate attention
- Frame rate < 10 FPS for > 5 seconds
- Memory exhaustion imminent
- Core systems failing

### Alert Actions

```json
{
  "alert_config": {
    "email": ["developer@game.com"],
    "slack_webhook": "https://hooks.slack.com/...",
    "log_level": "warn",
    "screenshot_on_anomaly": true,
    "state_capture": true
  }
}
```

## Performance Impact

The anomaly detection system is designed for minimal performance impact:

- **CPU Usage**: < 2% average, < 5% during analysis
- **Memory Usage**: < 50MB for monitoring data
- **Latency**: < 1ms additional latency per frame
- **Storage**: < 10MB per hour of monitoring data

## Best Practices

### 1. Establish Good Baselines
Run games in known-good state for baseline learning:
```json
{"baseline_period": 600, "clean_environment": true}
```

### 2. Tune Sensitivity Appropriately
- **Development**: Higher sensitivity (0.9) to catch small issues
- **Production**: Lower sensitivity (0.7) to reduce false alarms

### 3. Monitor Gradually
Start with key metrics, then expand coverage:
```
Day 1: Frame time anomalies
Day 2: Add memory monitoring  
Week 1: Add entity lifecycle monitoring
Week 2: Add system performance monitoring
```

### 4. Combine with Other Tools
Use anomaly detection as early warning, then investigate with other tools:
```
1. detect_anomaly alerts to performance issue
2. observe to examine current state
3. experiment to reproduce the issue
4. hypothesis to test root cause theory
```

## Troubleshooting

### Too Many False Positives
- Reduce sensitivity parameter
- Increase window_size for more stable detection
- Extend baseline_period for better learning

### Missing Real Anomalies
- Increase sensitivity parameter
- Reduce detection thresholds  
- Check if baseline period captured representative behavior

### High Resource Usage
- Reduce monitoring frequency
- Limit number of metrics monitored simultaneously
- Use sampling for high-frequency data

## Integration Examples

### CI/CD Pipeline
```yaml
- name: Performance Regression Detection
  run: |
    bevy-debugger-mcp detect_anomaly frame_time --baseline=ci-baseline.json
```

### Real-time Dashboard
```javascript
// Monitor anomalies in real-time
setInterval(async () => {
  const anomalies = await mcpClient.callTool("detect_anomaly", {
    detection_type: "performance_summary",
    window_size: 30
  });
  updateDashboard(anomalies);
}, 10000);
```

## Related Tools

- **[observe](observe.md)**: Investigate anomalies detected by the system
- **[experiment](experiment.md)**: Reproduce conditions that trigger anomalies
- **[hypothesis](hypothesis.md)**: Test theories about anomaly root causes
- **[stress_test](stress_test.md)**: Verify anomaly detection under extreme conditions

---

*Anomaly detection turns your debugger into a vigilant assistant that never sleeps, catching issues before they become critical problems.*
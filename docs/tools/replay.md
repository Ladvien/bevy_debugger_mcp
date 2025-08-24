# Replay Tool

**Description**: Record and replay game state for time-travel debugging, allowing you to capture problematic moments and replay them for detailed analysis.

The `replay` tool provides deterministic recording and playback of game sessions, enabling you to "travel back in time" to examine exactly what happened when bugs occurred or performance issues manifested.

## When to Use

- **Bug Reproduction**: Capture the exact sequence leading to a bug
- **Performance Analysis**: Replay performance drops for detailed investigation
- **State Investigation**: Examine game state at specific moments in time
- **Regression Testing**: Replay scenarios to ensure fixes work correctly
- **Demo Creation**: Create reproducible gameplay demonstrations

## Parameters

- `action` (string, required): Action to perform (record, replay, stop, list, analyze)
- `checkpoint_id` (string, optional): ID of checkpoint to replay or analyze
- `speed_multiplier` (number, optional): Playback speed (0.1 = slow motion, 2.0 = 2x speed). Default: 1.0
- `start_frame` (integer, optional): Frame to start replay from. Default: 0
- `end_frame` (integer, optional): Frame to stop replay at. Default: end of recording

## Replay Actions

### Recording Actions

#### `record`
Starts recording game state for later replay.

**Parameters**:
- `session_name` (string): Name for the recording session
- `duration` (integer): Maximum recording duration in seconds
- `quality` (string): Recording quality (full, minimal, performance)
- `auto_checkpoint` (boolean): Automatically create checkpoints at intervals

**Example**:
```json
{
  "action": "record",
  "params": {
    "session_name": "bug_reproduction_001",
    "duration": 300,
    "quality": "full",
    "auto_checkpoint": true,
    "checkpoint_interval": 30
  }
}
```

#### `stop`
Stops current recording and saves the session.

**Parameters**:
- `save_location` (string): Where to save the recording
- `compress` (boolean): Compress recording data

#### `checkpoint`
Creates a checkpoint at the current moment during recording.

**Parameters**:
- `name` (string): Name for the checkpoint
- `description` (string): Description of what's happening

### Playback Actions

#### `replay`
Replays a recorded session from a specific checkpoint.

**Example**:
```json
{
  "action": "replay", 
  "checkpoint_id": "bug_reproduction_001_frame_1250",
  "speed_multiplier": 0.5,
  "start_frame": 1200,
  "end_frame": 1300
}
```

#### `step`
Steps through replay frame by frame.

**Parameters**:
- `frames` (integer): Number of frames to advance. Default: 1
- `direction` (string): Forward or backward. Default: "forward"

#### `seek`
Jumps to a specific frame in the replay.

**Parameters**:
- `target_frame` (integer): Frame to jump to
- `relative` (boolean): Whether target is relative to current position

### Analysis Actions

#### `analyze`
Analyzes a recorded session for patterns and issues.

**Parameters**:
- `analysis_type` (string): Type of analysis (performance, behavior, state)
- `focus_area` (array): Specific systems or components to analyze

#### `compare`
Compares two recording sessions to identify differences.

**Parameters**:
- `baseline_session` (string): Reference session for comparison
- `comparison_metrics` (array): Metrics to compare

## Example Usage

### Example 1: Bug Reproduction Recording

**Goal**: Record a session to capture a collision detection bug.

**Command**:
```
Start recording gameplay to capture the collision bug that happens randomly
```

**Generated Recording**:
```json
{
  "action": "record",
  "params": {
    "session_name": "collision_bug_hunt",
    "duration": 600,
    "quality": "full",
    "trigger_conditions": {
      "collision_failure": true,
      "entity_overlap": true,
      "physics_anomaly": true
    },
    "auto_checkpoint_on_event": true
  }
}
```

**Results**:
```json
{
  "recording_started": true,
  "session_id": "collision_bug_hunt_2024_01_15",
  "estimated_size": "245 MB",
  "checkpoints_enabled": true,
  "triggers_active": 3,
  "status": "Recording... Waiting for trigger conditions"
}
```

### Example 2: Performance Issue Replay

**Goal**: Replay a frame drop incident for analysis.

**Command**:
```
Replay the performance drop that occurred at frame 2847 in slow motion
```

**Generated Replay**:
```json
{
  "action": "replay",
  "checkpoint_id": "performance_session_frame_2847",
  "speed_multiplier": 0.25,
  "start_frame": 2800,
  "end_frame": 2900,
  "analysis_mode": true
}
```

**Results**:
```json
{
  "replay_status": "active",
  "current_frame": 2847,
  "playback_speed": "0.25x (slow motion)",
  "frame_analysis": {
    "frame_time_ms": 45.7,
    "systems_over_budget": ["MovementSystem", "PhysicsSystem"],
    "entity_count": 1547,
    "memory_pressure": "high"
  },
  "identified_issues": [
    "MovementSystem processed 847 entities with complex pathfinding",
    "Physics system detected 23 collision pairs simultaneously",
    "Memory allocation spike of 45MB during particle effect"
  ]
}
```

### Example 3: Time-Travel Debugging

**Goal**: Step through a bug frame by frame to understand what happened.

**Command**:
```
Step through the replay frame by frame starting from when the bug occurred
```

**Generated Analysis**:
```json
{
  "action": "step",
  "checkpoint_id": "bug_occurrence_frame_1205",
  "analysis_each_frame": true,
  "track_entities": ["player", "enemy_that_disappeared"],
  "track_components": ["Transform", "Health", "Collision"]
}
```

**Step-by-Step Results**:
```json
{
  "frame_1205": {
    "player": {"pos": [10.5, 0, 5.2], "health": 100},
    "enemy": {"pos": [12.1, 0, 5.8], "health": 50},
    "collision_detected": false
  },
  "frame_1206": {
    "player": {"pos": [11.2, 0, 5.2], "health": 100},
    "enemy": {"pos": [11.8, 0, 5.8], "health": 50}, 
    "collision_detected": true,
    "collision_result": "overlap_but_no_damage"
  },
  "frame_1207": {
    "player": {"pos": [11.9, 0, 5.2], "health": 100},
    "enemy": "ENTITY_NOT_FOUND",
    "issue_detected": "Enemy entity despawned without cleanup"
  }
}
```

### Example 4: A/B Testing with Replay

**Goal**: Compare how the same scenario plays out with different game settings.

**Command**:
```
Replay the same scenario with old AI settings and new AI settings to compare behavior
```

**Generated Comparison**:
```json
{
  "action": "compare",
  "baseline_session": "ai_test_old_settings",
  "comparison_session": "ai_test_new_settings", 
  "comparison_metrics": [
    "battle_duration",
    "player_damage_taken",
    "enemy_decision_quality",
    "frame_time_impact"
  ]
}
```

**Comparison Results**:
```json
{
  "comparison_summary": {
    "battle_duration": {
      "old_ai": {"avg": 45.2, "std_dev": 8.7},
      "new_ai": {"avg": 62.1, "std_dev": 12.4},
      "difference": "+37.4% longer battles"
    },
    "player_damage": {
      "old_ai": {"avg": 35.8, "std_dev": 12.1},
      "new_ai": {"avg": 28.3, "std_dev": 9.4},
      "difference": "-20.9% less damage to player"
    },
    "frame_performance": {
      "old_ai": {"avg_ms": 12.1, "spikes": 3},
      "new_ai": {"avg_ms": 15.7, "spikes": 8},
      "difference": "+29.8% higher frame times"
    }
  },
  "behavioral_differences": [
    "New AI uses more sophisticated pathfinding (causing performance cost)",
    "New AI makes more defensive decisions (reducing player damage)",
    "New AI evaluates more options per decision (extending battle length)"
  ]
}
```

## Advanced Features

### Selective Recording

Record only specific aspects of the game:

```json
{
  "action": "record",
  "selective_recording": {
    "entities": ["player", "bosses", "critical_npcs"],
    "systems": ["PhysicsSystem", "CombatSystem"],
    "components": ["Transform", "Health", "Velocity"],
    "events": ["collision", "damage", "state_change"]
  }
}
```

### Conditional Checkpoints

Automatically create checkpoints when specific conditions occur:

```json
{
  "checkpoint_conditions": [
    {"type": "health_below", "value": 25, "entity": "player"},
    {"type": "frame_time_spike", "threshold": 30},
    {"type": "entity_count_change", "threshold": 100},
    {"type": "custom_event", "event_name": "boss_phase_change"}
  ]
}
```

### Interactive Replay

Allow interaction during replay for "what if" scenarios:

```json
{
  "action": "interactive_replay",
  "checkpoint_id": "decision_point_frame_567",
  "allow_modifications": {
    "player_input": true,
    "entity_state": false,
    "system_parameters": true
  }
}
```

### Deterministic Replay

Ensure perfect reproducibility:

```json
{
  "deterministic_mode": {
    "seed_rng": true,
    "lock_timestamps": true,
    "normalize_floating_point": true,
    "record_external_inputs": true
  }
}
```

## Recording Quality Levels

### Full Quality
Records complete game state every frame:
- **Pros**: Perfect reproduction, complete analysis capability
- **Cons**: Large file sizes (1-5 GB per minute), performance impact
- **Use**: Critical bug investigation, detailed analysis

### Minimal Quality  
Records only essential state changes:
- **Pros**: Small file sizes (10-50 MB per minute), minimal performance impact
- **Cons**: Limited analysis capability, may miss some details
- **Use**: Long-term monitoring, performance testing

### Performance Quality
Optimized for performance analysis:
- **Pros**: Detailed performance data, moderate file sizes
- **Cons**: Limited entity state information
- **Use**: Frame time analysis, system profiling

### Custom Quality
User-defined recording parameters:
```json
{
  "quality": "custom",
  "record_frequency": 30,
  "state_compression": true,
  "performance_metrics": true,
  "entity_detail_level": "transforms_only"
}
```

## Replay Analysis Features

### Frame-by-Frame Analysis

```json
{
  "analysis_type": "frame_by_frame",
  "metrics": [
    "entity_positions",
    "component_changes", 
    "system_execution_times",
    "memory_allocations"
  ],
  "visualization": "timeline_graph"
}
```

### Pattern Detection

```json
{
  "analysis_type": "pattern_detection",
  "patterns": [
    "repeating_behaviors",
    "performance_cycles",
    "state_oscillations",
    "anomaly_precursors"
  ]
}
```

### Causal Analysis

```json
{
  "analysis_type": "causal_chain",
  "focus_event": "entity_despawn_frame_1207",
  "trace_backwards": 10,
  "identify_root_cause": true
}
```

## Best Practices

### 1. Strategic Recording
Don't record everything - focus on problem areas:
```json
{
  "record_triggers": ["performance_drop", "state_anomaly", "user_report"],
  "max_concurrent_recordings": 2
}
```

### 2. Manageable Sessions
Keep recordings focused and manageable:
```json
{
  "duration": 120,
  "auto_split": true,
  "split_on_checkpoint": true
}
```

### 3. Meaningful Checkpoints
Create checkpoints at important moments:
```json
{
  "checkpoint_on": ["level_start", "boss_fight", "player_death", "performance_issue"]
}
```

### 4. Efficient Storage
Use compression and cleanup:
```json
{
  "compression": true,
  "cleanup_policy": "after_30_days",
  "max_storage_gb": 10
}
```

## Performance Considerations

### Recording Impact
- **CPU**: 5-15% overhead depending on quality
- **Memory**: 50-200 MB buffer for recording data
- **Disk**: 10 MB to 5 GB per minute depending on quality
- **Network**: Minimal impact (local storage only)

### Optimization Strategies
```json
{
  "performance_optimizations": {
    "async_writing": true,
    "memory_mapped_files": true,
    "compression_level": 6,
    "selective_recording": true
  }
}
```

## Troubleshooting

### Recording Too Large
- Reduce recording quality
- Use selective recording for specific entities/systems
- Enable compression
- Set shorter duration limits

### Replay Desync
- Check for non-deterministic code (timestamps, random numbers)
- Verify external inputs are recorded
- Ensure consistent floating-point behavior
- Use deterministic mode

### Poor Replay Performance
- Replay at reduced speed
- Use frame stepping instead of continuous playback
- Reduce analysis overhead during replay
- Optimize storage device access

## Integration Examples

### Bug Report Integration
```javascript
// Automatically start recording when user reports bug
function onBugReport() {
  mcpClient.callTool("replay", {
    action: "record",
    params: {
      session_name: `bug_report_${Date.now()}`,
      duration: 60,
      quality: "full"
    }
  });
}
```

### CI/CD Integration
```yaml
# Replay regression tests in CI
- name: Regression Test Replay
  run: |
    bevy-debugger-mcp replay --action=replay --session=regression_test_baseline
    bevy-debugger-mcp replay --action=compare --baseline=expected_behavior
```

## Related Tools

- **[observe](observe.md)**: Examine specific moments captured in replays
- **[experiment](experiment.md)**: Use replay data to design controlled experiments
- **[hypothesis](hypothesis.md)**: Test theories using replay evidence
- **[detect_anomaly](detect_anomaly.md)**: Identify unusual patterns in replay data

---

*Replay enables true time-travel debugging - capture the moment bugs occur and examine them with perfect hindsight.*
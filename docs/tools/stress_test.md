# Stress Test Tool

**Description**: Run stress tests to find performance limits and bottlenecks by pushing your game to its breaking point.

The `stress_test` tool systematically increases load on your game systems to identify performance limits, breaking points, and optimization opportunities. It helps you understand how your game behaves under extreme conditions.

## When to Use

- **Performance Limits**: Find maximum entity counts, rendering loads, or system capacities
- **Scalability Testing**: Determine how well your game scales with increased complexity
- **Bottleneck Identification**: Pinpoint which systems fail first under load
- **Optimization Validation**: Verify that optimizations actually improve performance under stress
- **Stability Testing**: Ensure your game remains stable under extreme conditions

## Parameters

- `test_type` (string, required): Type of stress test to run
- `intensity` (number, optional): Test intensity multiplier (1.0 = normal, 2.0 = double). Default: 2.0
- `duration` (integer, optional): Duration of stress test in seconds. Default: 60
- `incremental` (boolean, optional): Gradually increase load vs immediate full load. Default: true
- `safety_limits` (object, optional): Safety thresholds to prevent system damage

## Stress Test Types

### Entity Stress Tests

#### `entity_spawn`
Spawns increasing numbers of entities to test ECS performance.

**Parameters**:
- `max_entities` (integer): Maximum entities to spawn
- `entity_type` (string): Type of entities to spawn
- `spawn_rate` (integer): Entities spawned per second
- `spatial_distribution` (string): How to distribute entities (random, grid, cluster)

**Example**:
```json
{
  "test_type": "entity_spawn",
  "intensity": 3.0,
  "params": {
    "max_entities": 10000,
    "entity_type": "moving_enemy",
    "spawn_rate": 100,
    "spatial_distribution": "random"
  }
}
```

#### `component_density`
Tests performance with entities having many components.

**Parameters**:
- `components_per_entity` (integer): Number of components per entity
- `entity_count` (integer): Number of entities to create
- `component_types` (array): Types of components to attach

### System Stress Tests

#### `system_overload`
Overloads specific systems with excessive workload.

**Parameters**:
- `target_system` (string): System to stress test
- `workload_multiplier` (number): Increase system workload by factor
- `concurrent_operations` (integer): Number of simultaneous operations

**Example**:
```json
{
  "test_type": "system_overload",
  "params": {
    "target_system": "PhysicsSystem",
    "workload_multiplier": 5.0,
    "concurrent_operations": 1000
  }
}
```

#### `system_cascade`
Tests system performance when multiple systems are stressed simultaneously.

**Parameters**:
- `systems` (array): Systems to stress in parallel
- `cascade_delay` (integer): Delay between stressing each system

### Rendering Stress Tests

#### `draw_call_flood`
Floods the renderer with excessive draw calls.

**Parameters**:
- `max_draw_calls` (integer): Maximum draw calls to generate
- `primitive_type` (string): Type of primitives to render
- `texture_variety` (integer): Number of different textures to use

**Example**:
```json
{
  "test_type": "draw_call_flood",
  "intensity": 4.0,
  "params": {
    "max_draw_calls": 5000,
    "primitive_type": "sprite",
    "texture_variety": 100
  }
}
```

#### `shader_complexity`
Tests GPU performance with complex shaders.

**Parameters**:
- `shader_complexity` (integer): Complexity level (1-10)
- `render_targets` (integer): Number of render targets
- `post_processing_effects` (integer): Number of post-processing passes

### Memory Stress Tests

#### `memory_pressure`
Creates memory pressure to test garbage collection and allocation performance.

**Parameters**:
- `allocation_rate` (integer): MB allocated per second
- `allocation_size` (integer): Size of individual allocations
- `hold_time` (integer): How long to hold allocations

**Example**:
```json
{
  "test_type": "memory_pressure",
  "params": {
    "allocation_rate": 100,
    "allocation_size": 1024,
    "hold_time": 5
  }
}
```

#### `cache_thrashing`
Tests performance when exceeding cache limits.

**Parameters**:
- `data_size` (integer): Size of working data set
- `access_pattern` (string): How to access data (sequential, random, strided)

### Network Stress Tests

#### `network_saturation`
Saturates network bandwidth with high-frequency updates.

**Parameters**:
- `message_rate` (integer): Messages per second
- `message_size` (integer): Size of each message in bytes
- `concurrent_connections` (integer): Number of simulated clients

**Example**:
```json
{
  "test_type": "network_saturation",
  "params": {
    "message_rate": 1000,
    "message_size": 512,
    "concurrent_connections": 50
  }
}
```

## Example Usage

### Example 1: Entity Performance Limits

**Goal**: Find the maximum number of moving entities before frame rate drops below 30 FPS.

**Command**:
```
Run a stress test to find the maximum number of moving entities the game can handle
```

**Generated Stress Test**:
```json
{
  "test_type": "entity_spawn",
  "intensity": 2.0,
  "duration": 120,
  "incremental": true,
  "params": {
    "max_entities": 5000,
    "entity_type": "moving_enemy",
    "spawn_rate": 50,
    "fps_threshold": 30
  },
  "safety_limits": {
    "min_fps": 10,
    "max_memory_mb": 2048
  }
}
```

**Results**:
```json
{
  "test_completed": true,
  "breaking_point": {
    "entity_count": 2847,
    "fps_at_break": 29.3,
    "memory_usage_mb": 456.7,
    "time_to_break": "78.2 seconds"
  },
  "performance_curve": [
    {"entities": 500, "fps": 59.8, "memory_mb": 245.1},
    {"entities": 1000, "fps": 58.2, "memory_mb": 298.4},
    {"entities": 1500, "fps": 55.1, "memory_mb": 352.8},
    {"entities": 2000, "fps": 48.7, "memory_mb": 398.2},
    {"entities": 2500, "fps": 38.4, "memory_mb": 431.5},
    {"entities": 2847, "fps": 29.3, "memory_mb": 456.7}
  ],
  "bottleneck_analysis": {
    "primary_bottleneck": "MovementSystem CPU usage",
    "secondary_bottleneck": "Transform hierarchy updates",
    "gpu_utilization": "67% - not limiting",
    "memory_utilization": "22% - not limiting"
  },
  "recommendations": [
    "Optimize MovementSystem with SIMD operations",
    "Consider entity pooling for frequent spawn/despawn",
    "Implement level-of-detail for distant entities"
  ]
}
```

### Example 2: Rendering Performance Stress

**Goal**: Test how many draw calls the renderer can handle before performance degrades.

**Command**:
```
Stress test the renderer to find the maximum number of draw calls per frame
```

**Generated Stress Test**:
```json
{
  "test_type": "draw_call_flood",
  "intensity": 3.0,
  "duration": 60,
  "params": {
    "max_draw_calls": 10000,
    "primitive_type": "sprite",
    "increment_per_second": 100,
    "target_fps": 60
  }
}
```

**Results**:
```json
{
  "breaking_point": {
    "draw_calls": 4231,
    "fps_at_break": 58.7,
    "gpu_utilization": "95%",
    "cpu_utilization": "78%"
  },
  "performance_characteristics": {
    "linear_degradation": "0-3000 draw calls",
    "exponential_degradation": "3000+ draw calls",
    "gpu_bound_threshold": 3500,
    "cpu_bound_threshold": 4000
  },
  "optimization_opportunities": [
    "Implement draw call batching",
    "Use instanced rendering for similar objects",
    "Consider GPU-driven rendering for high counts"
  ]
}
```

### Example 3: Memory Allocation Stress

**Goal**: Test garbage collection performance under high allocation pressure.

**Command**:
```
Stress test memory allocation to find garbage collection performance limits
```

**Generated Stress Test**:
```json
{
  "test_type": "memory_pressure",
  "duration": 180,
  "params": {
    "allocation_rate": 50,
    "allocation_patterns": ["small_frequent", "large_infrequent", "mixed"],
    "gc_trigger_threshold": 0.8
  }
}
```

**Results**:
```json
{
  "gc_performance": {
    "avg_gc_pause_ms": 12.4,
    "max_gc_pause_ms": 47.8,
    "gc_frequency_per_minute": 8.3,
    "memory_fragmentation_percent": 15.2
  },
  "allocation_patterns": {
    "small_frequent": {"avg_pause": 8.1, "frequency": 12.1},
    "large_infrequent": {"avg_pause": 23.7, "frequency": 2.4},
    "mixed": {"avg_pause": 15.9, "frequency": 7.8}
  },
  "recommendations": [
    "Use object pools for frequently allocated objects",
    "Increase initial heap size to reduce GC frequency",
    "Consider manual memory management for critical paths"
  ]
}
```

### Example 4: Multi-System Cascade Test

**Goal**: Test how systems interact under stress and identify cascade failures.

**Command**:
```
Run a cascade stress test that loads multiple systems simultaneously
```

**Generated Stress Test**:
```json
{
  "test_type": "system_cascade",
  "duration": 90,
  "params": {
    "systems": [
      {"name": "PhysicsSystem", "load_multiplier": 3.0},
      {"name": "RenderSystem", "load_multiplier": 2.5},
      {"name": "MovementSystem", "load_multiplier": 4.0},
      {"name": "AudioSystem", "load_multiplier": 2.0}
    ],
    "cascade_interval": 15
  }
}
```

**Results**:
```json
{
  "cascade_results": {
    "phase_1_physics": {"duration": 15, "stable": true, "avg_fps": 58.2},
    "phase_2_physics_render": {"duration": 15, "stable": true, "avg_fps": 48.7},
    "phase_3_physics_render_movement": {"duration": 12, "stable": false, "avg_fps": 23.1},
    "failure_point": "MovementSystem cascade caused system lockup"
  },
  "system_interactions": {
    "physics_render": "Minimal negative interaction",
    "physics_movement": "High contention on Transform components",
    "render_movement": "GPU stalls waiting for CPU updates"
  },
  "critical_path": [
    "MovementSystem updates Transform",
    "PhysicsSystem reads Transform (contention)",
    "RenderSystem waits for both (cascade delay)"
  ]
}
```

## Progressive Stress Testing

### Gradual Load Increase

```json
{
  "test_type": "progressive_entity_spawn",
  "phases": [
    {"duration": 30, "entity_rate": 10, "name": "warmup"},
    {"duration": 60, "entity_rate": 25, "name": "ramp_up"},
    {"duration": 120, "entity_rate": 50, "name": "sustained"},
    {"duration": 60, "entity_rate": 100, "name": "peak_load"},
    {"duration": 30, "entity_rate": 0, "name": "cooldown"}
  ]
}
```

### Adaptive Load Testing

```json
{
  "test_type": "adaptive_stress",
  "params": {
    "target_fps": 45,
    "load_adjustment_rate": 1.1,
    "measurement_window": 10,
    "convergence_threshold": 0.05
  }
}
```

## Safety Features

### Automatic Circuit Breakers

Stress tests include safety mechanisms to prevent system damage:

```json
{
  "safety_limits": {
    "min_fps": 5,
    "max_memory_percent": 90,
    "max_cpu_percent": 95,
    "max_gpu_temp_celsius": 85,
    "emergency_stop_conditions": [
      "system_hang",
      "memory_exhaustion",
      "thermal_throttling"
    ]
  }
}
```

### Recovery Procedures

```json
{
  "recovery_config": {
    "cleanup_spawned_entities": true,
    "restore_original_settings": true,
    "cooldown_period": 30,
    "health_check_required": true
  }
}
```

## Performance Analysis

### Real-time Metrics

During stress tests, these metrics are continuously monitored:

- **Frame Time**: Min, max, average, 95th percentile
- **Memory Usage**: Heap, stack, GPU memory
- **CPU Utilization**: Per-core usage, system load
- **GPU Metrics**: Utilization, memory, temperature
- **Entity Counts**: By archetype and total
- **System Performance**: Execution time per system

### Bottleneck Identification

```json
{
  "bottleneck_analysis": {
    "cpu_bound": {
      "systems": ["MovementSystem", "PhysicsSystem"],
      "utilization": 94.2,
      "recommendation": "Optimize hot loops, use SIMD"
    },
    "gpu_bound": {
      "draw_calls": 4521,
      "fill_rate": 89.7,
      "recommendation": "Reduce overdraw, batch rendering"
    },
    "memory_bound": {
      "allocation_rate": "145 MB/s",
      "gc_pressure": "high",
      "recommendation": "Object pooling, reduce allocations"
    }
  }
}
```

## Best Practices

### 1. Start Conservative
Begin with lower intensities and shorter durations:
```json
{"intensity": 1.5, "duration": 30}
```

### 2. Monitor System Health
Always include safety limits:
```json
{
  "safety_limits": {
    "min_fps": 10,
    "max_memory_percent": 85,
    "thermal_protection": true
  }
}
```

### 3. Test One Thing at a Time
Isolate variables to identify specific bottlenecks:
```json
{"test_type": "entity_spawn", "entity_type": "simple_sprite"}
```
vs
```json
{"test_type": "entity_spawn", "entity_type": "complex_physics"}
```

### 4. Document Baselines
Record baseline performance before optimization attempts:
```json
{"establish_baseline": true, "baseline_duration": 60}
```

## Common Stress Test Scenarios

### Performance Regression Testing
```
"Verify that recent changes didn't reduce the entity limit from 2000 to below 1800"
```

### Scalability Planning
```
"Test if the game can handle 4x the current entity count for multiplayer expansion"
```

### Optimization Validation
```
"Compare performance before and after MovementSystem optimization"
```

### Platform Compatibility
```
"Test performance limits on minimum system requirements"
```

## Integration with CI/CD

```yaml
# Performance regression detection in CI
- name: Stress Test Performance
  run: |
    bevy-debugger-mcp stress_test entity_spawn --max-entities=2000 --duration=60
    if [ $ENTITY_LIMIT -lt 1800 ]; then exit 1; fi
```

## Troubleshooting

### Test Terminates Early
- Check safety limits aren't too restrictive
- Verify system has sufficient resources
- Look for memory leaks or resource exhaustion

### Inconsistent Results
- Run multiple iterations and average results
- Check for background processes affecting performance
- Ensure consistent starting state

### System Becomes Unresponsive
- Reduce test intensity
- Implement shorter test phases
- Add more frequent safety checks

## Related Tools

- **[observe](observe.md)**: Monitor system state during stress tests
- **[detect_anomaly](detect_anomaly.md)**: Automatically detect when stress causes unusual behavior
- **[experiment](experiment.md)**: Run controlled experiments based on stress test findings
- **[hypothesis](hypothesis.md)**: Test theories about performance limits discovered

---

*Stress testing reveals the true character of your game's performance - use it to build systems that gracefully handle extreme conditions.*
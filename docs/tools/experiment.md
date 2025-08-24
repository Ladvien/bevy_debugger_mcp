# Experiment Tool

**Description**: Run controlled experiments on your Bevy game to test behavior and performance.

The `experiment` tool allows you to make controlled changes to your game state and measure the results. It's perfect for testing hypotheses, reproducing bugs, and validating fixes in a systematic way.

## When to Use

- **Bug Reproduction**: Recreate specific conditions that trigger bugs
- **Performance Testing**: Measure impact of changes on game performance
- **System Validation**: Test how systems respond to different inputs
- **Edge Case Testing**: Explore boundary conditions and unusual scenarios
- **Feature Verification**: Confirm new features work as expected

## Parameters

- `experiment_type` (string, required): Type of experiment to run
- `params` (object, optional): Experiment-specific parameters
- `duration` (integer, optional): Duration in seconds. Default: 10
- `iterations` (integer, optional): Number of times to repeat. Default: 1

## Experiment Types

### Performance Experiments

#### `performance_test`
Measures game performance under specific conditions.

**Parameters**:
- `metrics` (array): Performance metrics to track (frame_time, memory, cpu, gpu)
- `duration_seconds` (integer): How long to measure
- `baseline` (boolean): Establish performance baseline

**Example**:
```json
{
  "experiment_type": "performance_test",
  "params": {
    "metrics": ["frame_time", "memory", "entity_count"],
    "duration_seconds": 30,
    "baseline": true
  }
}
```

#### `load_test`
Tests game behavior under increasing load.

**Parameters**:
- `load_type` (string): Type of load (entities, systems, network)
- `increment` (integer): How much to increase load each step
- `max_load` (integer): Maximum load to test

### Entity Experiments

#### `entity_spawn`
Spawns entities to test system behavior.

**Parameters**:
- `count` (integer): Number of entities to spawn
- `archetype` (string): Type of entities to spawn
- `location` (array): Where to spawn entities [x, y, z]
- `spread` (number): Random spread around location

**Example**:
```json
{
  "experiment_type": "entity_spawn",
  "params": {
    "count": 100,
    "archetype": "enemy",
    "location": [0.0, 0.0, 0.0],
    "spread": 10.0
  }
}
```

#### `entity_modify`
Modifies existing entities to test component changes.

**Parameters**:
- `selector` (string): Query to select entities
- `modifications` (object): Changes to make
- `revert_after` (boolean): Revert changes after experiment

### System Experiments

#### `system_disable`
Temporarily disables systems to test their impact.

**Parameters**:
- `systems` (array): System names to disable
- `duration_seconds` (integer): How long to disable

#### `system_stress`
Stresses specific systems with high workloads.

**Parameters**:
- `system_name` (string): System to stress test
- `workload_multiplier` (number): Increase workload by factor

### Physics Experiments

#### `physics_test`
Tests physics behavior and performance.

**Parameters**:
- `gravity_multiplier` (number): Modify gravity strength
- `collision_count` (integer): Number of collision objects
- `force_application` (object): Apply forces to entities

### Network Experiments

#### `network_simulation`
Simulates network conditions for multiplayer testing.

**Parameters**:
- `latency_ms` (integer): Simulated network latency
- `packet_loss_percent` (number): Percentage of packets to drop
- `bandwidth_kbps` (integer): Simulated bandwidth limit

## Example Usage

### Example 1: Performance Impact Testing

**Goal**: Test how spawning many enemies affects performance.

**Command**: 
```
Test how spawning 500 enemies affects game performance
```

**Generated Experiment**:
```json
{
  "experiment_type": "performance_test",
  "params": {
    "pre_experiment": {
      "type": "entity_spawn",
      "count": 500,
      "archetype": "enemy"
    },
    "metrics": ["frame_time", "memory", "cpu"],
    "duration_seconds": 60
  }
}
```

**Results**:
- Baseline: 60 FPS (16.67ms frame time)
- With 500 enemies: 45 FPS (22.22ms frame time)  
- Memory usage: +120MB
- CPU usage: +25%

### Example 2: System Isolation Testing

**Goal**: Determine which system is causing frame drops.

**Command**:
```
Test performance with MovementSystem disabled for 30 seconds
```

**Generated Experiment**:
```json
{
  "experiment_type": "system_disable",
  "params": {
    "systems": ["MovementSystem"],
    "duration_seconds": 30,
    "measure_performance": true
  }
}
```

**Results**:
- Performance improved by 8ms per frame
- MovementSystem identified as bottleneck
- Recommendation: Optimize MovementSystem implementation

### Example 3: Edge Case Reproduction

**Goal**: Reproduce collision detection bug at world boundaries.

**Command**:
```
Spawn 50 entities at world boundary coordinates to test collision detection
```

**Generated Experiment**:
```json
{
  "experiment_type": "entity_spawn",
  "params": {
    "count": 50,
    "archetype": "physics_entity",
    "location": [999.0, 999.0, 0.0],
    "spread": 10.0,
    "monitor_collisions": true
  },
  "duration": 30
}
```

**Results**:
- Bug reproduced: 3 entities fell through world boundary
- Collision detection fails at coordinates > 1000
- Issue logged with reproduction steps

### Example 4: Load Testing

**Goal**: Find the maximum number of entities the game can handle.

**Command**:
```
Run a load test to find the maximum number of moving entities before performance drops below 30 FPS
```

**Generated Experiment**:
```json
{
  "experiment_type": "load_test",
  "params": {
    "load_type": "moving_entities",
    "start_count": 100,
    "increment": 100,
    "max_count": 2000,
    "fps_threshold": 30,
    "stop_on_threshold": true
  }
}
```

**Results**:
- Maximum entities: 1,200 moving entities
- Performance cliff at 1,300 entities
- Recommendation: Cap entity count at 1,000 for safety margin

### Example 5: Physics Parameter Testing

**Goal**: Test how gravity changes affect gameplay.

**Command**:
```
Experiment with different gravity settings to see how they affect jump mechanics
```

**Generated Experiment**:
```json
{
  "experiment_type": "physics_test",
  "params": {
    "gravity_multipliers": [0.5, 1.0, 1.5, 2.0],
    "test_duration_each": 15,
    "record_player_jumps": true,
    "measure_jump_heights": true
  }
}
```

**Results**:
- 0.5x gravity: Jump height 2.3m (too floaty)
- 1.0x gravity: Jump height 1.2m (baseline)
- 1.5x gravity: Jump height 0.9m (responsive)
- 2.0x gravity: Jump height 0.6m (too heavy)

## Advanced Features

### Multi-Phase Experiments

Run complex experiments with multiple phases:

```json
{
  "experiment_type": "multi_phase",
  "params": {
    "phases": [
      {
        "name": "baseline",
        "type": "performance_test",
        "duration": 10
      },
      {
        "name": "load_entities", 
        "type": "entity_spawn",
        "count": 1000
      },
      {
        "name": "measure_impact",
        "type": "performance_test", 
        "duration": 30
      },
      {
        "name": "cleanup",
        "type": "entity_cleanup",
        "filter": "spawned_entities"
      }
    ]
  }
}
```

### Experiment Chaining

Chain experiments to build complex test scenarios:

```json
{
  "experiment_type": "chained",
  "params": {
    "experiments": [
      {"type": "entity_spawn", "count": 100, "name": "wave1"},
      {"type": "wait", "duration": 5},
      {"type": "entity_spawn", "count": 200, "name": "wave2"}, 
      {"type": "system_stress", "system": "CollisionSystem"},
      {"type": "performance_measure", "duration": 30}
    ]
  }
}
```

### Randomized Testing

Use randomized parameters for broader testing:

```json
{
  "experiment_type": "randomized_spawn",
  "params": {
    "iterations": 50,
    "random_count": {"min": 10, "max": 1000},
    "random_location": {"bounds": [[-100, -100], [100, 100]]},
    "random_archetype": ["enemy", "neutral", "powerup"]
  }
}
```

## Experiment Results

All experiments return structured results:

```json
{
  "success": true,
  "data": {
    "experiment_id": "exp_2024_01_15_001",
    "type": "performance_test",
    "duration_ms": 30000,
    "parameters": { /* original parameters */ },
    "results": {
      "metrics": {
        "avg_frame_time_ms": 18.5,
        "min_frame_time_ms": 12.1, 
        "max_frame_time_ms": 45.2,
        "memory_mb": 234.7,
        "entity_count": 1547
      },
      "violations": [
        {
          "metric": "frame_time",
          "threshold": 16.67,
          "actual": 45.2,
          "timestamp": "2024-01-15T10:30:15Z"
        }
      ],
      "recommendations": [
        "Consider optimizing MovementSystem for better performance",
        "Entity count approaching recommended maximum"
      ]
    },
    "artifacts": [
      "screenshots/experiment_001_start.png",
      "logs/experiment_001_detailed.json"
    ]
  }
}
```

## Performance Monitoring

Experiments automatically track key metrics:

- **Frame Time**: Average, min, max frame times
- **Memory Usage**: RAM and VRAM consumption  
- **CPU Usage**: Processing load percentage
- **Entity Count**: Total entities and by archetype
- **System Performance**: Per-system execution times
- **Network**: Bandwidth, latency, packet loss (if applicable)

## Best Practices

### 1. Establish Baselines
Always measure baseline performance before running experiments:

```json
{
  "experiment_type": "performance_test",
  "params": {"baseline": true, "duration_seconds": 10}
}
```

### 2. Isolate Variables
Change only one thing at a time:

```json
// Good: Test one system
{"experiment_type": "system_disable", "systems": ["MovementSystem"]}

// Bad: Test multiple systems
{"experiment_type": "system_disable", "systems": ["MovementSystem", "PhysicsSystem", "RenderSystem"]}
```

### 3. Use Controls
Include control conditions to validate results:

```json
{
  "experiment_type": "entity_spawn_comparison",
  "params": {
    "test_group": {"count": 1000, "archetype": "complex_entity"},
    "control_group": {"count": 1000, "archetype": "simple_entity"}
  }
}
```

### 4. Repeat Important Tests
Run critical experiments multiple times:

```json
{
  "experiment_type": "performance_test",
  "params": {"iterations": 5, "duration_seconds": 20}
}
```

## Safety Features

### Automatic Rollback
Experiments automatically revert changes if they cause:
- Frame rate < 5 FPS for > 3 seconds
- Memory usage > 95% of available RAM
- System crashes or hangs

### Resource Limits
Built-in limits prevent experiments from overwhelming the system:
- Maximum 10,000 spawned entities per experiment
- Maximum 60 second duration for destructive tests
- CPU usage monitoring with automatic throttling

### State Preservation
Game state is captured before dangerous experiments:
- Entity snapshots taken
- Component states saved
- System configurations backed up

## Troubleshooting

### Experiment Fails to Start
- Check parameter syntax is correct
- Verify game is running and responsive
- Ensure sufficient system resources available

### Results Seem Invalid
- Run baseline measurement first
- Check for external system load
- Verify game state is consistent between runs

### Performance Impact Too High
- Reduce experiment duration or intensity
- Use throttling parameters
- Run during off-peak system usage

## Related Tools

- **[observe](observe.md)**: Examine entities before/after experiments
- **[hypothesis](hypothesis.md)**: Form theories to test with experiments
- **[stress_test](stress_test.md)**: Specialized high-intensity experiments
- **[detect_anomaly](detect_anomaly.md)**: Automatically detect experiment side effects

---

*Use experiments to move from "I think this might be the problem" to "I know exactly what causes this issue and how to fix it."*
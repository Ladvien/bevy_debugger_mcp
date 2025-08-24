# Hypothesis Tool

**Description**: Test hypotheses about game behavior and state using systematic testing and statistical analysis.

The `hypothesis` tool helps you move from guessing to knowing by providing rigorous testing of your theories about game behavior. It uses statistical methods to determine whether your hypotheses are supported by evidence.

## When to Use

- **Root Cause Analysis**: Test theories about why bugs occur
- **Performance Investigation**: Validate assumptions about performance bottlenecks
- **Behavior Verification**: Confirm how game systems interact
- **Feature Validation**: Test if new features work as designed
- **Regression Testing**: Verify fixes don't break other functionality

## Parameters

- `hypothesis` (string, required): The hypothesis to test in natural language
- `confidence` (number, optional): Required confidence level (0.0-1.0). Default: 0.95
- `test_duration` (integer, optional): How long to test in seconds. Default: 30
- `sample_size` (integer, optional): Minimum number of observations. Default: 100

## Hypothesis Types

### Performance Hypotheses

Test theories about game performance and optimization.

**Examples**:
- "Frame rate drops when entity count exceeds 1000"
- "MovementSystem is the primary performance bottleneck"
- "Memory usage increases linearly with enemy spawns"
- "GPU usage spikes during explosion effects"

### Behavioral Hypotheses

Test theories about how game systems behave.

**Examples**:
- "Player movement speed affects collision detection accuracy"
- "Enemies spawn more frequently near the player"
- "Health regeneration rate depends on player level"
- "Physics simulation accuracy decreases at high speeds"

### Causal Hypotheses

Test cause-and-effect relationships in your game.

**Examples**:
- "Reducing render distance improves frame rate"
- "Disabling particle effects reduces memory usage"
- "Increasing enemy AI complexity causes CPU spikes"
- "Network lag causes entity position desynchronization"

## Example Usage

### Example 1: Performance Bottleneck Investigation

**Hypothesis**: "MovementSystem consumes more than 5ms per frame when there are over 500 moving entities"

**Command**:
```
Test the hypothesis that MovementSystem is slow with many entities
```

**Generated Test**:
```json
{
  "hypothesis": "MovementSystem execution time > 5ms when entity count > 500",
  "confidence": 0.95,
  "test_duration": 60,
  "test_parameters": {
    "entity_counts": [100, 300, 500, 750, 1000],
    "measurement": "system_execution_time",
    "target_system": "MovementSystem"
  }
}
```

**Results**:
```json
{
  "hypothesis_supported": true,
  "confidence": 0.98,
  "evidence": {
    "100_entities": {"avg_time_ms": 1.2, "samples": 120},
    "300_entities": {"avg_time_ms": 3.1, "samples": 120},
    "500_entities": {"avg_time_ms": 5.8, "samples": 120},
    "750_entities": {"avg_time_ms": 8.4, "samples": 120},
    "1000_entities": {"avg_time_ms": 12.1, "samples": 120}
  },
  "statistical_analysis": {
    "correlation": 0.97,
    "p_value": 0.001,
    "regression": "y = 0.012x + 0.5"
  },
  "conclusion": "Strong evidence supports the hypothesis. MovementSystem execution time increases linearly with entity count, exceeding 5ms threshold at ~480 entities."
}
```

### Example 2: Bug Reproduction Hypothesis

**Hypothesis**: "Collision detection fails when entities move faster than 50 units per second"

**Command**:
```
Test if collision detection accuracy depends on entity movement speed
```

**Generated Test**:
```json
{
  "hypothesis": "collision_detection_accuracy < 0.9 when entity_speed > 50",
  "confidence": 0.95,
  "test_duration": 45,
  "test_parameters": {
    "speed_ranges": [10, 25, 50, 75, 100, 150],
    "collision_pairs": 50,
    "accuracy_threshold": 0.9
  }
}
```

**Results**:
```json
{
  "hypothesis_supported": true,
  "confidence": 0.99,
  "evidence": {
    "speed_10": {"accuracy": 0.98, "collisions_detected": 49, "total": 50},
    "speed_25": {"accuracy": 0.96, "collisions_detected": 48, "total": 50}, 
    "speed_50": {"accuracy": 0.94, "collisions_detected": 47, "total": 50},
    "speed_75": {"accuracy": 0.82, "collisions_detected": 41, "total": 50},
    "speed_100": {"accuracy": 0.74, "collisions_detected": 37, "total": 50}
  },
  "conclusion": "Hypothesis confirmed. Collision accuracy drops below 90% at speeds above 60 units/second."
}
```

### Example 3: Feature Effectiveness Hypothesis

**Hypothesis**: "The new enemy AI makes battles last 30% longer on average"

**Command**:
```
Test if the new enemy AI affects battle duration compared to the old AI
```

**Generated Test**:
```json
{
  "hypothesis": "new_ai_battle_duration > old_ai_battle_duration * 1.3",
  "confidence": 0.95,
  "test_duration": 300,
  "test_parameters": {
    "battle_scenarios": 25,
    "ai_versions": ["old", "new"],
    "duration_metric": "time_to_defeat_all_enemies"
  }
}
```

**Results**:
```json
{
  "hypothesis_supported": true,
  "confidence": 0.94,
  "evidence": {
    "old_ai": {"avg_duration_sec": 45.2, "std_dev": 8.7, "battles": 25},
    "new_ai": {"avg_duration_sec": 61.8, "std_dev": 12.4, "battles": 25}
  },
  "statistical_tests": {
    "t_test_p_value": 0.003,
    "effect_size": 1.47,
    "duration_increase_percent": 36.7
  },
  "conclusion": "New AI increases battle duration by 36.7% on average (exceeds 30% threshold)."
}
```

### Example 4: Memory Leak Investigation

**Hypothesis**: "Memory usage increases by more than 10MB per minute during gameplay"

**Command**:
```
Test if there's a memory leak during normal gameplay
```

**Generated Test**:
```json
{
  "hypothesis": "memory_growth_rate > 10 MB/minute during gameplay",
  "confidence": 0.95,
  "test_duration": 600,
  "test_parameters": {
    "measurement_interval": 10,
    "gameplay_simulation": true,
    "memory_threshold": 10
  }
}
```

**Results**:
```json
{
  "hypothesis_supported": false,
  "confidence": 0.97,
  "evidence": {
    "memory_growth_rate_mb_per_min": 2.3,
    "total_growth_mb": 23.1,
    "test_duration_min": 10,
    "growth_pattern": "linear_with_plateaus"
  },
  "conclusion": "No significant memory leak detected. Growth rate (2.3 MB/min) is well below threshold."
}
```

## Advanced Hypothesis Testing

### Multi-Variable Hypotheses

Test complex relationships between multiple variables:

```json
{
  "hypothesis": "frame_time = f(entity_count, particle_count, light_count)",
  "test_parameters": {
    "variables": {
      "entity_count": [100, 500, 1000],
      "particle_count": [0, 50, 100],
      "light_count": [1, 5, 10]
    },
    "full_factorial": true
  }
}
```

### Time-Series Hypotheses

Test hypotheses that involve changes over time:

```json
{
  "hypothesis": "Player skill improvement follows a logarithmic curve",
  "test_parameters": {
    "time_series": true,
    "measurement": "player_score",
    "duration_minutes": 60,
    "expected_curve": "logarithmic"
  }
}
```

### Comparative Hypotheses

Compare different implementations or configurations:

```json
{
  "hypothesis": "Algorithm A is 20% faster than Algorithm B for pathfinding",
  "test_parameters": {
    "algorithms": ["A", "B"],
    "test_scenarios": ["maze", "open_field", "obstacles"],
    "performance_metric": "execution_time"
  }
}
```

## Statistical Analysis

The hypothesis tool provides rigorous statistical analysis:

### Confidence Levels
- **0.90 (90%)**: Quick validation, some uncertainty acceptable
- **0.95 (95%)**: Standard scientific confidence level
- **0.99 (99%)**: High confidence for critical decisions

### Test Types
- **t-test**: Compare means between groups
- **ANOVA**: Compare multiple groups
- **Correlation**: Measure relationship strength
- **Regression**: Model relationships between variables

### Effect Size Calculation
Measures practical significance beyond statistical significance:
- **Small effect**: 0.2-0.5
- **Medium effect**: 0.5-0.8  
- **Large effect**: 0.8+

## Hypothesis Result Interpretation

### Supported Hypothesis
```json
{
  "hypothesis_supported": true,
  "confidence": 0.96,
  "practical_significance": "medium",
  "recommendation": "Evidence strongly supports the hypothesis. Consider implementing the proposed optimization."
}
```

### Rejected Hypothesis
```json
{
  "hypothesis_supported": false,
  "confidence": 0.92,
  "alternative_explanations": [
    "The effect may be smaller than hypothesized",
    "Other factors might be influencing the outcome"
  ],
  "recommendation": "Revise hypothesis or investigate alternative causes."
}
```

### Inconclusive Results
```json
{
  "hypothesis_supported": null,
  "confidence": 0.78,
  "issues": [
    "Sample size too small for reliable conclusion",
    "High variability in measurements"
  ],
  "recommendation": "Collect more data or refine testing conditions."
}
```

## Best Practices

### 1. Specific Hypotheses
Make hypotheses specific and measurable:

**Good**: "Frame rate drops below 30 FPS when rendering more than 500 particles"
**Bad**: "Too many particles cause performance issues"

### 2. Appropriate Sample Sizes
Use sufficient data for reliable conclusions:
- **Simple comparisons**: 30+ observations per group
- **Complex relationships**: 100+ observations
- **High precision needed**: 1000+ observations

### 3. Control Variables
Control for confounding factors:
```json
{
  "hypothesis": "New shader improves performance",
  "controls": {
    "scene_complexity": "constant",
    "lighting_conditions": "identical", 
    "camera_angle": "fixed"
  }
}
```

### 4. Realistic Effect Sizes
Set reasonable expectations for effect sizes:
- **Performance**: 5-20% improvements are significant
- **User metrics**: 10-30% changes are meaningful
- **Error rates**: Even 1-5% improvements matter

## Common Hypothesis Patterns

### Performance Hypotheses
```
"System X takes longer than Y milliseconds when condition Z is true"
"Memory usage grows by more than X MB per minute during scenario Y"
"CPU usage exceeds X% when feature Y is enabled"
```

### Functional Hypotheses
```
"Feature X works correctly in Y% of test cases"
"Bug X occurs when conditions Y and Z are both present"
"System X fails when input exceeds threshold Y"
```

### Comparative Hypotheses
```
"Algorithm X is faster than Algorithm Y by at least Z%"
"Version X has fewer bugs than Version Y"
"Configuration X uses less memory than Configuration Y"
```

## Integration with Other Tools

### Hypothesis → Experiment
```
1. Form hypothesis about performance issue
2. Use experiment tool to create controlled test
3. Analyze results to confirm or reject hypothesis
```

### Observe → Hypothesis → Stress Test
```
1. Observe unusual behavior patterns
2. Form hypothesis about the cause
3. Use stress_test to validate hypothesis under extreme conditions
```

### Hypothesis → Replay
```
1. Test hypothesis about bug reproduction
2. Use replay tool to capture state when hypothesis is confirmed
3. Analyze replay data for root cause
```

## Troubleshooting

### Low Confidence Results
- Increase sample size with `sample_size` parameter
- Extend test duration with `test_duration`
- Reduce variability by controlling more variables

### Contradictory Results
- Check for confounding variables
- Verify measurement accuracy
- Consider interaction effects between variables

### Test Takes Too Long
- Reduce sample size for preliminary testing
- Use shorter test duration for initial validation
- Focus on most critical variables first

## Related Tools

- **[experiment](experiment.md)**: Run controlled experiments to generate hypothesis test data
- **[observe](observe.md)**: Gather initial observations to form hypotheses  
- **[detect_anomaly](detect_anomaly.md)**: Automatically detect patterns that suggest hypotheses
- **[stress_test](stress_test.md)**: Test hypotheses under extreme conditions

---

*Use hypothesis testing to transform debugging from guesswork into scientific investigation.*
# Observe Tool

**Description**: Observe and query Bevy game state in real-time with optional reflection-based component inspection.

The `observe` tool is your primary window into your Bevy game's ECS world. It allows you to query entities, components, resources, and system state using natural language queries.

## When to Use

- **Entity Investigation**: Find specific entities by component or behavior
- **Component Inspection**: Examine component values and relationships  
- **System Debugging**: Understand how systems are affecting entities
- **Performance Analysis**: Count entities and identify potential bottlenecks
- **State Verification**: Confirm game state matches expectations

## Parameters

- `query` (string, required): Natural language query describing what to observe
- `diff` (boolean, optional): Show changes since last observation. Default: false
- `reflection` (boolean, optional): Enable deep component analysis with field inspection. Default: false

## Query Syntax Examples

### Basic Entity Queries

```
"all entities"
"entities with Transform"
"entities with Transform and Velocity"
"entities without Health component"
"player entities"
```

### Filtered Queries

```
"entities with health < 50"
"fast moving entities"
"entities near the origin"
"entities spawned in the last frame"
"inactive entities"
```

### Component-Specific Queries

```
"Transform components"
"Camera entities with their settings"
"all Sprite components"
"entities with custom Player component"
```

### System and Performance Queries

```
"system performance metrics"
"entity count by archetype"
"recently changed entities"
"memory usage by component type"
```

## Example Usage

### Example 1: Basic Entity Inspection

**Query**: 
```json
{
  "query": "entities with Transform and Velocity components"
}
```

**Response**:
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
              "translation": [10.5, 0.0, -5.2],
              "rotation": [0.0, 0.0, 0.0, 1.0],
              "scale": [1.0, 1.0, 1.0]
            }
          },
          {
            "type_name": "Velocity",
            "data": {
              "linear": [2.5, 0.0, 0.0],
              "angular": 0.1
            }
          }
        ]
      }
    ],
    "total_count": 1,
    "query_time_ms": 12.4
  }
}
```

### Example 2: Player Character Investigation

**Command**: 
```
Show me all player entities with their health and position
```

**Query Generated**: 
```json
{
  "query": "player entities with health and position details",
  "reflection": true
}
```

**Use Case**: Debug player state issues, verify health calculations, check positioning.

### Example 3: Performance Bottleneck Detection

**Command**: 
```
Find entities that might be causing performance issues
```

**Query Generated**: 
```json
{
  "query": "entities with high component count or complex systems"
}
```

**Use Case**: Identify entities with too many components or systems affecting too many entities.

### Example 4: Differential Analysis

**Command**: 
```
What changed in my game state since the last observation?
```

**Query Generated**: 
```json
{
  "query": "all entities",
  "diff": true
}
```

**Response Shows**: 
- New entities spawned
- Entities despawned  
- Component value changes
- System state changes

### Example 5: Deep Component Inspection

**Command**: 
```
Give me detailed information about the Camera entity including all field values
```

**Query Generated**: 
```json
{
  "query": "Camera entities",
  "reflection": true
}
```

**Response Includes**:
- All component fields with their values
- Type information for complex types
- Nested struct/enum contents
- Option and Vec contents

## Advanced Features

### Reflection Mode

When `reflection: true` is enabled, the tool provides deep inspection of component data:

- **Field-level inspection**: See individual struct fields
- **Type information**: Understand component structure
- **Collection contents**: See inside Vec, HashMap, Option types
- **Custom inspectors**: Handle complex game-specific types

Example with reflection:
```json
{
  "query": "player inventory system",
  "reflection": true
}
```

Returns detailed inventory item data, quantities, equipped status, etc.

### Diff Mode

When `diff: true` is enabled, only changes since the last observation are shown:

- **Added entities**: New spawns
- **Removed entities**: Despawned entities  
- **Modified components**: Changed values
- **Performance impact**: Lower overhead for monitoring

### Query Language Features

The observe tool supports sophisticated natural language queries:

**Spatial queries**:
```
"entities within 10 units of position (0,0,0)"
"entities in the upper-right quadrant"
"entities between the player and the camera"
```

**Conditional queries**:
```
"enemies with health > 50 and speed < 2.0"
"entities that moved more than 5 units this frame"
"projectiles older than 3 seconds"
```

**Relationship queries**:
```
"all children of the player entity"
"entities with the same parent as entity 123"
"hierarchical relationships in the scene"
```

## Performance Considerations

### Query Complexity
- **Simple queries** (< 50ms): Basic component filters
- **Complex queries** (< 200ms): Multi-component with conditions
- **Reflection queries**: Additional overhead for deep inspection

### Memory Usage  
- Results are streamed for large entity sets
- Entity data cached with LRU eviction
- Diff mode reduces memory overhead

### Best Practices

1. **Start Broad, Then Narrow**: Begin with general queries, then get specific
   ```
   "all entities" → "entities with Transform" → "moving entities"
   ```

2. **Use Diff Mode for Monitoring**: Enable diff when tracking changes over time
   ```json
   {"query": "performance critical entities", "diff": true}
   ```

3. **Combine with Screenshots**: Visual confirmation of entity state
   ```
   First: observe "player position and rotation"
   Then: take screenshot for visual verification
   ```

4. **Leverage Reflection Carefully**: Only use when you need deep inspection
   ```json
   {"query": "complex component state", "reflection": true}
   ```

## Common Query Patterns

### Debugging Entity Spawning
```
"entities spawned in the last second"
"entities without required components"  
"duplicate entities by archetype"
```

### Performance Monitoring
```
"entities with >10 components"
"systems affecting >1000 entities"
"high-frequency component updates"
```

### Game State Validation
```
"player health and position consistency"
"UI elements and their visibility state"
"collision detection entity pairs"
```

### System Behavior Analysis
```
"entities modified by MovementSystem"
"components updated this frame"
"system execution order effects"
```

## Troubleshooting

### "No entities found"
- Check component names are correct
- Verify entities exist with basic query: `"all entities"`
- Try broader query: `"entities with any component"`

### "Query too complex"
- Simplify conditions
- Break into multiple queries
- Use basic filters first

### "Reflection failed"
- Component may not implement Reflect trait
- Use basic inspection without reflection
- Check component registration

### Poor Performance
- Avoid querying all entities frequently
- Use specific component filters
- Consider diff mode for repeated queries

## Integration Examples

### With Other Tools

**Observe + Experiment**:
```
1. observe "entities with low health"
2. experiment "heal all low health entities" 
3. observe "entities with low health" (verify healing)
```

**Observe + Screenshot**:
```
1. observe "player position and camera settings"
2. screenshot with description "player at observed position"
```

**Observe + Stress Test**:
```
1. observe "entity count baseline" 
2. stress_test "spawn 1000 entities"
3. observe "entity count after stress test"
```

### Continuous Monitoring

Set up monitoring workflows:

```javascript
// Monitor entity health every 5 seconds
setInterval(async () => {
  const result = await mcpClient.callTool("observe", {
    query: "entities with health < 25",
    diff: true
  });
  if (result.data.total_count > 0) {
    console.log("Low health entities detected:", result.data);
  }
}, 5000);
```

## Related Tools

- **[experiment](experiment.md)**: Test theories discovered through observation
- **[hypothesis](hypothesis.md)**: Form hypotheses about observed behavior  
- **[detect_anomaly](detect_anomaly.md)**: Automatically detect unusual patterns
- **[stress_test](stress_test.md)**: Test system limits with many entities

---

*The observe tool is the foundation of debugging - use it to understand your game's current state before taking corrective actions.*
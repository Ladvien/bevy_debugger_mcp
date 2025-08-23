# Bevy 0.16 BRP Protocol Migration Guide

This document outlines the changes made to support Bevy 0.16's Bevy Remote Protocol (BRP) and provides migration guidance for users upgrading from previous versions.

## Summary of Changes

The Bevy Debugger MCP has been updated to support Bevy 0.16's enhanced BRP protocol. This includes new message types, improved entity handling, and backward-compatible enhancements.

## New Features in Bevy 0.16 BRP Support

### 1. Enhanced Query Operations with Strict Mode

**New Feature**: The `bevy/query` operation now supports a `strict` parameter that controls error handling behavior when components are missing or invalid.

**Before** (Legacy behavior):
```json
{
  "method": "bevy/query",
  "params": {
    "filter": {
      "with": ["Transform", "NonExistentComponent"]
    }
  }
}
```
This would return an error if `NonExistentComponent` doesn't exist.

**After** (Bevy 0.16 with strict mode):
```json
{
  "method": "bevy/query",
  "params": {
    "filter": {
      "with": ["Transform", "NonExistentComponent"]  
    },
    "strict": false
  }
}
```
- `strict: false` (default): Skips missing/invalid components, returns entities with available components
- `strict: true`: Returns error for missing/invalid components (legacy behavior)

### 2. New BRP Methods

#### bevy/insert
Insert components into an existing entity:
```json
{
  "method": "bevy/insert",
  "params": {
    "entity": 12345,
    "components": {
      "Transform": {
        "translation": [0.0, 1.0, 0.0],
        "rotation": [0.0, 0.0, 0.0, 1.0],
        "scale": [1.0, 1.0, 1.0]
      }
    }
  }
}
```

#### bevy/remove  
Remove specific components from an entity:
```json
{
  "method": "bevy/remove",
  "params": {
    "entity": 12345,
    "components": ["Transform", "Velocity"]
  }
}
```

#### bevy/reparent
Change entity parent-child relationships:
```json
{
  "method": "bevy/reparent", 
  "params": {
    "entity": 12345,
    "parent": 67890
  }
}
```

### 3. Enhanced Entity Representation

**New**: `EntityWithGeneration` structure for proper entity lifecycle handling:

```rust
// Entity with generation for reuse detection
pub struct EntityWithGeneration {
    pub index: u32,      // Entity index
    pub generation: u32, // Generation for reuse detection
}
```

**Benefits**:
- Proper detection of entity reuse
- Better debugging of entity lifecycle issues
- Compatibility with Bevy's internal entity representation

### 4. Enhanced Component Type Support

**Improved**: Better support for fully-qualified component type names:
- `bevy_transform::components::transform::Transform`
- `bevy_render::view::visibility::Visibility`
- `my_game::components::Player`

## Backward Compatibility

### ✅ Fully Backward Compatible

1. **Legacy Query Format**: Queries without `strict` parameter continue to work with default behavior
2. **Existing Entity IDs**: All existing `EntityId` (u64) representations remain valid
3. **Component Type Names**: Both simple names (`"Transform"`) and fully-qualified names work
4. **Existing BRP Methods**: All previously supported methods (`bevy/get`, `bevy/set`, `bevy/spawn`, `bevy/destroy`) work unchanged

### ⚠️ Behavior Changes

1. **Default Query Error Handling**: 
   - **Before**: Missing components caused query failures
   - **After**: Missing components are skipped by default (use `strict: true` to restore old behavior)

## Migration Steps

### For Applications Using Default Query Behavior

No changes required - applications will automatically benefit from more lenient query handling.

### For Applications Requiring Strict Component Validation

Add `"strict": true` to query requests:

```diff
{
  "method": "bevy/query",
  "params": {
    "filter": {
      "with": ["Transform", "Velocity"]
    },
+   "strict": true
  }
}
```

### For Applications Managing Entity Lifecycles

Consider using `EntityWithGeneration` for better entity reuse detection:

```rust
// Convert between formats
let entity_with_gen = EntityWithGeneration::from_entity_id(entity_id);
let entity_id = entity_with_gen.to_entity_id();
```

## New Error Codes

The following error codes have been added for Bevy 0.16 features:

- `component_insertion_error`: Component insertion failed
- `component_removal_error`: Component removal failed  
- `reparenting_error`: Entity reparenting failed
- `strict_validation_error`: Strict mode validation failed

## Testing Your Migration

### 1. Test Query Behavior
Verify that queries with missing components behave as expected:
```bash
# Should return entities even with missing components (default)
curl -X POST http://localhost:15702 -d '{
  "jsonrpc": "2.0", 
  "method": "bevy/query",
  "id": 1,
  "params": {"filter": {"with": ["Transform", "NonExistent"]}}
}'

# Should return error with strict mode
curl -X POST http://localhost:15702 -d '{
  "jsonrpc": "2.0",
  "method": "bevy/query", 
  "id": 2,
  "params": {"filter": {"with": ["Transform", "NonExistent"]}, "strict": true}
}'
```

### 2. Test New Methods
Verify new BRP methods work correctly:
```bash
# Test insert
curl -X POST http://localhost:15702 -d '{
  "jsonrpc": "2.0",
  "method": "bevy/insert",
  "id": 3, 
  "params": {"entity": 123, "components": {"Transform": {"translation": [0,0,0]}}}
}'

# Test remove  
curl -X POST http://localhost:15702 -d '{
  "jsonrpc": "2.0",
  "method": "bevy/remove",
  "id": 4,
  "params": {"entity": 123, "components": ["Transform"]}
}'
```

### 3. Integration Test
Use the provided integration test:
```bash
cargo test brp_bevy_16_compatibility
```

## Performance Considerations

### Query Performance
- **Lenient mode** (default): May be faster as it doesn't fail on missing components
- **Strict mode**: May be slower due to additional validation but provides precise error reporting

### Entity Generation Tracking
- **EntityWithGeneration**: Minimal overhead for better debugging capabilities
- **Legacy EntityId**: Still supported with zero overhead

## Troubleshooting

### Common Issues

**Issue**: Queries that previously failed now return empty results
**Solution**: Check if components exist, or enable strict mode to get error feedback

**Issue**: Entity operations fail with new error codes
**Solution**: Verify entity exists and has required components using `bevy/get`

**Issue**: Component type names not recognized  
**Solution**: Use fully-qualified names like `bevy_transform::components::transform::Transform`

### Debug Tips

1. **Enable Debug Logging**: Set `RUST_LOG=debug` to see detailed BRP message processing
2. **Use EntityWithGeneration**: For better entity lifecycle debugging
3. **Test with Strict Mode**: Use `strict: true` to get precise error reporting during development

## Examples

See `tests/brp_bevy_16_compatibility.rs` for comprehensive examples of:
- Strict vs lenient query behavior
- New BRP method usage
- Entity generation handling
- Component type compatibility
- JSON-RPC 2.0 format examples

## Version Support

- **Minimum Bevy Version**: 0.16.0
- **Backward Compatibility**: Maintains compatibility with code written for pre-0.16 BRP
- **Future Compatibility**: Designed to adapt to future Bevy BRP changes

---

For questions or issues with the migration, please check the integration tests or create an issue in the project repository.
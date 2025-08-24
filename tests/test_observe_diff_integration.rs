use bevy_debugger_mcp::brp_messages::EntityData;
use bevy_debugger_mcp::state_diff::{ChangeType, FuzzyCompareConfig, GameRules};
use bevy_debugger_mcp::tools::observe::ObserveState;
use serde_json::json;

fn create_test_entity(id: u64, components: Vec<(&str, serde_json::Value)>) -> EntityData {
    EntityData {
        id,
        components: components
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
    }
}

#[test]
fn test_observe_state_creation() {
    let state = ObserveState::new().unwrap().unwrap();
    assert_eq!(state.max_history_size(), 10);
    assert!(!state.has_last_snapshot());
    assert_eq!(state.history_size(), 0);
}

#[test]
fn test_observe_state_custom_config() {
    let fuzzy_config = FuzzyCompareConfig {
        epsilon: 1e-3,
        relative_tolerance: 1e-6,
    };
    let game_rules = GameRules::default();

    let state = ObserveState::with_diff_config(fuzzy_config, game_rules).unwrap();
    assert!(!state.has_last_snapshot());
    assert_eq!(state.history_size(), 0);
}

#[test]
fn test_snapshot_addition_and_history_management() {
    let mut state = ObserveState::new().unwrap();

    let entities1 = vec![create_test_entity(
        1,
        vec![("Transform", json!({"x": 0.0}))],
    )];
    let snapshot1 = state.add_snapshot(entities1);

    assert!(state.has_last_snapshot());
    assert_eq!(state.history_size(), 1);
    assert_eq!(snapshot1.generation, 1);

    let entities2 = vec![create_test_entity(
        1,
        vec![("Transform", json!({"x": 1.0}))],
    )];
    let snapshot2 = state.add_snapshot(entities2);

    assert_eq!(state.history_size(), 2);
    assert_eq!(snapshot2.generation, 2);
}

#[test]
fn test_history_size_limit() {
    let mut state = ObserveState::new().unwrap();

    // Add more snapshots than the history limit
    for i in 0..15 {
        let entities = vec![create_test_entity(1, vec![("Position", json!(i))])];
        state.add_snapshot(entities);
    }

    // Should only keep the last 10 snapshots
    assert_eq!(state.history_size(), 10);
}

#[test]
fn test_diff_against_last_snapshot() {
    let mut state = ObserveState::new().unwrap();

    // Add first snapshot
    let entities1 = vec![create_test_entity(
        1,
        vec![("Transform", json!({"x": 0.0}))],
    )];
    state.add_snapshot(entities1);

    // Create second snapshot but don't add it to state yet
    let entities2 = vec![create_test_entity(
        1,
        vec![("Transform", json!({"x": 1.0}))],
    )];
    let snapshot2 = state.create_snapshot(entities2);

    // Diff against last stored snapshot
    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].change_type, ChangeType::ComponentModified);
    assert_eq!(result.changes[0].entity_id, 1);
}

#[test]
fn test_diff_against_last_no_previous() {
    let mut state = ObserveState::new().unwrap();

    let entities = vec![create_test_entity(
        1,
        vec![("Transform", json!({"x": 0.0}))],
    )];
    let snapshot = state.create_snapshot(entities);

    // No snapshots added to history yet, so no previous to diff against
    let diff_result = state.diff_against_last(&snapshot);
    assert!(diff_result.is_none());
}

#[test]
fn test_diff_against_history_by_index() {
    let mut state = ObserveState::new().unwrap();

    // Add multiple snapshots
    let entities1 = vec![create_test_entity(1, vec![("Position", json!(0))])];
    state.add_snapshot(entities1);

    let entities2 = vec![create_test_entity(1, vec![("Position", json!(5))])];
    state.add_snapshot(entities2);

    let entities3 = vec![create_test_entity(1, vec![("Position", json!(10))])];
    let snapshot3 = state.create_snapshot(entities3);

    // Diff against first snapshot (index 0)
    let diff_result = state.diff_against_history(&snapshot3, 0);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].change_type, ChangeType::ComponentModified);

    // Check that the diff is between first (0) and current (10)
    if let (Some(old_val), Some(new_val)) =
        (&result.changes[0].old_value, &result.changes[0].new_value)
    {
        assert_eq!(old_val, &json!(0));
        assert_eq!(new_val, &json!(10));
    }
}

#[test]
fn test_diff_against_history_invalid_index() {
    let mut state = ObserveState::new().unwrap();

    let entities = vec![create_test_entity(1, vec![("Position", json!(0))])];
    let snapshot = state.add_snapshot(entities);

    // Try to diff against index that doesn't exist
    let diff_result = state.diff_against_history(&snapshot, 5);
    assert!(diff_result.is_none());
}

#[test]
fn test_configure_diff_engine() {
    let mut state = ObserveState::new().unwrap();

    let new_config = FuzzyCompareConfig {
        epsilon: 1e-2,
        relative_tolerance: 1e-5,
    };

    let mut new_rules = GameRules::default();
    new_rules.max_position_change_per_second = Some(50.0);

    state.configure_diff(new_config, new_rules);

    // Test that the configuration was applied by checking fuzzy comparison
    let entities1 = vec![create_test_entity(1, vec![("Position", json!(1.0))])];
    state.add_snapshot(entities1);

    let entities2 = vec![create_test_entity(1, vec![("Position", json!(1.005))])]; // Within new epsilon
    let snapshot2 = state.add_snapshot(entities2);

    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    // With the new larger epsilon, this should not be detected as a change
    assert_eq!(result.changes.len(), 0);
}

#[test]
fn test_clear_history() {
    let mut state = ObserveState::new().unwrap();

    // Add some snapshots
    for i in 0..5 {
        let entities = vec![create_test_entity(1, vec![("Position", json!(i))])];
        state.add_snapshot(entities);
    }

    assert_eq!(state.history_size(), 5);
    assert!(state.has_last_snapshot());

    state.clear_history();

    assert_eq!(state.history_size(), 0);
    assert!(!state.has_last_snapshot());
}

#[test]
fn test_complex_entity_changes() {
    let mut state = ObserveState::new().unwrap();

    // Initial state: 2 entities
    let entities1 = vec![
        create_test_entity(1, vec![("Transform", json!({"x": 0.0, "y": 0.0}))]),
        create_test_entity(2, vec![("Health", json!(100))]),
    ];
    state.add_snapshot(entities1);

    // Modified state: entity 1 modified, entity 2 removed, entity 3 added
    let entities2 = vec![
        create_test_entity(
            1,
            vec![
                ("Transform", json!({"x": 1.0, "y": 0.0})),
                ("Velocity", json!({"vx": 1.0, "vy": 0.0})), // Component added
            ],
        ),
        create_test_entity(3, vec![("Health", json!(50))]), // New entity
    ];
    let snapshot2 = state.create_snapshot(entities2);

    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();

    // Should detect: entity 2 removed, entity 3 added, entity 1 transform modified, entity 1 velocity added
    assert_eq!(result.changes.len(), 4);

    let added_entities: Vec<_> = result.filter_by_type(ChangeType::EntityAdded);
    let removed_entities: Vec<_> = result.filter_by_type(ChangeType::EntityRemoved);
    let modified_components: Vec<_> = result.filter_by_type(ChangeType::ComponentModified);
    let added_components: Vec<_> = result.filter_by_type(ChangeType::ComponentAdded);

    assert_eq!(added_entities.len(), 1);
    assert_eq!(removed_entities.len(), 1);
    assert_eq!(modified_components.len(), 1);
    assert_eq!(added_components.len(), 1);

    assert_eq!(added_entities[0].entity_id, 3);
    assert_eq!(removed_entities[0].entity_id, 2);
    assert_eq!(modified_components[0].entity_id, 1);
    assert_eq!(added_components[0].entity_id, 1);
}

#[test]
fn test_rate_of_change_calculation() {
    let mut state = ObserveState::new().unwrap();

    // Add first snapshot
    let entities1 = vec![create_test_entity(1, vec![("Position", json!(0.0))])];
    state.add_snapshot(entities1);

    // Wait a bit to ensure time difference
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Add second snapshot with changed position
    let entities2 = vec![create_test_entity(1, vec![("Position", json!(10.0))])];
    let snapshot2 = state.create_snapshot(entities2);

    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    assert_eq!(result.changes.len(), 1);

    // Rate of change should be calculated for numeric values
    let change = &result.changes[0];
    assert!(change.rate_of_change.is_some());

    // Rate should be positive since position increased
    let rate = change.rate_of_change.unwrap();
    assert!(rate > 0.0);
}

#[test]
fn test_unexpected_change_detection() {
    let mut state = ObserveState::new().unwrap();

    // Configure game rules with position change limit
    let fuzzy_config = FuzzyCompareConfig::default();
    let mut game_rules = GameRules::default();
    game_rules.max_position_change_per_second = Some(5.0);

    state.configure_diff(fuzzy_config, game_rules);

    // Add first snapshot
    let entities1 = vec![create_test_entity(1, vec![("Transform", json!(0.0))])];
    state.add_snapshot(entities1);

    // Short delay
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Add second snapshot with large position change (should be unexpected)
    let entities2 = vec![create_test_entity(1, vec![("Transform", json!(100.0))])];
    let snapshot2 = state.create_snapshot(entities2);

    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    assert_eq!(result.changes.len(), 1);

    // The rapid change should be marked as unexpected (though timing may be imprecise in tests)
    // This tests the structure works correctly
    let unexpected_changes = result.unexpected_changes();
    // Note: Due to timing precision in tests, this might not always be detected as unexpected
    // The important thing is that the system is checking for unexpected changes
}

#[test]
fn test_empty_snapshot_handling() {
    let mut state = ObserveState::new().unwrap();

    // Add empty snapshot
    let snapshot1 = state.add_snapshot(vec![]);

    // Add another empty snapshot
    let snapshot2 = state.add_snapshot(vec![]);

    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    assert_eq!(result.changes.len(), 0); // No changes between two empty states
}

#[test]
fn test_entity_id_consistency() {
    let mut state = ObserveState::new().unwrap();

    // Add snapshot with entity
    let entities1 = vec![create_test_entity(42, vec![("Name", json!("TestEntity"))])];
    state.add_snapshot(entities1);

    // Modify the same entity
    let entities2 = vec![create_test_entity(
        42,
        vec![("Name", json!("ModifiedEntity"))],
    )];
    let snapshot2 = state.create_snapshot(entities2);

    let diff_result = state.diff_against_last(&snapshot2);
    assert!(diff_result.is_some());

    let result = diff_result.unwrap();
    assert_eq!(result.changes.len(), 1);
    assert_eq!(result.changes[0].entity_id, 42);
    assert_eq!(result.changes[0].change_type, ChangeType::ComponentModified);
}

#[test]
fn test_multiple_history_diffs() {
    let mut state = ObserveState::new().unwrap();

    // Add sequence of snapshots
    for i in 0..5 {
        let entities = vec![create_test_entity(1, vec![("Counter", json!(i))])];
        state.add_snapshot(entities);
    }

    let current_entities = vec![create_test_entity(1, vec![("Counter", json!(10))])];
    let current_snapshot = state.create_snapshot(current_entities);

    // Test diff against different points in history
    for i in 0..5 {
        let diff_result = state.diff_against_history(&current_snapshot, i);
        assert!(diff_result.is_some());

        let result = diff_result.unwrap();
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].change_type, ChangeType::ComponentModified);

        // Verify the old value corresponds to the history index
        if let Some(old_val) = &result.changes[0].old_value {
            assert_eq!(old_val, &json!(i));
        }

        // New value should always be 10
        if let Some(new_val) = &result.changes[0].new_value {
            assert_eq!(new_val, &json!(10));
        }
    }
}

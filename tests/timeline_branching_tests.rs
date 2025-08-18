use bevy_debugger_mcp::playback_system::RecordingVersion;
use bevy_debugger_mcp::recording_system::{EntityState, Frame, Recording, RecordingConfig};
use bevy_debugger_mcp::timeline_branching::*;
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_branch_id_operations() {
    let id1 = BranchId::new();
    let id2 = BranchId::new();

    // IDs should be unique
    assert_ne!(id1, id2);

    // String conversion should work
    let id_str = id1.to_string();
    let parsed_id = BranchId::from_string(&id_str).unwrap();
    assert_eq!(id1, parsed_id);

    // Invalid UUID should fail
    assert!(BranchId::from_string("invalid-uuid").is_err());
}

#[test]
fn test_modification_layer_creation_and_application() {
    let mut layer = ModificationLayer::new(5, "Test modifications".to_string());

    // Add entity modification
    layer.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(100),
    });

    // Add entity removal
    layer.add_modification(Modification::EntityRemoval { entity_id: 2 });

    assert_eq!(layer.modifications.len(), 2);
    assert_eq!(layer.frame_number, 5);

    // Test applying modifications to a frame
    let mut frame = Frame {
        frame_number: 5,
        timestamp: Duration::from_secs(5),
        entities: HashMap::new(),
        events: Vec::new(),
        checksum: None,
    };

    // Add entities to test modification
    let mut entity1 = EntityState {
        entity_id: 1,
        components: HashMap::new(),
        active: true,
    };
    entity1
        .components
        .insert("Health".to_string(), serde_json::json!(50));
    frame.entities.insert(1, entity1);

    let entity2 = EntityState {
        entity_id: 2,
        components: HashMap::new(),
        active: true,
    };
    frame.entities.insert(2, entity2);

    // Apply modifications
    layer.apply_to_frame(&mut frame).unwrap();

    // Check results
    assert_eq!(
        frame.entities[&1].components["Health"],
        serde_json::json!(100)
    );
    assert!(!frame.entities[&2].active); // Should be marked inactive
}

#[test]
fn test_timeline_branch_creation_and_modifications() {
    let mut branch = TimelineBranch::new("test_branch".to_string(), None, 0);

    // Test initial state
    assert_eq!(branch.metadata.name, "test_branch");
    assert_eq!(branch.metadata.branch_point_frame, 0);
    assert!(branch.metadata.parent_id.is_none());
    assert_eq!(branch.modification_count(), 0);

    // Add a modification layer
    let mut layer = ModificationLayer::new(3, "Initial mod".to_string());
    layer.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Transform".to_string(),
        new_value: serde_json::json!({"x": 10, "y": 20}),
    });

    branch.add_modification_layer(layer).unwrap();

    assert!(branch.has_modifications_at(3));
    assert!(!branch.has_modifications_at(5));
    assert_eq!(branch.modification_count(), 1);

    // Test modification limit
    for i in 0..1001 {
        let layer = ModificationLayer::new(i + 10, format!("Mod {}", i));
        if i < 999 {
            assert!(branch.add_modification_layer(layer).is_ok());
        } else {
            assert!(branch.add_modification_layer(layer).is_err());
        }
    }
}

#[test]
fn test_timeline_branch_manager() {
    let mut manager = TimelineBranchManager::new();

    // Create test recording
    let recording = create_test_recording();
    manager.set_base_recording(recording);

    // Should have master branch
    assert!(manager.master_branch_id().is_some());
    assert_eq!(manager.list_branches().len(), 1);

    // Create a new branch
    let master_id = manager.master_branch_id().unwrap();
    let branch_id = manager
        .create_branch("feature_branch".to_string(), Some(master_id), 5)
        .unwrap();

    assert_eq!(manager.list_branches().len(), 2);
    assert!(manager.get_branch(branch_id).is_some());

    // Test active branch switching
    manager.set_active_branch(branch_id).unwrap();
    assert_eq!(manager.get_active_branch(), Some(branch_id));

    // Test invalid branch switching
    let invalid_id = BranchId::new();
    assert!(manager.set_active_branch(invalid_id).is_err());
}

#[test]
fn test_branch_hierarchy_and_cycle_detection() {
    let mut manager = TimelineBranchManager::new();
    let recording = create_test_recording();
    manager.set_base_recording(recording);

    let master_id = manager.master_branch_id().unwrap();

    // Create branch hierarchy: master -> branch1 -> branch2
    let branch1_id = manager
        .create_branch("branch1".to_string(), Some(master_id), 0)
        .unwrap();
    let branch2_id = manager
        .create_branch("branch2".to_string(), Some(branch1_id), 0)
        .unwrap();

    // Test deletion with children (should fail)
    assert!(manager.delete_branch(branch1_id).is_err());

    // Delete leaf branch first
    assert!(manager.delete_branch(branch2_id).is_ok());
    assert!(manager.delete_branch(branch1_id).is_ok());

    // Test master branch deletion (should fail)
    assert!(manager.delete_branch(master_id).is_err());
}

#[test]
fn test_frame_retrieval_with_modifications() {
    let mut manager = TimelineBranchManager::new();
    let recording = create_test_recording();
    manager.set_base_recording(recording);

    let master_id = manager.master_branch_id().unwrap();
    let branch_id = manager
        .create_branch("test_branch".to_string(), Some(master_id), 0)
        .unwrap();

    // Add modification to the branch
    let mut layer = ModificationLayer::new(0, "Test mod".to_string());
    layer.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(200),
    });

    let branch = manager.get_branch_mut(branch_id).unwrap();
    branch.add_modification_layer(layer).unwrap();

    // Get frame from branch
    let frame = manager.get_frame_from_branch(branch_id, 0).unwrap();

    // Check that modification was applied
    if let Some(entity) = frame.entities.get(&1) {
        assert_eq!(
            entity.components.get("Health"),
            Some(&serde_json::json!(200))
        );
    }
}

#[test]
fn test_merge_conflict_detection() {
    let mut manager = TimelineBranchManager::new();
    let recording = create_test_recording();
    manager.set_base_recording(recording);

    let master_id = manager.master_branch_id().unwrap();
    let branch_id = manager
        .create_branch("conflict_branch".to_string(), Some(master_id), 0)
        .unwrap();

    // Add conflicting modifications to both branches
    let mut master_layer = ModificationLayer::new(0, "Master mod".to_string());
    master_layer.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(100),
    });

    let mut branch_layer = ModificationLayer::new(0, "Branch mod".to_string());
    branch_layer.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(150),
    });

    let master_branch = manager.get_branch_mut(master_id).unwrap();
    master_branch.add_modification_layer(master_layer).unwrap();

    let branch = manager.get_branch_mut(branch_id).unwrap();
    branch.add_modification_layer(branch_layer).unwrap();

    // Test merge with conflicts
    let result = manager.merge_branch(branch_id, MergeStrategy::TakeSource);
    assert!(result.is_ok());

    // Branch should be deleted after merge
    assert!(manager.get_branch(branch_id).is_none());
}

#[test]
fn test_branch_comparison() {
    let mut manager = TimelineBranchManager::new();
    let recording = create_test_recording();
    manager.set_base_recording(recording);

    let master_id = manager.master_branch_id().unwrap();
    let branch_a = manager
        .create_branch("branch_a".to_string(), Some(master_id), 0)
        .unwrap();
    let branch_b = manager
        .create_branch("branch_b".to_string(), Some(master_id), 0)
        .unwrap();

    // Add different modifications to each branch
    let mut layer_a = ModificationLayer::new(0, "Branch A mod".to_string());
    layer_a.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(100),
    });

    let mut layer_b = ModificationLayer::new(0, "Branch B mod".to_string());
    layer_b.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(150),
    });

    let branch_a_ref = manager.get_branch_mut(branch_a).unwrap();
    branch_a_ref.add_modification_layer(layer_a).unwrap();

    let branch_b_ref = manager.get_branch_mut(branch_b).unwrap();
    branch_b_ref.add_modification_layer(layer_b).unwrap();

    // Compare branches
    let comparison = manager.compare_branches(branch_a, branch_b, 0..1).unwrap();

    assert_eq!(comparison.branch_a, branch_a);
    assert_eq!(comparison.branch_b, branch_b);
    assert!(!comparison.frame_differences.is_empty());
}

#[test]
fn test_branch_tree_visualization() {
    let mut manager = TimelineBranchManager::new();
    let recording = create_test_recording();
    manager.set_base_recording(recording);

    let master_id = manager.master_branch_id().unwrap();

    // Create a tree structure
    let feature_a = manager
        .create_branch("feature_a".to_string(), Some(master_id), 0)
        .unwrap();
    let feature_b = manager
        .create_branch("feature_b".to_string(), Some(master_id), 0)
        .unwrap();
    let hotfix = manager
        .create_branch("hotfix".to_string(), Some(feature_a), 5)
        .unwrap();

    let tree = manager.get_branch_tree();

    // Should have one root (master)
    assert_eq!(tree.roots.len(), 1);

    // Master should have 2 children
    let master_node = &tree.roots[0];
    assert_eq!(master_node.children.len(), 2);

    // Feature A should have 1 child (hotfix)
    let feature_a_node = master_node
        .children
        .iter()
        .find(|child| child.metadata.name == "feature_a")
        .unwrap();
    assert_eq!(feature_a_node.children.len(), 1);
    assert_eq!(feature_a_node.children[0].metadata.name, "hotfix");
}

#[test]
fn test_garbage_collection() {
    let mut manager = TimelineBranchManager::new();
    manager.set_max_branches(3); // Small limit for testing

    let recording = create_test_recording();
    manager.set_base_recording(recording);

    let master_id = manager.master_branch_id().unwrap();

    // Create branches up to the limit
    let _branch1 = manager
        .create_branch("branch1".to_string(), Some(master_id), 0)
        .unwrap();
    let _branch2 = manager
        .create_branch("branch2".to_string(), Some(master_id), 0)
        .unwrap();

    // This should trigger garbage collection
    let _branch3 = manager
        .create_branch("branch3".to_string(), Some(master_id), 0)
        .unwrap();

    // Should still have master branch
    assert!(manager.get_branch(master_id).is_some());
}

#[test]
fn test_modification_types() {
    let mut frame = create_test_frame();
    let mut layer = ModificationLayer::new(0, "All modification types".to_string());

    // Entity modification
    layer.add_modification(Modification::EntityModification {
        entity_id: 1,
        component_name: "Health".to_string(),
        new_value: serde_json::json!(200),
    });

    // Entity removal
    layer.add_modification(Modification::EntityRemoval { entity_id: 2 });

    // Entity addition
    layer.add_modification(Modification::EntityAddition {
        entity: EntityState {
            entity_id: 5,
            components: {
                let mut comps = HashMap::new();
                comps.insert("Name".to_string(), serde_json::json!("New Entity"));
                comps
            },
            active: true,
        },
    });

    // Event injection
    layer.add_modification(Modification::EventInjection {
        event: bevy_debugger_mcp::recording_system::RecordedEvent {
            event_type: "TestEvent".to_string(),
            entity_id: Some(1),
            data: serde_json::json!({"test": true}),
            timestamp: Duration::from_secs(0),
        },
    });

    // Script modification
    layer.add_modification(Modification::ScriptModification {
        script: "console.log('test');".to_string(),
        description: "Test script".to_string(),
    });

    // Apply all modifications
    layer.apply_to_frame(&mut frame).unwrap();

    // Verify results
    assert_eq!(
        frame.entities[&1].components["Health"],
        serde_json::json!(200)
    );
    assert!(!frame.entities[&2].active);
    assert!(frame.entities.contains_key(&5));
    assert_eq!(frame.events.len(), 1);
    assert_eq!(frame.events[0].event_type, "TestEvent");
}

// Helper functions

fn create_test_recording() -> Recording {
    let mut frames = Vec::new();

    for i in 0..10 {
        let mut frame = Frame {
            frame_number: i,
            timestamp: Duration::from_secs(i as u64),
            entities: HashMap::new(),
            events: Vec::new(),
            checksum: None,
        };

        // Add test entities
        for entity_id in 1..=3 {
            let mut entity = EntityState {
                entity_id,
                components: HashMap::new(),
                active: true,
            };
            entity
                .components
                .insert("Health".to_string(), serde_json::json!(100));
            entity.components.insert(
                "Transform".to_string(),
                serde_json::json!({"x": i * 10, "y": entity_id * 5}),
            );
            frame.entities.insert(entity_id, entity);
        }

        frames.push(frame);
    }

    Recording {
        config: RecordingConfig::default(),
        frames,
        delta_frames: Vec::new(),
        markers: Vec::new(),
        total_frames: 10,
        duration: Duration::from_secs(10),
        version: RecordingVersion::current(),
    }
}

fn create_test_frame() -> Frame {
    let mut frame = Frame {
        frame_number: 0,
        timestamp: Duration::from_secs(0),
        entities: HashMap::new(),
        events: Vec::new(),
        checksum: None,
    };

    // Add test entities
    for entity_id in 1..=3 {
        let mut entity = EntityState {
            entity_id,
            components: HashMap::new(),
            active: true,
        };
        entity
            .components
            .insert("Health".to_string(), serde_json::json!(100));
        frame.entities.insert(entity_id, entity);
    }

    frame
}

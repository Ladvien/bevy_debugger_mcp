use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::time::SystemTime;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::recording_system::{EntityState, Frame, RecordedEvent, Recording};

/// A unique identifier for timeline branches
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchId(Uuid);

impl BranchId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_string(s: &str) -> Result<Self> {
        Ok(Self(Uuid::parse_str(s)?))
    }

    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Default for BranchId {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a modification that can be applied during replay
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Modification {
    /// Modify entity component values
    EntityModification {
        entity_id: u64,
        component_name: String,
        new_value: serde_json::Value,
    },
    /// Inject new events
    EventInjection { event: RecordedEvent },
    /// Remove/disable entities
    EntityRemoval { entity_id: u64 },
    /// Add new entities
    EntityAddition { entity: EntityState },
    /// Custom script modification
    ScriptModification { script: String, description: String },
}

/// A layer of modifications applied at a specific frame
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModificationLayer {
    pub frame_number: usize,
    pub modifications: Vec<Modification>,
    pub description: String,
    pub applied_at: SystemTime,
}

impl ModificationLayer {
    pub fn new(frame_number: usize, description: String) -> Self {
        Self {
            frame_number,
            modifications: Vec::new(),
            description,
            applied_at: SystemTime::now(),
        }
    }

    pub fn add_modification(&mut self, modification: Modification) {
        self.modifications.push(modification);
    }

    /// Apply this modification layer to a frame
    pub fn apply_to_frame(&self, frame: &mut Frame) -> Result<()> {
        for modification in &self.modifications {
            self.apply_modification(frame, modification)?;
        }
        Ok(())
    }

    fn apply_modification(&self, frame: &mut Frame, modification: &Modification) -> Result<()> {
        match modification {
            Modification::EntityModification {
                entity_id,
                component_name,
                new_value,
            } => {
                if let Some(entity) = frame.entities.get_mut(entity_id) {
                    entity
                        .components
                        .insert(component_name.clone(), new_value.clone());
                    debug!(
                        "Applied entity modification: {} -> {}",
                        component_name, new_value
                    );
                } else {
                    warn!("Entity {} not found for modification", entity_id);
                }
            }
            Modification::EventInjection { event } => {
                frame.events.push(event.clone());
                debug!("Injected event: {}", event.event_type);
            }
            Modification::EntityRemoval { entity_id } => {
                if let Some(entity) = frame.entities.get_mut(entity_id) {
                    entity.active = false;
                    debug!("Removed entity: {}", entity_id);
                }
            }
            Modification::EntityAddition { entity } => {
                frame.entities.insert(entity.entity_id, entity.clone());
                debug!("Added entity: {}", entity.entity_id);
            }
            Modification::ScriptModification {
                script,
                description,
            } => {
                // For now, log the script - in a real implementation, this would execute custom logic
                info!("Script modification: {} - {}", description, script);
            }
        }
        Ok(())
    }
}

/// Metadata about a timeline branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchMetadata {
    pub id: BranchId,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<BranchId>,
    pub branch_point_frame: usize,
    pub created_at: SystemTime,
    pub last_modified: SystemTime,
    pub tags: Vec<String>,
}

impl BranchMetadata {
    pub fn new(name: String, parent_id: Option<BranchId>, branch_point_frame: usize) -> Self {
        let now = SystemTime::now();
        Self {
            id: BranchId::new(),
            name,
            description: None,
            parent_id,
            branch_point_frame,
            created_at: now,
            last_modified: now,
            tags: Vec::new(),
        }
    }
}

/// A timeline branch with copy-on-write semantics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineBranch {
    pub metadata: BranchMetadata,
    /// Modifications applied in this branch, indexed by frame number
    modification_layers: BTreeMap<usize, ModificationLayer>,
    /// Frames that have been modified in this branch (copy-on-write)
    modified_frames: HashMap<usize, Frame>,
    /// Reference to the base recording
    base_recording_id: Option<String>,
}

impl TimelineBranch {
    pub fn new(name: String, parent_id: Option<BranchId>, branch_point_frame: usize) -> Self {
        Self {
            metadata: BranchMetadata::new(name, parent_id, branch_point_frame),
            modification_layers: BTreeMap::new(),
            modified_frames: HashMap::new(),
            base_recording_id: None,
        }
    }

    /// Add a modification layer at a specific frame
    pub fn add_modification_layer(&mut self, layer: ModificationLayer) -> Result<()> {
        // Check modification limit
        if self.modification_layers.len() >= 1000 {
            // Max modifications per branch
            return Err(Error::Validation(
                "Too many modifications in branch".to_string(),
            ));
        }

        let frame_number = layer.frame_number;
        self.modification_layers.insert(frame_number, layer);
        self.metadata.last_modified = SystemTime::now();

        // Invalidate cached frames from this point forward
        self.modified_frames
            .retain(|&frame_num, _| frame_num < frame_number);

        Ok(())
    }

    /// Get a frame with all modifications applied up to that point
    pub fn get_frame(&mut self, base_frame: &Frame, frame_number: usize) -> Result<Frame> {
        // Check if we already have this frame cached
        if let Some(cached_frame) = self.modified_frames.get(&frame_number) {
            return Ok(cached_frame.clone());
        }

        // Start with the base frame
        let mut result_frame = base_frame.clone();

        // Apply all modification layers up to and including this frame
        for (layer_frame, layer) in self.modification_layers.range(..=frame_number) {
            if *layer_frame <= frame_number {
                layer.apply_to_frame(&mut result_frame)?;
            }
        }

        // Cache the result (copy-on-write)
        self.modified_frames
            .insert(frame_number, result_frame.clone());

        Ok(result_frame)
    }

    /// Get all modifications for visualization
    pub fn get_modifications(&self) -> &BTreeMap<usize, ModificationLayer> {
        &self.modification_layers
    }

    /// Check if this branch has modifications at a specific frame
    pub fn has_modifications_at(&self, frame_number: usize) -> bool {
        self.modification_layers.contains_key(&frame_number)
    }

    /// Get the total number of modifications in this branch
    pub fn modification_count(&self) -> usize {
        self.modification_layers
            .values()
            .map(|layer| layer.modifications.len())
            .sum()
    }
}

/// Manages multiple timeline branches with garbage collection
#[derive(Debug)]
pub struct TimelineBranchManager {
    /// All branches indexed by their ID
    branches: HashMap<BranchId, TimelineBranch>,
    /// The main/master branch ID
    master_branch_id: Option<BranchId>,
    /// Base recording for all branches
    base_recording: Option<Recording>,
    /// Reference counting for garbage collection
    branch_refs: HashMap<BranchId, usize>,
    /// Maximum number of branches to keep
    max_branches: usize,
    /// Maximum number of modification layers per branch
    max_modifications_per_branch: usize,
    /// Currently active branch for playback
    active_branch_id: Option<BranchId>,
}

impl TimelineBranchManager {
    pub fn new() -> Self {
        Self {
            branches: HashMap::new(),
            master_branch_id: None,
            base_recording: None,
            branch_refs: HashMap::new(),
            max_branches: 100,
            max_modifications_per_branch: 1000,
            active_branch_id: None,
        }
    }

    /// Get the master branch ID
    pub fn master_branch_id(&self) -> Option<BranchId> {
        self.master_branch_id
    }

    /// Get the maximum number of branches
    pub fn max_branches(&self) -> usize {
        self.max_branches
    }

    /// Set the maximum number of branches (for testing)
    pub fn set_max_branches(&mut self, max_branches: usize) {
        self.max_branches = max_branches;
    }

    /// Set the base recording for all branches
    pub fn set_base_recording(&mut self, recording: Recording) {
        self.base_recording = Some(recording);

        // Create master branch if none exists
        if self.master_branch_id.is_none() {
            let master_branch = TimelineBranch::new("master".to_string(), None, 0);
            let master_id = master_branch.metadata.id;
            self.branches.insert(master_id, master_branch);
            self.master_branch_id = Some(master_id);
            self.branch_refs.insert(master_id, 1);
        }
    }

    /// Create a new branch from a parent at a specific frame
    pub fn create_branch(
        &mut self,
        name: String,
        parent_id: Option<BranchId>,
        branch_point_frame: usize,
    ) -> Result<BranchId> {
        // Validate parent exists if specified
        if let Some(parent) = parent_id {
            if !self.branches.contains_key(&parent) {
                return Err(Error::Validation(format!(
                    "Parent branch {} not found",
                    parent.to_string()
                )));
            }

            // Check for circular dependencies - we don't need this check here
            // since we're creating a new branch, not modifying an existing one
        }

        // Check branch limit
        if self.branches.len() >= self.max_branches {
            self.garbage_collect();
        }

        let branch = TimelineBranch::new(name, parent_id, branch_point_frame);
        let branch_id = branch.metadata.id;

        self.branches.insert(branch_id, branch);
        self.branch_refs.insert(branch_id, 1);

        info!(
            "Created branch {} at frame {}",
            branch_id.to_string(),
            branch_point_frame
        );
        Ok(branch_id)
    }

    /// Get a mutable reference to a branch
    pub fn get_branch_mut(&mut self, branch_id: BranchId) -> Option<&mut TimelineBranch> {
        self.branches.get_mut(&branch_id)
    }

    /// Get a branch
    pub fn get_branch(&self, branch_id: BranchId) -> Option<&TimelineBranch> {
        self.branches.get(&branch_id)
    }

    /// Get a frame from a specific branch
    pub fn get_frame_from_branch(
        &mut self,
        branch_id: BranchId,
        frame_number: usize,
    ) -> Result<Frame> {
        let base_recording = self
            .base_recording
            .as_ref()
            .ok_or_else(|| Error::Validation("No base recording loaded".to_string()))?;

        // Get the base frame from the recording
        let base_frame = base_recording
            .frames
            .iter()
            .find(|f| f.frame_number == frame_number)
            .ok_or_else(|| {
                Error::Validation(format!("Frame {frame_number} not found in base recording"))
            })?;

        // Apply branch modifications
        let branch = self.branches.get_mut(&branch_id).ok_or_else(|| {
            Error::Validation(format!("Branch {} not found", branch_id.to_string()))
        })?;

        branch.get_frame(base_frame, frame_number)
    }

    /// Set the active branch for playback
    pub fn set_active_branch(&mut self, branch_id: BranchId) -> Result<()> {
        if !self.branches.contains_key(&branch_id) {
            return Err(Error::Validation(format!(
                "Branch {} not found",
                branch_id.to_string()
            )));
        }

        self.active_branch_id = Some(branch_id);
        info!("Set active branch to {}", branch_id.to_string());
        Ok(())
    }

    /// Get the currently active branch
    pub fn get_active_branch(&self) -> Option<BranchId> {
        self.active_branch_id
    }

    /// Check if creating a branch would create a circular dependency
    fn would_create_cycle(
        &self,
        potential_parent: BranchId,
        new_parent: Option<BranchId>,
    ) -> Result<bool> {
        if let Some(new_parent) = new_parent {
            let mut visited = std::collections::HashSet::new();
            let mut current = Some(potential_parent);

            while let Some(branch_id) = current {
                if visited.contains(&branch_id) {
                    return Ok(true); // Cycle detected
                }
                visited.insert(branch_id);

                if branch_id == new_parent {
                    return Ok(true); // Would create cycle
                }

                current = self
                    .branches
                    .get(&branch_id)
                    .and_then(|b| b.metadata.parent_id);
            }
        }

        Ok(false)
    }

    /// List all branches
    pub fn list_branches(&self) -> Vec<&BranchMetadata> {
        self.branches.values().map(|b| &b.metadata).collect()
    }

    /// Delete a branch
    pub fn delete_branch(&mut self, branch_id: BranchId) -> Result<()> {
        if Some(branch_id) == self.master_branch_id {
            return Err(Error::Validation("Cannot delete master branch".to_string()));
        }

        // Check if branch has children
        let has_children = self
            .branches
            .values()
            .any(|b| b.metadata.parent_id == Some(branch_id));

        if has_children {
            return Err(Error::Validation(
                "Cannot delete branch with children".to_string(),
            ));
        }

        self.branches.remove(&branch_id);
        self.branch_refs.remove(&branch_id);

        info!("Deleted branch {}", branch_id.to_string());
        Ok(())
    }

    /// Get branch tree visualization
    pub fn get_branch_tree(&self) -> BranchTree {
        let mut tree = BranchTree::new();

        // Find root branches (no parent)
        for branch in self.branches.values() {
            if branch.metadata.parent_id.is_none() {
                tree.add_branch(branch, &self.branches);
            }
        }

        tree
    }

    /// Merge a branch back into its parent
    pub fn merge_branch(
        &mut self,
        branch_id: BranchId,
        strategy: MergeStrategy,
    ) -> Result<Vec<ConflictResolution>> {
        let branch = self.branches.get(&branch_id).ok_or_else(|| {
            Error::Validation(format!("Branch {} not found", branch_id.to_string()))
        })?;

        let parent_id = branch
            .metadata
            .parent_id
            .ok_or_else(|| Error::Validation("Branch has no parent to merge into".to_string()))?;

        let conflicts = self.detect_merge_conflicts(branch_id, parent_id)?;
        let resolutions = self.resolve_conflicts(conflicts, strategy)?;

        // Apply merge
        let branch_modifications = self.branches[&branch_id].modification_layers.clone();
        let parent_branch = self
            .branches
            .get_mut(&parent_id)
            .ok_or_else(|| Error::Validation("Parent branch not found".to_string()))?;

        for (frame_number, layer) in branch_modifications {
            parent_branch
                .modification_layers
                .insert(frame_number, layer);
        }

        // Delete the merged branch
        self.delete_branch(branch_id)?;

        info!(
            "Merged branch {} into {}",
            branch_id.to_string(),
            parent_id.to_string()
        );
        Ok(resolutions)
    }

    /// Compare outcomes between two branches
    pub fn compare_branches(
        &mut self,
        branch_a: BranchId,
        branch_b: BranchId,
        frame_range: std::ops::Range<usize>,
    ) -> Result<BranchComparison> {
        let mut differences = Vec::new();

        for frame_number in frame_range {
            let frame_a = self.get_frame_from_branch(branch_a, frame_number)?;
            let frame_b = self.get_frame_from_branch(branch_b, frame_number)?;

            let diff = self.compare_frames(&frame_a, &frame_b, frame_number);
            if !diff.is_empty() {
                differences.push(FrameDifference {
                    frame_number,
                    differences: diff,
                });
            }
        }

        Ok(BranchComparison {
            branch_a,
            branch_b,
            frame_differences: differences,
        })
    }

    /// Garbage collect unused branches
    fn garbage_collect(&mut self) {
        let mut to_remove = Vec::new();

        for (branch_id, ref_count) in &self.branch_refs {
            if *ref_count == 0 && Some(*branch_id) != self.master_branch_id {
                to_remove.push(*branch_id);
            }
        }

        for branch_id in to_remove {
            self.branches.remove(&branch_id);
            self.branch_refs.remove(&branch_id);
            warn!("Garbage collected branch {}", branch_id.to_string());
        }
    }

    fn detect_merge_conflicts(
        &self,
        source_branch: BranchId,
        target_branch: BranchId,
    ) -> Result<Vec<MergeConflict>> {
        let source = &self.branches[&source_branch];
        let target = &self.branches[&target_branch];

        let mut conflicts = Vec::new();

        // Check for overlapping modifications
        for (frame_number, source_layer) in &source.modification_layers {
            if let Some(target_layer) = target.modification_layers.get(frame_number) {
                // Check for conflicting modifications on the same entities/components
                for source_mod in &source_layer.modifications {
                    for target_mod in &target_layer.modifications {
                        if self.modifications_conflict(source_mod, target_mod) {
                            conflicts.push(MergeConflict {
                                frame_number: *frame_number,
                                source_modification: source_mod.clone(),
                                target_modification: target_mod.clone(),
                            });
                        }
                    }
                }
            }
        }

        Ok(conflicts)
    }

    fn modifications_conflict(&self, mod_a: &Modification, mod_b: &Modification) -> bool {
        match (mod_a, mod_b) {
            (
                Modification::EntityModification {
                    entity_id: id_a,
                    component_name: comp_a,
                    ..
                },
                Modification::EntityModification {
                    entity_id: id_b,
                    component_name: comp_b,
                    ..
                },
            ) => id_a == id_b && comp_a == comp_b,
            (
                Modification::EntityRemoval { entity_id: id_a },
                Modification::EntityModification {
                    entity_id: id_b, ..
                },
            ) => id_a == id_b,
            (
                Modification::EntityModification {
                    entity_id: id_a, ..
                },
                Modification::EntityRemoval { entity_id: id_b },
            ) => id_a == id_b,
            _ => false,
        }
    }

    fn resolve_conflicts(
        &self,
        conflicts: Vec<MergeConflict>,
        strategy: MergeStrategy,
    ) -> Result<Vec<ConflictResolution>> {
        let mut resolutions = Vec::new();

        for conflict in conflicts {
            let resolution = match strategy {
                MergeStrategy::TakeSource => ConflictResolution {
                    conflict: conflict.clone(),
                    chosen_modification: conflict.source_modification,
                },
                MergeStrategy::TakeTarget => ConflictResolution {
                    conflict: conflict.clone(),
                    chosen_modification: conflict.target_modification,
                },
                MergeStrategy::Manual => {
                    // For now, default to source - in a real implementation, this would prompt the user
                    ConflictResolution {
                        conflict: conflict.clone(),
                        chosen_modification: conflict.source_modification,
                    }
                }
            };
            resolutions.push(resolution);
        }

        Ok(resolutions)
    }

    fn compare_frames(
        &self,
        frame_a: &Frame,
        frame_b: &Frame,
        frame_number: usize,
    ) -> Vec<EntityDifference> {
        let mut differences = Vec::new();

        // Compare entities
        for (entity_id, entity_a) in &frame_a.entities {
            if let Some(entity_b) = frame_b.entities.get(entity_id) {
                if entity_a != entity_b {
                    differences.push(EntityDifference {
                        entity_id: *entity_id,
                        difference_type: DifferenceType::Modified,
                        details: format!("Entity {entity_id} modified"),
                    });
                }
            } else {
                differences.push(EntityDifference {
                    entity_id: *entity_id,
                    difference_type: DifferenceType::OnlyInA,
                    details: format!("Entity {entity_id} only in branch A"),
                });
            }
        }

        // Check for entities only in B
        for entity_id in frame_b.entities.keys() {
            if !frame_a.entities.contains_key(entity_id) {
                differences.push(EntityDifference {
                    entity_id: *entity_id,
                    difference_type: DifferenceType::OnlyInB,
                    details: format!("Entity {entity_id} only in branch B"),
                });
            }
        }

        differences
    }
}

impl Default for TimelineBranchManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Strategies for merging branches
#[derive(Debug, Clone, Copy)]
pub enum MergeStrategy {
    /// Take modifications from the source branch
    TakeSource,
    /// Take modifications from the target branch
    TakeTarget,
    /// Manual resolution required
    Manual,
}

/// Represents a conflict during branch merging
#[derive(Debug, Clone)]
pub struct MergeConflict {
    pub frame_number: usize,
    pub source_modification: Modification,
    pub target_modification: Modification,
}

/// Resolution of a merge conflict
#[derive(Debug, Clone)]
pub struct ConflictResolution {
    pub conflict: MergeConflict,
    pub chosen_modification: Modification,
}

/// Visualization of branch relationships
#[derive(Debug, Clone)]
pub struct BranchTree {
    pub roots: Vec<BranchNode>,
}

#[derive(Debug, Clone)]
pub struct BranchNode {
    pub metadata: BranchMetadata,
    pub children: Vec<BranchNode>,
}

impl Default for BranchTree {
    fn default() -> Self {
        Self::new()
    }
}

impl BranchTree {
    pub fn new() -> Self {
        Self { roots: Vec::new() }
    }

    fn add_branch(
        &mut self,
        branch: &TimelineBranch,
        all_branches: &HashMap<BranchId, TimelineBranch>,
    ) {
        let node = BranchNode {
            metadata: branch.metadata.clone(),
            children: self.find_children(branch.metadata.id, all_branches),
        };
        self.roots.push(node);
    }

    fn find_children(
        &self,
        parent_id: BranchId,
        all_branches: &HashMap<BranchId, TimelineBranch>,
    ) -> Vec<BranchNode> {
        all_branches
            .values()
            .filter(|b| b.metadata.parent_id == Some(parent_id))
            .map(|b| BranchNode {
                metadata: b.metadata.clone(),
                children: self.find_children(b.metadata.id, all_branches),
            })
            .collect()
    }
}

/// Comparison between two branches
#[derive(Debug, Clone)]
pub struct BranchComparison {
    pub branch_a: BranchId,
    pub branch_b: BranchId,
    pub frame_differences: Vec<FrameDifference>,
}

#[derive(Debug, Clone)]
pub struct FrameDifference {
    pub frame_number: usize,
    pub differences: Vec<EntityDifference>,
}

#[derive(Debug, Clone)]
pub struct EntityDifference {
    pub entity_id: u64,
    pub difference_type: DifferenceType,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DifferenceType {
    Modified,
    OnlyInA,
    OnlyInB,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording_system::RecordingConfig;

    #[test]
    fn test_branch_id_creation() {
        let id1 = BranchId::new();
        let id2 = BranchId::new();
        assert_ne!(id1, id2);

        let id_str = id1.to_string();
        let parsed_id = BranchId::from_string(&id_str).unwrap();
        assert_eq!(id1, parsed_id);
    }

    #[test]
    fn test_modification_layer() {
        let mut layer = ModificationLayer::new(0, "Test modifications".to_string());

        layer.add_modification(Modification::EntityModification {
            entity_id: 1,
            component_name: "Transform".to_string(),
            new_value: serde_json::json!({"x": 10, "y": 20}),
        });

        assert_eq!(layer.modifications.len(), 1);
    }

    #[test]
    fn test_timeline_branch() {
        let mut branch = TimelineBranch::new("test_branch".to_string(), None, 0);

        let mut layer = ModificationLayer::new(0, "Initial modifications".to_string());
        layer.add_modification(Modification::EntityModification {
            entity_id: 1,
            component_name: "Health".to_string(),
            new_value: serde_json::json!(100),
        });

        branch.add_modification_layer(layer);

        assert!(branch.has_modifications_at(0));
        assert!(!branch.has_modifications_at(1));
        assert_eq!(branch.modification_count(), 1);
    }

    #[test]
    fn test_branch_manager() {
        let mut manager = TimelineBranchManager::new();

        // Create a test recording
        let recording = Recording {
            config: RecordingConfig::default(),
            frames: vec![Frame {
                frame_number: 0,
                timestamp: chrono::Duration::seconds(0),
                entities: std::collections::HashMap::new(),
                events: Vec::new(),
                checksum: None,
            }],
            delta_frames: Vec::new(),
            markers: Vec::new(),
            total_frames: 1,
            duration: std::time::Duration::from_secs(1),
            version: crate::playback_system::RecordingVersion::current(),
        };

        manager.set_base_recording(recording);

        // Create a branch
        let branch_id = manager
            .create_branch("test_branch".to_string(), manager.master_branch_id, 0)
            .unwrap();

        assert!(manager.get_branch(branch_id).is_some());
        assert_eq!(manager.list_branches().len(), 2); // master + test_branch
    }

    #[test]
    fn test_merge_conflict_detection() {
        let mod1 = Modification::EntityModification {
            entity_id: 1,
            component_name: "Health".to_string(),
            new_value: serde_json::json!(100),
        };

        let mod2 = Modification::EntityModification {
            entity_id: 1,
            component_name: "Health".to_string(),
            new_value: serde_json::json!(50),
        };

        let mod3 = Modification::EntityModification {
            entity_id: 2,
            component_name: "Health".to_string(),
            new_value: serde_json::json!(75),
        };

        let manager = TimelineBranchManager::new();

        assert!(manager.modifications_conflict(&mod1, &mod2));
        assert!(!manager.modifications_conflict(&mod1, &mod3));
    }
}

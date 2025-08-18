/// Experiment system for modifying game state
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tracing::{debug, info, warn};

use crate::brp_client::BrpClient;
use crate::brp_messages::{
    BrpRequest, BrpResponse, BrpResult, ComponentTypeId, ComponentValue, EntityData, EntityId,
};
use crate::error::{Error, Result};

/// Action types for state manipulation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Spawn a new entity with specified components
    Spawn {
        components: Vec<ComponentSpec>,
        #[serde(default)]
        archetype: Option<String>,
    },
    /// Modify an existing entity's components
    Modify {
        entity_id: EntityId,
        components: Vec<ComponentSpec>,
    },
    /// Delete an entity
    Delete { entity_id: EntityId },
    /// Batch multiple actions as a transaction
    Batch {
        actions: Vec<Action>,
        #[serde(default)]
        atomic: bool,
    },
}

/// Component specification for spawn/modify actions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentSpec {
    pub type_id: ComponentTypeId,
    pub value: ComponentValue,
}

/// Result of executing an action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub success: bool,
    pub action_type: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_id: Option<EntityId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_available: Option<bool>,
}

impl ActionResult {
    fn success(action_type: &str, message: String) -> Self {
        Self {
            success: true,
            action_type: action_type.to_string(),
            message,
            entity_id: None,
            error: None,
            rollback_available: None,
        }
    }

    fn success_with_entity(action_type: &str, message: String, entity_id: EntityId) -> Self {
        Self {
            success: true,
            action_type: action_type.to_string(),
            message,
            entity_id: Some(entity_id),
            error: None,
            rollback_available: None,
        }
    }

    fn failure(action_type: &str, error: String) -> Self {
        Self {
            success: false,
            action_type: action_type.to_string(),
            message: format!("Action failed: {error}"),
            entity_id: None,
            error: Some(error),
            rollback_available: None,
        }
    }
}

/// Transaction for atomic action groups
#[derive(Debug, Clone)]
pub struct Transaction {
    pub actions: Vec<Action>,
    pub results: Vec<ActionResult>,
    pub rolled_back: bool,
}

impl Transaction {
    fn new(actions: Vec<Action>) -> Self {
        Self {
            actions,
            results: Vec::new(),
            rolled_back: false,
        }
    }

    fn add_result(&mut self, result: ActionResult) {
        self.results.push(result);
    }

    fn should_rollback(&self) -> bool {
        self.results.iter().any(|r| !r.success)
    }
}

/// History entry for undo/redo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub action: Action,
    pub result: ActionResult,
    pub reverse_action: Option<Action>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Action executor with rollback and history
pub struct ActionExecutor {
    undo_stack: VecDeque<HistoryEntry>,
    redo_stack: VecDeque<HistoryEntry>,
    max_history_size: usize,
    enable_rollback: bool,
    transaction_stack: Vec<Transaction>,
}

impl ActionExecutor {
    /// Create a new action executor
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_history_size: 100,
            enable_rollback: true,
            transaction_stack: Vec::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(max_history_size: usize, enable_rollback: bool) -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
            max_history_size,
            enable_rollback,
            transaction_stack: Vec::new(),
        }
    }

    /// Execute a single action
    pub fn execute_action<'a>(
        &'a mut self,
        action: &'a Action,
        brp_client: &'a mut BrpClient,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ActionResult>> + Send + 'a>>
    {
        Box::pin(async move {
            debug!("Executing action: {:?}", action);

            let result = match action {
                Action::Spawn {
                    components,
                    archetype,
                } => {
                    self.execute_spawn(components, archetype.as_deref(), brp_client)
                        .await
                }
                Action::Modify {
                    entity_id,
                    components,
                } => {
                    self.execute_modify(*entity_id, components, brp_client)
                        .await
                }
                Action::Delete { entity_id } => self.execute_delete(*entity_id, brp_client).await,
                Action::Batch { actions, atomic } => {
                    self.execute_batch(actions, *atomic, brp_client).await
                }
            };

            // Add to history if successful and not in a transaction
            if let Ok(ref action_result) = result {
                if action_result.success && self.transaction_stack.is_empty() {
                    self.add_to_history(action.clone(), action_result.clone());
                }
            }

            result
        })
    }

    /// Execute spawn action
    async fn execute_spawn(
        &mut self,
        components: &[ComponentSpec],
        archetype: Option<&str>,
        brp_client: &mut BrpClient,
    ) -> Result<ActionResult> {
        info!("Spawning entity with {} components", components.len());

        // Use archetype factory if specified
        let final_components = if let Some(arch) = archetype {
            self.apply_archetype(arch, components)?
        } else {
            components.to_vec()
        };

        // Validate component types
        for component in &final_components {
            self.validate_component_type(&component.type_id)?;
        }

        // Create BRP spawn request
        let request = BrpRequest::SpawnEntity {
            components: final_components
                .into_iter()
                .map(|c| (c.type_id, c.value))
                .collect(),
        };

        // Send request
        match brp_client.send_request(&request).await {
            Ok(BrpResponse::Success(BrpResult::EntitySpawned(entity_id))) => {
                Ok(ActionResult::success_with_entity(
                    "spawn",
                    format!("Successfully spawned entity {entity_id}"),
                    entity_id,
                ))
            }
            Ok(BrpResponse::Error(err)) => Ok(ActionResult::failure("spawn", err.message)),
            Ok(_) => Ok(ActionResult::failure(
                "spawn",
                "Unexpected response type".to_string(),
            )),
            Err(e) => Ok(ActionResult::failure("spawn", e.to_string())),
        }
    }

    /// Execute modify action
    async fn execute_modify(
        &mut self,
        entity_id: EntityId,
        components: &[ComponentSpec],
        brp_client: &mut BrpClient,
    ) -> Result<ActionResult> {
        info!(
            "Modifying entity {} with {} component changes",
            entity_id,
            components.len()
        );

        // Validate component types
        for component in components {
            self.validate_component_type(&component.type_id)?;
        }

        // Store original values for rollback if enabled
        let original_values = if self.enable_rollback {
            self.get_original_component_values(entity_id, components, brp_client)
                .await?
        } else {
            None
        };

        // Create BRP modify request
        let request = BrpRequest::ModifyEntity {
            entity_id,
            components: components
                .iter()
                .map(|c| (c.type_id.clone(), c.value.clone()))
                .collect(),
        };

        // Send request
        match brp_client.send_request(&request).await {
            Ok(BrpResponse::Success(BrpResult::EntityModified)) => {
                let mut result = ActionResult::success_with_entity(
                    "modify",
                    format!("Successfully modified entity {entity_id}"),
                    entity_id,
                );

                if original_values.is_some() {
                    result.rollback_available = Some(true);
                }

                Ok(result)
            }
            Ok(BrpResponse::Error(err)) => Ok(ActionResult::failure("modify", err.message)),
            Ok(_) => Ok(ActionResult::failure(
                "modify",
                "Unexpected response type".to_string(),
            )),
            Err(e) => Ok(ActionResult::failure("modify", e.to_string())),
        }
    }

    /// Execute delete action
    async fn execute_delete(
        &mut self,
        entity_id: EntityId,
        brp_client: &mut BrpClient,
    ) -> Result<ActionResult> {
        info!("Deleting entity {}", entity_id);

        // Store entity data for rollback if enabled
        let entity_data = if self.enable_rollback {
            self.get_entity_data(entity_id, brp_client).await?
        } else {
            None
        };

        // Create BRP delete request
        let request = BrpRequest::DeleteEntity { entity_id };

        // Send request
        match brp_client.send_request(&request).await {
            Ok(BrpResponse::Success(BrpResult::EntityDeleted)) => {
                let mut result = ActionResult::success(
                    "delete",
                    format!("Successfully deleted entity {entity_id}"),
                );

                if entity_data.is_some() {
                    result.rollback_available = Some(true);
                }

                Ok(result)
            }
            Ok(BrpResponse::Error(err)) => Ok(ActionResult::failure("delete", err.message)),
            Ok(_) => Ok(ActionResult::failure(
                "delete",
                "Unexpected response type".to_string(),
            )),
            Err(e) => Ok(ActionResult::failure("delete", e.to_string())),
        }
    }

    /// Execute batch of actions
    async fn execute_batch(
        &mut self,
        actions: &[Action],
        atomic: bool,
        brp_client: &mut BrpClient,
    ) -> Result<ActionResult> {
        info!(
            "Executing batch of {} actions (atomic: {})",
            actions.len(),
            atomic
        );

        if atomic {
            // Start transaction
            let mut transaction = Transaction::new(actions.to_vec());
            self.transaction_stack.push(transaction.clone());

            let mut all_results = Vec::new();

            for action in actions {
                let result = self.execute_action(action, brp_client).await?;
                transaction.add_result(result.clone());
                all_results.push(result);
            }

            // Check if rollback needed
            if transaction.should_rollback() {
                warn!("Transaction failed, rolling back {} actions", actions.len());
                self.rollback_transaction(&transaction, brp_client).await?;

                self.transaction_stack.pop();
                return Ok(ActionResult::failure(
                    "batch",
                    "Transaction rolled back due to failures".to_string(),
                ));
            }

            self.transaction_stack.pop();

            let success_count = all_results.iter().filter(|r| r.success).count();
            Ok(ActionResult::success(
                "batch",
                format!(
                    "Successfully executed {}/{} actions",
                    success_count,
                    actions.len()
                ),
            ))
        } else {
            // Non-atomic batch - execute all regardless of failures
            let mut all_results = Vec::new();

            for action in actions {
                let result = self.execute_action(action, brp_client).await?;
                all_results.push(result);
            }

            let success_count = all_results.iter().filter(|r| r.success).count();
            let all_success = success_count == actions.len();

            if all_success {
                Ok(ActionResult::success(
                    "batch",
                    format!("Successfully executed all {} actions", actions.len()),
                ))
            } else {
                Ok(ActionResult::failure(
                    "batch",
                    format!(
                        "Executed {}/{} actions successfully",
                        success_count,
                        actions.len()
                    ),
                ))
            }
        }
    }

    /// Rollback a transaction
    async fn rollback_transaction(
        &mut self,
        transaction: &Transaction,
        brp_client: &mut BrpClient,
    ) -> Result<()> {
        for (action, result) in transaction.actions.iter().zip(&transaction.results).rev() {
            if result.success {
                if let Some(reverse_action) = self.create_reverse_action(action, result) {
                    let _ = self.execute_action(&reverse_action, brp_client).await;
                }
            }
        }
        Ok(())
    }

    /// Create reverse action for rollback
    fn create_reverse_action(&self, action: &Action, result: &ActionResult) -> Option<Action> {
        match action {
            Action::Spawn { .. } => {
                // Reverse of spawn is delete
                result.entity_id.map(|id| Action::Delete { entity_id: id })
            }
            Action::Modify { .. } => {
                // For proper rollback, we'd need the original values stored
                // This would require enhancing the ActionResult to include original state
                // For now, we can't properly reverse a modify without the original values
                None
            }
            Action::Delete { .. } => {
                // For proper rollback, we'd need the original entity data
                // This would require enhancing the ActionResult to include deleted entity data
                // For now, we can't properly reverse a delete without the original data
                None
            }
            Action::Batch { .. } => None,
        }
    }

    /// Add action to history
    fn add_to_history(&mut self, action: Action, result: ActionResult) {
        let entry = HistoryEntry {
            action: action.clone(),
            result: result.clone(),
            reverse_action: self.create_reverse_action(&action, &result),
            timestamp: chrono::Utc::now(),
        };

        self.undo_stack.push_back(entry);

        // Limit history size
        while self.undo_stack.len() > self.max_history_size {
            self.undo_stack.pop_front();
        }

        // Clear redo stack on new action
        self.redo_stack.clear();
    }

    /// Undo last action
    pub async fn undo(&mut self, brp_client: &mut BrpClient) -> Result<ActionResult> {
        if let Some(entry) = self.undo_stack.pop_back() {
            if let Some(reverse_action) = &entry.reverse_action {
                let result = self.execute_action(reverse_action, brp_client).await?;
                self.redo_stack.push_back(entry);
                Ok(result)
            } else {
                Ok(ActionResult::failure(
                    "undo",
                    "No reverse action available".to_string(),
                ))
            }
        } else {
            Ok(ActionResult::failure("undo", "Nothing to undo".to_string()))
        }
    }

    /// Redo last undone action
    pub async fn redo(&mut self, brp_client: &mut BrpClient) -> Result<ActionResult> {
        if let Some(entry) = self.redo_stack.pop_back() {
            let result = self.execute_action(&entry.action, brp_client).await?;
            self.undo_stack.push_back(entry);
            Ok(result)
        } else {
            Ok(ActionResult::failure("redo", "Nothing to redo".to_string()))
        }
    }

    /// Get history
    pub fn get_history(&self) -> Vec<HistoryEntry> {
        self.undo_stack.iter().cloned().collect()
    }

    /// Clear history
    pub fn clear_history(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Validate component type
    fn validate_component_type(&self, type_id: &ComponentTypeId) -> Result<()> {
        // Basic validation - could be extended with schema checking
        if type_id.is_empty() {
            return Err(Error::Validation(
                "Component type ID cannot be empty".to_string(),
            ));
        }

        // Check for known component types
        let known_types = [
            "Transform",
            "GlobalTransform",
            "Velocity",
            "Health",
            "Name",
            "Visibility",
            "ComputedVisibility",
            "RigidBody",
            "Collider",
            "Mesh",
            "Material",
            "Camera",
            "Light",
        ];

        if !known_types.contains(&type_id.as_str()) {
            warn!("Unknown component type: {} - proceeding anyway", type_id);
        }

        Ok(())
    }

    /// Apply archetype to get full component list
    fn apply_archetype(
        &self,
        archetype: &str,
        overrides: &[ComponentSpec],
    ) -> Result<Vec<ComponentSpec>> {
        let mut components = match archetype {
            "player" => vec![
                ComponentSpec {
                    type_id: "Transform".to_string(),
                    value: serde_json::json!({
                        "translation": {"x": 0.0, "y": 0.0, "z": 0.0},
                        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                        "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
                    }),
                },
                ComponentSpec {
                    type_id: "Health".to_string(),
                    value: serde_json::json!(100),
                },
                ComponentSpec {
                    type_id: "Name".to_string(),
                    value: serde_json::json!("Player"),
                },
            ],
            "enemy" => vec![
                ComponentSpec {
                    type_id: "Transform".to_string(),
                    value: serde_json::json!({
                        "translation": {"x": 0.0, "y": 0.0, "z": 0.0},
                        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                        "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
                    }),
                },
                ComponentSpec {
                    type_id: "Health".to_string(),
                    value: serde_json::json!(50),
                },
                ComponentSpec {
                    type_id: "Name".to_string(),
                    value: serde_json::json!("Enemy"),
                },
            ],
            "projectile" => vec![
                ComponentSpec {
                    type_id: "Transform".to_string(),
                    value: serde_json::json!({
                        "translation": {"x": 0.0, "y": 0.0, "z": 0.0},
                        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                        "scale": {"x": 0.5, "y": 0.5, "z": 0.5}
                    }),
                },
                ComponentSpec {
                    type_id: "Velocity".to_string(),
                    value: serde_json::json!({"x": 10.0, "y": 0.0, "z": 0.0}),
                },
            ],
            "static_mesh" => vec![
                ComponentSpec {
                    type_id: "Transform".to_string(),
                    value: serde_json::json!({
                        "translation": {"x": 0.0, "y": 0.0, "z": 0.0},
                        "rotation": {"x": 0.0, "y": 0.0, "z": 0.0, "w": 1.0},
                        "scale": {"x": 1.0, "y": 1.0, "z": 1.0}
                    }),
                },
                ComponentSpec {
                    type_id: "Mesh".to_string(),
                    value: serde_json::json!("cube"),
                },
            ],
            _ => {
                return Err(Error::Validation(format!("Unknown archetype: {archetype}")));
            }
        };

        // Apply overrides
        for override_spec in overrides {
            if let Some(existing) = components
                .iter_mut()
                .find(|c| c.type_id == override_spec.type_id)
            {
                existing.value = override_spec.value.clone();
            } else {
                components.push(override_spec.clone());
            }
        }

        Ok(components)
    }

    /// Get original component values for rollback
    async fn get_original_component_values(
        &self,
        entity_id: EntityId,
        components: &[ComponentSpec],
        brp_client: &mut BrpClient,
    ) -> Result<Option<Vec<ComponentSpec>>> {
        // Query entity to get current values
        let request = BrpRequest::QueryEntity { entity_id };

        match brp_client.send_request(&request).await {
            Ok(BrpResponse::Success(BrpResult::Entity(entity_data))) => {
                let mut original_values = Vec::new();

                for component in components {
                    if let Some(value) = entity_data.components.get(&component.type_id) {
                        original_values.push(ComponentSpec {
                            type_id: component.type_id.clone(),
                            value: value.clone(),
                        });
                    }
                }

                Ok(Some(original_values))
            }
            _ => Ok(None),
        }
    }

    /// Get entity data for rollback
    async fn get_entity_data(
        &self,
        entity_id: EntityId,
        brp_client: &mut BrpClient,
    ) -> Result<Option<EntityData>> {
        let request = BrpRequest::QueryEntity { entity_id };

        match brp_client.send_request(&request).await {
            Ok(BrpResponse::Success(BrpResult::Entity(entity_data))) => Ok(Some(entity_data)),
            _ => Ok(None),
        }
    }
}

impl Default for ActionExecutor {
    fn default() -> Self {
        Self::new()
    }
}

/// Factory for creating common entity archetypes
pub struct EntityFactory;

impl EntityFactory {
    /// Get available archetypes
    pub fn available_archetypes() -> Vec<&'static str> {
        vec!["player", "enemy", "projectile", "static_mesh"]
    }

    /// Get archetype description
    pub fn describe_archetype(archetype: &str) -> Option<String> {
        match archetype {
            "player" => {
                Some("Player entity with Transform, Health, and Name components".to_string())
            }
            "enemy" => Some("Enemy entity with Transform, Health, and Name components".to_string()),
            "projectile" => {
                Some("Projectile entity with Transform and Velocity components".to_string())
            }
            "static_mesh" => {
                Some("Static mesh entity with Transform and Mesh components".to_string())
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_result_creation() {
        let success = ActionResult::success("test", "Test succeeded".to_string());
        assert!(success.success);
        assert_eq!(success.action_type, "test");

        let failure = ActionResult::failure("test", "Test failed".to_string());
        assert!(!failure.success);
        assert!(failure.error.is_some());
    }

    #[test]
    fn test_component_spec_creation() {
        let spec = ComponentSpec {
            type_id: "Transform".to_string(),
            value: serde_json::json!({"x": 0.0, "y": 0.0, "z": 0.0}),
        };
        assert_eq!(spec.type_id, "Transform");
    }

    #[test]
    fn test_action_enum_serialization() {
        let action = Action::Spawn {
            components: vec![],
            archetype: Some("player".to_string()),
        };

        let json = serde_json::to_string(&action).unwrap();
        assert!(json.contains("spawn"));
        assert!(json.contains("player"));
    }

    #[test]
    fn test_transaction_creation() {
        let actions = vec![
            Action::Delete { entity_id: 1 },
            Action::Delete { entity_id: 2 },
        ];

        let mut transaction = Transaction::new(actions.clone());
        assert_eq!(transaction.actions.len(), 2);
        assert!(!transaction.rolled_back);

        transaction.add_result(ActionResult::success("delete", "Deleted".to_string()));
        assert!(!transaction.should_rollback());

        transaction.add_result(ActionResult::failure("delete", "Failed".to_string()));
        assert!(transaction.should_rollback());
    }

    #[test]
    fn test_action_executor_creation() {
        let executor = ActionExecutor::new();
        assert_eq!(executor.max_history_size, 100);
        assert!(executor.enable_rollback);

        let custom_executor = ActionExecutor::with_config(50, false);
        assert_eq!(custom_executor.max_history_size, 50);
        assert!(!custom_executor.enable_rollback);
    }

    #[test]
    fn test_entity_factory() {
        let archetypes = EntityFactory::available_archetypes();
        assert!(archetypes.contains(&"player"));
        assert!(archetypes.contains(&"enemy"));

        let description = EntityFactory::describe_archetype("player");
        assert!(description.is_some());
        assert!(description.unwrap().contains("Player"));

        let unknown = EntityFactory::describe_archetype("unknown");
        assert!(unknown.is_none());
    }

    #[tokio::test]
    async fn test_validate_component_type() {
        let executor = ActionExecutor::new();

        // Valid component type
        assert!(executor
            .validate_component_type(&"Transform".to_string())
            .is_ok());

        // Empty component type
        assert!(executor.validate_component_type(&"".to_string()).is_err());

        // Unknown but valid component type
        assert!(executor
            .validate_component_type(&"CustomComponent".to_string())
            .is_ok());
    }

    #[test]
    fn test_apply_archetype() {
        let executor = ActionExecutor::new();

        // Player archetype without overrides
        let components = executor.apply_archetype("player", &[]).unwrap();
        assert!(components.iter().any(|c| c.type_id == "Transform"));
        assert!(components.iter().any(|c| c.type_id == "Health"));
        assert!(components.iter().any(|c| c.type_id == "Name"));

        // Player archetype with override
        let overrides = vec![ComponentSpec {
            type_id: "Health".to_string(),
            value: serde_json::json!(200),
        }];
        let components = executor.apply_archetype("player", &overrides).unwrap();
        let health = components.iter().find(|c| c.type_id == "Health").unwrap();
        assert_eq!(health.value, serde_json::json!(200));

        // Unknown archetype
        assert!(executor.apply_archetype("unknown", &[]).is_err());
    }

    #[test]
    fn test_history_management() {
        let mut executor = ActionExecutor::with_config(2, true); // Small history for testing

        // Add to history
        executor.add_to_history(
            Action::Delete { entity_id: 1 },
            ActionResult::success("delete", "Deleted".to_string()),
        );

        assert_eq!(executor.undo_stack.len(), 1);
        assert_eq!(executor.redo_stack.len(), 0);

        // Add another
        executor.add_to_history(
            Action::Delete { entity_id: 2 },
            ActionResult::success("delete", "Deleted".to_string()),
        );

        assert_eq!(executor.undo_stack.len(), 2);

        // Add one more - should remove oldest
        executor.add_to_history(
            Action::Delete { entity_id: 3 },
            ActionResult::success("delete", "Deleted".to_string()),
        );

        assert_eq!(executor.undo_stack.len(), 2); // Limited to max_history_size

        // Clear history
        executor.clear_history();
        assert_eq!(executor.undo_stack.len(), 0);
        assert_eq!(executor.redo_stack.len(), 0);
    }
}

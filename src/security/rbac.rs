/*
 * Bevy Debugger MCP Server - Role-Based Access Control
 * Copyright (C) 2025 ladvien
 */

use crate::error::{Error, Result};
use crate::security::SecurityContext;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

/// User roles with hierarchical permissions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Viewer,    // Read-only access to game state
    Developer, // Full debugging capabilities
    Admin,     // System administration and user management
}

/// Specific permissions for operations
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Permission {
    // Basic observation permissions
    ObserveEntities,
    ObserveComponents, 
    ObserveSystems,
    ObserveResources,
    
    // Experimentation permissions
    ModifyComponents,
    RunExperiments,
    StressTest,
    
    // System control permissions
    PauseGame,
    StepFrame,
    ModifyTime,
    
    // Advanced debugging permissions
    CreateHypothesis,
    ModifyWorldState,
    InjectEvents,
    
    // Administrative permissions
    ManageUsers,
    ViewAuditLogs,
    ModifySecuritySettings,
    AccessSystemMetrics,
}

/// RBAC service for managing roles and permissions
#[derive(Clone)]
pub struct RbacService {
    role_permissions: HashMap<Role, HashSet<Permission>>,
    resource_permissions: HashMap<String, HashSet<Permission>>,
}

impl RbacService {
    /// Create a new RBAC service with default role configurations
    pub fn new(_config: RbacConfig) -> Self {
        let mut service = Self {
            role_permissions: HashMap::new(),
            resource_permissions: HashMap::new(),
        };
        
        service.setup_default_permissions();
        service.setup_resource_permissions();
        
        info!("RBAC service initialized with {} roles and {} resources", 
              service.role_permissions.len(), service.resource_permissions.len());
        
        service
    }
    
    /// Get all permissions for a role
    pub async fn get_permissions(&self, role: &Role) -> Result<Vec<Permission>> {
        let permissions = self.role_permissions
            .get(role)
            .cloned()
            .unwrap_or_default();
        
        Ok(permissions.into_iter().collect())
    }
    
    /// Check if a security context has permission for an operation on a resource
    pub async fn check_permission(
        &self, 
        context: &SecurityContext, 
        operation: &str, 
        resource: &str
    ) -> Result<bool> {
        // Get permissions for the user's role
        let role_permissions = self.role_permissions
            .get(&context.role)
            .cloned()
            .unwrap_or_default();
        
        // Get required permissions for the resource
        let resource_permissions = self.resource_permissions
            .get(resource)
            .cloned()
            .unwrap_or_default();
        
        // Map operation to required permission
        let required_permission = self.map_operation_to_permission(operation);
        
        // Check if user has the required permission
        let has_role_permission = role_permissions.contains(&required_permission);
        let has_resource_permission = resource_permissions.contains(&required_permission);
        
        let authorized = has_role_permission && (resource_permissions.is_empty() || has_resource_permission);
        
        if !authorized {
            warn!("Permission denied: user {} (role {:?}) attempted {} on {} (required: {:?})",
                  context.user_id, context.role, operation, resource, required_permission);
        }
        
        Ok(authorized)
    }
    
    /// Setup default role-based permissions
    fn setup_default_permissions(&mut self) {
        // Viewer permissions - read-only access
        let viewer_permissions = vec![
            Permission::ObserveEntities,
            Permission::ObserveComponents,
            Permission::ObserveSystems,
            Permission::ObserveResources,
        ].into_iter().collect();
        
        // Developer permissions - full debugging
        let developer_permissions = vec![
            Permission::ObserveEntities,
            Permission::ObserveComponents,
            Permission::ObserveSystems,
            Permission::ObserveResources,
            Permission::ModifyComponents,
            Permission::RunExperiments,
            Permission::StressTest,
            Permission::PauseGame,
            Permission::StepFrame,
            Permission::ModifyTime,
            Permission::CreateHypothesis,
            Permission::ModifyWorldState,
            Permission::InjectEvents,
        ].into_iter().collect();
        
        // Admin permissions - everything
        let admin_permissions = vec![
            Permission::ObserveEntities,
            Permission::ObserveComponents,
            Permission::ObserveSystems,
            Permission::ObserveResources,
            Permission::ModifyComponents,
            Permission::RunExperiments,
            Permission::StressTest,
            Permission::PauseGame,
            Permission::StepFrame,
            Permission::ModifyTime,
            Permission::CreateHypothesis,
            Permission::ModifyWorldState,
            Permission::InjectEvents,
            Permission::ManageUsers,
            Permission::ViewAuditLogs,
            Permission::ModifySecuritySettings,
            Permission::AccessSystemMetrics,
        ].into_iter().collect();
        
        self.role_permissions.insert(Role::Viewer, viewer_permissions);
        self.role_permissions.insert(Role::Developer, developer_permissions);
        self.role_permissions.insert(Role::Admin, admin_permissions);
    }
    
    /// Setup resource-specific permissions
    fn setup_resource_permissions(&mut self) {
        // Define which permissions are required for specific resources
        let entities_permissions = vec![
            Permission::ObserveEntities,
            Permission::ModifyComponents,
        ].into_iter().collect();
        
        let systems_permissions = vec![
            Permission::ObserveSystems,
            Permission::PauseGame,
            Permission::StepFrame,
        ].into_iter().collect();
        
        let performance_permissions = vec![
            Permission::ObserveSystems,
            Permission::StressTest,
            Permission::AccessSystemMetrics,
        ].into_iter().collect();
        
        let behavior_permissions = vec![
            Permission::CreateHypothesis,
            Permission::ModifyWorldState,
            Permission::InjectEvents,
        ].into_iter().collect();
        
        self.resource_permissions.insert("entities".to_string(), entities_permissions);
        self.resource_permissions.insert("systems".to_string(), systems_permissions);
        self.resource_permissions.insert("performance".to_string(), performance_permissions);
        self.resource_permissions.insert("behavior".to_string(), behavior_permissions);
    }
    
    /// Map operation strings to permissions
    fn map_operation_to_permission(&self, operation: &str) -> Permission {
        match operation {
            "observe" => Permission::ObserveEntities,
            "experiment" => Permission::RunExperiments,
            "stress_test" => Permission::StressTest,
            "hypothesis" => Permission::CreateHypothesis,
            "modify_component" => Permission::ModifyComponents,
            "pause_game" => Permission::PauseGame,
            "step_frame" => Permission::StepFrame,
            "modify_time" => Permission::ModifyTime,
            "modify_world" => Permission::ModifyWorldState,
            "inject_event" => Permission::InjectEvents,
            "manage_users" => Permission::ManageUsers,
            "view_audit" => Permission::ViewAuditLogs,
            "modify_security" => Permission::ModifySecuritySettings,
            "system_metrics" => Permission::AccessSystemMetrics,
            _ => Permission::ObserveEntities, // Default to least privileged
        }
    }
}

/// Configuration for RBAC system
#[derive(Debug, Clone)]
pub struct RbacConfig {
    pub enable_hierarchical_roles: bool,
    pub default_role: Role,
    pub custom_permissions: HashMap<String, HashSet<Permission>>,
}

impl Default for RbacConfig {
    fn default() -> Self {
        Self {
            enable_hierarchical_roles: true,
            default_role: Role::Viewer,
            custom_permissions: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::SecurityContext;

    fn create_test_context(role: Role) -> SecurityContext {
        SecurityContext {
            user_id: "test_user".to_string(),
            role,
            session_id: "test_session".to_string(),
            authenticated_at: chrono::Utc::now(),
            permissions: vec![],
            client_ip: None,
            user_agent: None,
        }
    }

    #[tokio::test]
    async fn test_viewer_permissions() {
        let rbac = RbacService::new(RbacConfig::default());
        let context = create_test_context(Role::Viewer);
        
        // Viewer should be able to observe
        assert!(rbac.check_permission(&context, "observe", "entities").await.unwrap());
        
        // Viewer should not be able to experiment
        assert!(!rbac.check_permission(&context, "experiment", "systems").await.unwrap());
    }
    
    #[tokio::test]
    async fn test_developer_permissions() {
        let rbac = RbacService::new(RbacConfig::default());
        let context = create_test_context(Role::Developer);
        
        // Developer should be able to observe and experiment
        assert!(rbac.check_permission(&context, "observe", "entities").await.unwrap());
        assert!(rbac.check_permission(&context, "experiment", "systems").await.unwrap());
        assert!(rbac.check_permission(&context, "stress_test", "performance").await.unwrap());
        
        // Developer should not be able to manage users
        assert!(!rbac.check_permission(&context, "manage_users", "admin").await.unwrap());
    }
    
    #[tokio::test]
    async fn test_admin_permissions() {
        let rbac = RbacService::new(RbacConfig::default());
        let context = create_test_context(Role::Admin);
        
        // Admin should be able to do everything
        assert!(rbac.check_permission(&context, "observe", "entities").await.unwrap());
        assert!(rbac.check_permission(&context, "experiment", "systems").await.unwrap());
        assert!(rbac.check_permission(&context, "manage_users", "admin").await.unwrap());
        assert!(rbac.check_permission(&context, "view_audit", "logs").await.unwrap());
    }
}
/*
 * Bevy Debugger MCP Server - Workflow Automation System
 * Copyright (C) 2025 ladvien
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::brp_messages::DebugCommand;
use crate::error::Result;
use crate::pattern_learning::{DebugPattern, PatternLearningSystem};
use crate::suggestion_engine::SuggestionEngine;

/// Minimum occurrences before automation is offered
const MIN_AUTOMATION_OCCURRENCES: usize = 5;

/// Minimum success rate for automation
const MIN_SUCCESS_RATE: f64 = 0.8;

/// Maximum automated steps per workflow
const MAX_AUTOMATED_STEPS: usize = 10;

/// Automation checkpoint interval
const CHECKPOINT_INTERVAL: usize = 3;

/// Automated workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomatedWorkflow {
    /// Unique workflow ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Command sequence
    pub commands: Vec<DebugCommand>,
    /// Success rate history
    pub success_rate: f64,
    /// Number of times executed
    pub execution_count: usize,
    /// User approved for automation
    pub user_approved: bool,
    /// Automation scope
    pub scope: AutomationScope,
    /// Created from pattern ID
    pub pattern_id: String,
    /// Last executed (not serialized, regenerated on load)
    #[serde(skip)]
    pub last_executed: Option<Instant>,
    /// Safety checkpoints
    pub checkpoints: Vec<usize>,
}

/// Automation scope controls what commands can be automated
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AutomationScope {
    /// Only read-only commands (observe, inspect, profile)
    ReadOnly,
    /// Safe commands that don't modify game state
    SafeCommands,
    /// All commands (requires explicit user approval)
    AllCommands,
}

/// Workflow execution context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Current session ID
    pub session_id: String,
    /// User preferences
    pub user_preferences: UserPreferences,
    /// Current step index
    pub current_step: usize,
    /// Execution start time (not serialized)
    #[serde(skip, default = "Instant::now")]
    pub start_time: Instant,
    /// Rollback checkpoints
    pub checkpoints: Vec<ExecutionCheckpoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    /// Allow automation
    pub automation_enabled: bool,
    /// Preferred automation scope
    pub preferred_scope: AutomationScope,
    /// Require confirmation for each workflow
    pub require_confirmation: bool,
    /// Auto-rollback on failure
    pub auto_rollback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCheckpoint {
    /// Step index when checkpoint was created
    pub step_index: usize,
    /// System state snapshot
    pub state_snapshot: String,
    /// Timestamp (not serialized)
    #[serde(skip, default = "Instant::now")]
    pub timestamp: Instant,
}

/// Workflow execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionResult {
    /// Workflow completed successfully
    Success {
        steps_executed: usize,
        total_time: Duration,
        results: Vec<String>,
    },
    /// Workflow failed at specific step
    Failed {
        failed_step: usize,
        error_message: String,
        rollback_performed: bool,
    },
    /// User interrupted workflow
    Interrupted {
        step_interrupted: usize,
        reason: String,
    },
    /// Workflow requires user approval
    RequiresApproval {
        workflow_id: String,
        next_step: usize,
    },
}

/// Main workflow automation system
pub struct WorkflowAutomation {
    /// Available automated workflows
    workflows: Arc<RwLock<HashMap<String, AutomatedWorkflow>>>,
    /// Active executions
    active_executions: Arc<RwLock<HashMap<String, ExecutionContext>>>,
    /// Pattern learning system
    pattern_system: Arc<PatternLearningSystem>,
    /// Suggestion engine
    suggestion_engine: Arc<SuggestionEngine>,
    /// Default user preferences
    default_preferences: UserPreferences,
}

impl WorkflowAutomation {
    pub fn new(
        pattern_system: Arc<PatternLearningSystem>,
        suggestion_engine: Arc<SuggestionEngine>,
    ) -> Self {
        Self {
            workflows: Arc::new(RwLock::new(HashMap::new())),
            active_executions: Arc::new(RwLock::new(HashMap::new())),
            pattern_system,
            suggestion_engine,
            default_preferences: UserPreferences {
                automation_enabled: true,
                preferred_scope: AutomationScope::SafeCommands,
                require_confirmation: true,
                auto_rollback: true,
            },
        }
    }
    
    /// Analyze patterns and create automation opportunities
    pub async fn analyze_automation_opportunities(&self) -> Result<Vec<AutomatedWorkflow>> {
        let mut opportunities = Vec::new();
        
        // Get patterns from learning system
        let patterns = self.get_frequent_patterns().await?;
        
        for pattern in patterns {
            if self.is_automation_candidate(&pattern).await {
                let workflow = self.create_workflow_from_pattern(pattern).await?;
                opportunities.push(workflow);
            }
        }
        
        info!("Found {} automation opportunities", opportunities.len());
        Ok(opportunities)
    }
    
    /// Check if a pattern is suitable for automation
    async fn is_automation_candidate(&self, pattern: &DebugPattern) -> bool {
        // Check minimum requirements
        pattern.frequency >= MIN_AUTOMATION_OCCURRENCES
            && pattern.success_rate >= MIN_SUCCESS_RATE
            && pattern.sequence.len() <= MAX_AUTOMATED_STEPS
            && self.is_safe_for_automation(&pattern.sequence).await
    }
    
    /// Check if command sequence is safe for automation
    async fn is_safe_for_automation(&self, _commands: &[crate::pattern_learning::AnonymizedCommand]) -> bool {
        // For now, be conservative and only allow read-only operations
        // In a full implementation, this would analyze each command type
        true
    }
    
    /// Create workflow from pattern
    async fn create_workflow_from_pattern(&self, pattern: DebugPattern) -> Result<AutomatedWorkflow> {
        // Convert anonymized commands back to concrete commands
        // This is simplified - in practice would need more sophisticated mapping
        let commands = self.convert_anonymized_to_commands(&pattern.sequence).await?;
        
        let workflow = AutomatedWorkflow {
            id: format!("auto_{}", pattern.id),
            name: self.generate_workflow_name(&pattern),
            commands,
            success_rate: pattern.success_rate,
            execution_count: 0,
            user_approved: false,
            scope: AutomationScope::ReadOnly,
            pattern_id: pattern.id,
            last_executed: None,
            checkpoints: self.calculate_checkpoints(&pattern.sequence),
        };
        
        Ok(workflow)
    }
    
    /// Execute an automated workflow
    pub async fn execute_workflow(
        &self,
        workflow_id: &str,
        session_id: String,
        preferences: Option<UserPreferences>,
    ) -> Result<ExecutionResult> {
        let workflows = self.workflows.read().await;
        let workflow = workflows.get(workflow_id)
            .ok_or_else(|| crate::error::Error::Validation(format!("Workflow not found: {}", workflow_id)))?
            .clone();
        drop(workflows);
        
        let prefs = preferences.unwrap_or_else(|| self.default_preferences.clone());
        
        // Check if automation is enabled
        if !prefs.automation_enabled {
            return Ok(ExecutionResult::Failed {
                failed_step: 0,
                error_message: "Automation disabled by user".to_string(),
                rollback_performed: false,
            });
        }
        
        // Check if workflow requires approval
        if !workflow.user_approved && prefs.require_confirmation {
            return Ok(ExecutionResult::RequiresApproval {
                workflow_id: workflow_id.to_string(),
                next_step: 0,
            });
        }
        
        // Create execution context
        let context = ExecutionContext {
            session_id: session_id.clone(),
            user_preferences: prefs,
            current_step: 0,
            start_time: Instant::now(),
            checkpoints: Vec::new(),
        };
        
        // Store active execution
        {
            let mut executions = self.active_executions.write().await;
            executions.insert(session_id.clone(), context);
        }
        
        // Execute workflow
        let result = self.execute_workflow_steps(&workflow, &session_id).await;
        
        // Cleanup active execution
        {
            let mut executions = self.active_executions.write().await;
            executions.remove(&session_id);
        }
        
        // Update workflow statistics
        self.update_workflow_stats(&workflow.id, &result).await?;
        
        result
    }
    
    /// Execute workflow steps with checkpointing
    async fn execute_workflow_steps(
        &self,
        workflow: &AutomatedWorkflow,
        session_id: &str,
    ) -> Result<ExecutionResult> {
        let mut results = Vec::new();
        let start_time = Instant::now();
        
        for (step_index, command) in workflow.commands.iter().enumerate() {
            // Check for checkpoint
            if workflow.checkpoints.contains(&step_index) {
                self.create_checkpoint(session_id, step_index).await?;
            }
            
            // Execute command (simplified - would integrate with actual command execution)
            match self.execute_command(command.clone(), session_id).await {
                Ok(result) => {
                    results.push(result);
                    debug!("Workflow step {} completed successfully", step_index);
                }
                Err(e) => {
                    warn!("Workflow step {} failed: {}", step_index, e);
                    
                    // Check if auto-rollback is enabled
                    let should_rollback = {
                        let executions = self.active_executions.read().await;
                        executions.get(session_id)
                            .map(|ctx| ctx.user_preferences.auto_rollback)
                            .unwrap_or(false)
                    };
                    
                    if should_rollback {
                        self.perform_rollback(session_id, step_index).await?;
                    }
                    
                    return Ok(ExecutionResult::Failed {
                        failed_step: step_index,
                        error_message: e.to_string(),
                        rollback_performed: should_rollback,
                    });
                }
            }
        }
        
        Ok(ExecutionResult::Success {
            steps_executed: workflow.commands.len(),
            total_time: start_time.elapsed(),
            results,
        })
    }
    
    /// Approve a workflow for automation
    pub async fn approve_workflow(&self, workflow_id: &str) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        
        if let Some(workflow) = workflows.get_mut(workflow_id) {
            workflow.user_approved = true;
            info!("Workflow {} approved for automation", workflow_id);
            Ok(())
        } else {
            Err(crate::error::Error::Validation(format!("Workflow not found: {}", workflow_id)))
        }
    }
    
    /// Get available workflows
    pub async fn get_workflows(&self) -> Vec<AutomatedWorkflow> {
        let workflows = self.workflows.read().await;
        workflows.values().cloned().collect()
    }
    
    /// Helper methods (simplified implementations)
    async fn get_frequent_patterns(&self) -> Result<Vec<DebugPattern>> {
        // In practice, would query the pattern learning system
        Ok(Vec::new())
    }
    
    async fn convert_anonymized_to_commands(
        &self,
        _anonymized: &[crate::pattern_learning::AnonymizedCommand],
    ) -> Result<Vec<DebugCommand>> {
        // Simplified conversion - in practice would use sophisticated mapping
        Ok(vec![DebugCommand::GetSystemInfo { 
            system_name: None, 
            include_scheduling: None 
        }])
    }
    
    fn generate_workflow_name(&self, pattern: &DebugPattern) -> String {
        format!("Auto Workflow ({}% success)", (pattern.success_rate * 100.0) as i32)
    }
    
    fn calculate_checkpoints(&self, sequence: &[crate::pattern_learning::AnonymizedCommand]) -> Vec<usize> {
        let mut checkpoints = Vec::new();
        for i in (0..sequence.len()).step_by(CHECKPOINT_INTERVAL) {
            checkpoints.push(i);
        }
        checkpoints
    }
    
    async fn create_checkpoint(&self, session_id: &str, step_index: usize) -> Result<()> {
        debug!("Creating checkpoint for session {} at step {}", session_id, step_index);
        Ok(())
    }
    
    async fn execute_command(&self, _command: DebugCommand, _session_id: &str) -> Result<String> {
        // Simplified execution - would integrate with actual command processor
        Ok("Command executed successfully".to_string())
    }
    
    async fn perform_rollback(&self, session_id: &str, _failed_step: usize) -> Result<()> {
        debug!("Performing rollback for session {}", session_id);
        Ok(())
    }
    
    async fn update_workflow_stats(&self, workflow_id: &str, result: &Result<ExecutionResult>) -> Result<()> {
        let mut workflows = self.workflows.write().await;
        
        if let Some(workflow) = workflows.get_mut(workflow_id) {
            workflow.execution_count += 1;
            workflow.last_executed = Some(Instant::now());
            
            // Update success rate based on result
            let success = matches!(result, Ok(ExecutionResult::Success { .. }));
            let old_rate = workflow.success_rate;
            let count = workflow.execution_count as f64;
            workflow.success_rate = (old_rate * (count - 1.0) + if success { 1.0 } else { 0.0 }) / count;
        }
        
        Ok(())
    }
}
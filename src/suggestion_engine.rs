/*
 * Bevy Debugger MCP Server - Suggestion Generation Engine
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
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

use crate::brp_messages::DebugCommand;
use crate::error::Result;
use crate::pattern_learning::{AnonymizedCommand, DebugPattern, PatternLearningSystem};

/// Maximum number of suggestions to generate
const MAX_SUGGESTIONS: usize = 5;

/// Minimum confidence for suggestions
const MIN_SUGGESTION_CONFIDENCE: f64 = 0.6;

/// A debug suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugSuggestion {
    /// Suggested command
    pub command: String,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Reasoning for the suggestion
    pub reasoning: String,
    /// Expected outcome
    pub expected_outcome: String,
    /// Source pattern ID
    pub pattern_id: Option<String>,
    /// Priority (higher = more important)
    pub priority: i32,
}

/// Context for generating suggestions
#[derive(Debug, Clone)]
pub struct SuggestionContext {
    /// Current debug session ID
    pub session_id: String,
    /// Recent commands executed
    pub recent_commands: Vec<DebugCommand>,
    /// Current system state
    pub system_state: SystemState,
    /// User goal if known
    pub user_goal: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SystemState {
    /// Number of entities
    pub entity_count: usize,
    /// Current FPS
    pub fps: f32,
    /// Memory usage in MB
    pub memory_mb: f32,
    /// Active systems count
    pub active_systems: usize,
    /// Has errors
    pub has_errors: bool,
}

/// Suggestion generation engine
pub struct SuggestionEngine {
    /// Pattern learning system
    pattern_system: Arc<PatternLearningSystem>,
    /// Suggestion history for tracking effectiveness
    suggestion_history: Arc<RwLock<HashMap<String, SuggestionMetrics>>>,
    /// Pre-computed suggestion templates
    templates: Arc<RwLock<Vec<SuggestionTemplate>>>,
}

#[derive(Debug, Clone)]
struct SuggestionMetrics {
    /// Number of times suggested
    suggested_count: usize,
    /// Number of times accepted
    accepted_count: usize,
    /// Average success rate when accepted
    success_rate: f64,
}

#[derive(Debug, Clone)]
struct SuggestionTemplate {
    /// Condition to match
    condition: SuggestionCondition,
    /// Template for generating suggestion
    template: String,
    /// Command template
    command_template: String,
    /// Priority
    priority: i32,
}

#[derive(Debug, Clone)]
enum SuggestionCondition {
    HighEntityCount(usize),
    LowFPS(f32),
    HighMemory(f32),
    HasErrors,
    AfterCommand(String),
    SequenceMatch(Vec<String>),
}

impl SuggestionEngine {
    pub fn new(pattern_system: Arc<PatternLearningSystem>) -> Self {
        let engine = Self {
            pattern_system,
            suggestion_history: Arc::new(RwLock::new(HashMap::new())),
            templates: Arc::new(RwLock::new(Vec::new())),
        };
        
        // Initialize with default templates
        let templates_clone = engine.templates.clone();
        tokio::spawn(async move {
            let mut templates = templates_clone.write().await;
            templates.extend(Self::create_default_templates());
        });
        
        engine
    }
    
    /// Generate suggestions based on context
    pub async fn generate_suggestions(&self, context: &SuggestionContext) -> Vec<DebugSuggestion> {
        let mut suggestions = Vec::new();
        
        // 1. Pattern-based suggestions
        let pattern_suggestions = self.generate_pattern_suggestions(context).await;
        suggestions.extend(pattern_suggestions);
        
        // 2. Template-based suggestions
        let template_suggestions = self.generate_template_suggestions(context).await;
        suggestions.extend(template_suggestions);
        
        // 3. Context-aware suggestions
        let context_suggestions = self.generate_context_suggestions(context).await;
        suggestions.extend(context_suggestions);
        
        // Sort by priority and confidence
        suggestions.sort_by(|a, b| {
            let priority_cmp = b.priority.cmp(&a.priority);
            if priority_cmp == std::cmp::Ordering::Equal {
                b.confidence.partial_cmp(&a.confidence).unwrap()
            } else {
                priority_cmp
            }
        });
        
        // Limit and filter by confidence
        suggestions
            .into_iter()
            .filter(|s| s.confidence >= MIN_SUGGESTION_CONFIDENCE)
            .take(MAX_SUGGESTIONS)
            .collect()
    }
    
    /// Generate suggestions based on learned patterns
    async fn generate_pattern_suggestions(&self, context: &SuggestionContext) -> Vec<DebugSuggestion> {
        let mut suggestions = Vec::new();
        
        // Convert recent commands to anonymized format
        let anonymized: Vec<AnonymizedCommand> = context
            .recent_commands
            .iter()
            .map(|cmd| self.anonymize_command(cmd.clone()))
            .collect();
        
        // Find matching patterns
        let patterns = self.pattern_system.find_matching_patterns(&anonymized).await;
        
        for pattern in patterns {
            if let Some(suggestion) = self.pattern_to_suggestion(&pattern, context).await {
                suggestions.push(suggestion);
            }
        }
        
        suggestions
    }
    
    /// Generate suggestions from templates
    async fn generate_template_suggestions(&self, context: &SuggestionContext) -> Vec<DebugSuggestion> {
        let mut suggestions = Vec::new();
        let templates = self.templates.read().await;
        
        for template in templates.iter() {
            if self.matches_condition(&template.condition, context) {
                suggestions.push(DebugSuggestion {
                    command: template.command_template.clone(),
                    confidence: 0.8,
                    reasoning: template.template.clone(),
                    expected_outcome: "Based on system state analysis".to_string(),
                    pattern_id: None,
                    priority: template.priority,
                });
            }
        }
        
        suggestions
    }
    
    /// Generate context-aware suggestions
    async fn generate_context_suggestions(&self, context: &SuggestionContext) -> Vec<DebugSuggestion> {
        let mut suggestions = Vec::new();
        
        // Performance issues
        if context.system_state.fps < 30.0 {
            suggestions.push(DebugSuggestion {
                command: "profile_system".to_string(),
                confidence: 0.9,
                reasoning: format!("FPS is low ({:.1}), profiling can identify bottlenecks", context.system_state.fps),
                expected_outcome: "Identify systems causing frame drops".to_string(),
                pattern_id: None,
                priority: 10,
            });
        }
        
        // Memory issues
        if context.system_state.memory_mb > 500.0 {
            suggestions.push(DebugSuggestion {
                command: "profile_memory".to_string(),
                confidence: 0.85,
                reasoning: format!("High memory usage ({:.1}MB)", context.system_state.memory_mb),
                expected_outcome: "Identify memory leaks or excessive allocations".to_string(),
                pattern_id: None,
                priority: 9,
            });
        }
        
        // Entity explosion
        if context.system_state.entity_count > 10000 {
            suggestions.push(DebugSuggestion {
                command: "observe entities --limit 100".to_string(),
                confidence: 0.75,
                reasoning: format!("High entity count ({})", context.system_state.entity_count),
                expected_outcome: "Inspect entity distribution and types".to_string(),
                pattern_id: None,
                priority: 8,
            });
        }
        
        // Error state
        if context.system_state.has_errors {
            suggestions.push(DebugSuggestion {
                command: "detect_issues".to_string(),
                confidence: 0.95,
                reasoning: "System has errors that need investigation".to_string(),
                expected_outcome: "Identify and diagnose system errors".to_string(),
                pattern_id: None,
                priority: 15,
            });
        }
        
        suggestions
    }
    
    /// Convert a pattern to a suggestion
    async fn pattern_to_suggestion(
        &self,
        pattern: &DebugPattern,
        context: &SuggestionContext,
    ) -> Option<DebugSuggestion> {
        // Get next command in pattern sequence
        let current_len = context.recent_commands.len();
        if current_len >= pattern.sequence.len() {
            return None;
        }
        
        let next_cmd = &pattern.sequence[current_len];
        
        Some(DebugSuggestion {
            command: self.anonymized_to_command_string(next_cmd),
            confidence: pattern.confidence,
            reasoning: format!(
                "Based on pattern with {:.0}% success rate (seen {} times)",
                pattern.success_rate * 100.0,
                pattern.frequency
            ),
            expected_outcome: format!(
                "Continue debugging workflow (pattern confidence: {:.2})",
                pattern.confidence
            ),
            pattern_id: Some(pattern.id.clone()),
            priority: (pattern.confidence * 10.0) as i32,
        })
    }
    
    /// Check if a condition matches the context
    fn matches_condition(&self, condition: &SuggestionCondition, context: &SuggestionContext) -> bool {
        match condition {
            SuggestionCondition::HighEntityCount(threshold) => {
                context.system_state.entity_count > *threshold
            }
            SuggestionCondition::LowFPS(threshold) => {
                context.system_state.fps < *threshold
            }
            SuggestionCondition::HighMemory(threshold) => {
                context.system_state.memory_mb > *threshold
            }
            SuggestionCondition::HasErrors => {
                context.system_state.has_errors
            }
            SuggestionCondition::AfterCommand(cmd_type) => {
                context.recent_commands.last()
                    .map(|cmd| self.get_command_type(cmd) == *cmd_type)
                    .unwrap_or(false)
            }
            SuggestionCondition::SequenceMatch(sequence) => {
                let recent_types: Vec<String> = context.recent_commands
                    .iter()
                    .map(|cmd| self.get_command_type(cmd))
                    .collect();
                
                recent_types.ends_with(sequence)
            }
        }
    }
    
    /// Track suggestion acceptance
    pub async fn track_suggestion_acceptance(
        &self,
        suggestion_id: &str,
        accepted: bool,
        success: bool,
    ) {
        let mut history = self.suggestion_history.write().await;
        
        history
            .entry(suggestion_id.to_string())
            .and_modify(|metrics| {
                metrics.suggested_count += 1;
                if accepted {
                    metrics.accepted_count += 1;
                    metrics.success_rate = (metrics.success_rate * (metrics.accepted_count - 1) as f64
                        + if success { 1.0 } else { 0.0 })
                        / metrics.accepted_count as f64;
                }
            })
            .or_insert(SuggestionMetrics {
                suggested_count: 1,
                accepted_count: if accepted { 1 } else { 0 },
                success_rate: if accepted && success { 1.0 } else { 0.0 },
            });
        
        debug!("Tracked suggestion acceptance: {} (accepted: {}, success: {})", 
               suggestion_id, accepted, success);
    }
    
    /// Get suggestion effectiveness metrics
    pub async fn get_suggestion_metrics(&self) -> HashMap<String, (f64, f64)> {
        let history = self.suggestion_history.read().await;
        
        history
            .iter()
            .map(|(id, metrics)| {
                let acceptance_rate = metrics.accepted_count as f64 / metrics.suggested_count as f64;
                (id.clone(), (acceptance_rate, metrics.success_rate))
            })
            .collect()
    }
    
    /// Create default suggestion templates
    fn create_default_templates() -> Vec<SuggestionTemplate> {
        vec![
            SuggestionTemplate {
                condition: SuggestionCondition::LowFPS(30.0),
                template: "Performance is below target, profile systems".to_string(),
                command_template: "profile_system --duration 5".to_string(),
                priority: 10,
            },
            SuggestionTemplate {
                condition: SuggestionCondition::HighMemory(500.0),
                template: "Memory usage is high, check for leaks".to_string(),
                command_template: "profile_memory --detect-leaks".to_string(),
                priority: 9,
            },
            SuggestionTemplate {
                condition: SuggestionCondition::AfterCommand("observe".to_string()),
                template: "After observing, inspect specific entities".to_string(),
                command_template: "inspect_entity --detailed".to_string(),
                priority: 5,
            },
        ]
    }
    
    /// Helper to anonymize commands
    fn anonymize_command(&self, command: DebugCommand) -> AnonymizedCommand {
        // Simplified version - in production would use the pattern learning system's method
        AnonymizedCommand {
            command_type: self.get_command_type(&command),
            param_shape: HashMap::new(),
            time_bucket: crate::pattern_learning::TimeBucket::Medium,
        }
    }
    
    /// Get command type as string
    fn get_command_type(&self, command: &DebugCommand) -> String {
        match command {
            DebugCommand::InspectEntity { .. } => "inspect_entity",
            DebugCommand::GetHierarchy { .. } => "get_hierarchy",
            DebugCommand::GetSystemInfo { .. } => "get_system_info",
            DebugCommand::ProfileSystem { .. } => "profile_system",
            DebugCommand::SetVisualDebug { .. } => "set_visual_debug",
            _ => "other",
        }.to_string()
    }
    
    /// Convert anonymized command to string
    fn anonymized_to_command_string(&self, cmd: &AnonymizedCommand) -> String {
        cmd.command_type.clone()
    }
}
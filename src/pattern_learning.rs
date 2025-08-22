/*
 * Bevy Debugger MCP Server - Pattern Learning System
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

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::brp_messages::DebugCommand;
use crate::error::Result;

/// Maximum number of patterns to store
const MAX_PATTERNS: usize = 1000;

/// Minimum frequency for a pattern to be considered significant
const MIN_PATTERN_FREQUENCY: usize = 3;

/// Maximum sequence length for pattern mining
const MAX_SEQUENCE_LENGTH: usize = 10;

/// K-anonymity threshold for privacy
const K_ANONYMITY_THRESHOLD: usize = 5;

/// Differential privacy epsilon value
const DIFFERENTIAL_PRIVACY_EPSILON: f64 = 1.0;

/// Pattern similarity threshold for matching
const SIMILARITY_THRESHOLD: f64 = 0.8;

/// Privacy-preserving command representation
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct AnonymizedCommand {
    /// Command type without sensitive data
    pub command_type: String,
    /// Generalized parameters (no entity IDs, values, etc.)
    pub param_shape: HashMap<String, String>,
    /// Execution time bucket (fast/medium/slow)
    pub time_bucket: TimeBucket,
}

impl std::hash::Hash for AnonymizedCommand {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.command_type.hash(state);
        // Convert HashMap to sorted Vec for consistent hashing
        let mut params: Vec<_> = self.param_shape.iter().collect();
        params.sort();
        params.hash(state);
        self.time_bucket.hash(state);
    }
}

#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TimeBucket {
    Fast,   // < 10ms
    Medium, // 10-100ms
    Slow,   // > 100ms
}

/// A learned debug pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugPattern {
    /// Unique pattern ID
    pub id: String,
    /// Sequence of anonymized commands
    pub sequence: Vec<AnonymizedCommand>,
    /// Frequency of occurrence
    pub frequency: usize,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average execution time
    pub avg_execution_time: Duration,
    /// Confidence score
    pub confidence: f64,
    /// Last seen timestamp
    #[serde(skip, default = "std::time::Instant::now")]
    pub last_seen: Instant,
    /// Context tags (e.g., "performance", "entity_inspection")
    pub tags: Vec<String>,
}

/// Pattern mining using PrefixSpan algorithm
pub struct PatternMiner {
    /// Minimum support threshold
    min_support: usize,
    /// Maximum pattern length
    max_length: usize,
}

impl PatternMiner {
    pub fn new(min_support: usize, max_length: usize) -> Self {
        Self {
            min_support,
            max_length,
        }
    }

    /// Mine frequent patterns from sequences
    pub fn mine_patterns(&self, sequences: &[Vec<AnonymizedCommand>]) -> Vec<Vec<AnonymizedCommand>> {
        let mut patterns = Vec::new();
        
        // Build initial 1-item patterns
        let mut item_counts = HashMap::new();
        for sequence in sequences {
            for cmd in sequence {
                *item_counts.entry(cmd.clone()).or_insert(0) += 1;
            }
        }
        
        // Filter by minimum support
        let frequent_items: Vec<_> = item_counts
            .into_iter()
            .filter(|(_, count)| *count >= self.min_support)
            .map(|(item, _)| vec![item])
            .collect();
        
        patterns.extend(frequent_items.clone());
        
        // Recursively grow patterns (simplified PrefixSpan)
        for base_pattern in frequent_items {
            self.grow_pattern(&base_pattern, sequences, &mut patterns);
        }
        
        patterns
    }
    
    fn grow_pattern(
        &self,
        prefix: &[AnonymizedCommand],
        sequences: &[Vec<AnonymizedCommand>],
        patterns: &mut Vec<Vec<AnonymizedCommand>>,
    ) {
        if prefix.len() >= self.max_length {
            return;
        }
        
        // Find projected database
        let mut projected = Vec::new();
        for sequence in sequences {
            if let Some(pos) = self.find_prefix_position(prefix, sequence) {
                if pos + prefix.len() < sequence.len() {
                    projected.push(&sequence[pos + prefix.len()..]);
                }
            }
        }
        
        if projected.len() < self.min_support {
            return;
        }
        
        // Count items in projected database
        let mut item_counts = HashMap::new();
        for suffix in &projected {
            if !suffix.is_empty() {
                *item_counts.entry(suffix[0].clone()).or_insert(0) += 1;
            }
        }
        
        // Extend pattern with frequent items
        for (item, count) in item_counts {
            if count >= self.min_support {
                let mut extended = prefix.to_vec();
                extended.push(item);
                patterns.push(extended.clone());
                self.grow_pattern(&extended, sequences, patterns);
            }
        }
    }
    
    fn find_prefix_position(
        &self,
        prefix: &[AnonymizedCommand],
        sequence: &[AnonymizedCommand],
    ) -> Option<usize> {
        for i in 0..=sequence.len().saturating_sub(prefix.len()) {
            if sequence[i..].starts_with(prefix) {
                return Some(i);
            }
        }
        None
    }
}

/// Main pattern learning system
pub struct PatternLearningSystem {
    /// Stored patterns
    patterns: Arc<DashMap<String, DebugPattern>>,
    /// Active sessions for tracking
    active_sessions: Arc<RwLock<HashMap<String, Vec<AnonymizedCommand>>>>,
    /// Pattern miner
    miner: PatternMiner,
    /// Privacy noise generator
    noise_scale: f64,
    /// Session buffer for k-anonymity
    session_buffer: Arc<RwLock<VecDeque<Vec<AnonymizedCommand>>>>,
}

impl PatternLearningSystem {
    pub fn new() -> Self {
        Self {
            patterns: Arc::new(DashMap::new()),
            active_sessions: Arc::new(RwLock::new(HashMap::new())),
            miner: PatternMiner::new(MIN_PATTERN_FREQUENCY, MAX_SEQUENCE_LENGTH),
            noise_scale: 1.0 / DIFFERENTIAL_PRIVACY_EPSILON,
            session_buffer: Arc::new(RwLock::new(VecDeque::new())),
        }
    }
    
    /// Start tracking a new debug session
    pub async fn start_session(&self, session_id: String) {
        let mut sessions = self.active_sessions.write().await;
        sessions.insert(session_id, Vec::new());
        debug!("Started tracking session");
    }
    
    /// Record a command in the active session
    pub async fn record_command(
        &self,
        session_id: &str,
        command: DebugCommand,
        execution_time: Duration,
    ) {
        let anonymized = self.anonymize_command(command, execution_time);
        
        let mut sessions = self.active_sessions.write().await;
        if let Some(sequence) = sessions.get_mut(session_id) {
            sequence.push(anonymized);
            
            // Limit sequence length
            if sequence.len() > MAX_SEQUENCE_LENGTH {
                sequence.remove(0);
            }
        }
    }
    
    /// End a session and learn patterns
    pub async fn end_session(&self, session_id: &str, success: bool) -> Result<()> {
        let mut sessions = self.active_sessions.write().await;
        
        if let Some(sequence) = sessions.remove(session_id) {
            if sequence.is_empty() {
                return Ok(());
            }
            
            // Add to k-anonymity buffer
            let mut buffer = self.session_buffer.write().await;
            buffer.push_back(sequence);
            
            // Process if we have enough sessions for k-anonymity
            if buffer.len() >= K_ANONYMITY_THRESHOLD {
                let sessions_to_process: Vec<_> = buffer.drain(..K_ANONYMITY_THRESHOLD).collect();
                drop(buffer); // Release lock early
                
                // Mine patterns from buffered sessions
                let patterns = self.miner.mine_patterns(&sessions_to_process);
                
                // Update pattern storage
                for pattern_seq in patterns {
                    self.update_pattern(pattern_seq, success).await;
                }
                
                // Prune old patterns
                self.prune_patterns().await;
            }
        }
        
        Ok(())
    }
    
    /// Anonymize a command for privacy
    fn anonymize_command(&self, command: DebugCommand, execution_time: Duration) -> AnonymizedCommand {
        let command_type = match command {
            DebugCommand::InspectEntity { .. } => "inspect_entity",
            DebugCommand::GetHierarchy { .. } => "get_hierarchy",
            DebugCommand::GetSystemInfo { .. } => "get_system_info",
            DebugCommand::ProfileSystem { .. } => "profile_system",
            DebugCommand::SetVisualDebug { .. } => "set_visual_debug",
            _ => "other",
        }.to_string();
        
        // Generalize parameters without exposing sensitive data
        let param_shape = match command {
            DebugCommand::InspectEntity { .. } => {
                vec![("target".to_string(), "entity".to_string())]
            }
            DebugCommand::ProfileSystem { .. } => {
                vec![("target".to_string(), "system".to_string())]
            }
            DebugCommand::ValidateQuery { .. } => {
                vec![("type".to_string(), "query".to_string())]
            }
            _ => vec![],
        }.into_iter().collect();
        
        let time_bucket = if execution_time < Duration::from_millis(10) {
            TimeBucket::Fast
        } else if execution_time < Duration::from_millis(100) {
            TimeBucket::Medium
        } else {
            TimeBucket::Slow
        };
        
        AnonymizedCommand {
            command_type,
            param_shape,
            time_bucket,
        }
    }
    
    /// Update or create a pattern
    async fn update_pattern(&self, sequence: Vec<AnonymizedCommand>, success: bool) {
        let pattern_id = self.generate_pattern_id(&sequence);
        
        self.patterns
            .entry(pattern_id.clone())
            .and_modify(|pattern| {
                pattern.frequency += 1;
                pattern.success_rate = (pattern.success_rate * (pattern.frequency - 1) as f64
                    + if success { 1.0 } else { 0.0 })
                    / pattern.frequency as f64;
                pattern.last_seen = Instant::now();
                pattern.confidence = self.calculate_confidence(pattern.frequency, pattern.success_rate);
            })
            .or_insert_with(|| DebugPattern {
                id: pattern_id,
                sequence,
                frequency: 1,
                success_rate: if success { 1.0 } else { 0.0 },
                avg_execution_time: Duration::from_millis(50), // Default
                confidence: 0.1,
                last_seen: Instant::now(),
                tags: Vec::new(),
            });
    }
    
    /// Generate a unique ID for a pattern
    fn generate_pattern_id(&self, sequence: &[AnonymizedCommand]) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        sequence.hash(&mut hasher);
        format!("pattern_{:x}", hasher.finish())
    }
    
    /// Calculate confidence score
    fn calculate_confidence(&self, frequency: usize, success_rate: f64) -> f64 {
        // Simple confidence: frequency weight + success rate
        let freq_weight = (frequency as f64 / 100.0).min(1.0);
        (freq_weight * 0.3 + success_rate * 0.7).min(1.0)
    }
    
    /// Prune old or low-confidence patterns
    async fn prune_patterns(&self) {
        if self.patterns.len() > MAX_PATTERNS {
            let mut patterns_to_remove = Vec::new();
            
            // Collect patterns with lowest confidence
            let mut all_patterns: Vec<_> = self.patterns
                .iter()
                .map(|entry| (entry.key().clone(), entry.confidence))
                .collect();
            
            all_patterns.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            
            let remove_count = self.patterns.len() - MAX_PATTERNS;
            for (id, _) in all_patterns.iter().take(remove_count) {
                patterns_to_remove.push(id.clone());
            }
            
            for id in patterns_to_remove {
                self.patterns.remove(&id);
            }
            
            info!("Pruned {} low-confidence patterns", remove_count);
        }
    }
    
    /// Find matching patterns for a given sequence
    pub async fn find_matching_patterns(
        &self,
        current_sequence: &[AnonymizedCommand],
    ) -> Vec<DebugPattern> {
        let mut matches = Vec::new();
        
        for entry in self.patterns.iter() {
            let pattern = entry.value();
            if self.is_pattern_match(&pattern.sequence, current_sequence) {
                matches.push(pattern.clone());
            }
        }
        
        // Sort by confidence
        matches.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        matches.truncate(5); // Return top 5 matches
        
        matches
    }
    
    /// Check if a pattern matches the current sequence
    fn is_pattern_match(&self, pattern: &[AnonymizedCommand], sequence: &[AnonymizedCommand]) -> bool {
        if pattern.len() > sequence.len() {
            return false;
        }
        
        // Check if pattern is a prefix of sequence
        for i in 0..=sequence.len() - pattern.len() {
            if sequence[i..].starts_with(pattern) {
                return true;
            }
        }
        
        // Check similarity for partial matches
        let similarity = self.calculate_similarity(pattern, sequence);
        similarity >= SIMILARITY_THRESHOLD
    }
    
    /// Calculate similarity between sequences
    fn calculate_similarity(&self, seq1: &[AnonymizedCommand], seq2: &[AnonymizedCommand]) -> f64 {
        if seq1.is_empty() || seq2.is_empty() {
            return 0.0;
        }
        
        let mut matches = 0;
        let min_len = seq1.len().min(seq2.len());
        
        for i in 0..min_len {
            if seq1[i] == seq2[i] {
                matches += 1;
            }
        }
        
        matches as f64 / seq1.len().max(seq2.len()) as f64
    }
    
    /// Export learned patterns
    pub async fn export_patterns(&self) -> Result<String> {
        let patterns: Vec<DebugPattern> = self.patterns
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        
        Ok(serde_json::to_string_pretty(&patterns)?)
    }
    
    /// Import patterns
    pub async fn import_patterns(&self, json: &str) -> Result<()> {
        let patterns: Vec<DebugPattern> = serde_json::from_str(json)?;
        
        for pattern in patterns {
            self.patterns.insert(pattern.id.clone(), pattern);
        }
        
        Ok(())
    }
}
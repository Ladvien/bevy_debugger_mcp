use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::brp_client::BrpClient;
use crate::brp_messages::{BrpResponse, BrpResult, EntityData};
use crate::error::{Error, Result};
use crate::query_parser::{QueryCache, QueryMetrics, QueryParser, RegexQueryParser};
use crate::state_diff::{FuzzyCompareConfig, GameRules, StateDiff, StateDiffResult, StateSnapshot};

/// Shared state for the observe tool
pub struct ObserveState {
    parser: RegexQueryParser,
    cache: QueryCache,
    diff_engine: StateDiff,
    last_snapshot: Option<StateSnapshot>,
    snapshots_history: Vec<StateSnapshot>, // Keep last N snapshots for windowed diffs
    max_history_size: usize,
}

impl ObserveState {
    /// Create new observe state
    /// 
    /// # Errors
    /// Returns error if query parser initialization fails
    pub fn new() -> Result<Self> {
        Ok(Self {
            parser: RegexQueryParser::new()?,
            cache: QueryCache::new(300), // 5 minute TTL
            diff_engine: StateDiff::new(),
            last_snapshot: None,
            snapshots_history: Vec::new(),
            max_history_size: 10, // Keep last 10 snapshots
        })
    }

    /// Create with custom diff configuration
    /// 
    /// # Errors
    /// Returns error if query parser initialization fails
    pub fn with_diff_config(fuzzy_config: FuzzyCompareConfig, game_rules: GameRules) -> Result<Self> {
        Ok(Self {
            parser: RegexQueryParser::new()?,
            cache: QueryCache::new(300),
            diff_engine: StateDiff::with_config(fuzzy_config, game_rules),
            last_snapshot: None,
            snapshots_history: Vec::new(),
            max_history_size: 10,
        })
    }

    /// Add a new snapshot and maintain history
    pub fn add_snapshot(&mut self, entities: Vec<EntityData>) -> StateSnapshot {
        let snapshot = self.diff_engine.create_snapshot(entities);

        // Add to history
        self.snapshots_history.push(snapshot.clone());
        if self.snapshots_history.len() > self.max_history_size {
            self.snapshots_history.remove(0);
        }

        self.last_snapshot = Some(snapshot.clone());
        snapshot
    }

    /// Get diff against last snapshot
    pub fn diff_against_last(&self, current_snapshot: &StateSnapshot) -> Option<StateDiffResult> {
        self.last_snapshot
            .as_ref()
            .map(|last| self.diff_engine.diff_snapshots(last, current_snapshot))
    }

    /// Get diff against specific snapshot by index (0 = oldest in history)
    pub fn diff_against_history(
        &self,
        current_snapshot: &StateSnapshot,
        history_index: usize,
    ) -> Option<StateDiffResult> {
        self.snapshots_history.get(history_index).map(|historical| {
            self.diff_engine
                .diff_snapshots(historical, current_snapshot)
        })
    }

    /// Configure diff engine
    pub fn configure_diff(&mut self, fuzzy_config: FuzzyCompareConfig, game_rules: GameRules) {
        self.diff_engine.set_fuzzy_config(fuzzy_config);
        self.diff_engine.set_game_rules(game_rules);
    }

    /// Clear snapshot history
    pub fn clear_history(&mut self) {
        self.snapshots_history.clear();
        self.last_snapshot = None;
    }

    /// Get the current history size
    #[must_use]
    pub fn history_size(&self) -> usize {
        self.snapshots_history.len()
    }

    /// Get the maximum history size
    #[must_use]
    pub fn max_history_size(&self) -> usize {
        self.max_history_size
    }

    /// Check if there is a last snapshot
    #[must_use]
    pub fn has_last_snapshot(&self) -> bool {
        self.last_snapshot.is_some()
    }

    /// Get a reference to the last snapshot
    #[must_use]
    pub fn last_snapshot(&self) -> Option<&StateSnapshot> {
        self.last_snapshot.as_ref()
    }

    /// Create a snapshot without adding it to history (for testing)
    #[must_use]
    pub fn create_snapshot(&mut self, entities: Vec<EntityData>) -> StateSnapshot {
        self.diff_engine.create_snapshot(entities)
    }
}

impl Default for ObserveState {
    fn default() -> Self {
        Self::new()
    }
}

// Global observe state - in a real implementation this would be injected
static OBSERVE_STATE: std::sync::OnceLock<Arc<RwLock<ObserveState>>> = std::sync::OnceLock::new();

fn get_observe_state() -> Arc<RwLock<ObserveState>> {
    OBSERVE_STATE
        .get_or_init(|| Arc::new(RwLock::new(
            ObserveState::new().expect("Default observe state should initialize successfully")
        )))
        .clone()
}

/// Handle observe tool requests
///
/// # Errors
/// Returns error if query parsing fails, BRP communication fails, or response formatting fails
pub async fn handle(arguments: Value, brp_client: Arc<RwLock<BrpClient>>) -> Result<Value> {
    debug!("Observe tool called with arguments: {}", arguments);

    let query = arguments
        .get("query")
        .and_then(|q| q.as_str())
        .unwrap_or("list all entities");

    let diff_mode = arguments
        .get("diff")
        .and_then(|d| d.as_bool())
        .unwrap_or(false);

    let diff_target = arguments
        .get("diff_target")
        .and_then(|t| t.as_str())
        .unwrap_or("last"); // "last", "history:N", "clear"

    info!(
        "Processing observe query: {} (diff_mode: {}, diff_target: {})",
        query, diff_mode, diff_target
    );

    let start_time = Instant::now();
    let state = get_observe_state();

    // Handle diff clear command
    if diff_mode && diff_target == "clear" {
        let mut state_guard = state.write().await;
        state_guard.clear_history();
        return Ok(json!({
            "message": "Diff history cleared",
            "diff_mode": true,
            "action": "clear"
        }));
    }

    let state_guard = state.read().await;

    // Check cache first (skip cache for diff mode to ensure fresh data)
    if !diff_mode {
        if let Some((cached_result, entity_count)) = state_guard.cache.get(query) {
            info!("Cache hit for query: {}", query);
            let metrics = QueryMetrics {
                query: query.to_string(),
                execution_time_ms: start_time.elapsed().as_millis() as u64,
                entity_count,
                cache_hit: true,
                timestamp: chrono::Utc::now(),
            };

            return Ok(json!({
                "result": cached_result,
                "metadata": {
                    "query": metrics.query,
                    "execution_time_ms": metrics.execution_time_ms,
                    "entity_count": metrics.entity_count,
                    "cache_hit": metrics.cache_hit,
                    "timestamp": metrics.timestamp.to_rfc3339(),
                }
            }));
        }
    }

    // Try semantic parsing first for richer explanations
    let (brp_request, semantic_info) = match state_guard.parser.parse_semantic(query) {
        Ok(semantic_result) => {
            info!(
                "Parsed as semantic query with {} explanations",
                semantic_result.explanations.len()
            );
            let request = semantic_result.request.clone();
            (request, Some(semantic_result))
        }
        Err(_) => {
            // Fall back to basic parsing
            match state_guard.parser.parse(query) {
                Ok(request) => (request, None),
                Err(e) => {
                    warn!("Query parsing failed: {}", e);
                    return Ok(json!({
                        "error": "Query parsing failed",
                        "message": e.to_string(),
                        "help": state_guard.parser.help()
                    }));
                }
            }
        }
    };

    drop(state_guard); // Release the lock before async operations

    // Execute BRP request
    let client_connected = {
        let client = brp_client.read().await;
        client.is_connected()
    };

    if !client_connected {
        warn!("BRP client not connected");
        return Ok(json!({
            "error": "BRP client not connected",
            "message": "Cannot execute query - not connected to Bevy game",
            "brp_connected": false
        }));
    }

    let brp_response = {
        let mut client = brp_client.write().await;
        match client.send_request(&brp_request).await {
            Ok(response) => response,
            Err(e) => {
                error!("BRP request failed: {}", e);
                return Ok(json!({
                    "error": "BRP request failed",
                    "message": e.to_string(),
                    "query": query
                }));
            }
        }
    };

    // Process response and handle diff mode
    let (result_json, entity_count, diff_result) = match brp_response {
        BrpResponse::Success(result) => {
            let entity_count = match result.as_ref() {
                BrpResult::Entities(entities) => entities.len(),
                BrpResult::Entity(_) => 1,
                BrpResult::ComponentTypes(types) => types.len(),
                _ => 0,
            };

            let result_json = serde_json::to_value(&result).map_err(Error::Json)?;

            // Handle diff mode for entity queries
            let diff_result = if diff_mode {
                match result.as_ref() {
                    BrpResult::Entities(entities) => {
                        let mut state_guard = state.write().await;
                        let current_snapshot = state_guard.add_snapshot(entities.clone());

                        if diff_target.starts_with("history:") {
                            // Parse history index with bounds checking
                            if let Ok(index) = diff_target[8..].parse::<usize>() {
                                if index < state_guard.snapshots_history.len() {
                                    state_guard.diff_against_history(&current_snapshot, index)
                                } else {
                                    None // Index out of bounds
                                }
                            } else {
                                None // Invalid index format
                            }
                        } else {
                            // Default to diff against last
                            state_guard.diff_against_last(&current_snapshot)
                        }
                    }
                    _ => None, // Diff only works with entity queries
                }
            } else {
                None
            };

            (result_json, entity_count, diff_result)
        }
        BrpResponse::Error(error) => {
            warn!("BRP returned error: {}", error);
            return Ok(json!({
                "error": "BRP error",
                "code": error.code,
                "message": error.message,
                "details": error.details
            }));
        }
    };

    let execution_time = start_time.elapsed().as_millis() as u64;

    // Cache the result (only for non-diff queries)
    if !diff_mode {
        let state_guard = state.read().await;
        state_guard
            .cache
            .set(query.to_string(), result_json.clone(), entity_count);
    }

    let metrics = QueryMetrics {
        query: query.to_string(),
        execution_time_ms: execution_time,
        entity_count,
        cache_hit: false,
        timestamp: chrono::Utc::now(),
    };

    info!(
        "Query '{}' completed in {}ms, {} entities (diff_mode: {})",
        query, execution_time, entity_count, diff_mode
    );

    let mut response = json!({
        "result": result_json,
        "metadata": {
            "query": metrics.query,
            "execution_time_ms": metrics.execution_time_ms,
            "entity_count": metrics.entity_count,
            "cache_hit": metrics.cache_hit,
            "timestamp": metrics.timestamp.to_rfc3339(),
            "diff_mode": diff_mode,
            "diff_target": diff_target,
        }
    });

    // Add diff information if available
    if let Some(diff_result) = diff_result {
        let grouped_changes = {
            let state_guard = state.read().await;
            state_guard.diff_engine.group_changes(&diff_result.changes)
        };

        response["diff"] = json!({
            "summary": diff_result.summary,
            "changes": diff_result.changes,
            "grouped_changes": grouped_changes,
            "before_timestamp": diff_result.before_snapshot.timestamp.to_rfc3339(),
            "after_timestamp": diff_result.after_snapshot.timestamp.to_rfc3339(),
            "colored_output": diff_result.format_colored(),
            "unexpected_changes_count": diff_result.unexpected_changes().len(),
        });
    }

    // Add semantic analysis information if available
    if let Some(semantic_result) = semantic_info {
        response["semantic_analysis"] = json!({
            "explanations": semantic_result.explanations,
            "suggestions": semantic_result.suggestions,
            "is_semantic_query": true
        });
    } else {
        response["semantic_analysis"] = json!({
            "is_semantic_query": false
        });
    }

    Ok(response)
}

/// Get query cache statistics
pub async fn get_cache_stats() -> Value {
    let state = get_observe_state();
    let state_guard = state.read().await;
    let stats = state_guard.cache.stats();
    json!(stats)
}

/// Clear query cache
pub async fn clear_cache() {
    let state = get_observe_state();
    let mut state_guard = state.write().await;
    *state_guard = ObserveState::new().expect("Default observe state should initialize successfully");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[tokio::test]
    async fn test_observe_query_parsing() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));

        let args = json!({"query": "list all entities"});
        let result = handle(args, brp_client).await.unwrap();

        // Should return error since BRP client is not connected
        assert!(result.get("error").is_some());
    }

    #[tokio::test]
    async fn test_invalid_query() {
        let config = Config {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3000,
        };
        let brp_client = Arc::new(RwLock::new(crate::brp_client::BrpClient::new(&config)));

        let args = json!({"query": "invalid query syntax"});
        let result = handle(args, brp_client).await.unwrap();

        assert_eq!(result.get("error").unwrap(), "Query parsing failed");
        assert!(result.get("help").is_some());
    }

    #[tokio::test]
    async fn test_cache_stats() {
        let stats = get_cache_stats().await;
        assert!(stats.get("total_entries").is_some());
    }
}

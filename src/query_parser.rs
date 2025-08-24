use crate::brp_messages::{BrpRequest, QueryFilter};
use crate::error::{Error, Result};
use crate::semantic_analyzer::{SemanticAnalyzer, SemanticQueryResult};
/// Query parsing for natural language to BRP conversion
use regex::Regex;
use std::collections::HashMap;

/// Trait for parsing natural language queries into BRP requests
pub trait QueryParser {
    /// Parse a natural language query into a BRP request
    ///
    /// # Errors
    /// Returns error if query cannot be parsed or is invalid
    fn parse(&self, query: &str) -> Result<BrpRequest>;

    /// Parse a semantic query and return detailed results
    ///
    /// # Errors
    /// Returns error if query cannot be parsed or analyzed
    fn parse_semantic(&self, query: &str) -> Result<SemanticQueryResult>;

    /// Get help text for supported query patterns
    fn help(&self) -> &str;
}

/// Basic pattern-based query parser using regex
pub struct RegexQueryParser {
    patterns: Vec<QueryPattern>,
    semantic_analyzer: SemanticAnalyzer,
}

struct QueryPattern {
    pattern: Regex,
    builder: fn(&regex::Captures) -> Result<BrpRequest>,
    description: &'static str,
}

impl RegexQueryParser {
    /// Create a new regex-based query parser
    /// 
    /// # Errors
    /// Returns error if any regex pattern fails to compile
    pub fn new() -> Result<Self> {
        let patterns = vec![
            // List all entities
            QueryPattern {
                pattern: Regex::new(r"^(?i)list\s+all\s+entities?$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |_| Ok(BrpRequest::ListEntities { filter: None }),
                description: "list all entities - List all entities in the game",
            },

            // Show specific entity
            QueryPattern {
                pattern: Regex::new(r"^(?i)show\s+entity\s+(\d+)$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |caps| {
                    let entity_id = caps[1].parse::<u64>()
                        .map_err(|_| Error::Brp("Invalid entity ID".to_string()))?;
                    Ok(BrpRequest::Get {
                        entity: entity_id,
                        components: None,
                    })
                },
                description: "show entity X - Show details for entity with ID X",
            },

            // Find entities with component
            QueryPattern {
                pattern: Regex::new(r"^(?i)find\s+entities\s+with\s+component\s+([a-zA-Z_:]+)$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |caps| {
                    let component_type = caps[1].to_string();
                    Ok(BrpRequest::Query {
                        filter: Some(QueryFilter {
                            with: Some(vec![component_type]),
                            without: None,
                            where_clause: None,
                        }),
                        limit: None,
                        strict: Some(false),
                    })
                },
                description: "find entities with component Y - Find all entities that have component Y",
            },

            // Find entities without component
            QueryPattern {
                pattern: Regex::new(r"^(?i)find\s+entities\s+without\s+component\s+([a-zA-Z_:]+)$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |caps| {
                    let component_type = caps[1].to_string();
                    Ok(BrpRequest::Query {
                        filter: Some(QueryFilter {
                            with: None,
                            without: Some(vec![component_type]),
                            where_clause: None,
                        }),
                        limit: None,
                        strict: Some(false),
                    })
                },
                description: "find entities without component Y - Find all entities that don't have component Y",
            },

            // List component types
            QueryPattern {
                pattern: Regex::new(r"^(?i)list\s+components?$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |_| Ok(BrpRequest::ListComponents),
                description: "list components - List all available component types",
            },

            // Find entities with multiple components
            QueryPattern {
                pattern: Regex::new(r"^(?i)find\s+entities\s+with\s+(?:components?\s+)?([a-zA-Z_:,\s]+)$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |caps| {
                    let components_str = &caps[1];
                    let components: Vec<String> = components_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if components.is_empty() {
                        return Err(Error::Brp("No components specified".to_string()));
                    }

                    Ok(BrpRequest::Query {
                        filter: Some(QueryFilter {
                            with: Some(components),
                            without: None,
                            where_clause: None,
                        }),
                        limit: None,
                        strict: Some(false),
                    })
                },
                description: "find entities with A, B, C - Find entities that have all specified components",
            },

            // Get entity with specific components only
            QueryPattern {
                pattern: Regex::new(r"^(?i)show\s+entity\s+(\d+)\s+components?\s+([a-zA-Z_:,\s]+)$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |caps| {
                    let entity_id = caps[1].parse::<u64>()
                        .map_err(|_| Error::Brp("Invalid entity ID".to_string()))?;
                    let components_str = &caps[2];
                    let components: Vec<String> = components_str
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    Ok(BrpRequest::Get {
                        entity: entity_id,
                        components: if components.is_empty() { None } else { Some(components) },
                    })
                },
                description: "show entity X components A, B - Show specific components of entity X",
            },

            // Query with limit
            QueryPattern {
                pattern: Regex::new(r"^(?i)find\s+(\d+)\s+entities\s+with\s+component\s+([a-zA-Z_:]+)$")
                    .map_err(|e| Error::Validation(format!("Invalid regex pattern: {}", e)))?,
                builder: |caps| {
                    let limit = caps[1].parse::<usize>()
                        .map_err(|_| Error::Brp("Invalid limit".to_string()))?;
                    let component_type = caps[2].to_string();
                    Ok(BrpRequest::Query {
                        filter: Some(QueryFilter {
                            with: Some(vec![component_type]),
                            without: None,
                            where_clause: None,
                        }),
                        limit: Some(limit),
                        strict: Some(false),
                    })
                },
                description: "find N entities with component Y - Find up to N entities with component Y",
            },
        ];

        Ok(Self {
            patterns,
            semantic_analyzer: SemanticAnalyzer::new()?,
        })
    }
}

impl Default for RegexQueryParser {
    fn default() -> Self {
        Self::new().expect("Default regex patterns should be valid")
    }
}

impl QueryParser for RegexQueryParser {
    fn parse(&self, query: &str) -> Result<BrpRequest> {
        let query = query.trim();

        if query.is_empty() {
            return Err(Error::Brp("Empty query".to_string()));
        }

        // Try semantic analysis first
        if let Ok(semantic_result) = self.semantic_analyzer.analyze(query) {
            return Ok(semantic_result.request);
        }

        // Fall back to basic pattern matching
        for pattern in &self.patterns {
            if let Some(captures) = pattern.pattern.captures(query) {
                return (pattern.builder)(&captures);
            }
        }

        Err(Error::Brp(format!(
            "Unrecognized query pattern: '{}'. {}",
            query,
            self.help()
        )))
    }

    fn parse_semantic(&self, query: &str) -> Result<SemanticQueryResult> {
        self.semantic_analyzer.analyze(query)
    }

    fn help(&self) -> &str {
        "Supported query patterns:\n\
        Basic queries:\n\
        - list all entities\n\
        - show entity X\n\
        - find entities with component Y\n\
        - find entities without component Y\n\
        - find entities with A, B, C\n\
        - show entity X components A, B\n\
        - find N entities with component Y\n\
        - list components\n\
        \n\
        Semantic queries:\n\
        - find stuck entities\n\
        - show fast moving objects\n\
        - find overlapping colliders\n\
        - find memory leaks\n\
        - find inconsistent state\n\
        - find physics violations\n\
        - find stuck and fast entities (compound)"
    }
}

/// Query execution metrics
#[derive(Debug, Clone)]
pub struct QueryMetrics {
    pub query: String,
    pub execution_time_ms: u64,
    pub entity_count: usize,
    pub cache_hit: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Query result with metadata
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub result: serde_json::Value,
    pub metrics: QueryMetrics,
}

/// Cached query result
#[derive(Debug, Clone)]
struct CachedResult {
    result: serde_json::Value,
    timestamp: std::time::Instant,
    entity_count: usize,
}

/// Query cache with TTL support
pub struct QueryCache {
    cache: dashmap::DashMap<String, CachedResult>,
    ttl_seconds: u64,
}

impl QueryCache {
    /// Create a new query cache with specified TTL
    #[must_use]
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            cache: dashmap::DashMap::new(),
            ttl_seconds,
        }
    }

    /// Get cached result if available and not expired
    pub fn get(&self, query: &str) -> Option<(serde_json::Value, usize)> {
        let entry = self.cache.get(query)?;

        if entry.timestamp.elapsed().as_secs() > self.ttl_seconds {
            drop(entry);
            self.cache.remove(query);
            return None;
        }

        Some((entry.result.clone(), entry.entity_count))
    }

    /// Cache a query result
    pub fn set(&self, query: String, result: serde_json::Value, entity_count: usize) {
        self.cache.insert(
            query,
            CachedResult {
                result,
                timestamp: std::time::Instant::now(),
                entity_count,
            },
        );
    }

    /// Clear expired entries from cache
    pub fn cleanup(&self) {
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(self.ttl_seconds);
        self.cache.retain(|_, entry| entry.timestamp > cutoff);
    }

    /// Get cache statistics
    pub fn stats(&self) -> HashMap<String, usize> {
        let mut stats = HashMap::new();
        stats.insert("total_entries".to_string(), self.cache.len());

        let expired_count = self
            .cache
            .iter()
            .filter(|entry| entry.timestamp.elapsed().as_secs() > self.ttl_seconds)
            .count();
        stats.insert("expired_entries".to_string(), expired_count);

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_query_parsing() {
        let parser = RegexQueryParser::new().unwrap();

        // Test list all entities
        let result = parser.parse("list all entities").unwrap();
        matches!(result, BrpRequest::ListEntities { .. });

        // Test show entity
        let result = parser.parse("show entity 123").unwrap();
        if let BrpRequest::Get { entity, .. } = result {
            assert_eq!(entity, 123);
        } else {
            panic!("Expected Get request");
        }

        // Test find with component
        let result = parser
            .parse("find entities with component Transform")
            .unwrap();
        if let BrpRequest::Query {
            filter: Some(filter),
            ..
        } = result
        {
            assert_eq!(filter.with, Some(vec!["Transform".to_string()]));
        } else {
            panic!("Expected Query request");
        }
    }

    #[test]
    fn test_case_insensitive_parsing() {
        let parser = RegexQueryParser::new().unwrap();

        let result1 = parser.parse("LIST ALL ENTITIES").unwrap();
        let result2 = parser.parse("list all entities").unwrap();

        matches!(result1, BrpRequest::ListEntities { .. });
        matches!(result2, BrpRequest::ListEntities { .. });
    }

    #[test]
    fn test_invalid_queries() {
        let parser = RegexQueryParser::new().unwrap();

        assert!(parser.parse("").is_err());
        assert!(parser.parse("invalid query").is_err());
        assert!(parser.parse("show entity abc").is_err()); // Invalid entity ID
    }

    #[test]
    fn test_query_cache() {
        let cache = QueryCache::new(300); // 5 minute TTL

        let query = "list all entities";
        let result = serde_json::json!({"entities": []});

        // Cache miss
        assert!(cache.get(query).is_none());

        // Cache set and hit
        cache.set(query.to_string(), result.clone(), 0);
        let cached = cache.get(query).unwrap();
        assert_eq!(cached.0, result);
        assert_eq!(cached.1, 0);
    }

    #[test]
    fn test_multiple_components_query() {
        let parser = RegexQueryParser::new().unwrap();

        let result = parser
            .parse("find entities with Transform, Velocity, Health")
            .unwrap();
        if let BrpRequest::Query {
            filter: Some(filter),
            ..
        } = result
        {
            let expected = vec![
                "Transform".to_string(),
                "Velocity".to_string(),
                "Health".to_string(),
            ];
            assert_eq!(filter.with, Some(expected));
        } else {
            panic!("Expected Query request");
        }
    }
}

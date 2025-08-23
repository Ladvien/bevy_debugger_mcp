/*
 * Bevy Debugger MCP Server - Query Optimization Guide
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

//! # Bevy ECS Query Optimization Guide
//!
//! This module provides comprehensive guidance on optimizing ECS queries for Bevy games
//! using the Bevy Debugger MCP server. The optimizations focus on Bevy's archetype-based
//! storage system and provide 10x+ performance improvements for large worlds.
//!
//! ## Performance Targets (BEVDBG-014)
//!
//! - **10x improvement** for large worlds (>10k entities)
//! - **QueryState caching** to avoid repeated query builds
//! - **Parallel iteration** using Bevy's par_iter patterns
//! - **Archetype-aware** query optimizations
//! - **Performance metrics** tracking and analysis
//!
//! ## Key Concepts
//!
//! ### Archetype-Based Storage
//!
//! Bevy uses an archetype-based ECS where entities with the same component set
//! are stored together in memory. This enables:
//!
//! - **Cache-friendly access patterns**: Components are stored contiguously
//! - **Efficient filtering**: Skip entire archetypes that don't match
//! - **Vectorized operations**: Process multiple entities simultaneously
//!
//! ### QueryState Caching
//!
//! Instead of rebuilding queries every time:
//!
//! ```rust,ignore
//! // ❌ BAD: Rebuilds query state every call
//! fn observe_entities(world: &World) {
//!     for entity in world.query::<&Transform>().iter() {
//!         // Process entity
//!     }
//! }
//!
//! // ✅ GOOD: Cache and reuse query state
//! fn observe_entities_optimized(world: &World, query_state: &mut QueryState<&Transform>) {
//!     for entity in query_state.iter(world) {
//!         // Process entity
//!     }
//! }
//! ```
//!
//! ### Parallel Processing
//!
//! For large entity sets, use parallel iteration:
//!
//! ```rust,ignore
//! // ✅ GOOD: Parallel processing for large datasets
//! fn process_large_dataset(world: &World, query_state: &mut QueryState<(&Transform, &mut Velocity)>) {
//!     query_state.par_for_each_mut(world, 32, |(transform, mut velocity)| {
//!         // Process entities in parallel batches of 32
//!         velocity.linear += transform.translation.normalize() * 0.1;
//!     });
//! }
//! ```

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Best practices for Bevy ECS query optimization
#[derive(Debug, Clone, Serialize)]
pub struct QueryOptimizationGuide {
    /// General optimization principles
    pub general_principles: Vec<OptimizationPrinciple>,
    /// Archetype-specific optimizations
    pub archetype_optimizations: Vec<ArchetypeOptimization>,
    /// Parallel processing guidelines
    pub parallel_guidelines: Vec<ParallelGuideline>,
    /// Performance anti-patterns to avoid
    pub anti_patterns: Vec<AntiPattern>,
    /// Measurement and profiling advice
    pub profiling_advice: Vec<ProfilingTip>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationPrinciple {
    pub title: String,
    pub description: String,
    pub example_good: String,
    pub example_bad: String,
    pub impact_level: ImpactLevel,
}

#[derive(Debug, Clone, Serialize)]
pub struct ArchetypeOptimization {
    pub scenario: String,
    pub technique: String,
    pub code_example: String,
    pub expected_speedup: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParallelGuideline {
    pub entity_threshold: usize,
    pub batch_size_recommendation: String,
    pub use_case: String,
    pub considerations: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AntiPattern {
    pub pattern_name: String,
    pub why_bad: String,
    pub common_occurrence: String,
    pub solution: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfilingTip {
    pub metric: String,
    pub how_to_measure: String,
    pub good_values: String,
    pub warning_signs: String,
}

#[derive(Debug, Clone, Serialize)]
pub enum ImpactLevel {
    Low,    // <2x improvement
    Medium, // 2-5x improvement
    High,   // 5-10x improvement
    Critical, // >10x improvement
}

impl QueryOptimizationGuide {
    /// Create the comprehensive query optimization guide
    pub fn new() -> Self {
        Self {
            general_principles: vec![
                OptimizationPrinciple {
                    title: "Cache QueryState Objects".to_string(),
                    description: "Reuse QueryState objects instead of creating new ones for each query".to_string(),
                    example_good: r#"
struct ObserveSystem {
    transform_query: QueryState<&Transform>,
}

impl ObserveSystem {
    fn observe_transforms(&mut self, world: &World) {
        for transform in self.transform_query.iter(world) {
            // Process efficiently
        }
    }
}"#.to_string(),
                    example_bad: r#"
fn observe_transforms(world: &World) {
    // Rebuilds query state every call - expensive!
    for transform in world.query::<&Transform>().iter() {
        // Process inefficiently
    }
}"#.to_string(),
                    impact_level: ImpactLevel::High,
                },
                OptimizationPrinciple {
                    title: "Minimize Component Sets in Queries".to_string(),
                    description: "Only query for components you actually need to reduce archetype misses".to_string(),
                    example_good: r#"
// Only query what you need
for (transform, velocity) in world.query::<(&Transform, &Velocity)>().iter() {
    // Process minimal component set
}
"#.to_string(),
                    example_bad: r#"
// Querying unnecessary components reduces matching archetypes
for (transform, velocity, health, name, mesh) in world.query::<(&Transform, &Velocity, &Health, &Name, &Handle<Mesh>)>().iter() {
    // Only using transform and velocity!
}
"#.to_string(),
                    impact_level: ImpactLevel::Medium,
                },
                OptimizationPrinciple {
                    title: "Use Parallel Iteration for Large Sets".to_string(),
                    description: "Use par_for_each for CPU-intensive operations on large entity sets".to_string(),
                    example_good: r#"
query_state.par_for_each_mut(world, 64, |(mut transform, velocity)| {
    // Parallel processing with batch size 64
    transform.translation += velocity.linear * delta_time;
});
"#.to_string(),
                    example_bad: r#"
for (mut transform, velocity) in query_state.iter_mut(world) {
    // Sequential processing - slow for large datasets
    transform.translation += velocity.linear * delta_time;
}
"#.to_string(),
                    impact_level: ImpactLevel::Critical,
                },
                OptimizationPrinciple {
                    title: "Leverage Without Filters Strategically".to_string(),
                    description: "Use Without<Component> to efficiently exclude large archetype groups".to_string(),
                    example_good: r#"
// Efficiently skip inactive entities
for entity in world.query_filtered::<Entity, Without<Inactive>>().iter() {
    // Process only active entities
}
"#.to_string(),
                    example_bad: r#"
for (entity, active) in world.query::<(Entity, Option<&Active>)>().iter() {
    if active.is_some() {  // Runtime filtering - inefficient
        // Process entity
    }
}
"#.to_string(),
                    impact_level: ImpactLevel::High,
                },
            ],
            archetype_optimizations: vec![
                ArchetypeOptimization {
                    scenario: "Large worlds with many entity types".to_string(),
                    technique: "Archetype-aware batching".to_string(),
                    code_example: r#"
// Group queries by expected archetype count
let simple_entities = world.query::<&Transform>(); // Many archetypes
let complex_entities = world.query::<(&Transform, &Velocity, &Health)>(); // Few archetypes

// Process high-archetype-count queries with smaller batches
simple_entities.par_for_each(world, 32, |transform| { /* ... */ });
complex_entities.par_for_each(world, 128, |(transform, velocity, health)| { /* ... */ });
"#.to_string(),
                    expected_speedup: "5-15x for mixed entity type worlds".to_string(),
                },
                ArchetypeOptimization {
                    scenario: "Sparse component queries".to_string(),
                    technique: "Component existence pre-filtering".to_string(),
                    code_example: r#"
// Check component existence before expensive queries
if world.component_archetype_count::<RareComponent>() > threshold {
    for entity in world.query::<(Entity, &RareComponent)>().iter() {
        // Only run expensive query if enough entities have the component
    }
}
"#.to_string(),
                    expected_speedup: "10-50x for sparse components (<1% of entities)".to_string(),
                },
                ArchetypeOptimization {
                    scenario: "Hierarchical entity queries".to_string(),
                    technique: "Parent-child optimization".to_string(),
                    code_example: r#"
// Process parents first, then children in batch
let parents: Vec<Entity> = world.query_filtered::<Entity, (With<Children>, Without<Parent>)>()
    .iter().collect();

for parent in parents {
    // Process parent
    let children = world.query_filtered::<Entity, With<Parent>>()
        .iter().filter(|child| /* child belongs to parent */)
        .collect::<Vec<_>>();
    // Batch process children
}
"#.to_string(),
                    expected_speedup: "3-8x for hierarchical scenes".to_string(),
                },
            ],
            parallel_guidelines: vec![
                ParallelGuideline {
                    entity_threshold: 1000,
                    batch_size_recommendation: "32-64 entities per batch".to_string(),
                    use_case: "Transform updates, simple component modifications".to_string(),
                    considerations: vec![
                        "Good cache locality".to_string(),
                        "Low per-entity computation cost".to_string(),
                        "Memory access patterns are sequential".to_string(),
                    ],
                },
                ParallelGuideline {
                    entity_threshold: 500,
                    batch_size_recommendation: "64-128 entities per batch".to_string(),
                    use_case: "Physics calculations, complex math operations".to_string(),
                    considerations: vec![
                        "Higher per-entity computation cost".to_string(),
                        "May require random memory access".to_string(),
                        "Thread synchronization for shared resources".to_string(),
                    ],
                },
                ParallelGuideline {
                    entity_threshold: 10000,
                    batch_size_recommendation: "128-256 entities per batch".to_string(),
                    use_case: "AI pathfinding, large-scale simulations".to_string(),
                    considerations: vec![
                        "Very high per-entity computation cost".to_string(),
                        "May benefit from work-stealing schedulers".to_string(),
                        "Consider NUMA topology for very large datasets".to_string(),
                    ],
                },
            ],
            anti_patterns: vec![
                AntiPattern {
                    pattern_name: "Query in Hot Loops".to_string(),
                    why_bad: "Creates new QueryState objects repeatedly, causing memory allocations and cache misses".to_string(),
                    common_occurrence: "Inside system update loops, event handlers".to_string(),
                    solution: "Move query creation outside the loop, cache QueryState objects".to_string(),
                },
                AntiPattern {
                    pattern_name: "Over-Generic Queries".to_string(),
                    why_bad: "Queries that match too many archetypes reduce filtering efficiency".to_string(),
                    common_occurrence: "Debugging queries that fetch all components".to_string(),
                    solution: "Create specific queries for each use case, use component-specific filters".to_string(),
                },
                AntiPattern {
                    pattern_name: "Synchronous Processing of Large Sets".to_string(),
                    why_bad: "Single-threaded processing leaves CPU cores idle".to_string(),
                    common_occurrence: "Entity cleanup, batch modifications".to_string(),
                    solution: "Use par_for_each for CPU-bound operations, implement work batching".to_string(),
                },
                AntiPattern {
                    pattern_name: "Inefficient Component Combinations".to_string(),
                    why_bad: "Queries combining common and rare components create archetype fragmentation".to_string(),
                    common_occurrence: "Queries mixing Transform (common) with rare components".to_string(),
                    solution: "Split queries, process common components first, then filter rare ones".to_string(),
                },
            ],
            profiling_advice: vec![
                ProfilingTip {
                    metric: "Query Execution Time".to_string(),
                    how_to_measure: "Use QueryPerformanceMetrics::execution_time_ms".to_string(),
                    good_values: "<1ms for <1k entities, <10ms for <10k entities".to_string(),
                    warning_signs: ">50ms for any query, linear scaling with entity count".to_string(),
                },
                ProfilingTip {
                    metric: "Cache Hit Rate".to_string(),
                    how_to_measure: "Monitor QueryOptimizer cache statistics".to_string(),
                    good_values: ">80% cache hit rate for repeated queries".to_string(),
                    warning_signs: "<50% cache hit rate, frequent cache evictions".to_string(),
                },
                ProfilingTip {
                    metric: "Parallel Speedup".to_string(),
                    how_to_measure: "Compare sequential vs parallel execution time".to_string(),
                    good_values: "5-8x speedup on 8-core systems for CPU-bound tasks".to_string(),
                    warning_signs: "<2x speedup, negative scaling with thread count".to_string(),
                },
                ProfilingTip {
                    metric: "Memory Usage".to_string(),
                    how_to_measure: "Track QueryExecutionResult::memory_usage_bytes".to_string(),
                    good_values: "<100 bytes per entity for simple queries".to_string(),
                    warning_signs: ">1KB per entity, exponential growth with query complexity".to_string(),
                },
                ProfilingTip {
                    metric: "Archetype Fragmentation".to_string(),
                    how_to_measure: "Monitor ArchetypeAccessPattern::matching_archetypes".to_string(),
                    good_values: "<10 archetypes for simple queries, <100 for complex ones".to_string(),
                    warning_signs: ">1000 matching archetypes, linear growth with world size".to_string(),
                },
            ],
        }
    }

    /// Get optimization recommendations for a specific scenario
    pub fn get_recommendations(&self, entity_count: usize, query_complexity: QueryComplexity) -> OptimizationRecommendation {
        let should_use_parallel = entity_count >= 1000;
        let batch_size = match query_complexity {
            QueryComplexity::Simple => 64,
            QueryComplexity::Medium => 128,
            QueryComplexity::Complex => 256,
        };

        let recommended_techniques = match (entity_count, query_complexity) {
            (count, QueryComplexity::Simple) if count > 10000 => vec![
                "Use parallel iteration with small batch sizes".to_string(),
                "Enable QueryState caching".to_string(),
                "Consider archetype pre-filtering".to_string(),
            ],
            (count, QueryComplexity::Medium) if count > 5000 => vec![
                "Use parallel iteration with medium batch sizes".to_string(),
                "Split complex queries into simpler ones".to_string(),
                "Enable aggressive caching".to_string(),
            ],
            (count, QueryComplexity::Complex) if count > 1000 => vec![
                "Use parallel iteration with large batch sizes".to_string(),
                "Consider work-stealing for load balancing".to_string(),
                "Profile memory access patterns".to_string(),
            ],
            _ => vec![
                "Use sequential processing".to_string(),
                "Focus on cache-friendly access patterns".to_string(),
            ],
        };

        OptimizationRecommendation {
            should_use_parallel,
            recommended_batch_size: batch_size,
            expected_speedup_range: self.estimate_speedup(entity_count, query_complexity),
            techniques: recommended_techniques,
            monitoring_points: vec![
                "Track query execution time".to_string(),
                "Monitor cache hit rate".to_string(),
                "Measure memory usage per entity".to_string(),
            ],
        }
    }

    fn estimate_speedup(&self, entity_count: usize, complexity: QueryComplexity) -> String {
        match (entity_count, complexity) {
            (count, QueryComplexity::Simple) if count > 10000 => "8-15x".to_string(),
            (count, QueryComplexity::Medium) if count > 5000 => "5-12x".to_string(),
            (count, QueryComplexity::Complex) if count > 1000 => "3-8x".to_string(),
            _ => "1-3x".to_string(),
        }
    }

    /// Generate a performance audit report
    pub fn audit_performance(&self, metrics: &[crate::query_optimization::QueryPerformanceMetrics]) -> PerformanceAudit {
        if metrics.is_empty() {
            return PerformanceAudit::empty();
        }

        let avg_execution_time = metrics.iter()
            .map(|m| m.execution_time_ms as f64)
            .sum::<f64>() / metrics.len() as f64;

        let parallel_usage_rate = metrics.iter()
            .filter(|m| m.parallel_processing)
            .count() as f64 / metrics.len() as f64;

        let cache_hit_rate = metrics.iter()
            .filter(|m| m.cache_hit)
            .count() as f64 / metrics.len() as f64;

        let issues = self.identify_performance_issues(avg_execution_time, parallel_usage_rate, cache_hit_rate);
        let recommendations = self.generate_audit_recommendations(&issues);

        PerformanceAudit {
            average_execution_time_ms: avg_execution_time,
            parallel_usage_rate,
            cache_hit_rate,
            identified_issues: issues,
            recommendations,
            overall_score: self.calculate_performance_score(avg_execution_time, parallel_usage_rate, cache_hit_rate),
        }
    }

    fn identify_performance_issues(&self, avg_time: f64, parallel_rate: f64, cache_rate: f64) -> Vec<String> {
        let mut issues = Vec::new();

        if avg_time > 50.0 {
            issues.push("High average execution time".to_string());
        }
        if parallel_rate < 0.3 {
            issues.push("Low parallel processing usage".to_string());
        }
        if cache_rate < 0.5 {
            issues.push("Poor cache hit rate".to_string());
        }

        issues
    }

    fn generate_audit_recommendations(&self, issues: &[String]) -> Vec<String> {
        let mut recommendations = Vec::new();

        for issue in issues {
            match issue.as_str() {
                "High average execution time" => {
                    recommendations.push("Consider using parallel processing for large queries".to_string());
                    recommendations.push("Review query complexity and component combinations".to_string());
                }
                "Low parallel processing usage" => {
                    recommendations.push("Enable parallel processing for queries with >1000 entities".to_string());
                    recommendations.push("Tune batch sizes for optimal performance".to_string());
                }
                "Poor cache hit rate" => {
                    recommendations.push("Increase QueryOptimizer cache size".to_string());
                    recommendations.push("Review query patterns for caching opportunities".to_string());
                }
                _ => {}
            }
        }

        recommendations
    }

    fn calculate_performance_score(&self, avg_time: f64, parallel_rate: f64, cache_rate: f64) -> f64 {
        let time_score = (100.0 - avg_time.min(100.0)) / 100.0;
        let parallel_score = parallel_rate;
        let cache_score = cache_rate;

        (time_score + parallel_score + cache_score) / 3.0
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum QueryComplexity {
    Simple,  // 1-2 components
    Medium,  // 3-5 components
    Complex, // 6+ components or complex filters
}

#[derive(Debug, Clone, Serialize)]
pub struct OptimizationRecommendation {
    pub should_use_parallel: bool,
    pub recommended_batch_size: usize,
    pub expected_speedup_range: String,
    pub techniques: Vec<String>,
    pub monitoring_points: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerformanceAudit {
    pub average_execution_time_ms: f64,
    pub parallel_usage_rate: f64,
    pub cache_hit_rate: f64,
    pub identified_issues: Vec<String>,
    pub recommendations: Vec<String>,
    pub overall_score: f64, // 0.0 to 1.0
}

impl PerformanceAudit {
    fn empty() -> Self {
        Self {
            average_execution_time_ms: 0.0,
            parallel_usage_rate: 0.0,
            cache_hit_rate: 0.0,
            identified_issues: vec!["No performance data available".to_string()],
            recommendations: vec!["Collect performance metrics first".to_string()],
            overall_score: 0.0,
        }
    }
}

impl Default for QueryOptimizationGuide {
    fn default() -> Self {
        Self::new()
    }
}
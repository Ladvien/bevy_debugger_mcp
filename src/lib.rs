/*
 * Bevy Debugger MCP Server - Library
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

//! # Bevy Debugger MCP
//!
//! AI-assisted debugging for Bevy games through Claude Code using Model Context Protocol.
//!
//! This crate provides a comprehensive debugging toolkit for Bevy games, enabling natural
//! language interaction with game state through Claude Code. It bridges the gap between
//! AI assistance and game development by offering intelligent observation, experimentation,
//! and analysis capabilities.
//!
//! ## Quick Start
//!
//! 1. **Add BRP to your Bevy game:**
//! ```toml
//! [dependencies]
//! bevy = { version = "0.12", features = ["bevy_remote_protocol"] }
//! ```
//!
//! 2. **Enable BRP in your game:**
//! ```rust,no_run
//! use bevy::prelude::*;
//! use bevy::remote::RemotePlugin;
//!
//! fn main() {
//!     App::new()
//!         .add_plugins(DefaultPlugins)
//!         .add_plugins(RemotePlugin::default())
//!         .run();
//! }
//! ```
//!
//! 3. **Install and configure the MCP server:**
//! ```bash
//! cargo install bevy_debugger_mcp
//! bevy-debugger-mcp --help
//! ```
//!
//! 4. **Configure Claude Code:**
//! ```json
//! {
//!   "mcpServers": {
//!     "bevy-debugger-mcp": {
//!       "command": "bevy-debugger-mcp",
//!       "args": [],
//!       "type": "stdio"
//!     }
//!   }
//! }
//! ```
//!
//! ## Features
//!
//! - **🔍 Natural Language Queries**: Ask about your game state in plain English
//! - **🧪 Controlled Experiments**: Test hypotheses with systematic variations
//! - **📊 Performance Analysis**: Monitor and analyze game performance in real-time
//! - **⏱️ Time-Travel Debugging**: Record, replay, and branch timelines
//! - **🚨 Anomaly Detection**: Automatically detect unusual patterns in game behavior
//! - **📈 Stress Testing**: Systematically test your game under various loads
//! - **🔧 Tool Orchestration**: Chain debugging operations into complex workflows
//!
//! ## Core Modules
//!
//! ### Observation and Query
//! - [`query_parser`] - Natural language to game queries
//! - [`semantic_analyzer`] - Semantic understanding of game concepts
//! - [`brp_client`] - Bevy Remote Protocol communication
//!
//! ### Experimentation and Testing
//! - [`experiment_system`] - Controlled game state experiments
//! - [`hypothesis_system`] - Property-based testing for games
//! - [`stress_test_system`] - Systematic stress testing
//!
//! ### Time and State Management
//! - [`recording_system`] - Game state recording and persistence
//! - [`playback_system`] - Deterministic replay capabilities
//! - [`timeline_branching`] - Multi-timeline debugging with branching
//! - [`checkpoint`] - Save/restore debugging sessions
//!
//! ### Analysis and Monitoring
//! - [`anomaly_detector`] - Pattern recognition for unusual behavior
//! - [`state_diff`] - Intelligent state comparison and diffing
//! - [`diagnostics`] - System health monitoring and reporting
//!
//! ### Infrastructure
//! - [`mcp_server`] - Model Context Protocol server implementation
//! - [`tool_orchestration`] - Complex debugging workflow coordination
//! - [`error`] - Comprehensive error handling and recovery
//! - [`config`] - Configuration management
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use bevy_debugger_mcp::prelude::*;
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Connect to your Bevy game
//! let mut client = BrpClient::connect("ws://localhost:15702").await?;
//!
//! // Parse natural language queries
//! let parser = RegexQueryParser::new();
//! let request = parser.parse("find entities with Transform and Velocity")?;
//!
//! // Execute the query
//! let response = client.send_request(request).await?;
//! println!("Found entities: {:?}", response);
//! # Ok(())
//! # }
//! ```
//!
//! ## Architecture
//!
//! The system follows a modular architecture:
//!
//! ```text
//! Claude Code ←→ MCP Protocol ←→ MCP Server ←→ BRP Protocol ←→ Bevy Game
//!                                    ↓
//!                              Tool Modules
//!                           (observe, experiment,
//!                            stress, replay, etc.)
//! ```
//!
//! ## Error Handling
//!
//! All operations return [`Result<T, Error>`](error::Error) types. The system includes
//! comprehensive error recovery with automatic reconnection, dead letter queues for
//! failed operations, and checkpoint/restore capabilities.

// Re-export commonly used types
pub mod prelude {
    //! Common imports for typical usage
    pub use crate::brp_client::BrpClient;
    pub use crate::brp_messages::{BrpRequest, BrpResponse};
    pub use crate::error::{Error, Result};
    pub use crate::query_parser::{QueryParser, RegexQueryParser};
    pub use crate::query_builder::{QueryBuilder, QueryValidator, QueryCostEstimator, QueryOptimizer};
    pub use crate::query_builder_processor::QueryBuilderProcessor;
    pub use crate::mcp_server::McpServer;
    pub use crate::experiment_system::{Action, ActionExecutor, ActionResult};
    pub use crate::hypothesis_system::{Hypothesis, TestRunner, TestResult};
    pub use crate::timeline_branching::{TimelineBranchManager, BranchId};
    pub use crate::recording_system::{Recording, RecordingState};
    pub use crate::playback_system::PlaybackState;
    pub use crate::session_manager::{SessionManager, SessionManagerConfig, DebugSession};
    pub use crate::session_processor::SessionProcessor;
}

// Core functionality
pub mod error;
pub mod config;
pub mod circuit_breaker;
pub mod connection_pool;
pub mod heartbeat;

// Communication
pub mod brp_client;
pub mod brp_client_v2;
pub mod brp_command_handler;
pub mod brp_integration;
pub mod brp_messages;
pub mod brp_validation;
pub mod debug_brp_handler;
pub mod debug_command_processor;
pub mod entity_inspector;
pub mod mcp_server;
pub mod mcp_server_v2;
pub mod mcp_tools;
pub mod query_builder_processor;

// Performance profiling and visual debugging
pub mod system_profiler;
pub mod system_profiler_processor;
pub mod memory_profiler;
pub mod memory_profiler_processor;
pub mod visual_debug_overlay;
pub mod visual_debug_overlay_processor;

// Issue detection
pub mod issue_detector;
pub mod issue_detector_processor;

// Performance budget monitoring
pub mod performance_budget;
pub mod performance_budget_processor;

#[cfg(feature = "visual_overlays")]
pub mod visual_overlays;

// Query and observation
pub mod query_parser;
pub mod query_builder;
pub mod semantic_analyzer;

// Experimentation and testing
pub mod experiment_system;
pub mod hypothesis_system;
pub mod stress_test_system;

// State management
pub mod recording_system;
pub mod playback_system;
pub mod timeline_branching;
pub mod checkpoint;
pub mod state_diff;
pub mod session_manager;
pub mod session_processor;
pub mod replay_actor;

// Analysis and monitoring
pub mod anomaly_detector;
pub mod diagnostics;
pub mod resource_manager;

// Infrastructure
pub mod tool_orchestration;
pub mod dead_letter_queue;
pub mod lazy_init;
pub mod command_cache;
pub mod response_pool;
pub mod profiling;
pub mod compile_opts;
pub mod memory_optimization_tracker;
pub mod memory_pools;
pub mod deadlock_detector;
pub mod lock_contention_benchmark;
pub mod benchmark_runner;
pub mod brp_client_refactored;
pub mod tools;

// Panic prevention and reliability testing
#[cfg(test)]
pub mod panic_prevention_tests;

// Machine learning and pattern recognition (Epic BEVDBG-013)
pub mod pattern_learning;
pub mod suggestion_engine;
pub mod workflow_automation;
pub mod hot_reload;

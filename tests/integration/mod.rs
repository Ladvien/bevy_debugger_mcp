/// Integration test infrastructure for Bevy Debugger MCP
/// 
/// This module provides comprehensive testing infrastructure including:
/// - Test harness with mock clients
/// - Performance regression testing
/// - Command coverage validation
/// - Acceptance criteria verification

pub mod harness;

pub use harness::{
    IntegrationTestHarness,
    TestConfig,
    PerformanceMetrics,
    CommandCoverage,
    AcceptanceCriteria,
    MockBrpClient,
};
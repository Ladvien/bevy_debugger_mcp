/// Mock clients and services for integration testing
/// 
/// This module provides mock implementations of external services
/// to enable comprehensive testing without external dependencies.

pub mod mcp_client;

pub use mcp_client::{
    MockMcpClient,
    DebugSessionReport,
    SessionStats,
    ProtocolCompliance,
    ToolDefinition,
};
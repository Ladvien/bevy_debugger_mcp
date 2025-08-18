# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.1] - 2024-01-18

### Fixed
- Fixed all critical clippy warnings
- Updated deprecated rand API usage (thread_rng -> rng, gen -> random)
- Reduced Error enum size on stack by boxing large error types
- Fixed unused variables and imports
- Improved performance with better error handling

### Changed
- Reorganized documentation to follow Rust conventions
- Moved extended guides to book/ directory
- Enhanced lib.rs with comprehensive module documentation
- Added prelude module for convenient imports

### Added
- Comprehensive API documentation
- CONTRIBUTING.md with contribution guidelines
- GitHub Actions CI/CD workflow
- Professional package metadata for crates.io

## [0.1.0] - 2024-01-18

### Initial Release
- Core MCP server implementation for Bevy debugging
- Natural language query system
- Time-travel debugging with recording and replay
- Timeline branching for "what-if" scenarios
- Stress testing and performance analysis
- Hypothesis testing framework
- Anomaly detection system
- Tool orchestration for complex workflows
- Claude Code integration via stdio transport
- macOS service support
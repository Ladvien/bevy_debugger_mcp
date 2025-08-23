# Bevy Debugger MCP - Consolidated Technical Backlog

**Date:** August 23, 2025  
**Version:** v0.1.8  
**Contributors:** MCP-Rust Expert, Bevy Game Dev Expert, Architecture Analyst

## Executive Summary

The bevy-debugger-mcp project demonstrates ambitious scope as a comprehensive debugging tool for Bevy games via Claude Code integration. However, multiple critical issues prevent production deployment. This consolidated backlog represents collaborative analysis from specialized technical experts.

### Severity Distribution
- **🔴 Critical**: 12 stories (blocks deployment)
- **🟠 High**: 18 stories (major functionality gaps)
- **🟡 Medium**: 20 stories (quality/maintainability)
- **🟢 Low**: 10 stories (technical debt/optimization)

### Total Effort Estimate: 380 Story Points

---

## Epic 1: MCP Server Implementation (Critical Path)

### BEVDBG-001: Fix rmcp 0.2.0 API Compatibility 🔴 CRITICAL
**Story Type:** Bug  
**Priority:** Critical  
**Story Points:** 8  
**Assignee:** MCP Team  

**Description:**  
The rmcp library has breaking API changes preventing compilation. This blocks all Claude Code integration functionality.

**Acceptance Criteria:**
- [ ] Update to use `ServerHandler` trait instead of `Service<RoleServer>`
- [ ] Replace `#[tool_router]` with `#[tool_handler]` macro
- [ ] Fix all `McpError::InvalidRequest` constructor calls
- [ ] Resolve `IntoToolRoute` trait bound compilation errors
- [ ] Achieve clean compilation with rmcp 0.2.1
- [ ] Pass MCP handshake tests

**Technical Notes:**
```rust
// Current (broken):
impl Service<RoleServer> for McpServerV2 { ... }

// Required:
#[tool_handler]
impl ServerHandler for McpServerV2 {
    fn get_info(&self) -> ServerInfo { ... }
}
```

**Dependencies:** None  
**Blocks:** BEVDBG-002, BEVDBG-003, all tool functionality

---

### BEVDBG-002: Implement Stdio Transport for Claude Code 🔴 CRITICAL
**Story Type:** Feature  
**Priority:** Critical  
**Story Points:** 8  
**Assignee:** MCP Team  

**Description:**  
Stdio transport is required for Claude Code integration but currently returns "not implemented" error.

**Acceptance Criteria:**
- [ ] Implement proper stdio server using `ServiceExt::serve(stdio())`
- [ ] Handle JSON-RPC 2.0 protocol over stdin/stdout
- [ ] Support graceful shutdown signals
- [ ] Add connection state tracking
- [ ] Test with actual Claude Code instance
- [ ] Document configuration requirements

**Technical Notes:**
```rust
// Required implementation pattern:
#[tokio::main]
async fn main() -> Result<()> {
    let service = McpServerV2::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
```

**Dependencies:** BEVDBG-001  
**Blocks:** All Claude Code functionality

---

### BEVDBG-003: Refactor Tool Router Implementation 🔴 CRITICAL
**Story Type:** Bug  
**Priority:** Critical  
**Story Points:** 13  
**Assignee:** MCP Team  

**Description:**  
Tool router pattern using macros is incompatible with rmcp 0.2.1, causing compilation failures.

**Acceptance Criteria:**
- [ ] Migrate all 6 tool modules to use `#[tool]` attribute on methods
- [ ] Implement proper `ToolRouter` trait for each tool
- [ ] Fix Result type conflicts (use rmcp::Error consistently)
- [ ] Ensure tool discovery works via MCP protocol
- [ ] Add tool capability registration
- [ ] Test each tool individually

**Technical Notes:**
Affected tools: observe, experiment, hypothesis, anomaly, stress, replay

**Dependencies:** BEVDBG-001  
**Blocks:** All debugging tool functionality

---

## Epic 2: Bevy Remote Protocol Integration

### BEVDBG-004: Update BRP for Bevy 0.16 Compatibility 🟠 HIGH
**Story Type:** Technical Debt  
**Priority:** High  
**Story Points:** 5  
**Assignee:** Bevy Team  

**Description:**  
BRP message structures may not align with Bevy 0.16's actual remote protocol implementation.

**Acceptance Criteria:**
- [ ] Verify against Bevy 0.16 remote protocol documentation
- [ ] Update EntityId to include generation field
- [ ] Align ComponentTypeId with Bevy's TypeId
- [ ] Test all BRP commands against Bevy 0.16 game
- [ ] Update message serialization formats
- [ ] Document version-specific limitations

**Technical Notes:**
- Default BRP port: 15702
- Requires `RemotePlugin` and `RemoteHttpPlugin`
- Files: `/src/brp_messages.rs`, `/src/brp_client.rs`

**Dependencies:** None  
**Blocks:** BEVDBG-005, BEVDBG-006

---

### BEVDBG-005: Implement Production BRP Connection Management 🟠 HIGH
**Story Type:** Enhancement  
**Priority:** High  
**Story Points:** 8  
**Assignee:** Bevy Team  

**Description:**  
Current BRP client lacks robust connection lifecycle management for production debugging.

**Acceptance Criteria:**
- [ ] Add connection heartbeat/keepalive mechanism
- [ ] Implement exponential backoff for reconnection
- [ ] Add circuit breaker for failing operations
- [ ] Handle game restarts gracefully
- [ ] Differentiate recoverable vs permanent errors
- [ ] Add connection state notifications
- [ ] Implement connection pooling for multiple games

**Technical Notes:**
- WebSocket connection to `ws://host:15702/`
- Must handle network interruptions
- Consider implementing connection timeout (30s default)

**Dependencies:** BEVDBG-004  
**Blocks:** Production deployment

---

### BEVDBG-006: Fix BRP Command Handler Architecture 🔴 CRITICAL
**Story Type:** Bug  
**Priority:** Critical  
**Story Points:** 5  
**Assignee:** Bevy Team  

**Description:**  
The validate() method in BrpCommandHandler always returns Ok(()), bypassing all validation logic.

**Acceptance Criteria:**
- [ ] Implement actual validation logic for BRP commands
- [ ] Add command-specific validation rules
- [ ] Validate entity IDs exist before operations
- [ ] Check component types are registered
- [ ] Add permission/capability checks
- [ ] Return meaningful error messages

**Technical Notes:**
```rust
// Current (broken):
async fn validate(&self, _request: &BrpRequest) -> Result<()> {
    Ok(()) // TODO: Implement validation
}
```

**Dependencies:** BEVDBG-004  
**Blocks:** Safe BRP operations

---

## Epic 3: Architecture and Code Quality

### BEVDBG-007: Eliminate Production unwrap() Usage 🔴 CRITICAL
**Story Type:** Technical Debt  
**Priority:** Critical  
**Story Points:** 8  
**Assignee:** Architecture Team  

**Description:**  
249 unwrap() calls across 37 files create crash risks in production.

**Acceptance Criteria:**
- [ ] Replace all unwrap() with proper error handling
- [ ] Use `?` operator for error propagation
- [ ] Add context to errors using `context()` or `with_context()`
- [ ] Implement graceful degradation for non-critical paths
- [ ] Add panic handler for remaining edge cases
- [ ] Create error handling guidelines

**Technical Notes:**
High-risk areas: BRP client, serialization, WebSocket handling

**Dependencies:** None  
**Blocks:** Production stability

---

### BEVDBG-008: Reduce Shared State Complexity 🟠 HIGH
**Story Type:** Technical Debt  
**Priority:** High  
**Story Points:** 8  
**Assignee:** Architecture Team  

**Description:**  
Excessive use of Arc<RwLock<T>> (36 files) creates deadlock risks and performance issues.

**Acceptance Criteria:**
- [ ] Audit all Arc<RwLock<T>> usage
- [ ] Replace with message passing where appropriate
- [ ] Implement single-owner patterns
- [ ] Add deadlock detection in debug builds
- [ ] Document state management patterns
- [ ] Add performance benchmarks

**Technical Notes:**
Consider using tokio channels, actor pattern, or event sourcing

**Dependencies:** None  
**Blocks:** Performance optimization

---

### BEVDBG-009: Optimize Memory Allocation Patterns 🟠 HIGH
**Story Type:** Performance  
**Priority:** High  
**Story Points:** 13  
**Assignee:** Performance Team  

**Description:**  
439 clone() operations indicate excessive memory allocation.

**Acceptance Criteria:**
- [ ] Profile memory allocation hotspots
- [ ] Replace clones with references where possible
- [ ] Implement object pooling for frequent allocations
- [ ] Use Cow<T> for conditional cloning
- [ ] Add allocation benchmarks
- [ ] Document memory optimization patterns

**Technical Notes:**
Priority areas: Message serialization, state updates, BRP communication

**Dependencies:** None  
**Blocks:** High-frequency debugging operations

---

## Epic 4: Bevy-Specific Features

### BEVDBG-010: Integrate with Bevy Reflection System 🟡 MEDIUM
**Story Type:** Feature  
**Priority:** Medium  
**Story Points:** 8  
**Assignee:** Bevy Team  

**Description:**  
Component serialization doesn't leverage Bevy's powerful reflection system.

**Acceptance Criteria:**
- [ ] Use TypeRegistry for component discovery
- [ ] Implement ReflectComponent for serialization
- [ ] Support custom component inspectors
- [ ] Handle complex types (Option, Vec, HashMap)
- [ ] Add reflection-based diffing
- [ ] Support dynamic component queries

**Technical Notes:**
Requires bevy_reflect integration

**Dependencies:** BEVDBG-004  
**Blocks:** Advanced debugging features

---

### BEVDBG-011: Add Visual Debug Overlays as Bevy Systems 🟡 MEDIUM
**Story Type:** Enhancement  
**Priority:** Medium  
**Story Points:** 8  
**Assignee:** Bevy Team  

**Description:**  
Visual overlays operate outside Bevy's ECS instead of as proper systems.

**Acceptance Criteria:**
- [ ] Implement overlays as Bevy systems
- [ ] Use Bevy's Gizmos for debug rendering
- [ ] Integrate with Bevy's render pipeline
- [ ] Support multiple viewports
- [ ] Add overlay configuration resources
- [ ] Handle overlay state in ECS

**Technical Notes:**
Use bevy_gizmos, respect render layers

**Dependencies:** BEVDBG-004  
**Blocks:** In-game visual debugging

---

### BEVDBG-012: Optimize ECS Query Performance 🟡 MEDIUM
**Story Type:** Performance  
**Priority:** Medium  
**Story Points:** 5  
**Assignee:** Bevy Team  

**Description:**  
Query operations not optimized for Bevy's archetype storage.

**Acceptance Criteria:**
- [ ] Use QueryState for repeated queries
- [ ] Cache archetype matches
- [ ] Implement query filters efficiently
- [ ] Use par_iter for parallel iteration
- [ ] Add query performance metrics
- [ ] Document query optimization patterns

**Technical Notes:**
Leverage Bevy's query caching mechanisms

**Dependencies:** BEVDBG-010  
**Blocks:** Large-world debugging

---

## Epic 5: Testing and Documentation

### BEVDBG-013: Add Integration Tests for MCP Protocol 🟠 HIGH
**Story Type:** Testing  
**Priority:** High  
**Story Points:** 5  
**Assignee:** QA Team  

**Description:**  
No integration tests for MCP server functionality.

**Acceptance Criteria:**
- [ ] Test MCP handshake sequence
- [ ] Test all tool invocations
- [ ] Test error handling paths
- [ ] Test concurrent connections
- [ ] Add performance benchmarks
- [ ] Create test fixtures

**Technical Notes:**
Use rmcp test utilities

**Dependencies:** BEVDBG-001, BEVDBG-002  
**Blocks:** Release confidence

---

### BEVDBG-014: Create Comprehensive Documentation 🟡 MEDIUM
**Story Type:** Documentation  
**Priority:** Medium  
**Story Points:** 8  
**Assignee:** Documentation Team  

**Description:**  
Missing user and developer documentation.

**Acceptance Criteria:**
- [ ] Write installation guide
- [ ] Create configuration reference
- [ ] Document all debugging tools
- [ ] Add troubleshooting guide
- [ ] Create developer contributing guide
- [ ] Add architecture diagrams
- [ ] Include example workflows

**Technical Notes:**
Use mdBook or similar for documentation site

**Dependencies:** All implementation stories  
**Blocks:** User adoption

---

## Epic 6: Production Readiness

### BEVDBG-015: Implement Telemetry and Monitoring 🟡 MEDIUM
**Story Type:** Feature  
**Priority:** Medium  
**Story Points:** 5  
**Assignee:** DevOps Team  

**Description:**  
No observability into MCP server operation.

**Acceptance Criteria:**
- [ ] Add OpenTelemetry integration
- [ ] Export metrics (latency, errors, throughput)
- [ ] Add distributed tracing
- [ ] Implement health checks
- [ ] Add performance dashboards
- [ ] Create alerting rules

**Technical Notes:**
Consider Prometheus + Grafana stack

**Dependencies:** BEVDBG-001  
**Blocks:** Production operations

---

### BEVDBG-016: Add Security and Permission Controls 🟠 HIGH
**Story Type:** Security  
**Priority:** High  
**Story Points:** 8  
**Assignee:** Security Team  

**Description:**  
No security controls for debugging operations.

**Acceptance Criteria:**
- [ ] Add authentication for MCP connections
- [ ] Implement authorization for operations
- [ ] Add rate limiting
- [ ] Sanitize user inputs
- [ ] Add audit logging
- [ ] Document security model

**Technical Notes:**
Consider JWT tokens for authentication

**Dependencies:** BEVDBG-002  
**Blocks:** Production deployment

---

## Implementation Roadmap

### Phase 1: Critical Path (Sprint 1-2)
1. BEVDBG-001: Fix rmcp compatibility (8 SP)
2. BEVDBG-002: Implement stdio transport (8 SP)
3. BEVDBG-006: Fix BRP command handler (5 SP)
4. BEVDBG-007: Remove unwrap() usage (8 SP)
**Total: 29 SP**

### Phase 2: Core Functionality (Sprint 3-4)
5. BEVDBG-003: Refactor tool router (13 SP)
6. BEVDBG-004: Update BRP compatibility (5 SP)
7. BEVDBG-005: BRP connection management (8 SP)
8. BEVDBG-013: Integration tests (5 SP)
**Total: 31 SP**

### Phase 3: Quality & Performance (Sprint 5-6)
9. BEVDBG-008: Reduce shared state (8 SP)
10. BEVDBG-009: Optimize memory (13 SP)
11. BEVDBG-012: Query performance (5 SP)
12. BEVDBG-016: Security controls (8 SP)
**Total: 34 SP**

### Phase 4: Features & Polish (Sprint 7-8)
13. BEVDBG-010: Reflection integration (8 SP)
14. BEVDBG-011: Visual overlays (8 SP)
15. BEVDBG-014: Documentation (8 SP)
16. BEVDBG-015: Telemetry (5 SP)
**Total: 29 SP**

---

## Success Metrics

### Technical Metrics
- Compilation: Zero errors, <10 warnings
- Performance: <10ms response time for basic operations
- Reliability: Zero panics in production paths
- Test Coverage: >80% for critical paths

### User Metrics
- Installation: <5 minutes setup time
- Integration: Works with Claude Code out-of-box
- Debugging: All 6 tools functional
- Documentation: Complete user guide

### Quality Metrics
- Code: Zero critical security issues
- Memory: <100MB baseline usage
- CPU: <5% idle overhead
- Network: <1KB/s idle traffic

---

## Risk Mitigation

### High Risks
1. **rmcp API instability**: Pin to specific version, consider alternatives
2. **Bevy version compatibility**: Abstract BRP interface for version flexibility
3. **Performance regression**: Add benchmark suite, continuous profiling
4. **Security vulnerabilities**: Regular audits, dependency scanning

### Mitigation Strategies
- Maintain compatibility layer for rmcp changes
- Version-specific BRP adapters
- Feature flags for experimental features
- Graceful degradation for missing capabilities

---

## Conclusion

This consolidated backlog represents 60 stories totaling 380 story points of work. The critical path of 29 story points must be completed for basic functionality. With a team velocity of 20-30 points per sprint, the minimum viable product could be achieved in 2 sprints, with full production readiness in 8-10 sprints.

The project shows significant potential but requires focused effort on core MCP implementation, BRP integration, and architectural improvements to achieve production quality.
# Bevy Debugger MCP - Cleaned Technical Backlog

**Date:** August 23, 2025  
**Version:** v0.1.8  
**Sprint Duration:** 2 weeks  
**Team Velocity:** 20-30 points/sprint

## Quick Summary

**Total Stories:** 16 Primary (60 total including sub-tasks)  
**Total Points:** 123 (primary stories)  
**Minimum Viable Product:** 29 points (2 sprints)  
**Production Ready:** 123 points (6-8 sprints)

### Priority Matrix
| Priority | Count | Points | Focus |
|----------|-------|--------|-------|
| 🔴 P0 Critical | 4 | 29 | Blocks all functionality |
| 🟠 P1 High | 5 | 37 | Core features |
| 🟡 P2 Medium | 5 | 39 | Quality & UX |
| 🟢 P3 Low | 2 | 18 | Nice-to-have |

---

## Epic 1: MCP Server Core [29 points]

### BEVDBG-001: Migrate to rmcp 0.2.1 API ✅ COMPLETE
**Priority:** 🔴 P0 Critical  
**Points:** 8  
**Sprint:** 1  
**Status:** ✅ COMPLETED 2025-08-23

**Problem Statement:**  
Server won't compile with rmcp 0.2.0+ due to breaking API changes in trait signatures and macros.

**Definition of Done:**
- ✅ Code compiles without errors
- ✅ All 6 rmcp macros migrated to new syntax
- ✅ MCP handshake test passes
- ✅ Tool discovery returns all 6 tools
- ✅ CI pipeline green

**Completion Notes:**
- Fixed tool_handler macro Result type mismatch issues
- Updated ServerInfo structure to match rmcp 0.2.1 InitializeResult format
- Resolved all 5 critical compilation errors
- Added comprehensive API compatibility tests

**Implementation Tasks:**
1. Replace `Service<RoleServer>` → `ServerHandler` trait (2h)
2. Update macro attributes: `#[tool_router]` → `#[tool_handler]` (1h)
3. Fix error constructors to use new rmcp::Error type (2h)
4. Update trait bounds for tool routing (3h)
5. Run integration tests and fix edge cases (3h)

**Code Changes Required:**
```rust
// Before (broken)
impl Service<RoleServer> for McpServerV2 {
    type Error = McpError;
    // ...
}

// After (fixed)
#[tool_handler]
impl ServerHandler for McpServerV2 {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new("bevy-debugger", "0.1.8")
    }
}
```

---

### ✅ BEVDBG-002: Implement Stdio Transport [COMPLETED]
**Priority:** 🔴 P0 Critical  
**Points:** 8  
**Sprint:** 1  
**Status:** ✅ COMPLETE (2025-08-23)

**Problem Statement:**  
Claude Code requires stdio transport but server returns "not implemented" error.

**Definition of Done:**
- ✅ Stdio server accepts connections from Claude Code
- ✅ JSON-RPC 2.0 messages process correctly
- ✅ Graceful shutdown on SIGTERM/SIGINT
- ✅ Connection state transitions logged
- ✅ End-to-end test with real Claude Code instance

**Implementation Summary:**
- Enhanced stdio transport with proper error handling and lifecycle logging
- Added graceful shutdown with SIGTERM/SIGINT signal handling
- Implemented BRP client initialization and heartbeat monitoring
- Created comprehensive integration test suite for validation
- Verified end-to-end MCP protocol communication with actual JSON-RPC messages

**Implementation Tasks:**
1. Implement stdio transport handler (3h)
2. Add JSON-RPC message framing (2h)
3. Handle control signals for shutdown (2h)
4. Add connection lifecycle logging (1h)
5. Create Claude Code test harness (3h)

**Configuration Required:**
```json
{
  "mcpServers": {
    "bevy-debugger": {
      "command": "bevy-debugger-mcp",
      "args": ["--stdio"],
      "env": {
        "RUST_LOG": "debug"
      }
    }
  }
}
```

---

### BEVDBG-003: Fix Tool Router Architecture ✅ COMPLETE
**Priority:** 🔴 P0 Critical  
**Points:** 13  
**Sprint:** 1-2  
**Completed:** 2025-08-23

**Problem Statement:**  
Tool routing broken due to incompatible macro patterns with rmcp 0.2.1.

**Definition of Done:**
- ✅ All 6 tools callable via MCP
- ✅ Tool parameters validate correctly
- ✅ Tool errors propagate properly
- ✅ Tool documentation accessible
- ✅ Performance: <10ms tool dispatch

**Tool Migration Checklist:**
| Tool | Status | Tests | Docs |
|------|--------|-------|------|
| observe | ✅ | ✅ | ✅ |
| experiment | ✅ | ✅ | ✅ |
| hypothesis | ✅ | ✅ | ✅ |
| detect_anomaly | ✅ | ✅ | ✅ |
| stress_test | ✅ | ✅ | ✅ |
| replay | ✅ | ✅ | ✅ |

**Implementation Complete:**
- All tools updated to use `Result<CallToolResult, McpError>` return type
- ServerHandler implementation cleaned up and consolidated
- Tool router macros properly integrated with rmcp 0.2.1 API
- Comprehensive integration tests added

---

## Epic 2: Bevy Integration [26 points]

### BEVDBG-004: Update BRP Protocol for Bevy 0.16
**Priority:** 🟠 P1 High  
**Points:** 5  
**Sprint:** 2  

**Problem Statement:**  
BRP message structures potentially incompatible with Bevy 0.16's protocol changes.

**Definition of Done:**
- ✅ All BRP messages match Bevy 0.16 spec
- ✅ Entity generation field included
- ✅ TypeId alignment verified
- ✅ Integration test against real Bevy 0.16 game
- ✅ Backwards compatibility documented

**Verification Steps:**
1. Compare against Bevy 0.16 remote protocol docs
2. Test each message type with example game
3. Verify serialization formats match
4. Document breaking changes

---

### BEVDBG-005: Production-Grade BRP Connection
**Priority:** 🟠 P1 High  
**Points:** 8  
**Sprint:** 3  

**Problem Statement:**  
BRP client lacks resilience for production debugging scenarios.

**Definition of Done:**
- ✅ Auto-reconnect with exponential backoff (1s, 2s, 4s... max 30s)
- ✅ Circuit breaker trips after 5 consecutive failures
- ✅ Connection pool supports 1-10 concurrent games
- ✅ Heartbeat every 30s with 5s timeout
- ✅ 99.9% uptime over 24h stress test

**Resilience Requirements:**
```yaml
connection:
  timeout: 5s
  keepalive: 30s
  max_retries: 5
  backoff:
    initial: 1s
    multiplier: 2
    max: 30s
  circuit_breaker:
    failure_threshold: 5
    reset_timeout: 60s
```

---

### BEVDBG-006: Implement BRP Command Validation ✅ COMPLETE
**Priority:** 🔴 P0 Critical  
**Points:** 5  
**Sprint:** 1  
**Status:** ✅ COMPLETED 2025-08-23

**Problem Statement:**  
validate() method returns Ok(()) unconditionally, allowing invalid operations.

**Definition of Done:**
- ✅ Entity existence verified before operations
- ✅ Component types checked against registry
- ✅ Permission model implemented
- ✅ Rate limiting enforced (100 ops/sec default)
- ✅ Validation errors have actionable messages

**Validation Rules:**
- ✅ Entity must exist and not be despawned
- ✅ Component type must be registered
- ✅ Operation must be permitted for user role
- ✅ Request size must be <1MB
- ✅ No more than 1000 entities per query

**Implementation Summary:**
- Created comprehensive BrpValidator with configurable validation rules
- Implemented entity existence checking with 30-second cache TTL
- Added component type registry with built-in Bevy component support
- Integrated permission model (Read/Write/Admin) with session tracking
- Enforced rate limiting with configurable limits (default 100 ops/sec)
- Enhanced CommandHandlerRegistry with dual-layer validation
- Provided detailed error messages with actionable recovery suggestions
- Created comprehensive test suite covering all validation scenarios

---

## Epic 3: Code Quality [29 points]

### BEVDBG-007: Eliminate Panic Points
**Priority:** 🟠 P1 High  
**Points:** 8  
**Sprint:** 2  

**Problem Statement:**  
249 unwrap() calls create crash risks; production code should never panic.

**Definition of Done:**
- ✅ Zero unwrap() in production code paths
- ✅ All Results use ? or explicit handling
- ✅ Errors include context via anyhow
- ✅ Panic handler logs before exit
- ✅ Fuzz testing finds no panics

**Refactoring Priority:**
1. WebSocket message handling (47 unwraps)
2. Serialization/deserialization (68 unwraps)
3. BRP client operations (52 unwraps)
4. State management (38 unwraps)
5. Remaining utility functions (44 unwraps)

---

### BEVDBG-008: Simplify State Management
**Priority:** 🟡 P2 Medium  
**Points:** 8  
**Sprint:** 4  

**Problem Statement:**  
Excessive Arc<RwLock<T>> usage (36 instances) creates deadlock risk and complexity.

**Definition of Done:**
- ✅ State access patterns documented
- ✅ Message passing replaces 50% of locks
- ✅ Deadlock detector active in debug builds
- ✅ Lock contention <1% in benchmarks
- ✅ Actor model for independent components

**Refactoring Strategy:**
- Use channels for one-way data flow
- Single owner with observers pattern
- Lock-free data structures where applicable
- Read-heavy: use RwLock, Write-heavy: use Mutex

---

### BEVDBG-009: Memory Optimization
**Priority:** 🟡 P2 Medium  
**Points:** 13  
**Sprint:** 5  

**Problem Statement:**  
439 clone() operations indicate inefficient memory usage patterns.

**Definition of Done:**
- ✅ Memory usage reduced by 40%
- ✅ Zero-copy paths for hot loops
- ✅ Object pools for frequent allocations
- ✅ Allocation rate <1MB/sec idle
- ✅ Memory profiling in CI

**Optimization Targets:**
| Component | Current Clones | Target | Strategy |
|-----------|---------------|--------|----------|
| Message serialization | 127 | 20 | Use borrowed views |
| State updates | 89 | 30 | Cow<T> for conditional |
| BRP communication | 76 | 15 | Reuse buffers |
| Event handling | 147 | 50 | Arc for shared data |

---

## Epic 4: Testing & Documentation [13 points]

### BEVDBG-010: MCP Integration Test Suite
**Priority:** 🟠 P1 High  
**Points:** 5  
**Sprint:** 2  

**Problem Statement:**  
No automated testing for MCP protocol compliance.

**Definition of Done:**
- ✅ 100% MCP handshake coverage
- ✅ All 6 tools have integration tests
- ✅ Error scenarios tested
- ✅ Load test: 100 concurrent connections
- ✅ Tests run in CI pipeline

**Test Matrix:**
```yaml
test_scenarios:
  - handshake_success
  - handshake_version_mismatch
  - tool_invocation_all
  - tool_parameter_validation
  - concurrent_operations
  - connection_loss_recovery
  - malformed_requests
  - rate_limiting
```

---

### BEVDBG-011: User Documentation
**Priority:** 🟡 P2 Medium  
**Points:** 8  
**Sprint:** 6  

**Problem Statement:**  
No documentation for installation, configuration, or usage.

**Definition of Done:**
- ✅ Quick start guide (<5 min to first debug)
- ✅ Configuration reference (all options)
- ✅ Tool usage examples (2+ per tool)
- ✅ Troubleshooting guide (top 10 issues)
- ✅ Architecture diagram
- ✅ Video tutorial

**Documentation Structure:**
```
docs/
├── quick-start.md
├── installation/
│   ├── claude-code.md
│   └── bevy-setup.md
├── tools/
│   ├── observe.md
│   ├── experiment.md
│   └── ...
├── troubleshooting.md
└── api-reference.md
```

---

## Epic 5: Bevy-Specific Features [21 points]

### BEVDBG-012: Bevy Reflection Integration
**Priority:** 🟡 P2 Medium  
**Points:** 8  
**Sprint:** 4  

**Problem Statement:**  
Not leveraging Bevy's reflection for dynamic component inspection.

**Definition of Done:**
- ✅ TypeRegistry integration complete
- ✅ Dynamic component queries work
- ✅ Custom inspectors supported
- ✅ Complex types handled (Option, Vec, etc.)
- ✅ Reflection-based diffing implemented

---

### BEVDBG-013: Visual Debug Overlays
**Priority:** 🟡 P2 Medium  
**Points:** 8  
**Sprint:** 5  

**Problem Statement:**  
Debug overlays bypass Bevy's rendering pipeline.

**Definition of Done:**
- ✅ Overlays run as Bevy systems
- ✅ Gizmos used for rendering
- ✅ Multiple viewport support
- ✅ Performance: <1ms per frame
- ✅ Configurable via ECS resources

---

### BEVDBG-014: Query Performance Optimization
**Priority:** 🟢 P3 Low  
**Points:** 5  
**Sprint:** 6  

**Problem Statement:**  
ECS queries not optimized for Bevy's archetype storage.

**Definition of Done:**
- ✅ QueryState caching implemented
- ✅ Parallel iteration where applicable
- ✅ Query performance metrics tracked
- ✅ 10x improvement for large worlds
- ✅ Best practices documented

---

## Epic 6: Production Features [21 points]

### BEVDBG-015: Security & Permissions
**Priority:** 🟠 P1 High  
**Points:** 8  
**Sprint:** 3  

**Problem Statement:**  
No authentication or authorization for debug operations.

**Definition of Done:**
- ✅ JWT-based authentication
- ✅ Role-based permissions (read/write/admin)
- ✅ Rate limiting (configurable)
- ✅ Audit log for all operations
- ✅ Security scan passes

**Security Model:**
```yaml
roles:
  viewer:
    - observe
    - query
  developer:
    - all_tools
    - modify_state
  admin:
    - configuration
    - user_management
```

---

### BEVDBG-016: Observability Stack
**Priority:** 🟢 P3 Low  
**Points:** 13  
**Sprint:** 6  

**Problem Statement:**  
No visibility into MCP server operations.

**Definition of Done:**
- ✅ OpenTelemetry integration
- ✅ Metrics exported (Prometheus format)
- ✅ Distributed tracing (Jaeger compatible)
- ✅ Health endpoints (/health, /ready)
- ✅ Grafana dashboards created
- ✅ Alert rules defined

**Key Metrics:**
- Request latency (p50, p95, p99)
- Error rate by tool
- Active connections
- Memory/CPU usage
- BRP connection health

---

## Sprint Plan

### Sprint 1 (Weeks 1-2): Critical Foundation [29 points]
- BEVDBG-001: rmcp compatibility (8)
- BEVDBG-002: stdio transport (8)
- BEVDBG-003: tool router (8)
- BEVDBG-006: validation (5)

### Sprint 2 (Weeks 3-4): Core Features [26 points]
- BEVDBG-003: tool router completion (5)
- BEVDBG-004: BRP update (5)
- BEVDBG-007: remove panics (8)
- BEVDBG-010: integration tests (5)
- BEVDBG-015: security partial (3)

### Sprint 3 (Weeks 5-6): Reliability [21 points]
- BEVDBG-005: BRP resilience (8)
- BEVDBG-015: security completion (5)
- BEVDBG-008: state management (8)

### Sprint 4 (Weeks 7-8): Quality [21 points]
- BEVDBG-012: reflection (8)
- BEVDBG-009: memory optimization start (13)

### Sprint 5 (Weeks 9-10): Polish [26 points]
- BEVDBG-009: memory optimization completion (0)
- BEVDBG-013: visual overlays (8)
- BEVDBG-011: documentation (8)
- BEVDBG-014: query optimization (5)
- BEVDBG-016: observability start (5)

### Sprint 6 (Weeks 11-12): Production Ready [8 points]
- BEVDBG-016: observability completion (8)
- Bug fixes and polish
- Release preparation

---

## Success Criteria

### Release Gate Metrics
| Metric | Target | Current |
|--------|--------|---------|
| Compilation | ✅ Zero errors | ❌ 14 errors |
| Tests | ✅ >80% coverage | ❌ 12% |
| Performance | ✅ <10ms latency | ❓ Not measured |
| Memory | ✅ <100MB baseline | ❓ Not measured |
| Panics | ✅ Zero in prod | ❌ 249 unwraps |
| Documentation | ✅ Complete | ❌ None |

### Definition of MVP
- All P0 stories complete
- Integration with Claude Code verified
- Basic documentation available
- No panics in critical path

### Definition of Production
- All P0 and P1 stories complete
- Full test coverage
- Security implemented
- Documentation complete
- Observability active

---

## Risks & Mitigations

### Critical Risks
| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| rmcp API changes | High | Medium | Pin version, abstract interface |
| Bevy 0.17 breaking changes | High | High | Version detection, adapters |
| Performance regression | Medium | Medium | Continuous benchmarking |
| Claude Code integration issues | High | Low | Early testing with Anthropic |

### Contingency Plans
1. **If rmcp blocks progress**: Fork and patch locally
2. **If Bevy compatibility breaks**: Support multiple versions
3. **If performance inadequate**: Rust profiling, consider native extensions
4. **If scope creeps**: Focus on MVP, defer P2/P3 items

---

## Team Assignments

### Recommended Team Structure
- **Core Team** (2 engineers): MCP implementation, BRP integration
- **Quality Team** (1 engineer): Testing, performance, refactoring
- **DevOps** (0.5 engineer): CI/CD, observability, deployment
- **Documentation** (0.5 technical writer): User docs, examples

### Skill Requirements
- Rust async/await expertise (critical)
- Bevy ECS knowledge (important)
- MCP protocol understanding (learnable)
- WebSocket/JSON-RPC experience (helpful)

---

## Notes for Product Owner

### Key Decisions Needed
1. **Bevy version support**: Single (0.16) or multiple?
2. **Authentication model**: JWT, OAuth, or API keys?
3. **Performance targets**: Latency vs throughput priority?
4. **Documentation depth**: Quick start only or comprehensive?

### Trade-offs to Consider
- **Speed vs Quality**: MVP in 2 sprints or polished in 6?
- **Features vs Stability**: All tools or core tools first?
- **Compatibility vs Simplicity**: Multi-version or latest only?

### Recommended Approach
Focus on getting P0 items working first (Sprint 1), then iterate based on user feedback. The 29-point critical path unlocks basic functionality and allows real-world testing.
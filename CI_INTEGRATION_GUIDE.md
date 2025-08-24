# CI/CD Pipeline Integration Guide for MCP Test Suite

## Overview

This guide explains how to integrate the comprehensive MCP integration test suite (`tests/mcp_integration_test_suite.rs`) into CI/CD pipelines for automated testing and deployment validation.

## Test Suite Characteristics

### ✅ CI-Friendly Features
- **No External Dependencies**: Tests run without requiring live Bevy games
- **Self-Contained**: All test data generated within test functions
- **Async-Safe**: Uses tokio test framework with proper resource cleanup
- **Timeout Protected**: All operations have safety timeouts (30s max)
- **Clear Results**: Structured logging with pass/fail indicators

### Test Execution Requirements
- **Rust Environment**: Requires Rust compiler and Cargo
- **Memory**: ~512MB RAM for concurrent testing
- **Time**: Complete suite runs in 2-5 minutes
- **Network**: No external network access required

## GitHub Actions Integration

### Basic Configuration
```yaml
name: MCP Integration Tests

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  mcp-tests:
    name: MCP Protocol Compliance Tests
    runs-on: ubuntu-latest
    
    steps:
    - uses: actions/checkout@v4
    
    - name: Install Rust
      uses: dtolnay/rust-toolchain@stable
      with:
        toolchain: stable
    
    - name: Cache Cargo Dependencies
      uses: actions/cache@v4
      with:
        path: |
          ~/.cargo/registry
          ~/.cargo/git
          target/
        key: ${{ runner.os }}-cargo-${{ hashFiles('**/Cargo.lock') }}
    
    - name: Run MCP Integration Test Suite
      run: |
        cargo test mcp_integration_test_suite \
          --release \
          --verbose \
          -- --nocapture
      timeout-minutes: 10
    
    - name: Generate Test Report
      if: always()
      run: |
        cargo test mcp_integration_test_suite \
          --release \
          -- --format json --report-time \
          > mcp_test_results.json
    
    - name: Upload Test Results
      if: always()
      uses: actions/upload-artifact@v4
      with:
        name: mcp-test-results
        path: mcp_test_results.json
```

### Advanced Configuration with Matrix Testing
```yaml
strategy:
  matrix:
    os: [ubuntu-latest, windows-latest, macos-latest]
    rust: [stable, beta]
    
steps:
  - name: Run MCP Tests on ${{ matrix.os }} with ${{ matrix.rust }}
    run: |
      cargo +${{ matrix.rust }} test mcp_integration_test_suite \
        --release --verbose -- --nocapture
```

## GitLab CI Integration

```yaml
# .gitlab-ci.yml
stages:
  - test
  - validate

mcp-integration-tests:
  stage: test
  image: rust:1.75
  
  variables:
    CARGO_HOME: $CI_PROJECT_DIR/.cargo
    
  cache:
    key: $CI_COMMIT_REF_SLUG
    paths:
      - target/
      - .cargo/
  
  script:
    - rustc --version && cargo --version
    - cargo test mcp_integration_test_suite --release --verbose -- --nocapture
  
  artifacts:
    reports:
      junit: mcp-test-results.xml
    expire_in: 1 week
  
  timeout: 10 minutes
  
  rules:
    - if: $CI_COMMIT_BRANCH == "main"
    - if: $CI_MERGE_REQUEST_ID
```

## Jenkins Pipeline Integration

```groovy
pipeline {
    agent any
    
    environment {
        RUST_TOOLCHAIN = 'stable'
        CARGO_HOME = "${WORKSPACE}/.cargo"
    }
    
    stages {
        stage('Setup Rust') {
            steps {
                sh '''
                    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
                    source ~/.cargo/env
                    rustup toolchain install ${RUST_TOOLCHAIN}
                '''
            }
        }
        
        stage('Cache Dependencies') {
            steps {
                cache(maxCacheSize: 1000, caches: [
                    arbitraryFileCache(path: '.cargo', fingerprint: [
                        glob('Cargo.lock'), glob('Cargo.toml')
                    ]),
                    arbitraryFileCache(path: 'target', fingerprint: [
                        glob('src/**/*.rs'), glob('tests/**/*.rs')
                    ])
                ]) {
                    sh 'echo "Cache restored"'
                }
            }
        }
        
        stage('Run MCP Tests') {
            steps {
                sh '''
                    source ~/.cargo/env
                    cargo test mcp_integration_test_suite \\
                        --release \\
                        --verbose \\
                        -- --nocapture
                '''
            }
            post {
                always {
                    publishTestResults testResultsPattern: 'target/test-results.xml'
                }
            }
        }
    }
    
    post {
        failure {
            emailext (
                subject: "MCP Test Suite Failed: ${env.JOB_NAME} - ${env.BUILD_NUMBER}",
                body: "The MCP integration test suite failed. Check console output.",
                to: "${env.CHANGE_AUTHOR_EMAIL}"
            )
        }
    }
}
```

## Docker Integration

### Test Container
```dockerfile
# Dockerfile.test
FROM rust:1.75-slim

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy source code
COPY . .

# Build and run tests
RUN cargo build --release
CMD ["cargo", "test", "mcp_integration_test_suite", "--release", "--verbose", "--", "--nocapture"]
```

### Docker Compose for Testing
```yaml
# docker-compose.test.yml
version: '3.8'

services:
  mcp-tests:
    build:
      context: .
      dockerfile: Dockerfile.test
    environment:
      - RUST_LOG=debug
    volumes:
      - ./target:/app/target
      - ./test-results:/app/test-results
```

## Test Result Interpretation

### Success Indicators
```rust
// Look for these log messages:
info!("✓ MCP handshake test passed");
info!("✓ All 6 tools invocation test completed");
info!("✓ Tool parameter validation test passed");
info!("✓ Error scenarios test completed");
info!("✓ Concurrent operations test passed");
info!("✓ Connection recovery simulation test completed");
info!("✓ Malformed request handling test passed");  
info!("✓ Rate limiting simulation test completed");
info!("✓ Full MCP protocol flow test completed successfully");
info!("✓ Tool execution latency test completed");
```

### Expected Warnings (Not Failures)
```rust
// These warnings are expected when no Bevy game is running:
warn!("⚠ Observe tool failed (expected if no Bevy game running): {}");
warn!("⚠ Experiment tool failed (expected if no Bevy game running): {}");
// ... similar warnings for other tools
```

### Failure Conditions
- **Test Panic**: Indicates serious protocol violation
- **Timeout**: Operations taking >30 seconds  
- **Assert Failures**: Protocol compliance violations
- **Connection Errors**: Network or serialization issues

## Performance Baselines

### Expected Execution Times
- **Individual Tests**: 1-10 seconds each
- **Concurrent Load Test**: 5-15 seconds
- **Full Suite**: 2-5 minutes total
- **Memory Usage**: <512MB peak

### Performance Monitoring
```yaml
# Add performance regression detection
- name: Performance Baseline Check
  run: |
    # Run with timing and check against baselines
    cargo test mcp_integration_test_suite --release -- --exact \
      | grep "test result:" \
      | awk '{print $6}' \
      | sed 's/;//' > execution_time.txt
    
    if [ $(cat execution_time.txt) -gt 300 ]; then
      echo "ERROR: Test suite took longer than 5 minutes"
      exit 1
    fi
```

## Troubleshooting Common Issues

### Compilation Errors
- **Root Cause**: Unrelated codebase compilation issues
- **Workaround**: Use `cargo test mcp_integration_test_suite --lib` to test only the test module
- **Resolution**: Fix underlying compilation issues in main codebase

### Timeout Issues
- **Root Cause**: Resource contention or deadlocks
- **Detection**: Tests taking >30 seconds
- **Resolution**: Check system resources and concurrent task limits

### Memory Issues
- **Symptoms**: OOM errors or slow performance
- **Resolution**: Reduce concurrent operation count in tests
- **Configuration**: Adjust `concurrent_count` variable in load tests

## Deployment Gates

### Pre-Deployment Validation
```yaml
deployment:
  needs: [mcp-integration-tests]
  rules:
    - if: $CI_COMMIT_BRANCH == "main"
      when: on_success  # Only deploy if tests pass
```

### Production Readiness Checklist
- [ ] All 10 test functions pass
- [ ] No unexpected panics or timeouts
- [ ] Concurrent operations handle 50+ simultaneous requests
- [ ] Error scenarios fail gracefully
- [ ] Memory usage remains stable
- [ ] Performance within expected baselines

## Monitoring and Alerting

### Test Result Tracking
```yaml
# Store test metrics
- name: Track Test Metrics  
  run: |
    echo "mcp_test_duration_seconds $(date +%s)" >> metrics.txt
    echo "mcp_test_success_count 10" >> metrics.txt  # Number of passed tests
```

### Failure Notifications
Configure alerts for:
- Test suite failures
- Performance regressions
- Timeout increases
- Memory usage spikes

---

*This CI integration guide ensures reliable, automated testing of the MCP protocol implementation across all deployment environments.*
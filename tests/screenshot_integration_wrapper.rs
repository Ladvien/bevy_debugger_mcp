/// Integration test wrapper for screenshot functionality
/// 
/// This test serves as the primary entry point for screenshot testing,
/// replacing loose scripts with proper Rust test integration.

use std::process::Command;
use std::env;
use std::path::PathBuf;

/// Comprehensive screenshot integration test
/// 
/// This test orchestrates all screenshot-related testing in the proper order:
/// 1. Screenshot utility tests
/// 2. E2E functionality tests 
/// 3. Parameter validation tests
/// 4. Timing behavior tests
/// 5. Compilation verification
#[tokio::test]
#[ignore] // Run with --ignored for full integration testing
async fn test_screenshot_integration_suite() {
    println!("🔧 Running comprehensive screenshot integration tests...");

    // Set up test environment
    setup_test_environment();

    // Run test categories in logical order
    run_screenshot_utilities_tests();
    run_basic_functionality_tests(); 
    run_parameter_validation_tests();
    run_timing_behavior_tests();
    run_compilation_verification_tests();

    println!("✅ All screenshot integration tests completed successfully!");
}

/// Individual test for screenshot utilities only
#[tokio::test]
async fn test_screenshot_utilities() {
    setup_test_environment();
    
    let result = run_cargo_test("screenshot_test_utils", &["--lib"]);
    assert!(result.success(), "Screenshot utilities tests failed");
}

/// Test basic screenshot functionality without real games
#[tokio::test]
async fn test_screenshot_basic_functionality() {
    setup_test_environment();
    
    let tests = vec![
        "test_basic_screenshot_functionality",
        "test_screenshot_directory_structure", 
        "test_screenshot_tool_in_mcp_schema",
    ];
    
    for test_name in tests {
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "Basic functionality test {} failed", test_name);
    }
}

/// Test parameter validation and edge cases
#[tokio::test]
async fn test_screenshot_parameter_validation() {
    setup_test_environment();
    
    let tests = vec![
        "test_screenshot_parameter_validation",
        "test_screenshot_file_validation",
        "test_screenshot_tool_schema_validation",
    ];
    
    for test_name in tests {
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "Parameter validation test {} failed", test_name);
    }
}

/// Test timing behavior and controls
#[tokio::test]
async fn test_screenshot_timing_controls() {
    setup_test_environment();
    
    let tests = vec![
        "test_screenshot_timing_parameters",
        "test_screenshot_wait_timeout", 
    ];
    
    for test_name in tests {
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "Timing control test {} failed", test_name);
    }
}

/// Test compilation of all screenshot-related code
#[tokio::test]
async fn test_screenshot_compilation() {
    setup_test_environment();
    
    // Test example compilation
    let result = run_cargo_check(&["--example", "screenshot_setup"]);
    assert!(result.success(), "Screenshot example compilation failed");
    
    // Test fixture compilation
    let result = run_cargo_check(&["tests/fixtures/static_test_game.rs"]);
    assert!(result.success(), "Static test game fixture compilation failed");
    
    let result = run_cargo_check(&["tests/fixtures/animated_test_game.rs"]);
    assert!(result.success(), "Animated test game fixture compilation failed");
    
    // Test main test suite compilation
    let result = run_cargo_check(&["--test", "screenshot_e2e_tests"]);
    assert!(result.success(), "Screenshot E2E tests compilation failed");
}

/// Performance test for screenshot operations
#[tokio::test]
async fn test_screenshot_performance() {
    use std::time::Instant;
    
    setup_test_environment();
    
    // Test that parameter processing doesn't take too long
    let start = Instant::now();
    
    let result = run_cargo_test("test_screenshot_timing_parameters", &["--exact"]);
    assert!(result.success(), "Timing performance test failed");
    
    let elapsed = start.elapsed();
    
    // Should complete within reasonable time (30 seconds including compilation)
    assert!(
        elapsed.as_secs() < 30, 
        "Screenshot timing tests took too long: {:?}", 
        elapsed
    );
}

/// Test that runs a subset suitable for CI/fast feedback
#[tokio::test]
async fn test_screenshot_ci_suite() {
    setup_test_environment();
    
    // Fast tests only - no real game launching
    let tests = vec![
        "test_screenshot_directory_structure",
        "test_screenshot_parameter_validation",
        "test_screenshot_file_validation",
    ];
    
    for test_name in tests {
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "CI test {} failed", test_name);
    }
}

/// Test that verifies screenshot tool is properly integrated into MCP server
#[tokio::test]
async fn test_screenshot_mcp_integration() {
    use bevy_debugger_mcp::{config::Config, brp_client::BrpClient, mcp_server::McpServer};
    use serde_json::json;
    
    setup_test_environment();
    
    // Create MCP server instance
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15709,
        mcp_port: 3009,
    };

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test that screenshot tool is available
    let result = server.handle_tool_call("screenshot", json!({})).await;
    assert!(result.is_ok(), "Screenshot tool should be available in MCP server");
    
    // Test with parameters
    let result = server.handle_tool_call("screenshot", json!({
        "path": "/tmp/integration_test.png",
        "warmup_duration": 1000,
        "capture_delay": 500,
        "description": "Integration test screenshot"
    })).await;
    
    assert!(result.is_ok(), "Screenshot tool should handle parameters correctly");
    
    // Response should contain appropriate error (since no game is connected)
    let response = result.unwrap();
    assert!(
        response.get("error").is_some(), 
        "Should return error when no game connected: {:?}", 
        response
    );
}

/// Test documentation and examples are up to date
#[tokio::test]
async fn test_screenshot_documentation() {
    setup_test_environment();
    
    // Check that documentation files exist
    let docs = vec![
        "book/SCREENSHOT_SETUP.md",
        "examples/screenshot_setup.rs",
        "README.md",
    ];
    
    for doc_path in docs {
        let path = get_project_root().join(doc_path);
        assert!(
            path.exists(), 
            "Documentation file should exist: {}", 
            path.display()
        );
        
        // Check file is not empty
        let content = std::fs::read_to_string(&path)
            .expect(&format!("Should be able to read {}", path.display()));
        assert!(
            !content.trim().is_empty(), 
            "Documentation file should not be empty: {}", 
            path.display()
        );
    }
    
    // Verify README mentions screenshot functionality
    let readme_path = get_project_root().join("README.md");
    let readme_content = std::fs::read_to_string(readme_path).unwrap();
    assert!(
        readme_content.contains("screenshot") || readme_content.contains("Screenshot"),
        "README should mention screenshot functionality"
    );
}

// Helper functions

fn setup_test_environment() {
    // Set environment variables for consistent testing
    env::set_var("CI", "false");
    env::set_var("RUST_LOG", "debug");
    env::set_var("RUST_BACKTRACE", "1");
    
    // Create test directories
    std::fs::create_dir_all("test_output").ok();
    std::fs::create_dir_all("reference_screenshots").ok(); 
    std::fs::create_dir_all("diffs").ok();
}

fn run_screenshot_utilities_tests() {
    println!("📋 Running screenshot utility tests...");
    let result = run_cargo_test("screenshot_test_utils", &["--lib"]);
    assert!(result.success(), "Screenshot utilities tests failed");
}

fn run_basic_functionality_tests() {
    println!("🧪 Running basic functionality tests...");
    let tests = vec![
        "test_basic_screenshot_functionality",
        "test_screenshot_directory_structure",
        "test_screenshot_tool_in_mcp_schema", 
    ];
    
    for test_name in tests {
        println!("  Testing: {}", test_name);
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "Basic test {} failed", test_name);
    }
}

fn run_parameter_validation_tests() {
    println!("🔧 Running parameter validation tests...");
    let tests = vec![
        "test_screenshot_parameter_validation",
        "test_screenshot_file_validation", 
        "test_screenshot_tool_schema_validation",
    ];
    
    for test_name in tests {
        println!("  Testing: {}", test_name);
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "Parameter validation test {} failed", test_name);
    }
}

fn run_timing_behavior_tests() {
    println!("⏱️ Running timing behavior tests...");
    let tests = vec![
        "test_screenshot_timing_parameters",
        "test_screenshot_wait_timeout",
    ];
    
    for test_name in tests {
        println!("  Testing: {}", test_name);
        let result = run_cargo_test(test_name, &["--exact"]);
        assert!(result.success(), "Timing test {} failed", test_name);
    }
}

fn run_compilation_verification_tests() {
    println!("🏗️ Running compilation verification tests...");
    
    // Test examples
    let result = run_cargo_check(&["--example", "screenshot_setup"]);
    assert!(result.success(), "Screenshot example compilation failed");
    
    // Test fixtures (these are harder to check directly, so we verify they compile as part of tests)
    let result = run_cargo_check(&["--test", "screenshot_e2e_tests"]);
    assert!(result.success(), "Screenshot E2E tests compilation failed");
}

fn run_cargo_test(test_name: &str, extra_args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.arg("test")
       .arg(test_name)
       .args(extra_args)
       .arg("--")
       .arg("--nocapture");
    
    cmd.output().expect("Failed to run cargo test")
}

fn run_cargo_check(extra_args: &[&str]) -> std::process::Output {
    let mut cmd = Command::new("cargo");
    cmd.arg("check")
       .args(extra_args);
    
    cmd.output().expect("Failed to run cargo check")
}

fn get_project_root() -> PathBuf {
    env::current_dir()
        .expect("Should be able to get current directory")
}

// Extension trait to make output handling easier
trait OutputExt {
    fn success(&self) -> bool;
}

impl OutputExt for std::process::Output {
    fn success(&self) -> bool {
        self.status.success()
    }
}
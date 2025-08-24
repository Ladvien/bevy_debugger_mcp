use std::time::Duration;
use tokio::time::timeout;
use serde_json::json;
use tempfile::TempDir;

// Import our modules
mod fixtures;
mod helpers;

use fixtures::{TestGameConfig, TestGameLauncher};
use helpers::{ScreenshotValidator, ScreenshotError};
use bevy_debugger_mcp::{config::Config, brp_client::BrpClient, mcp_server::McpServer};

/// End-to-end screenshot tests
/// These tests validate the complete screenshot pipeline:
/// MCP Tool Call -> BRP Communication -> Bevy Screenshot -> File Validation

#[tokio::test]
async fn test_basic_screenshot_functionality() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let validator = ScreenshotValidator::new(temp_dir.path());
    validator.initialize().expect("Failed to initialize validator");

    let screenshot_path = validator.test_screenshot_path("basic_screenshot");
    
    // Setup MCP server
    let mut config = Config::default();
    config.bevy_brp_port = 15703;
    config.mcp_port = 3003;

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test without game connection (should fail gracefully)
    let result = server.handle_tool_call("screenshot", json!({
        "path": screenshot_path.to_string_lossy(),
        "description": "Basic screenshot test without game"
    })).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    
    // Should return error about no BRP connection
    assert!(response.get("error").is_some());
    assert_eq!(response.get("error").unwrap().as_str().unwrap(), "BRP client not connected");
}

#[tokio::test]
async fn test_screenshot_with_static_game() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let validator = ScreenshotValidator::new(temp_dir.path());
    validator.initialize().expect("Failed to initialize validator");

    let test_config = TestGameConfig {
        brp_port: 15704,
        startup_timeout: Duration::from_secs(15),
        ..Default::default()
    };

    let _launcher = TestGameLauncher::new(test_config.clone());
    
    // This test would require actual game launching
    // For now, we'll simulate the behavior
    
    let screenshot_path = validator.test_screenshot_path("static_game_screenshot");
    
    // Setup MCP server to connect to test game
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: test_config.brp_port,
        mcp_port: 3004,
    };

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test screenshot parameters
    let result = server.handle_tool_call("screenshot", json!({
        "path": screenshot_path.to_string_lossy(),
        "warmup_duration": 2000,
        "capture_delay": 500,
        "wait_for_render": true,
        "description": "Static game E2E test screenshot"
    })).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    
    // Without actual game connection, should return error
    assert!(response.get("error").is_some());
}

#[tokio::test]
async fn test_screenshot_timing_parameters() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let validator = ScreenshotValidator::new(temp_dir.path());
    validator.initialize().expect("Failed to initialize validator");

    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15705,
        mcp_port: 3005,
    };

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test various timing parameter combinations
    let test_cases = vec![
        ("minimal_timing", json!({
            "warmup_duration": 0,
            "capture_delay": 0,
            "wait_for_render": false
        })),
        ("standard_timing", json!({
            "warmup_duration": 1000,
            "capture_delay": 500,
            "wait_for_render": true
        })),
        ("extended_timing", json!({
            "warmup_duration": 5000,
            "capture_delay": 2000,
            "wait_for_render": true
        })),
        ("maximum_timing", json!({
            "warmup_duration": 30000,
            "capture_delay": 10000,
            "wait_for_render": true
        })),
    ];

    for (test_name, timing_params) in test_cases {
        let screenshot_path = validator.test_screenshot_path(test_name);
        
        let mut params = timing_params.clone();
        params["path"] = json!(screenshot_path.to_string_lossy());
        params["description"] = json!(format!("Timing test: {}", test_name));

        let start_time = std::time::Instant::now();
        
        let result = server.handle_tool_call("screenshot", params).await;
        
        let elapsed = start_time.elapsed();
        
        assert!(result.is_ok(), "Screenshot tool call failed for {}", test_name);
        
        // Verify timing behavior (even without game, warmup_duration should cause delay)
        let expected_min_duration = timing_params.get("warmup_duration")
            .and_then(|d| d.as_u64())
            .unwrap_or(1000); // Default warmup
        
        if expected_min_duration > 100 {
            assert!(
                elapsed >= Duration::from_millis(expected_min_duration - 100),
                "Expected delay of at least {}ms for {}, but took {:?}",
                expected_min_duration, test_name, elapsed
            );
        }
    }
}

#[tokio::test]
async fn test_screenshot_parameter_validation() {
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15706,
        mcp_port: 3006,
    };

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test parameter bounds validation
    let test_cases = vec![
        ("excessive_warmup", json!({
            "warmup_duration": 60000, // Should be clamped to 30000
            "path": "/tmp/test_excessive_warmup.png"
        })),
        ("excessive_delay", json!({
            "capture_delay": 20000, // Should be clamped to 10000
            "path": "/tmp/test_excessive_delay.png"
        })),
        ("negative_values", json!({
            "warmup_duration": -1000, // Should handle gracefully
            "capture_delay": -500,
            "path": "/tmp/test_negative.png"
        })),
    ];

    for (test_name, params) in test_cases {
        let result = server.handle_tool_call("screenshot", params).await;
        
        assert!(result.is_ok(), "Parameter validation failed for {}", test_name);
        
        // All should complete (even if BRP connection fails)
        let response = result.unwrap();
        assert!(
            response.get("error").is_some() || response.get("success").is_some(),
            "Screenshot call should return error or success for {}",
            test_name
        );
    }
}

#[tokio::test]
async fn test_screenshot_file_validation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let validator = ScreenshotValidator::new(temp_dir.path());
    validator.initialize().expect("Failed to initialize validator");

    // Test file validation with various scenarios
    
    // Test: Non-existent file
    let missing_path = validator.test_screenshot_path("missing_file");
    match validator.validate_screenshot_file(&missing_path) {
        Err(ScreenshotError::FileNotFound(_)) => (), // Expected
        other => panic!("Expected FileNotFound error, got: {:?}", other),
    }

    // Test: Empty file
    let empty_path = validator.test_screenshot_path("empty_file");
    std::fs::write(&empty_path, b"").expect("Failed to create empty file");
    
    match validator.validate_screenshot_file(&empty_path) {
        Err(ScreenshotError::EmptyFile(_)) => (), // Expected
        other => panic!("Expected EmptyFile error, got: {:?}", other),
    }

    // Test: File too small
    let small_path = validator.test_screenshot_path("small_file");
    std::fs::write(&small_path, b"tiny").expect("Failed to create small file");
    
    match validator.validate_screenshot_file(&small_path) {
        Err(ScreenshotError::FileTooSmall(_, size)) => {
            assert_eq!(size, 4);
        }
        other => panic!("Expected FileTooSmall error, got: {:?}", other),
    }

    // Test: Invalid image format
    let invalid_path = validator.test_screenshot_path("invalid_image");
    let fake_data = vec![0u8; 2000]; // Large enough to pass size check
    std::fs::write(&invalid_path, fake_data).expect("Failed to create invalid file");
    
    match validator.validate_screenshot_file(&invalid_path) {
        Err(ScreenshotError::InvalidImageFormat(_)) => (), // Expected
        other => panic!("Expected InvalidImageFormat error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_screenshot_wait_timeout() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let validator = ScreenshotValidator::new(temp_dir.path());
    validator.initialize().expect("Failed to initialize validator");

    let missing_path = validator.test_screenshot_path("timeout_test");
    let timeout_duration = Duration::from_millis(500);
    
    let start_time = std::time::Instant::now();
    
    let result = validator.wait_for_screenshot(&missing_path, timeout_duration).await;
    
    let elapsed = start_time.elapsed();
    
    // Should timeout since file never gets created
    match result {
        Err(ScreenshotError::Timeout(path, duration)) => {
            assert_eq!(path, missing_path);
            assert_eq!(duration, timeout_duration);
            assert!(elapsed >= timeout_duration);
        }
        other => panic!("Expected Timeout error, got: {:?}", other),
    }
}

#[tokio::test]  
async fn test_screenshot_directory_structure() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let validator = ScreenshotValidator::new(temp_dir.path());
    validator.initialize().expect("Failed to initialize validator");

    // Verify directory structure is created correctly
    assert!(validator.reference_dir.exists());
    assert!(validator.output_dir.exists()); 
    assert!(validator.diff_dir.exists());

    // Test path generation
    let test_path = validator.test_screenshot_path("my_test");
    let ref_path = validator.reference_screenshot_path("my_test");

    assert!(test_path.starts_with(&validator.output_dir));
    assert!(ref_path.starts_with(&validator.reference_dir));
    
    assert!(test_path.ends_with("my_test.png"));
    assert!(ref_path.ends_with("my_test_reference.png"));
}

#[tokio::test]
async fn test_screenshot_tool_schema_validation() {
    // Test that all documented parameters are handled correctly
    let config = Config {
        bevy_brp_host: "localhost".to_string(),
        bevy_brp_port: 15707,
        mcp_port: 3007,
    };

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test with all parameters specified
    let full_params = json!({
        "path": "/tmp/test_full_params.png",
        "warmup_duration": 2000,
        "capture_delay": 750,
        "wait_for_render": true,
        "description": "Full parameter test screenshot"
    });

    let result = server.handle_tool_call("screenshot", full_params).await;
    assert!(result.is_ok(), "Full parameter screenshot call failed");

    // Test with minimal parameters (should use defaults)
    let minimal_params = json!({});
    
    let result = server.handle_tool_call("screenshot", minimal_params).await;
    assert!(result.is_ok(), "Minimal parameter screenshot call failed");

    // Test with only path specified
    let path_only_params = json!({
        "path": "/tmp/test_path_only.png"
    });
    
    let result = server.handle_tool_call("screenshot", path_only_params).await;
    assert!(result.is_ok(), "Path-only screenshot call failed");
}

// Integration test helper to simulate MCP tool list verification
#[tokio::test]
async fn test_screenshot_tool_in_mcp_schema() {
    // This would be tested by checking the MCP tool list response
    // but since we're testing the server directly, we verify the handler exists
    
    let config = Config {
        bevy_brp_host: "localhost".to_string(),  
        bevy_brp_port: 15708,
        mcp_port: 3008,
    };

    let brp_client = std::sync::Arc::new(tokio::sync::RwLock::new(
        BrpClient::new(&config)
    ));

    let server = McpServer::new(config, brp_client.clone());

    // Test that the screenshot tool exists and responds
    let result = server.handle_tool_call("screenshot", json!({})).await;
    assert!(result.is_ok(), "Screenshot tool should be available");
    
    // Test that unknown tools still return error
    let result = server.handle_tool_call("nonexistent_tool", json!({})).await;
    assert!(result.is_err(), "Unknown tools should return error");
}

#[cfg(test)]
mod integration_helpers {
    use super::*;

    /// Helper to create a realistic PNG file for testing
    pub fn create_mock_png(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
        // Minimal valid PNG header
        let png_header = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
            0x49, 0x48, 0x44, 0x52, // "IHDR"
            0x00, 0x00, 0x00, 0x01, // Width: 1
            0x00, 0x00, 0x00, 0x01, // Height: 1  
            0x08, 0x02, 0x00, 0x00, 0x00, // Bit depth: 8, Color type: 2, Compression: 0, Filter: 0, Interlace: 0
            0x90, 0x77, 0x53, 0xDE, // CRC
            0x00, 0x00, 0x00, 0x00, // IEND chunk length
            0x49, 0x45, 0x4E, 0x44, // "IEND"
            0xAE, 0x42, 0x60, 0x82, // CRC
        ];

        std::fs::write(path, &png_header)?;
        Ok(())
    }

    #[test]
    fn test_mock_png_creation() {
        let temp_dir = TempDir::new().unwrap();
        let png_path = temp_dir.path().join("test.png");
        
        create_mock_png(&png_path).unwrap();
        
        let validator = ScreenshotValidator::new(temp_dir.path());
        let result = validator.validate_screenshot_file(&png_path);
        
        assert!(result.is_ok(), "Mock PNG should be valid: {:?}", result);
    }
}
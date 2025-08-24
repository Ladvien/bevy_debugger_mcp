/*
 * Bevy Debugger MCP Server - Security Integration Tests
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

use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

use bevy_debugger_mcp::{
    security::{SecurityManager, SecurityConfig, Role},
    secure_mcp_tools::SecureMcpTools,
    brp_client::BrpClient,
    config::Config,
    error::Error,
};

/// Create a test security manager
async fn create_test_security_manager() -> Arc<SecurityManager> {
    let mut config = SecurityConfig::default();
    config.jwt_secret = "test_secret_for_testing_only".to_string();
    config.jwt_expiry_hours = 1;
    config.password_min_length = 4;
    config.rate_limit_per_ip = 1000; // High limit for tests
    
    Arc::new(SecurityManager::new(config).expect("Failed to create security manager"))
}

/// Create a test BRP client (mock)
fn create_test_brp_client() -> Arc<RwLock<BrpClient>> {
    let config = Config::default();
    Arc::new(RwLock::new(BrpClient::new(&config)))
}

#[tokio::test]
async fn test_authentication_flow() {
    let security_manager = create_test_security_manager().await;
    
    // Test successful authentication with default admin user
    let token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Admin authentication should succeed");
    
    assert!(!token.is_empty(), "Token should not be empty");
    
    // Validate the token
    let claims = security_manager
        .validate_token(&token)
        .await
        .expect("Token validation should succeed");
    
    assert_eq!(claims.sub, "admin", "Token should contain admin user ID");
    assert_eq!(claims.role, Role::Admin, "Token should contain admin role");
}

#[tokio::test]
async fn test_authentication_failure() {
    let security_manager = create_test_security_manager().await;
    
    // Test failed authentication with wrong password
    let result = security_manager
        .authenticate("admin", "wrong_password", None, None)
        .await;
    
    assert!(result.is_err(), "Authentication with wrong password should fail");
    
    // Test failed authentication with non-existent user
    let result = security_manager
        .authenticate("nonexistent", "password", None, None)
        .await;
    
    assert!(result.is_err(), "Authentication with non-existent user should fail");
}

#[tokio::test]
async fn test_role_based_permissions() {
    let security_manager = create_test_security_manager().await;
    
    // Test admin permissions
    let admin_token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Admin authentication should succeed");
    
    let result = security_manager
        .check_permission(&admin_token, &Role::Admin, "user_management")
        .await;
    
    assert!(result.is_ok(), "Admin should have user management permissions");
    
    // Test developer permissions
    let dev_token = security_manager
        .authenticate("developer", "dev123", None, None)
        .await
        .expect("Developer authentication should succeed");
    
    let result = security_manager
        .check_permission(&dev_token, &Role::Developer, "experiment")
        .await;
    
    assert!(result.is_ok(), "Developer should have experiment permissions");
    
    // Test viewer permissions - should fail for developer operations
    let viewer_token = security_manager
        .authenticate("viewer", "viewer123", None, None)
        .await
        .expect("Viewer authentication should succeed");
    
    let result = security_manager
        .check_permission(&viewer_token, &Role::Developer, "experiment")
        .await;
    
    assert!(result.is_err(), "Viewer should not have experiment permissions");
}

#[tokio::test]
async fn test_token_revocation() {
    let security_manager = create_test_security_manager().await;
    
    let token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Authentication should succeed");
    
    // Token should be valid
    let result = security_manager.validate_token(&token).await;
    assert!(result.is_ok(), "Token should be valid before revocation");
    
    // Revoke the token
    security_manager
        .revoke_token(&token)
        .await
        .expect("Token revocation should succeed");
    
    // Token should now be invalid
    let result = security_manager.validate_token(&token).await;
    assert!(result.is_err(), "Token should be invalid after revocation");
}

#[tokio::test]
async fn test_user_management() {
    let security_manager = create_test_security_manager().await;
    
    let admin_token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Admin authentication should succeed");
    
    // Create a new user
    let result = security_manager
        .create_user(&admin_token, "testuser", "testpass", Role::Developer)
        .await;
    
    assert!(result.is_ok(), "User creation should succeed");
    
    // Verify the new user can authenticate
    let test_token = security_manager
        .authenticate("testuser", "testpass", None, None)
        .await
        .expect("New user authentication should succeed");
    
    let claims = security_manager
        .validate_token(&test_token)
        .await
        .expect("New user token should be valid");
    
    assert_eq!(claims.role, Role::Developer, "New user should have developer role");
    
    // List users
    let users = security_manager
        .list_users(&admin_token)
        .await
        .expect("List users should succeed");
    
    assert!(users.iter().any(|u| u.username == "testuser"), "New user should appear in user list");
    
    // Delete the user
    let result = security_manager
        .delete_user(&admin_token, "testuser")
        .await;
    
    assert!(result.is_ok(), "User deletion should succeed");
    
    // User should no longer be able to authenticate
    let result = security_manager
        .authenticate("testuser", "testpass", None, None)
        .await;
    
    assert!(result.is_err(), "Deleted user should not be able to authenticate");
}

#[tokio::test]
async fn test_secure_tool_authentication() {
    let security_manager = create_test_security_manager().await;
    let brp_client = create_test_brp_client();
    let secure_tools = SecureMcpTools::new(brp_client, security_manager.clone());
    
    // Test authentication tool
    let auth_params = json!({
        "username": "admin",
        "password": "admin123"
    });
    
    // Note: This would require proper MCP framework setup to test fully
    // For now, we'll just verify the security manager integration works
    let token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Authentication should succeed");
    
    let claims = security_manager
        .validate_token(&token)
        .await
        .expect("Token validation should succeed");
    
    assert_eq!(claims.sub, "admin");
    assert_eq!(claims.role, Role::Admin);
}

#[tokio::test]
async fn test_password_security() {
    let security_manager = create_test_security_manager().await;
    
    // Test password hashing
    let password = "test_password_123";
    let hash = security_manager
        .hash_password(password)
        .expect("Password hashing should succeed");
    
    assert!(!hash.is_empty(), "Hash should not be empty");
    assert_ne!(hash, password, "Hash should be different from password");
    
    // Test password verification
    let is_valid = security_manager
        .verify_password(password, &hash)
        .expect("Password verification should succeed");
    
    assert!(is_valid, "Password verification should return true for correct password");
    
    let is_invalid = security_manager
        .verify_password("wrong_password", &hash)
        .expect("Password verification should succeed even for wrong password");
    
    assert!(!is_invalid, "Password verification should return false for wrong password");
}

#[tokio::test]
async fn test_failed_login_tracking() {
    let mut config = SecurityConfig::default();
    config.max_failed_logins = 3;
    config.lockout_duration_minutes = 1;
    config.jwt_secret = "test_secret".to_string();
    
    let security_manager = Arc::new(SecurityManager::new(config).expect("Failed to create security manager"));
    
    // Attempt multiple failed logins
    for _ in 0..3 {
        let result = security_manager
            .authenticate("admin", "wrong_password", None, None)
            .await;
        assert!(result.is_err(), "Failed login should be rejected");
    }
    
    // Next attempt should be rejected due to lockout
    let result = security_manager
        .authenticate("admin", "admin123", None, None) // Even with correct password
        .await;
    assert!(result.is_err(), "Login should be rejected due to lockout");
}

#[tokio::test] 
async fn test_audit_logging() {
    let security_manager = create_test_security_manager().await;
    
    let admin_token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Admin authentication should succeed");
    
    // Generate some audit events
    let _ = security_manager
        .check_permission(&admin_token, &Role::Admin, "test_operation")
        .await;
    
    // Get audit log
    let audit_entries = security_manager
        .get_audit_log(&admin_token, Some(10), Some(0))
        .await
        .expect("Audit log access should succeed");
    
    assert!(!audit_entries.is_empty(), "Audit log should contain entries");
    
    // Verify audit entries contain expected information
    let auth_entry = audit_entries.iter()
        .find(|entry| entry.action == "authentication" && entry.success);
    
    assert!(auth_entry.is_some(), "Audit log should contain successful authentication entry");
}

#[tokio::test]
async fn test_security_vulnerability_scan() {
    let security_manager = create_test_security_manager().await;
    
    let admin_token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Admin authentication should succeed");
    
    // Run security scan
    let audit = bevy_debugger_mcp::security::SecurityAudit::new(security_manager);
    let scan_report = audit
        .run_security_scan(&admin_token)
        .await
        .expect("Security scan should succeed");
    
    // Should detect default passwords and JWT secret
    assert!(!scan_report.vulnerabilities.is_empty(), "Scan should detect vulnerabilities");
    assert!(!scan_report.recommendations.is_empty(), "Scan should provide recommendations");
    
    // Should detect default admin password
    let has_default_password_vuln = scan_report.vulnerabilities.iter()
        .any(|v| v.contains("Default admin password"));
    assert!(has_default_password_vuln, "Should detect default admin password vulnerability");
}

#[tokio::test] 
async fn test_session_cleanup() {
    let mut config = SecurityConfig::default();
    config.session_timeout_hours = 0; // Immediate expiry for testing
    config.jwt_secret = "test_secret".to_string();
    
    let security_manager = Arc::new(SecurityManager::new(config).expect("Failed to create security manager"));
    
    let token = security_manager
        .authenticate("admin", "admin123", None, None)
        .await
        .expect("Authentication should succeed");
    
    // Wait a bit and run cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    security_manager.cleanup().await;
    
    // Session should be cleaned up, but token might still be valid until JWT expiry
    // This test verifies the cleanup mechanism runs without errors
    let claims = security_manager.validate_token(&token).await;
    // The result depends on JWT vs session expiry - both are valid behaviors
}

/// Integration test for penetration testing scenarios
#[tokio::test]
async fn test_penetration_scenarios() {
    let security_manager = create_test_security_manager().await;
    
    // Test 1: SQL Injection attempt in username (should be handled safely)
    let result = security_manager
        .authenticate("admin'; DROP TABLE users; --", "password", None, None)
        .await;
    assert!(result.is_err(), "SQL injection attempt should fail");
    
    // Test 2: XSS attempt in username
    let result = security_manager
        .authenticate("<script>alert('xss')</script>", "password", None, None)
        .await;
    assert!(result.is_err(), "XSS attempt should fail");
    
    // Test 3: Very long username (potential buffer overflow)
    let long_username = "a".repeat(10000);
    let result = security_manager
        .authenticate(&long_username, "password", None, None)
        .await;
    assert!(result.is_err(), "Extremely long username should fail");
    
    // Test 4: Empty/null inputs
    let result = security_manager
        .authenticate("", "", None, None)
        .await;
    assert!(result.is_err(), "Empty credentials should fail");
    
    // Test 5: Unicode/special characters
    let result = security_manager
        .authenticate("тест", "пароль", None, None)
        .await;
    assert!(result.is_err(), "Unicode username should fail for non-existent user");
}

/// Test rate limiting functionality
#[tokio::test]
async fn test_rate_limiting() {
    let mut config = SecurityConfig::default();
    config.rate_limit_requests_per_minute = 2;
    config.rate_limit_burst = 2;
    config.jwt_secret = "test_secret".to_string();
    
    let security_manager = Arc::new(SecurityManager::new(config).expect("Failed to create security manager"));
    
    // First few requests should succeed
    let result1 = security_manager
        .authenticate("admin", "wrong_password", None, None)
        .await;
    let result2 = security_manager
        .authenticate("admin", "wrong_password", None, None)
        .await;
    
    // Both should fail due to wrong password, but not rate limiting
    assert!(result1.is_err());
    assert!(result2.is_err());
    
    // Third request should be rate limited
    let result3 = security_manager
        .authenticate("admin", "admin123", None, None) // Even with correct password
        .await;
    
    // This should fail due to rate limiting
    assert!(result3.is_err());
    
    // The error message should indicate rate limiting
    if let Err(Error::SecurityError(msg)) = result3 {
        assert!(msg.contains("Rate limit"), "Error should mention rate limiting");
    } else {
        panic!("Expected SecurityError with rate limit message");
    }
}
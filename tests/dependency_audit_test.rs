// Tests for Story 4: Dependency Audit
// This test verifies that the security improvements have been properly implemented

#[cfg(test)]
mod dependency_audit_tests {
    use std::process::Command;

    #[test]
    fn test_no_md5_dependency() {
        // Verify that md5 is not in our dependencies
        let output = Command::new("cargo")
            .args(&["tree", "-p", "md5"])
            .output()
            .expect("Failed to execute cargo tree");
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // The output should indicate md5 is not found
        assert!(
            stdout.contains("error") || stdout.is_empty() || stdout.contains("not found"),
            "md5 dependency should not be present in the project"
        );
    }

    #[test]
    fn test_sha2_dependency_present() {
        // Verify that sha2 is in our dependencies (as we're using SHA-256 now)
        let output = Command::new("cargo")
            .args(&["tree", "-p", "sha2"])
            .output()
            .expect("Failed to execute cargo tree");
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // sha2 should be present
        assert!(
            stdout.contains("sha2"),
            "sha2 dependency should be present for SHA-256 hashing"
        );
    }

    #[test]
    fn test_is_terminal_dependency_present() {
        // Verify we're using is-terminal instead of the deprecated atty
        let output = Command::new("cargo")
            .args(&["tree", "-p", "is-terminal"])
            .output()
            .expect("Failed to execute cargo tree");
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        assert!(
            stdout.contains("is-terminal"),
            "is-terminal dependency should be present (not deprecated atty)"
        );
    }

    #[test]
    fn test_no_atty_dependency() {
        // Verify that deprecated atty is not in our dependencies
        let output = Command::new("cargo")
            .args(&["tree", "-p", "atty"])
            .output()
            .expect("Failed to execute cargo tree");
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        assert!(
            stdout.contains("error") || stdout.is_empty() || stdout.contains("not found"),
            "atty dependency should not be present (deprecated)"
        );
    }
}

#[cfg(test)]
mod hot_reload_hash_tests {
    use sha2::{Sha256, Digest};

    #[test]
    fn test_sha256_hashing_works() {
        // Test that SHA-256 hashing is working correctly
        let test_data = b"test content for hashing";
        let mut hasher = Sha256::new();
        hasher.update(test_data);
        let result = format!("{:x}", hasher.finalize());
        
        // Verify we get a proper SHA-256 hash (64 hex characters)
        assert_eq!(result.len(), 64, "SHA-256 hash should be 64 hex characters");
        
        // Verify deterministic hashing
        let mut hasher2 = Sha256::new();
        hasher2.update(test_data);
        let result2 = format!("{:x}", hasher2.finalize());
        
        assert_eq!(result, result2, "SHA-256 should produce deterministic results");
    }

    #[test]
    fn test_sha256_different_inputs() {
        // Test that different inputs produce different hashes
        let data1 = b"content1";
        let data2 = b"content2";
        
        let mut hasher1 = Sha256::new();
        hasher1.update(data1);
        let hash1 = format!("{:x}", hasher1.finalize());
        
        let mut hasher2 = Sha256::new();
        hasher2.update(data2);
        let hash2 = format!("{:x}", hasher2.finalize());
        
        assert_ne!(hash1, hash2, "Different inputs should produce different hashes");
    }
}
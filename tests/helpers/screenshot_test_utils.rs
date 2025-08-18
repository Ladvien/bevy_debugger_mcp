use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Duration, Instant};
use sha2::{Sha256, Digest};

/// Screenshot validation and comparison utilities for E2E testing
#[derive(Debug, Clone)]
pub struct ScreenshotValidator {
    /// Directory containing reference/expected screenshots
    pub reference_dir: PathBuf,
    /// Directory for test output screenshots  
    pub output_dir: PathBuf,
    /// Directory for diff images when comparisons fail
    pub diff_dir: PathBuf,
    /// Tolerance for pixel differences (0.0 = exact match, 1.0 = completely different)
    pub pixel_tolerance: f32,
    /// Maximum percentage of different pixels allowed
    pub max_diff_percentage: f32,
}

impl ScreenshotValidator {
    /// Create a new screenshot validator
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let base = base_dir.as_ref();
        Self {
            reference_dir: base.join("reference_screenshots"),
            output_dir: base.join("test_output"),
            diff_dir: base.join("diffs"),
            pixel_tolerance: 0.02, // 2% tolerance for slight rendering differences
            max_diff_percentage: 1.0, // Allow 1% of pixels to be different
        }
    }

    /// Initialize directories for screenshot testing
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::create_dir_all(&self.reference_dir)?;
        fs::create_dir_all(&self.output_dir)?;
        fs::create_dir_all(&self.diff_dir)?;
        Ok(())
    }

    /// Validate that a screenshot file exists and has reasonable properties
    pub fn validate_screenshot_file<P: AsRef<Path>>(&self, path: P) -> Result<ScreenshotInfo, ScreenshotError> {
        let path = path.as_ref();
        
        if !path.exists() {
            return Err(ScreenshotError::FileNotFound(path.to_path_buf()));
        }

        let metadata = fs::metadata(path)
            .map_err(|e| ScreenshotError::IoError(e))?;

        if metadata.len() == 0 {
            return Err(ScreenshotError::EmptyFile(path.to_path_buf()));
        }

        // Basic file size validation (reasonable bounds)
        if metadata.len() < 1000 {
            return Err(ScreenshotError::FileTooSmall(path.to_path_buf(), metadata.len()));
        }

        if metadata.len() > 50 * 1024 * 1024 {
            return Err(ScreenshotError::FileTooLarge(path.to_path_buf(), metadata.len()));
        }

        // Validate image format by reading header
        let is_valid_image = self.validate_image_format(path)?;
        if !is_valid_image {
            return Err(ScreenshotError::InvalidImageFormat(path.to_path_buf()));
        }

        Ok(ScreenshotInfo {
            path: path.to_path_buf(),
            size_bytes: metadata.len(),
            created: metadata.created().ok(),
            hash: self.compute_file_hash(path)?,
        })
    }

    /// Compare two screenshots for visual similarity
    pub fn compare_screenshots<P: AsRef<Path>, Q: AsRef<Path>>(
        &self, 
        reference: P, 
        actual: Q
    ) -> Result<ComparisonResult, ScreenshotError> {
        let ref_path = reference.as_ref();
        let actual_path = actual.as_ref();

        // First validate both files exist and are valid
        let ref_info = self.validate_screenshot_file(ref_path)?;
        let actual_info = self.validate_screenshot_file(actual_path)?;

        // Quick hash comparison for identical files
        if ref_info.hash == actual_info.hash {
            return Ok(ComparisonResult {
                identical: true,
                similarity_score: 1.0,
                different_pixels: 0,
                total_pixels: 0,
                diff_percentage: 0.0,
                reference_info: ref_info,
                actual_info,
                diff_image_path: None,
            });
        }

        // If we have image processing capabilities, do pixel comparison
        // For now, we'll use a simple approach
        self.compare_image_pixels(ref_path, actual_path)
    }

    /// Wait for a screenshot file to be created with timeout
    pub async fn wait_for_screenshot<P: AsRef<Path>>(
        &self, 
        path: P, 
        timeout: Duration
    ) -> Result<ScreenshotInfo, ScreenshotError> {
        let path = path.as_ref();
        let start = Instant::now();

        while start.elapsed() < timeout {
            if path.exists() {
                // File exists, but wait a moment for write completion
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                match self.validate_screenshot_file(path) {
                    Ok(info) => return Ok(info),
                    Err(ScreenshotError::FileTooSmall(..)) => {
                        // File might still be writing, continue waiting
                        tokio::time::sleep(Duration::from_millis(200)).await;
                        continue;
                    }
                    Err(e) => return Err(e),
                }
            }
            
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        Err(ScreenshotError::Timeout(path.to_path_buf(), timeout))
    }

    /// Generate a test screenshot path
    pub fn test_screenshot_path(&self, test_name: &str) -> PathBuf {
        self.output_dir.join(format!("{}.png", test_name))
    }

    /// Generate a reference screenshot path  
    pub fn reference_screenshot_path(&self, test_name: &str) -> PathBuf {
        self.reference_dir.join(format!("{}_reference.png", test_name))
    }

    /// Save a reference screenshot for future comparisons
    pub fn save_reference<P: AsRef<Path>, Q: AsRef<Path>>(
        &self, 
        source: P, 
        reference_name: Q
    ) -> Result<(), ScreenshotError> {
        let source = source.as_ref();
        let dest = self.reference_dir.join(reference_name.as_ref());
        
        fs::copy(source, &dest)
            .map_err(|e| ScreenshotError::IoError(e))?;
            
        println!("Saved reference screenshot: {}", dest.display());
        Ok(())
    }

    /// Compute SHA-256 hash of a file
    fn compute_file_hash<P: AsRef<Path>>(&self, path: P) -> Result<String, ScreenshotError> {
        let contents = fs::read(path.as_ref())
            .map_err(|e| ScreenshotError::IoError(e))?;
        
        let mut hasher = Sha256::new();
        hasher.update(&contents);
        let hash = hasher.finalize();
        
        Ok(format!("{:x}", hash))
    }

    /// Validate image format by checking file headers
    fn validate_image_format<P: AsRef<Path>>(&self, path: P) -> Result<bool, ScreenshotError> {
        let mut file = fs::File::open(path.as_ref())
            .map_err(|e| ScreenshotError::IoError(e))?;
        
        use std::io::Read;
        let mut header = [0u8; 8];
        file.read_exact(&mut header)
            .map_err(|e| ScreenshotError::IoError(e))?;

        // Check for common image format signatures
        let is_png = header.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
        let is_jpeg = header.starts_with(&[0xFF, 0xD8, 0xFF]);
        let is_bmp = header.starts_with(&[0x42, 0x4D]);
        
        Ok(is_png || is_jpeg || is_bmp)
    }

    /// Compare image pixels (simplified implementation)
    fn compare_image_pixels<P: AsRef<Path>, Q: AsRef<Path>>(
        &self, 
        _reference: P, 
        _actual: Q
    ) -> Result<ComparisonResult, ScreenshotError> {
        // For now, return a placeholder result
        // In a full implementation, you would use an image processing library
        // like image-rs to do pixel-by-pixel comparison
        
        Err(ScreenshotError::ComparisonNotImplemented)
    }
}

/// Information about a screenshot file
#[derive(Debug, Clone)]
pub struct ScreenshotInfo {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub created: Option<std::time::SystemTime>,
    pub hash: String,
}

/// Result of comparing two screenshots
#[derive(Debug, Clone)]
pub struct ComparisonResult {
    pub identical: bool,
    pub similarity_score: f32, // 0.0 = completely different, 1.0 = identical
    pub different_pixels: u64,
    pub total_pixels: u64, 
    pub diff_percentage: f32,
    pub reference_info: ScreenshotInfo,
    pub actual_info: ScreenshotInfo,
    pub diff_image_path: Option<PathBuf>,
}

impl ComparisonResult {
    /// Check if the comparison passes based on configured tolerances
    pub fn passes(&self, max_diff_percentage: f32, min_similarity: f32) -> bool {
        self.identical || 
        (self.diff_percentage <= max_diff_percentage && self.similarity_score >= min_similarity)
    }
}

/// Errors that can occur during screenshot validation and comparison
#[derive(Debug, thiserror::Error)]
pub enum ScreenshotError {
    #[error("Screenshot file not found: {0}")]
    FileNotFound(PathBuf),
    
    #[error("Screenshot file is empty: {0}")]
    EmptyFile(PathBuf),
    
    #[error("Screenshot file too small: {0} ({1} bytes)")]
    FileTooSmall(PathBuf, u64),
    
    #[error("Screenshot file too large: {0} ({1} bytes)")]  
    FileTooLarge(PathBuf, u64),
    
    #[error("Invalid image format: {0}")]
    InvalidImageFormat(PathBuf),
    
    #[error("Timeout waiting for screenshot: {0} (waited {1:?})")]
    Timeout(PathBuf, Duration),
    
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    
    #[error("Image comparison not implemented")]
    ComparisonNotImplemented,
}

/// Utility macros for screenshot assertions in tests
#[macro_export]
macro_rules! assert_screenshot_exists {
    ($validator:expr, $path:expr) => {
        match $validator.validate_screenshot_file($path) {
            Ok(_) => (),
            Err(e) => panic!("Screenshot validation failed: {}", e),
        }
    };
}

#[macro_export]
macro_rules! assert_screenshot_similar {
    ($validator:expr, $reference:expr, $actual:expr) => {
        match $validator.compare_screenshots($reference, $actual) {
            Ok(result) => {
                if !result.passes($validator.max_diff_percentage, 1.0 - $validator.pixel_tolerance) {
                    panic!(
                        "Screenshots not similar enough: {}% different (max: {}%)",
                        result.diff_percentage, $validator.max_diff_percentage
                    );
                }
            }
            Err(e) => panic!("Screenshot comparison failed: {}", e),
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_validator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let validator = ScreenshotValidator::new(temp_dir.path());
        
        assert!(validator.reference_dir.ends_with("reference_screenshots"));
        assert!(validator.output_dir.ends_with("test_output"));
        assert!(validator.diff_dir.ends_with("diffs"));
    }

    #[test]
    fn test_validator_initialization() {
        let temp_dir = TempDir::new().unwrap();
        let validator = ScreenshotValidator::new(temp_dir.path());
        
        validator.initialize().unwrap();
        
        assert!(validator.reference_dir.exists());
        assert!(validator.output_dir.exists());
        assert!(validator.diff_dir.exists());
    }

    #[test]
    fn test_screenshot_paths() {
        let temp_dir = TempDir::new().unwrap();
        let validator = ScreenshotValidator::new(temp_dir.path());
        
        let test_path = validator.test_screenshot_path("my_test");
        let ref_path = validator.reference_screenshot_path("my_test");
        
        assert!(test_path.ends_with("my_test.png"));
        assert!(ref_path.ends_with("my_test_reference.png"));
    }
}
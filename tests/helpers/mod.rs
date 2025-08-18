/// Test helper utilities for E2E screenshot testing

pub mod screenshot_test_utils;

pub use screenshot_test_utils::{
    ScreenshotValidator, 
    ScreenshotInfo, 
    ComparisonResult, 
    ScreenshotError
};
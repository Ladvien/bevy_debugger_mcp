use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

use crate::dead_letter_queue::{DeadLetterQueue, DeadLetterStats};
use crate::error::{ErrorContext, Result};

/// System information for diagnostic reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub arch: String,
    pub hostname: String,
    pub rust_version: String,
    pub crate_version: String,
    pub uptime_seconds: u64,
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: f32,
}

/// Environment information for debugging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    pub environment_variables: HashMap<String, String>,
    pub working_directory: String,
    pub config_values: HashMap<String, String>,
    pub active_features: Vec<String>,
}

/// Performance metrics snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSnapshot {
    pub timestamp: u64,
    pub memory_usage_bytes: u64,
    pub cpu_usage_percent: f32,
    pub active_connections: u32,
    pub request_count_last_minute: u32,
    pub error_count_last_minute: u32,
    pub avg_response_time_ms: f64,
}

/// Recent error summary for diagnostics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorSummary {
    pub total_errors: u32,
    pub error_by_severity: HashMap<String, u32>,
    pub error_by_component: HashMap<String, u32>,
    pub recent_errors: Vec<ErrorContext>,
    pub dead_letter_stats: Option<DeadLetterStats>,
}

/// Comprehensive diagnostic report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub report_id: String,
    pub generated_at: u64,
    pub system_info: SystemInfo,
    pub environment_info: EnvironmentInfo,
    pub performance_snapshot: PerformanceSnapshot,
    pub error_summary: ErrorSummary,
    pub recent_logs: Vec<String>,
    pub configuration_dump: HashMap<String, String>,
    pub health_checks: HashMap<String, bool>,
}

/// Diagnostic data collector for bug reports
#[derive(Debug)]
pub struct DiagnosticCollector {
    recent_errors: std::sync::Arc<std::sync::RwLock<Vec<ErrorContext>>>,
    max_errors: usize,
    start_time: SystemTime,
}

impl DiagnosticCollector {
    pub fn new(max_errors: usize) -> Self {
        Self {
            recent_errors: std::sync::Arc::new(std::sync::RwLock::new(Vec::new())),
            max_errors,
            start_time: SystemTime::now(),
        }
    }

    /// Record an error for diagnostic purposes
    pub fn record_error(&self, error_context: ErrorContext) {
        let mut errors = self.recent_errors.write().unwrap();

        // Add the new error
        errors.push(error_context);

        // Keep only the most recent errors
        if errors.len() > self.max_errors {
            let excess = errors.len() - self.max_errors;
            errors.drain(0..excess);
        }

        debug!(
            "Recorded error for diagnostics. Total errors: {}",
            errors.len()
        );
    }

    /// Generate a comprehensive diagnostic report
    pub async fn generate_report(
        &self,
        dead_letter_queue: Option<&DeadLetterQueue>,
    ) -> Result<DiagnosticReport> {
        let report_id = uuid::Uuid::new_v4().to_string();
        let generated_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        info!("Generating diagnostic report: {}", report_id);

        let system_info = self.collect_system_info().await?;
        let environment_info = self.collect_environment_info().await?;
        let performance_snapshot = self.collect_performance_snapshot().await?;
        let error_summary = self.collect_error_summary(dead_letter_queue).await?;
        let recent_logs = self.collect_recent_logs().await?;
        let configuration_dump = self.collect_configuration_dump().await?;
        let health_checks = self.collect_health_checks().await?;

        let report = DiagnosticReport {
            report_id,
            generated_at,
            system_info,
            environment_info,
            performance_snapshot,
            error_summary,
            recent_logs,
            configuration_dump,
            health_checks,
        };

        info!(
            "Diagnostic report generated successfully: {}",
            report.report_id
        );
        Ok(report)
    }

    /// Export diagnostic report to JSON
    pub async fn export_report_json(&self, report: &DiagnosticReport) -> Result<String> {
        serde_json::to_string_pretty(report).map_err(Into::into)
    }

    /// Save diagnostic report to file
    pub async fn save_report_to_file(
        &self,
        report: &DiagnosticReport,
        file_path: &str,
    ) -> Result<()> {
        let json = self.export_report_json(report).await?;
        tokio::fs::write(file_path, json).await?;
        info!("Diagnostic report saved to: {}", file_path);
        Ok(())
    }

    async fn collect_system_info(&self) -> Result<SystemInfo> {
        let uptime = self.start_time.elapsed().unwrap_or_default().as_secs();

        // Try to get system information
        let (memory_usage, cpu_usage) = self.get_system_metrics().await;

        Ok(SystemInfo {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            hostname: hostname::get()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            rust_version: rustc_version_runtime::version().to_string(),
            crate_version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: uptime,
            memory_usage_bytes: memory_usage,
            cpu_usage_percent: cpu_usage,
        })
    }

    async fn collect_environment_info(&self) -> Result<EnvironmentInfo> {
        let mut env_vars = HashMap::new();

        // Collect only safe environment variables (allowlist approach)
        for (key, value) in std::env::vars() {
            if Self::is_safe_env_var(&key) {
                env_vars.insert(key, value);
            }
            // Don't collect sensitive variables at all, not even redacted
        }

        let working_directory = std::env::current_dir()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        Ok(EnvironmentInfo {
            environment_variables: env_vars,
            working_directory,
            config_values: HashMap::new(), // TODO: Add config collection
            active_features: vec![],       // TODO: Add feature detection
        })
    }

    async fn collect_performance_snapshot(&self) -> Result<PerformanceSnapshot> {
        let (memory_usage, cpu_usage) = self.get_system_metrics().await;

        Ok(PerformanceSnapshot {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            memory_usage_bytes: memory_usage,
            cpu_usage_percent: cpu_usage,
            active_connections: 0,        // TODO: Get from connection manager
            request_count_last_minute: 0, // TODO: Get from metrics
            error_count_last_minute: 0,   // TODO: Get from metrics
            avg_response_time_ms: 0.0,    // TODO: Get from metrics
        })
    }

    async fn collect_error_summary(
        &self,
        dead_letter_queue: Option<&DeadLetterQueue>,
    ) -> Result<ErrorSummary> {
        let errors = self.recent_errors.read().unwrap().clone();

        let mut error_by_severity = HashMap::new();
        let mut error_by_component = HashMap::new();

        for error in &errors {
            // Count by severity
            let severity_key = format!("{:?}", error.severity);
            *error_by_severity.entry(severity_key).or_insert(0) += 1;

            // Count by component
            *error_by_component
                .entry(error.component.clone())
                .or_insert(0) += 1;
        }

        let dead_letter_stats = if let Some(dlq) = dead_letter_queue {
            Some(dlq.get_statistics().await)
        } else {
            None
        };

        Ok(ErrorSummary {
            total_errors: errors.len() as u32,
            error_by_severity,
            error_by_component,
            recent_errors: errors,
            dead_letter_stats,
        })
    }

    async fn collect_recent_logs(&self) -> Result<Vec<String>> {
        // TODO: Implement log collection from tracing subscriber
        Ok(vec!["Log collection not yet implemented".to_string()])
    }

    async fn collect_configuration_dump(&self) -> Result<HashMap<String, String>> {
        let mut config = HashMap::new();

        // Add basic configuration information
        config.insert("max_errors".to_string(), self.max_errors.to_string());
        config.insert(
            "start_time".to_string(),
            self.start_time
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
                .to_string(),
        );

        // TODO: Add more configuration from Config struct

        Ok(config)
    }

    async fn collect_health_checks(&self) -> Result<HashMap<String, bool>> {
        let mut checks = HashMap::new();

        // Basic health checks
        checks.insert("process_running".to_string(), true);
        checks.insert("memory_available".to_string(), true); // TODO: Implement actual check
        checks.insert("disk_space_available".to_string(), true); // TODO: Implement actual check

        // TODO: Add more specific health checks

        Ok(checks)
    }

    async fn get_system_metrics(&self) -> (u64, f32) {
        // Try to get current process memory usage
        let memory = std::process::id() as u64 * 1024; // Placeholder
        let cpu = 1.0; // Placeholder

        // TODO: Use sysinfo or similar for actual metrics
        (memory, cpu)
    }

    fn is_safe_env_var(key: &str) -> bool {
        // Only include safe environment variables
        let safe_prefixes = ["RUST_", "CARGO_", "PATH"];
        let unsafe_keys = ["PASSWORD", "SECRET", "TOKEN", "KEY", "AUTH"];

        let key_upper = key.to_uppercase();

        // Check if it's a safe prefix
        if safe_prefixes
            .iter()
            .any(|prefix| key_upper.starts_with(prefix))
        {
            return true;
        }

        // Check if it contains unsafe keywords
        if unsafe_keys
            .iter()
            .any(|unsafe_key| key_upper.contains(unsafe_key))
        {
            return false;
        }

        // Default to safe for known system variables
        matches!(
            key,
            "USER" | "HOME" | "SHELL" | "TERM" | "PWD" | "LANG" | "LC_ALL"
        )
    }
}

/// Create a formatted bug report from diagnostic data
pub fn create_bug_report(
    report: &DiagnosticReport,
    description: &str,
    steps_to_reproduce: &str,
) -> String {
    format!(
        r#"# Bug Report

## Description
{}

## Steps to Reproduce
{}

## Environment Information
- OS: {} ({})
- Rust Version: {}
- Crate Version: {}
- Hostname: {}
- Uptime: {} seconds

## Performance at Time of Issue
- Memory Usage: {} bytes
- CPU Usage: {:.2}%
- Recent Errors: {}

## Error Summary
{}

## System Health
{}

## Report ID
{}

## Generated At
{}

---
*This bug report was automatically generated by the Bevy Debugger MCP diagnostic system.*
"#,
        description,
        steps_to_reproduce,
        report.system_info.os,
        report.system_info.arch,
        report.system_info.rust_version,
        report.system_info.crate_version,
        report.system_info.hostname,
        report.system_info.uptime_seconds,
        report.performance_snapshot.memory_usage_bytes,
        report.performance_snapshot.cpu_usage_percent,
        report.error_summary.total_errors,
        format_error_summary(&report.error_summary),
        format_health_checks(&report.health_checks),
        report.report_id,
        chrono::DateTime::from_timestamp(report.generated_at as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "Unknown".to_string())
    )
}

fn format_error_summary(summary: &ErrorSummary) -> String {
    let mut result = String::new();

    if !summary.error_by_severity.is_empty() {
        result.push_str("By Severity:\n");
        for (severity, count) in &summary.error_by_severity {
            result.push_str(&format!("  {severity}: {count}\n"));
        }
    }

    if !summary.error_by_component.is_empty() {
        result.push_str("By Component:\n");
        for (component, count) in &summary.error_by_component {
            result.push_str(&format!("  {component}: {count}\n"));
        }
    }

    result
}

fn format_health_checks(checks: &HashMap<String, bool>) -> String {
    let mut result = String::new();
    for (check, status) in checks {
        result.push_str(&format!(
            "  {}: {}\n",
            check,
            if *status { "✓" } else { "✗" }
        ));
    }
    result
}

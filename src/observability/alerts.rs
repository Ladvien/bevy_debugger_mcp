/*
 * Bevy Debugger MCP - Alert Rules and Definitions
 * Copyright (C) 2025 ladvien
 *
 * Production monitoring alert rules and Grafana dashboard configurations
 */

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Alert severity levels
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}

/// Alert rule definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertRule {
    pub name: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub metric: String,
    pub condition: AlertCondition,
    pub threshold: f64,
    pub duration: String, // e.g., "5m", "1h"
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
}

/// Alert condition types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertCondition {
    GreaterThan,
    LessThan,
    Equal,
    NotEqual,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

/// Comprehensive alert rules for production monitoring
pub struct AlertRules;

impl AlertRules {
    /// Get all production alert rules
    pub fn get_production_rules() -> Vec<AlertRule> {
        vec![
            Self::high_request_latency(),
            Self::high_error_rate(),
            Self::brp_connection_down(),
            Self::high_memory_usage(),
            Self::high_cpu_usage(),
            Self::service_unavailable(),
            Self::panic_detected(),
            Self::connection_pool_exhausted(),
            Self::disk_space_low(),
            Self::high_request_rate(),
            Self::long_running_operations(),
            Self::health_check_failing(),
        ]
    }

    /// High request latency alert (p99 > 5s)
    fn high_request_latency() -> AlertRule {
        AlertRule {
            name: "HighRequestLatency".to_string(),
            description: "MCP request latency p99 is above 5 seconds".to_string(),
            severity: AlertSeverity::Warning,
            metric: "histogram_quantile(0.99, mcp_request_duration_seconds)".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 5.0,
            duration: "2m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "request-handling".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "High request latency detected".to_string()),
                ("description".to_string(), "P99 latency is {{ $value }}s, exceeding 5s threshold".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/high-latency".to_string()),
            ]),
        }
    }

    /// High error rate alert (>5% errors)
    fn high_error_rate() -> AlertRule {
        AlertRule {
            name: "HighErrorRate".to_string(),
            description: "Error rate is above 5% for sustained period".to_string(),
            severity: AlertSeverity::Critical,
            metric: "rate(mcp_errors_total[5m]) / rate(mcp_requests_total[5m]) * 100".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 5.0,
            duration: "1m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "error-handling".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "High error rate detected".to_string()),
                ("description".to_string(), "Error rate is {{ $value }}%, exceeding 5% threshold".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/high-errors".to_string()),
            ]),
        }
    }

    /// BRP connection down alert
    fn brp_connection_down() -> AlertRule {
        AlertRule {
            name: "BRPConnectionDown".to_string(),
            description: "BRP connection is not healthy".to_string(),
            severity: AlertSeverity::Critical,
            metric: "brp_connection_status".to_string(),
            condition: AlertCondition::LessThan,
            threshold: 1.0,
            duration: "30s".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "brp-connection".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "BRP connection is down".to_string()),
                ("description".to_string(), "BRP connection health check is failing".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/brp-connection".to_string()),
            ]),
        }
    }

    /// High memory usage alert (>80%)
    fn high_memory_usage() -> AlertRule {
        AlertRule {
            name: "HighMemoryUsage".to_string(),
            description: "Memory usage is above 80%".to_string(),
            severity: AlertSeverity::Warning,
            metric: "process_memory_usage_bytes / (1024*1024*1024)".to_string(), // Convert to GB
            condition: AlertCondition::GreaterThan,
            threshold: 0.8, // 800MB
            duration: "5m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "system-resources".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "High memory usage detected".to_string()),
                ("description".to_string(), "Memory usage is {{ $value }}GB, approaching limits".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/high-memory".to_string()),
            ]),
        }
    }

    /// High CPU usage alert (>70%)
    fn high_cpu_usage() -> AlertRule {
        AlertRule {
            name: "HighCPUUsage".to_string(),
            description: "CPU usage is above 70%".to_string(),
            severity: AlertSeverity::Warning,
            metric: "process_cpu_usage_percent".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 70.0,
            duration: "3m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "system-resources".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "High CPU usage detected".to_string()),
                ("description".to_string(), "CPU usage is {{ $value }}%, exceeding 70% threshold".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/high-cpu".to_string()),
            ]),
        }
    }

    /// Service unavailable alert
    fn service_unavailable() -> AlertRule {
        AlertRule {
            name: "ServiceUnavailable".to_string(),
            description: "Service health check is failing".to_string(),
            severity: AlertSeverity::Critical,
            metric: "mcp_health_status".to_string(),
            condition: AlertCondition::LessThan,
            threshold: 1.0,
            duration: "1m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "health-check".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "Service is unavailable".to_string()),
                ("description".to_string(), "Health check status is {{ $value }}, service may be down".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/service-down".to_string()),
            ]),
        }
    }

    /// Panic detected alert
    fn panic_detected() -> AlertRule {
        AlertRule {
            name: "PanicDetected".to_string(),
            description: "Application panic has been detected".to_string(),
            severity: AlertSeverity::Critical,
            metric: "increase(mcp_panics_total[1m])".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 0.0,
            duration: "0s".to_string(), // Immediate alert
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "stability".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "Application panic detected".to_string()),
                ("description".to_string(), "{{ $value }} panic(s) detected in the last minute".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/panic".to_string()),
            ]),
        }
    }

    /// Connection pool exhausted alert
    fn connection_pool_exhausted() -> AlertRule {
        AlertRule {
            name: "ConnectionPoolExhausted".to_string(),
            description: "Connection pool is nearly exhausted".to_string(),
            severity: AlertSeverity::Warning,
            metric: "mcp_connections_active".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 90.0, // Assuming max 100 connections
            duration: "2m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "connection-pool".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "Connection pool nearly exhausted".to_string()),
                ("description".to_string(), "{{ $value }} active connections, approaching pool limit".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/connection-pool".to_string()),
            ]),
        }
    }

    /// Low disk space alert
    fn disk_space_low() -> AlertRule {
        AlertRule {
            name: "DiskSpaceLow".to_string(),
            description: "Available disk space is running low".to_string(),
            severity: AlertSeverity::Warning,
            metric: "node_filesystem_avail_bytes / node_filesystem_size_bytes * 100".to_string(),
            condition: AlertCondition::LessThan,
            threshold: 20.0, // Less than 20% free space
            duration: "5m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "storage".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "Low disk space".to_string()),
                ("description".to_string(), "Only {{ $value }}% disk space remaining".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/disk-space".to_string()),
            ]),
        }
    }

    /// High request rate alert (potential DoS)
    fn high_request_rate() -> AlertRule {
        AlertRule {
            name: "HighRequestRate".to_string(),
            description: "Unusually high request rate detected".to_string(),
            severity: AlertSeverity::Warning,
            metric: "rate(mcp_requests_total[1m]) * 60".to_string(), // Requests per minute
            condition: AlertCondition::GreaterThan,
            threshold: 1000.0, // More than 1000 requests/min
            duration: "2m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "rate-limiting".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "High request rate detected".to_string()),
                ("description".to_string(), "{{ $value }} requests/min, unusually high traffic".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/high-traffic".to_string()),
            ]),
        }
    }

    /// Long running operations alert
    fn long_running_operations() -> AlertRule {
        AlertRule {
            name: "LongRunningOperations".to_string(),
            description: "Operations taking longer than expected".to_string(),
            severity: AlertSeverity::Warning,
            metric: "histogram_quantile(0.95, mcp_tool_duration_seconds)".to_string(),
            condition: AlertCondition::GreaterThan,
            threshold: 30.0, // 30 seconds
            duration: "1m".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "tool-execution".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "Long running operations detected".to_string()),
                ("description".to_string(), "P95 tool execution time is {{ $value }}s".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/long-operations".to_string()),
            ]),
        }
    }

    /// Health check failing alert
    fn health_check_failing() -> AlertRule {
        AlertRule {
            name: "HealthCheckFailing".to_string(),
            description: "Health check endpoint is failing".to_string(),
            severity: AlertSeverity::Critical,
            metric: "up".to_string(), // Prometheus up metric
            condition: AlertCondition::LessThan,
            threshold: 1.0,
            duration: "30s".to_string(),
            labels: HashMap::from([
                ("service".to_string(), "bevy-debugger-mcp".to_string()),
                ("component".to_string(), "health-endpoint".to_string()),
            ]),
            annotations: HashMap::from([
                ("summary".to_string(), "Health check endpoint failing".to_string()),
                ("description".to_string(), "Health check endpoint is not responding".to_string()),
                ("runbook_url".to_string(), "https://docs.example.com/runbooks/health-check".to_string()),
            ]),
        }
    }

    /// Export alert rules in Prometheus format
    pub fn export_prometheus_rules() -> String {
        let rules = Self::get_production_rules();
        let mut output = String::new();
        
        output.push_str("groups:\n");
        output.push_str("  - name: bevy_debugger_mcp_alerts\n");
        output.push_str("    rules:\n");
        
        for rule in rules {
            output.push_str(&format!("      - alert: {}\n", rule.name));
            output.push_str(&format!("        expr: {} {} {}\n", 
                rule.metric, 
                Self::condition_to_operator(&rule.condition),
                rule.threshold
            ));
            output.push_str(&format!("        for: {}\n", rule.duration));
            
            output.push_str("        labels:\n");
            for (key, value) in rule.labels {
                output.push_str(&format!("          {}: '{}'\n", key, value));
            }
            
            output.push_str("        annotations:\n");
            for (key, value) in rule.annotations {
                output.push_str(&format!("          {}: '{}'\n", key, value));
            }
            
            output.push('\n');
        }
        
        output
    }

    /// Convert alert condition to Prometheus operator
    fn condition_to_operator(condition: &AlertCondition) -> &'static str {
        match condition {
            AlertCondition::GreaterThan => ">",
            AlertCondition::LessThan => "<",
            AlertCondition::Equal => "==",
            AlertCondition::NotEqual => "!=",
            AlertCondition::GreaterThanOrEqual => ">=",
            AlertCondition::LessThanOrEqual => "<=",
        }
    }

    /// Export alert rules in JSON format for API consumption
    pub fn export_json_rules() -> Result<String, serde_json::Error> {
        let rules = Self::get_production_rules();
        serde_json::to_string_pretty(&rules)
    }
}

/// Grafana dashboard configuration
pub struct GrafanaDashboard;

impl GrafanaDashboard {
    /// Generate Grafana dashboard JSON for MCP server monitoring
    pub fn generate_dashboard() -> serde_json::Value {
        serde_json::json!({
            "dashboard": {
                "id": null,
                "title": "Bevy Debugger MCP Server Monitoring",
                "tags": ["bevy", "mcp", "debugging", "observability"],
                "style": "dark",
                "timezone": "browser",
                "editable": true,
                "hideControls": false,
                "graphTooltip": 1,
                "time": {
                    "from": "now-1h",
                    "to": "now"
                },
                "timepicker": {
                    "refresh_intervals": ["5s", "10s", "30s", "1m", "5m", "15m", "30m", "1h", "2h", "1d"],
                    "time_options": ["5m", "15m", "1h", "6h", "12h", "24h", "2d", "7d", "30d"]
                },
                "templating": {
                    "list": [
                        {
                            "name": "instance",
                            "type": "query",
                            "query": "label_values(mcp_requests_total, instance)",
                            "refresh": 1,
                            "multi": false,
                            "includeAll": false
                        }
                    ]
                },
                "panels": [
                    Self::create_request_rate_panel(),
                    Self::create_request_latency_panel(),
                    Self::create_error_rate_panel(),
                    Self::create_active_connections_panel(),
                    Self::create_brp_health_panel(),
                    Self::create_system_resources_panel(),
                    Self::create_tool_performance_panel(),
                    Self::create_health_status_panel(),
                ]
            }
        })
    }

    fn create_request_rate_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 1,
            "title": "Request Rate",
            "type": "graph",
            "span": 6,
            "targets": [
                {
                    "expr": "rate(mcp_requests_total[$__interval]) * 60",
                    "legendFormat": "Requests/min",
                    "refId": "A"
                }
            ],
            "yAxes": [
                {
                    "label": "Requests per minute",
                    "min": 0
                }
            ],
            "xAxis": {
                "mode": "time"
            },
            "tooltip": {
                "shared": true
            }
        })
    }

    fn create_request_latency_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 2,
            "title": "Request Latency",
            "type": "graph",
            "span": 6,
            "targets": [
                {
                    "expr": "histogram_quantile(0.50, mcp_request_duration_seconds)",
                    "legendFormat": "p50",
                    "refId": "A"
                },
                {
                    "expr": "histogram_quantile(0.95, mcp_request_duration_seconds)",
                    "legendFormat": "p95",
                    "refId": "B"
                },
                {
                    "expr": "histogram_quantile(0.99, mcp_request_duration_seconds)",
                    "legendFormat": "p99",
                    "refId": "C"
                }
            ],
            "yAxes": [
                {
                    "label": "Seconds",
                    "min": 0
                }
            ],
            "tooltip": {
                "shared": true
            }
        })
    }

    fn create_error_rate_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 3,
            "title": "Error Rate",
            "type": "graph",
            "span": 6,
            "targets": [
                {
                    "expr": "rate(mcp_errors_total[$__interval]) / rate(mcp_requests_total[$__interval]) * 100",
                    "legendFormat": "Error Rate %",
                    "refId": "A"
                }
            ],
            "yAxes": [
                {
                    "label": "Error Rate (%)",
                    "min": 0,
                    "max": 100
                }
            ],
            "alert": {
                "conditions": [
                    {
                        "query": {
                            "queryType": "",
                            "refId": "A"
                        },
                        "reducer": {
                            "params": [],
                            "type": "last"
                        },
                        "evaluator": {
                            "params": [5],
                            "type": "gt"
                        }
                    }
                ],
                "executionErrorState": "alerting",
                "for": "1m",
                "frequency": "10s",
                "handler": 1,
                "name": "High Error Rate",
                "noDataState": "no_data"
            }
        })
    }

    fn create_active_connections_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 4,
            "title": "Active Connections",
            "type": "singlestat",
            "span": 3,
            "targets": [
                {
                    "expr": "mcp_connections_active",
                    "refId": "A"
                }
            ],
            "valueName": "current",
            "colorBackground": false,
            "colorValue": true,
            "thresholds": "50,90",
            "colors": ["#299c46", "#e5ac0e", "#bf1b00"]
        })
    }

    fn create_brp_health_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 5,
            "title": "BRP Connection Health",
            "type": "singlestat",
            "span": 3,
            "targets": [
                {
                    "expr": "brp_connection_status",
                    "refId": "A"
                }
            ],
            "valueName": "current",
            "colorBackground": true,
            "colorValue": false,
            "thresholds": "0.5,1",
            "colors": ["#bf1b00", "#e5ac0e", "#299c46"],
            "valueMaps": [
                {"value": "0", "text": "DOWN"},
                {"value": "1", "text": "UP"}
            ]
        })
    }

    fn create_system_resources_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 6,
            "title": "System Resources",
            "type": "graph",
            "span": 6,
            "targets": [
                {
                    "expr": "process_cpu_usage_percent",
                    "legendFormat": "CPU Usage %",
                    "refId": "A"
                },
                {
                    "expr": "process_memory_usage_bytes / (1024*1024)",
                    "legendFormat": "Memory Usage MB",
                    "refId": "B"
                }
            ],
            "yAxes": [
                {
                    "label": "Usage",
                    "min": 0
                }
            ]
        })
    }

    fn create_tool_performance_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 7,
            "title": "Tool Performance",
            "type": "graph",
            "span": 6,
            "targets": [
                {
                    "expr": "rate(mcp_tool_requests_total[$__interval]) by (tool)",
                    "legendFormat": "{{tool}} requests/sec",
                    "refId": "A"
                }
            ],
            "yAxes": [
                {
                    "label": "Requests per second",
                    "min": 0
                }
            ]
        })
    }

    fn create_health_status_panel() -> serde_json::Value {
        serde_json::json!({
            "id": 8,
            "title": "Health Status",
            "type": "table",
            "span": 12,
            "targets": [
                {
                    "expr": "mcp_component_health",
                    "format": "table",
                    "refId": "A"
                }
            ],
            "columns": [
                {"text": "Component", "value": "component"},
                {"text": "Status", "value": "Value"},
                {"text": "Time", "value": "Time"}
            ],
            "sort": {"col": 1, "desc": false}
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_rules_generation() {
        let rules = AlertRules::get_production_rules();
        assert!(!rules.is_empty());
        
        // Check that we have the expected critical alerts
        let critical_rules: Vec<_> = rules.iter()
            .filter(|r| r.severity == AlertSeverity::Critical)
            .collect();
        assert!(!critical_rules.is_empty());
        
        // Verify specific rules exist
        assert!(rules.iter().any(|r| r.name == "HighErrorRate"));
        assert!(rules.iter().any(|r| r.name == "BRPConnectionDown"));
        assert!(rules.iter().any(|r| r.name == "ServiceUnavailable"));
    }

    #[test]
    fn test_prometheus_export() {
        let prometheus_rules = AlertRules::export_prometheus_rules();
        assert!(prometheus_rules.contains("groups:"));
        assert!(prometheus_rules.contains("bevy_debugger_mcp_alerts"));
        assert!(prometheus_rules.contains("HighErrorRate"));
    }

    #[test]
    fn test_json_export() {
        let json_rules = AlertRules::export_json_rules();
        assert!(json_rules.is_ok());
        
        let json_str = json_rules.unwrap();
        assert!(json_str.contains("HighErrorRate"));
        assert!(json_str.contains("Critical"));
    }

    #[test]
    fn test_grafana_dashboard() {
        let dashboard = GrafanaDashboard::generate_dashboard();
        assert!(dashboard["dashboard"]["title"].as_str().is_some());
        assert!(dashboard["dashboard"]["panels"].as_array().is_some());
        
        let panels = dashboard["dashboard"]["panels"].as_array().unwrap();
        assert!(!panels.is_empty());
    }

    #[test]
    fn test_condition_operators() {
        assert_eq!(AlertRules::condition_to_operator(&AlertCondition::GreaterThan), ">");
        assert_eq!(AlertRules::condition_to_operator(&AlertCondition::LessThan), "<");
        assert_eq!(AlertRules::condition_to_operator(&AlertCondition::Equal), "==");
    }
}
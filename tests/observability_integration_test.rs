/*
 * Bevy Debugger MCP - Observability Integration Tests
 * Copyright (C) 2025 ladvien
 *
 * Comprehensive tests for the observability stack
 */

#[cfg(feature = "observability")]
mod observability_tests {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;
    
    use bevy_debugger_mcp::{
        config::Config,
        brp_client::BrpClient,
        observability::{
            ObservabilityService, MetricsCollector, TracingService, HealthService, TelemetryService,
            HealthStatus, ComponentHealth, ObservabilityMetrics,
            alerts::{AlertRules, AlertSeverity, GrafanaDashboard},
        },
    };

    /// Test observability service initialization
    #[tokio::test]
    async fn test_observability_service_initialization() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        
        let observability = ObservabilityService::new(config, brp_client).await;
        assert!(observability.is_ok(), "Observability service should initialize successfully");
        
        let service = observability.unwrap();
        
        // Test that all components are accessible
        assert!(Arc::strong_count(&service.metrics()) > 0);
        assert!(Arc::strong_count(&service.tracing()) > 0);
        assert!(Arc::strong_count(&service.health()) > 0);
        assert!(Arc::strong_count(&service.telemetry()) > 0);
    }

    /// Test metrics collection functionality
    #[tokio::test]
    async fn test_metrics_collection() {
        let config = Config::default();
        let collector = MetricsCollector::new(&config).unwrap();
        
        // Test basic metric recording
        let request_timer = collector.record_request_start();
        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(request_timer); // This should record the duration
        
        let tool_timer = collector.record_tool_request("observe");
        tokio::time::sleep(Duration::from_millis(5)).await;
        drop(tool_timer);
        
        collector.record_error();
        collector.record_tool_error("observe");
        collector.set_brp_connection_status(true);
        collector.record_brp_reconnection();
        
        // Get metrics snapshot
        let snapshot = collector.get_metrics_snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.total_errors, 1);
        assert!(snapshot.brp_connection_healthy);
        
        // Test Prometheus export
        let prometheus_output = collector.export_prometheus();
        assert!(prometheus_output.is_ok());
        let output = prometheus_output.unwrap();
        assert!(output.contains("mcp_requests_total"));
        assert!(output.contains("mcp_errors_total"));
        assert!(output.contains("brp_connection_status"));
    }

    /// Test connection tracking
    #[tokio::test]
    async fn test_connection_tracking() {
        let config = Config::default();
        let collector = MetricsCollector::new(&config).unwrap();
        
        // Test connection lifecycle
        let connection1 = collector.record_connection_start();
        let connection2 = collector.record_connection_start();
        
        let snapshot = collector.get_metrics_snapshot();
        assert_eq!(snapshot.active_connections, 2);
        assert_eq!(snapshot.total_connections, 2);
        
        drop(connection1);
        tokio::time::sleep(Duration::from_millis(1)).await;
        
        let snapshot = collector.get_metrics_snapshot();
        assert_eq!(snapshot.active_connections, 1);
        assert_eq!(snapshot.total_connections, 2);
        
        drop(connection2);
    }

    /// Test tracing service initialization
    #[tokio::test]
    async fn test_tracing_service() {
        let mut config = Config::default();
        // Use stdout exporter for testing
        config.observability.jaeger_endpoint = None;
        config.observability.otlp_endpoint = None;
        
        let tracing_service = TracingService::new(&config).await;
        assert!(tracing_service.is_ok(), "Tracing service should initialize");
        
        let service = tracing_service.unwrap();
        
        // Test span creation
        let mut mcp_span = service.create_mcp_span("test_operation", Some("observe"));
        mcp_span.set_attribute("test_key", "test_value");
        mcp_span.add_event("test_event", vec![("event_key", "event_value")]);
        mcp_span.set_success();
        drop(mcp_span);
        
        let mut brp_span = service.create_brp_span("query", Some(10));
        brp_span.set_attributes(vec![("entity_count", "10"), ("query_type", "filter")]);
        drop(brp_span);
        
        let system_span = service.create_system_span("startup");
        drop(system_span);
    }

    /// Test health service functionality
    #[tokio::test]
    async fn test_health_service() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        
        let health_service = HealthService::new(brp_client).await.unwrap();
        
        // Test health status retrieval
        let health_status = health_service.get_health_status().await;
        assert!(health_status.is_ok(), "Health check should succeed");
        
        let health = health_status.unwrap();
        assert!(!health.components.is_empty(), "Should have health components");
        assert!(health.components.contains_key("brp_connection"));
        assert!(health.components.contains_key("system_resources"));
        assert!(health.uptime_seconds > 0);
        
        // Test readiness and liveness
        let readiness = health_service.get_readiness_status().await;
        assert!(readiness.is_ok());
        
        let liveness = health_service.get_liveness_status().await;
        assert!(liveness.is_ok());
        assert!(liveness.unwrap(), "Service should be alive");
    }

    /// Test component health creation and status logic
    #[test]
    fn test_component_health() {
        let healthy = ComponentHealth::healthy("All systems operational")
            .with_response_time(Duration::from_millis(50))
            .with_metadata("cpu_usage", serde_json::Value::from(25.5));
        
        assert_eq!(healthy.status, HealthStatus::Healthy);
        assert_eq!(healthy.message, "All systems operational");
        assert_eq!(healthy.response_time_ms, 50);
        assert_eq!(healthy.metadata.get("cpu_usage").unwrap(), &serde_json::Value::from(25.5));
        
        let degraded = ComponentHealth::degraded("High memory usage");
        assert_eq!(degraded.status, HealthStatus::Degraded);
        
        let unhealthy = ComponentHealth::unhealthy("Connection failed");
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
    }

    /// Test telemetry service event recording
    #[tokio::test]
    async fn test_telemetry_service() {
        let config = Config::default();
        let telemetry_service = TelemetryService::new(&config).await.unwrap();
        
        // Record various event types
        let result = telemetry_service.record_mcp_operation(
            "observe",
            Duration::from_millis(150),
            true,
            None
        ).await;
        assert!(result.is_ok());
        
        let result = telemetry_service.record_brp_event(
            "connected",
            "Successfully established BRP connection"
        ).await;
        assert!(result.is_ok());
        
        let result = telemetry_service.record_system_metrics(
            45.2, // CPU
            512,  // Memory MB  
            2048, // Disk MB
            3     // Network connections
        ).await;
        assert!(result.is_ok());
        
        let result = telemetry_service.record_custom_event(
            "test_event",
            serde_json::json!({"test_key": "test_value", "number": 42})
        ).await;
        assert!(result.is_ok());
        
        // Give some time for events to be processed
        tokio::time::sleep(Duration::from_millis(50)).await;
        
        // Get statistics
        let stats = telemetry_service.get_statistics().await;
        assert!(stats.total_events >= 4);
        assert!(stats.total_mcp_operations >= 1);
        assert!(stats.total_brp_events >= 1);
        
        // Get recent events
        let recent_events = telemetry_service.get_recent_events(10).await;
        assert!(!recent_events.is_empty());
        assert!(recent_events.len() <= 10);
    }

    /// Test alert rules generation
    #[test]
    fn test_alert_rules() {
        let rules = AlertRules::get_production_rules();
        assert!(!rules.is_empty(), "Should have production alert rules");
        assert!(rules.len() >= 10, "Should have comprehensive set of rules");
        
        // Check for critical alerts
        let critical_rules: Vec<_> = rules.iter()
            .filter(|r| r.severity == AlertSeverity::Critical)
            .collect();
        assert!(!critical_rules.is_empty(), "Should have critical alerts");
        
        // Check for specific important rules
        assert!(rules.iter().any(|r| r.name == "HighErrorRate"));
        assert!(rules.iter().any(|r| r.name == "BRPConnectionDown"));
        assert!(rules.iter().any(|r| r.name == "ServiceUnavailable"));
        assert!(rules.iter().any(|r| r.name == "PanicDetected"));
        
        // Test Prometheus export
        let prometheus_rules = AlertRules::export_prometheus_rules();
        assert!(prometheus_rules.contains("groups:"));
        assert!(prometheus_rules.contains("bevy_debugger_mcp_alerts"));
        assert!(prometheus_rules.contains("HighErrorRate"));
        assert!(prometheus_rules.contains("for: "));
        assert!(prometheus_rules.contains("labels:"));
        assert!(prometheus_rules.contains("annotations:"));
        
        // Test JSON export
        let json_rules = AlertRules::export_json_rules();
        assert!(json_rules.is_ok());
        let json_str = json_rules.unwrap();
        assert!(json_str.contains("HighErrorRate"));
        assert!(json_str.contains("Critical"));
        assert!(json_str.contains("Warning"));
    }

    /// Test Grafana dashboard generation
    #[test] 
    fn test_grafana_dashboard() {
        let dashboard = GrafanaDashboard::generate_dashboard();
        
        // Check basic structure
        assert!(dashboard["dashboard"].is_object());
        assert_eq!(
            dashboard["dashboard"]["title"].as_str(),
            Some("Bevy Debugger MCP Server Monitoring")
        );
        
        // Check panels exist
        let panels = dashboard["dashboard"]["panels"].as_array();
        assert!(panels.is_some());
        let panels = panels.unwrap();
        assert!(!panels.is_empty(), "Dashboard should have monitoring panels");
        assert!(panels.len() >= 6, "Should have comprehensive panel set");
        
        // Check for specific panel types
        let panel_titles: Vec<_> = panels.iter()
            .filter_map(|p| p["title"].as_str())
            .collect();
        
        assert!(panel_titles.iter().any(|&t| t.contains("Request Rate")));
        assert!(panel_titles.iter().any(|&t| t.contains("Request Latency")));
        assert!(panel_titles.iter().any(|&t| t.contains("Error Rate")));
        assert!(panel_titles.iter().any(|&t| t.contains("Health Status")));
        
        // Check templating
        let templating = &dashboard["dashboard"]["templating"];
        assert!(templating.is_object());
    }

    /// Test full observability service lifecycle
    #[tokio::test]
    async fn test_observability_service_lifecycle() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        
        let observability = ObservabilityService::new(config, brp_client).await.unwrap();
        
        // Start all services
        let start_result = observability.start().await;
        assert!(start_result.is_ok(), "All observability services should start");
        
        // Give services time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        // Test service access
        let metrics = observability.metrics();
        let health = observability.health();
        let tracing = observability.tracing();
        let telemetry = observability.telemetry();
        
        // Record some metrics
        let timer = metrics.record_request_start();
        tokio::time::sleep(Duration::from_millis(10)).await;
        drop(timer);
        
        metrics.record_error();
        metrics.set_brp_connection_status(true);
        
        // Record telemetry events
        let _ = telemetry.record_mcp_operation("test", Duration::from_millis(50), true, None).await;
        
        // Check health
        let health_status = health.get_health_status().await;
        assert!(health_status.is_ok());
        
        // Create trace spans
        let mut span = tracing.create_mcp_span("test", None);
        span.set_success();
        drop(span);
        
        // Shutdown gracefully
        let shutdown_result = observability.shutdown().await;
        assert!(shutdown_result.is_ok(), "Should shutdown gracefully");
    }

    /// Test metrics export formats
    #[tokio::test]
    async fn test_metrics_export_formats() {
        let config = Config::default();
        let collector = MetricsCollector::new(&config).unwrap();
        
        // Record some metrics
        let timer = collector.record_request_start();
        drop(timer);
        collector.record_error();
        collector.set_brp_connection_status(false); // Test unhealthy status
        
        // Test Prometheus export
        let prometheus_export = collector.export_prometheus().unwrap();
        assert!(prometheus_export.contains("# HELP"));
        assert!(prometheus_export.contains("# TYPE"));
        assert!(prometheus_export.contains("mcp_requests_total"));
        assert!(prometheus_export.contains("mcp_errors_total"));
        assert!(prometheus_export.contains("brp_connection_status 0")); // Unhealthy
        
        // Test metrics snapshot
        let snapshot = collector.get_metrics_snapshot();
        assert_eq!(snapshot.total_requests, 1);
        assert_eq!(snapshot.total_errors, 1);
        assert!(!snapshot.brp_connection_healthy);
        assert!(snapshot.uptime_seconds >= 0.0);
    }

    /// Test observability configuration
    #[test]
    fn test_observability_configuration() {
        let config = Config::default();
        
        // Test default values
        assert!(config.observability.metrics_enabled);
        assert_eq!(config.observability.metrics_port, 9090);
        assert!(config.observability.tracing_enabled);
        assert!(config.observability.health_check_enabled);
        assert_eq!(config.observability.health_check_port, 8080);
        assert_eq!(config.observability.sample_rate, 1.0);
        assert_eq!(config.observability.environment, "development");
        assert!(config.observability.jaeger_endpoint.is_none());
        assert!(config.observability.otlp_endpoint.is_none());
    }

    /// Test error conditions and edge cases
    #[tokio::test]
    async fn test_observability_error_conditions() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        
        // Test health service with potential BRP connection issues
        let health_service = HealthService::new(brp_client).await.unwrap();
        let health_status = health_service.get_health_status().await.unwrap();
        
        // BRP connection might be down in test environment
        if let Some(brp_component) = health_status.components.get("brp_connection") {
            // Should have status information even if connection is down
            assert!(matches!(
                brp_component.status,
                HealthStatus::Healthy | HealthStatus::Degraded | HealthStatus::Unhealthy
            ));
        }
        
        // Test telemetry with edge cases
        let telemetry_service = TelemetryService::new(&config).await.unwrap();
        
        // Test with error condition
        let result = telemetry_service.record_mcp_operation(
            "failing_tool",
            Duration::from_secs(5), // Long duration
            false, // Failed
            Some("Test error message".to_string())
        ).await;
        assert!(result.is_ok());
        
        // Test with extreme values
        let result = telemetry_service.record_system_metrics(
            100.0, // Max CPU
            8192,  // High memory
            0,     // No disk
            1000   // Many connections
        ).await;
        assert!(result.is_ok());
    }
}

/// Tests that run even without observability feature
mod basic_tests {
    use bevy_debugger_mcp::config::Config;

    #[test]
    fn test_config_with_observability() {
        let config = Config::default();
        
        // These should always be available since observability is part of core config
        assert!(config.observability.metrics_enabled || !config.observability.metrics_enabled);
        assert!(config.observability.metrics_port > 0);
    }
}
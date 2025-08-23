/*
 * Bevy Debugger MCP - Distributed Tracing
 * Copyright (C) 2025 ladvien
 *
 * OpenTelemetry and Jaeger-compatible distributed tracing
 */

use opentelemetry::{
    global,
    trace::{TraceError, Tracer, TracerProvider},
    KeyValue,
};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    trace::{self, RandomIdGenerator, Sampler},
    Resource,
};
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{info, warn, error};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

use crate::config::Config;
use crate::error::{Result, Error};

/// Distributed tracing service using OpenTelemetry
pub struct TracingService {
    config: Config,
    tracer: Arc<dyn Tracer + Send + Sync>,
    is_initialized: bool,
}

impl TracingService {
    /// Create a new tracing service
    pub async fn new(config: &Config) -> Result<Self> {
        let tracer = Self::init_tracer(config).await?;
        
        Ok(Self {
            config: config.clone(),
            tracer: Arc::new(tracer),
            is_initialized: false,
        })
    }
    
    /// Initialize OpenTelemetry tracer with Jaeger export
    async fn init_tracer(config: &Config) -> Result<Box<dyn Tracer + Send + Sync>> {
        info!("Initializing OpenTelemetry tracing with Jaeger export");

        // Create resource describing this service
        let resource = Resource::new(vec![
            KeyValue::new("service.name", "bevy-debugger-mcp"),
            KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
            KeyValue::new("service.namespace", "debugging"),
            KeyValue::new("deployment.environment", config.environment.clone()),
        ]);

        // For now, use a simple in-memory tracer for development
        // TODO: Add proper Jaeger/OTLP configuration when compatible versions are available
        info!("Initializing basic OpenTelemetry tracer");
        
        let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_config(
                trace::config()
                    .with_sampler(Sampler::AlwaysOn)
                    .with_id_generator(RandomIdGenerator::default())
                    .with_resource(resource),
            )
            .build();
        
        let tracer = tracer_provider.tracer("bevy-debugger-mcp");

        Ok(Box::new(tracer))
    }
    
    /// Start the tracing service
    pub async fn start(&self) -> Result<()> {
        if self.is_initialized {
            warn!("Tracing service already started");
            return Ok(());
        }

        info!("Starting distributed tracing service");

        // Set up tracing subscriber with OpenTelemetry layer
        let telemetry_layer = tracing_opentelemetry::layer().with_tracer(self.tracer.clone());
        
        // Combine with existing tracing setup
        let subscriber = Registry::default()
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer())
            .with(telemetry_layer);

        // Only set global subscriber if not already set
        if let Err(e) = tracing::subscriber::set_global_default(subscriber) {
            warn!("Could not set global tracing subscriber (may already be set): {}", e);
        }

        info!("Distributed tracing service started");
        Ok(())
    }
    
    /// Shutdown the tracing service
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down tracing service");
        
        // Flush any remaining spans
        global::shutdown_tracer_provider();
        
        info!("Tracing service shutdown complete");
        Ok(())
    }
    
    /// Create a new trace span for MCP operation
    pub fn create_mcp_span(&self, operation: &str, tool_name: Option<&str>) -> TraceSpan {
        let mut attributes = vec![
            KeyValue::new("mcp.operation", operation.to_string()),
            KeyValue::new("service.name", "bevy-debugger-mcp"),
        ];
        
        if let Some(tool) = tool_name {
            attributes.push(KeyValue::new("mcp.tool", tool.to_string()));
        }
        
        TraceSpan::new(&self.tracer, operation, attributes)
    }
    
    /// Create a new trace span for BRP operation  
    pub fn create_brp_span(&self, operation: &str, entity_count: Option<u64>) -> TraceSpan {
        let mut attributes = vec![
            KeyValue::new("brp.operation", operation.to_string()),
            KeyValue::new("service.name", "bevy-debugger-mcp"),
        ];
        
        if let Some(count) = entity_count {
            attributes.push(KeyValue::new("brp.entity_count", count.to_string()));
        }
        
        TraceSpan::new(&self.tracer, &format!("brp.{}", operation), attributes)
    }
    
    /// Create a new trace span for system operation
    pub fn create_system_span(&self, operation: &str) -> TraceSpan {
        let attributes = vec![
            KeyValue::new("system.operation", operation.to_string()),
            KeyValue::new("service.name", "bevy-debugger-mcp"),
        ];
        
        TraceSpan::new(&self.tracer, &format!("system.{}", operation), attributes)
    }
}

/// Wrapper for OpenTelemetry span with automatic completion
pub struct TraceSpan {
    span: opentelemetry::trace::Span,
}

impl TraceSpan {
    fn new(tracer: &dyn Tracer, name: &str, attributes: Vec<KeyValue>) -> Self {
        let span = tracer
            .span_builder(name)
            .with_attributes(attributes)
            .start(tracer);
            
        Self { span }
    }
    
    /// Add an attribute to the span
    pub fn set_attribute(&mut self, key: &str, value: &str) {
        self.span.set_attribute(KeyValue::new(key, value.to_string()));
    }
    
    /// Add multiple attributes to the span
    pub fn set_attributes(&mut self, attributes: Vec<(&str, &str)>) {
        for (key, value) in attributes {
            self.span.set_attribute(KeyValue::new(key, value.to_string()));
        }
    }
    
    /// Record an event in the span
    pub fn add_event(&mut self, name: &str, attributes: Vec<(&str, &str)>) {
        let kv_attributes: Vec<KeyValue> = attributes
            .into_iter()
            .map(|(k, v)| KeyValue::new(k, v.to_string()))
            .collect();
            
        self.span.add_event(name, kv_attributes);
    }
    
    /// Record an error in the span
    pub fn record_error(&mut self, error: &dyn std::error::Error) {
        self.span.set_status(opentelemetry::trace::Status::Error {
            description: error.to_string().into(),
        });
        
        self.span.set_attribute(KeyValue::new("error", true));
        self.span.set_attribute(KeyValue::new("error.message", error.to_string()));
    }
    
    /// Mark span as successful
    pub fn set_success(&mut self) {
        self.span.set_status(opentelemetry::trace::Status::Ok);
    }
}

impl Drop for TraceSpan {
    fn drop(&mut self) {
        self.span.end();
    }
}

/// Tracing configuration from environment or config file
#[derive(Debug, Clone)]
pub struct TracingConfig {
    pub enabled: bool,
    pub jaeger_endpoint: Option<String>,
    pub otlp_endpoint: Option<String>,
    pub sample_rate: f64,
    pub service_name: String,
    pub service_version: String,
    pub environment: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            jaeger_endpoint: std::env::var("JAEGER_ENDPOINT").ok(),
            otlp_endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").ok(),
            sample_rate: std::env::var("OTEL_TRACES_SAMPLER_ARG")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0),
            service_name: "bevy-debugger-mcp".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            environment: std::env::var("DEPLOYMENT_ENVIRONMENT")
                .unwrap_or_else(|_| "development".to_string()),
        }
    }
}

/// Macro for creating traced functions
#[macro_export]
macro_rules! traced_async {
    ($span_name:expr, $operation:expr) => {
        async move {
            let _span = $crate::observability::tracing::create_span($span_name).await;
            $operation.await
        }
    };
}

/// Utility function to create a span from current context
pub async fn create_span(name: &str) -> TraceSpan {
    // Get tracer from global context
    let tracer = global::tracer("bevy-debugger-mcp");
    let attributes = vec![KeyValue::new("service.name", "bevy-debugger-mcp")];
    TraceSpan::new(&tracer, name, attributes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tracing_service_creation() {
        let config = Config::default();
        let tracing_service = TracingService::new(&config).await;
        assert!(tracing_service.is_ok());
    }
    
    #[tokio::test] 
    async fn test_span_creation() {
        let config = Config::default();
        let tracing_service = TracingService::new(&config).await.unwrap();
        
        let mut span = tracing_service.create_mcp_span("test_operation", Some("observe"));
        span.set_attribute("test_attr", "test_value");
        span.add_event("test_event", vec![("key", "value")]);
        span.set_success();
        
        // Span should drop and complete automatically
        drop(span);
    }
    
    #[test]
    fn test_tracing_config() {
        let config = TracingConfig::default();
        assert_eq!(config.service_name, "bevy-debugger-mcp");
        assert!(config.enabled);
        assert_eq!(config.sample_rate, 1.0);
    }
}
use crate::error::{Error, Result};
use std::env;
use std::time::Duration;

/// Circuit breaker configuration for production-grade resilience
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    pub failure_threshold: u32,
    pub reset_timeout: Duration,
    pub half_open_max_requests: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(60),
            half_open_max_requests: 3,
        }
    }
}

/// Connection pool configuration
#[derive(Debug, Clone)]
pub struct ConnectionPoolConfig {
    pub min_connections: u32,
    pub max_connections: u32,
    pub connection_timeout: Duration,
    pub idle_timeout: Duration,
    pub max_connection_lifetime: Duration,
}

impl Default for ConnectionPoolConfig {
    fn default() -> Self {
        Self {
            min_connections: 1,
            max_connections: 10,
            connection_timeout: Duration::from_secs(5),
            idle_timeout: Duration::from_secs(300), // 5 minutes
            max_connection_lifetime: Duration::from_secs(3600), // 1 hour
        }
    }
}

/// Retry policy configuration with exponential backoff
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
    pub multiplier: f32,
    pub jitter: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
            jitter: true,
        }
    }
}

/// Heartbeat configuration
#[derive(Debug, Clone)]
pub struct HeartbeatConfig {
    pub interval: Duration,
    pub timeout: Duration,
    pub max_missed: u32,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(5),
            max_missed: 3,
        }
    }
}

/// Production-grade resilience configuration
#[derive(Debug, Clone)]
pub struct ResilienceConfig {
    pub circuit_breaker: CircuitBreakerConfig,
    pub connection_pool: ConnectionPoolConfig,
    pub retry: RetryConfig,
    pub heartbeat: HeartbeatConfig,
    pub request_timeout: Duration,
    pub enable_adaptive_sampling: bool,
}

/// Observability configuration for metrics and tracing
#[derive(Debug, Clone)]
pub struct ObservabilityConfig {
    pub metrics_enabled: bool,
    pub metrics_port: u16,
    pub tracing_enabled: bool,
    pub jaeger_endpoint: Option<String>,
    pub otlp_endpoint: Option<String>,
    pub health_check_enabled: bool,
    pub health_check_port: u16,
    pub sample_rate: f64,
    pub environment: String,
}

impl Default for ResilienceConfig {
    fn default() -> Self {
        Self {
            circuit_breaker: CircuitBreakerConfig::default(),
            connection_pool: ConnectionPoolConfig::default(),
            retry: RetryConfig::default(),
            heartbeat: HeartbeatConfig::default(),
            request_timeout: Duration::from_secs(10),
            enable_adaptive_sampling: true,
        }
    }
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            metrics_enabled: true,
            metrics_port: 9090,
            tracing_enabled: true,
            jaeger_endpoint: None,
            otlp_endpoint: None,
            health_check_enabled: true,
            health_check_port: 8080,
            sample_rate: 1.0,
            environment: "development".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub bevy_brp_host: String,
    pub bevy_brp_port: u16,
    pub mcp_port: u16,
    pub resilience: ResilienceConfig,
    pub observability: ObservabilityConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bevy_brp_host: "localhost".to_string(),
            bevy_brp_port: 15702,
            mcp_port: 3001,
            resilience: ResilienceConfig::default(),
            observability: ObservabilityConfig::default(),
        }
    }
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let bevy_brp_host = env::var("BEVY_BRP_HOST").unwrap_or_else(|_| "localhost".to_string());
        let bevy_brp_port = env::var("BEVY_BRP_PORT")
            .unwrap_or_else(|_| "15702".to_string())
            .parse::<u16>()
            .map_err(|_| Error::Config("Invalid BEVY_BRP_PORT".to_string()))?;
        let mcp_port = env::var("MCP_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse::<u16>()
            .map_err(|_| Error::Config("Invalid MCP_PORT".to_string()))?;

        let mut resilience = ResilienceConfig::default();
        
        // Parse resilience configuration from environment
        if let Ok(val) = env::var("BRP_CIRCUIT_BREAKER_THRESHOLD") {
            resilience.circuit_breaker.failure_threshold = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_CIRCUIT_BREAKER_THRESHOLD".to_string()))?;
        }
        
        if let Ok(val) = env::var("BRP_CIRCUIT_BREAKER_RESET_TIMEOUT") {
            let seconds: u64 = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_CIRCUIT_BREAKER_RESET_TIMEOUT".to_string()))?;
            resilience.circuit_breaker.reset_timeout = Duration::from_secs(seconds);
        }
        
        if let Ok(val) = env::var("BRP_MAX_CONNECTIONS") {
            resilience.connection_pool.max_connections = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_MAX_CONNECTIONS".to_string()))?;
        }
        
        if let Ok(val) = env::var("BRP_CONNECTION_TIMEOUT") {
            let seconds: u64 = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_CONNECTION_TIMEOUT".to_string()))?;
            resilience.connection_pool.connection_timeout = Duration::from_secs(seconds);
        }
        
        if let Ok(val) = env::var("BRP_HEARTBEAT_INTERVAL") {
            let seconds: u64 = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_HEARTBEAT_INTERVAL".to_string()))?;
            resilience.heartbeat.interval = Duration::from_secs(seconds);
        }
        
        if let Ok(val) = env::var("BRP_HEARTBEAT_TIMEOUT") {
            let seconds: u64 = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_HEARTBEAT_TIMEOUT".to_string()))?;
            resilience.heartbeat.timeout = Duration::from_secs(seconds);
        }
        
        if let Ok(val) = env::var("BRP_RETRY_MAX_ATTEMPTS") {
            resilience.retry.max_attempts = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_RETRY_MAX_ATTEMPTS".to_string()))?;
        }
        
        if let Ok(val) = env::var("BRP_RETRY_INITIAL_DELAY") {
            let milliseconds: u64 = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_RETRY_INITIAL_DELAY".to_string()))?;
            resilience.retry.initial_delay = Duration::from_millis(milliseconds);
        }
        
        if let Ok(val) = env::var("BRP_RETRY_MAX_DELAY") {
            let seconds: u64 = val.parse()
                .map_err(|_| Error::Config("Invalid BRP_RETRY_MAX_DELAY".to_string()))?;
            resilience.retry.max_delay = Duration::from_secs(seconds);
        }

        let mut observability = ObservabilityConfig::default();
        
        // Parse observability configuration from environment
        if let Ok(val) = env::var("METRICS_ENABLED") {
            observability.metrics_enabled = val.parse()
                .map_err(|_| Error::Config("Invalid METRICS_ENABLED".to_string()))?;
        }
        
        if let Ok(val) = env::var("METRICS_PORT") {
            observability.metrics_port = val.parse()
                .map_err(|_| Error::Config("Invalid METRICS_PORT".to_string()))?;
        }
        
        if let Ok(val) = env::var("TRACING_ENABLED") {
            observability.tracing_enabled = val.parse()
                .map_err(|_| Error::Config("Invalid TRACING_ENABLED".to_string()))?;
        }
        
        if let Ok(val) = env::var("JAEGER_ENDPOINT") {
            observability.jaeger_endpoint = Some(val);
        }
        
        if let Ok(val) = env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            observability.otlp_endpoint = Some(val);
        }
        
        if let Ok(val) = env::var("HEALTH_CHECK_ENABLED") {
            observability.health_check_enabled = val.parse()
                .map_err(|_| Error::Config("Invalid HEALTH_CHECK_ENABLED".to_string()))?;
        }
        
        if let Ok(val) = env::var("HEALTH_CHECK_PORT") {
            observability.health_check_port = val.parse()
                .map_err(|_| Error::Config("Invalid HEALTH_CHECK_PORT".to_string()))?;
        }
        
        if let Ok(val) = env::var("OTEL_TRACES_SAMPLER_ARG") {
            observability.sample_rate = val.parse()
                .map_err(|_| Error::Config("Invalid OTEL_TRACES_SAMPLER_ARG".to_string()))?;
        }
        
        if let Ok(val) = env::var("DEPLOYMENT_ENVIRONMENT") {
            observability.environment = val;
        }

        Ok(Config {
            bevy_brp_host,
            bevy_brp_port,
            mcp_port,
            resilience,
            observability,
        })
    }

    #[must_use]
    pub fn brp_url(&self) -> String {
        format!("ws://{}:{}", self.bevy_brp_host, self.bevy_brp_port)
    }
    
    /// Validate configuration values
    pub fn validate(&self) -> Result<()> {
        if self.resilience.circuit_breaker.failure_threshold == 0 {
            return Err(Error::Config("Circuit breaker failure threshold must be > 0".to_string()));
        }
        
        if self.resilience.connection_pool.max_connections == 0 {
            return Err(Error::Config("Max connections must be > 0".to_string()));
        }
        
        if self.resilience.connection_pool.max_connections < self.resilience.connection_pool.min_connections {
            return Err(Error::Config("Max connections must be >= min connections".to_string()));
        }
        
        if self.resilience.retry.max_attempts == 0 {
            return Err(Error::Config("Retry max attempts must be > 0".to_string()));
        }
        
        if self.resilience.heartbeat.max_missed == 0 {
            return Err(Error::Config("Heartbeat max missed must be > 0".to_string()));
        }
        
        Ok(())
    }
}

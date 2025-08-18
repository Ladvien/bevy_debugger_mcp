use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, Error>;

/// Rich error context for debugging and recovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorContext {
    /// Unique error ID for tracking
    pub error_id: String,
    /// Timestamp when error occurred
    pub timestamp: u64,
    /// The operation that was being performed
    pub operation: String,
    /// Component or module where error occurred
    pub component: String,
    /// Stack of error causes
    pub error_chain: Vec<String>,
    /// Additional context data
    pub context_data: HashMap<String, String>,
    /// Recovery suggestions
    pub recovery_suggestions: Vec<String>,
    /// Whether the operation can be retried
    pub is_retryable: bool,
    /// Severity level
    pub severity: ErrorSeverity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

impl ErrorContext {
    pub fn new(operation: &str, component: &str) -> Self {
        Self {
            error_id: uuid::Uuid::new_v4().to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_else(|_| {
                    tracing::warn!("Failed to get system time for error context, using epoch");
                    std::time::Duration::from_secs(0)
                })
                .as_secs(),
            operation: operation.to_string(),
            component: component.to_string(),
            error_chain: Vec::new(),
            context_data: HashMap::new(),
            recovery_suggestions: Vec::new(),
            is_retryable: false,
            severity: ErrorSeverity::Error,
        }
    }

    pub fn add_cause(mut self, cause: &str) -> Self {
        self.error_chain.push(cause.to_string());
        self
    }

    pub fn add_context(mut self, key: &str, value: &str) -> Self {
        // Sanitize sensitive data before storing
        let sanitized_value = if key.to_lowercase().contains("password")
            || key.to_lowercase().contains("token")
            || key.to_lowercase().contains("secret")
            || key.to_lowercase().contains("key")
        {
            "[REDACTED]".to_string()
        } else {
            value.to_string()
        };
        self.context_data.insert(key.to_string(), sanitized_value);
        self
    }

    pub fn add_recovery_suggestion(mut self, suggestion: &str) -> Self {
        self.recovery_suggestions.push(suggestion.to_string());
        self
    }

    pub fn set_retryable(mut self, retryable: bool) -> Self {
        self.is_retryable = retryable;
        self
    }

    pub fn set_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Format the error context as a detailed error message
    pub fn format_detailed(&self) -> String {
        let mut message = format!(
            "Error [{}] in {} during {}\n",
            self.error_id, self.component, self.operation
        );

        if !self.error_chain.is_empty() {
            message.push_str("Error Chain:\n");
            for (i, cause) in self.error_chain.iter().enumerate() {
                message.push_str(&format!("  {}: {}\n", i + 1, cause));
            }
        }

        if !self.context_data.is_empty() {
            message.push_str("Context:\n");
            for (key, value) in &self.context_data {
                message.push_str(&format!("  {key}: {value}\n"));
            }
        }

        if !self.recovery_suggestions.is_empty() {
            message.push_str("Recovery Suggestions:\n");
            for suggestion in &self.recovery_suggestions {
                message.push_str(&format!("  - {suggestion}\n"));
            }
        }

        message.push_str(&format!("Retryable: {}\n", self.is_retryable));
        message.push_str(&format!("Severity: {:?}\n", self.severity));

        message
    }
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} in {} ({})",
            self.operation, self.component, self.error_id
        )
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] Box<tokio_tungstenite::tungstenite::Error>),

    #[error("IO error: {0}")]
    Io(#[from] Box<std::io::Error>),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Connection error: {0}")]
    Connection(String),

    #[error("MCP protocol error: {0}")]
    Mcp(String),

    #[error("BRP error: {0}")]
    Brp(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    /// Rich error with full context
    #[error("Error: {context}")]
    WithContext {
        context: ErrorContext,
        #[source]
        source: Option<Box<Error>>,
    },
}

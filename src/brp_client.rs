//! HTTP JSON-RPC client for the Bevy Remote Protocol.
//!
//! Since Bevy 0.16 the Bevy Remote Protocol is JSON-RPC 2.0 over HTTP POST
//! (see `bevy_remote::http`). This client speaks that protocol directly:
//! every request is a POST of `{"jsonrpc":"2.0","id":N,"method":..., "params":...}`
//! to the BRP root URL, and the response is the JSON-RPC response object.

use reqwest::Client;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::Instant;
use tracing::{debug, info, warn};

use crate::brp::{builtin_methods, BrpRequest, BrpResponse};
use crate::brp_command_handler::{CommandHandlerRegistry, CoreBrpHandler, BrpCommandHandler};
use crate::config::Config;
use crate::debug_command_processor::DebugCommandRouter;
use crate::error::{Error, Result};
use crate::resource_manager::ResourceManager;

/// Request timeout for a single BRP call.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// BRP client with extensible command handler support.
///
/// Stateless over HTTP: "connected" means the last `rpc.discover` health
/// check returned a 2xx response.
pub struct BrpClient {
    config: Config,
    http: Client,
    url: String,
    connected: Arc<AtomicBool>,
    retry_count: u32,
    resource_manager: Option<Arc<RwLock<ResourceManager>>>,
    request_id: Arc<AtomicU64>,
    command_registry: Arc<CommandHandlerRegistry>,
    debug_router: Option<Arc<DebugCommandRouter>>,
}

impl std::fmt::Debug for BrpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrpClient")
            .field("config", &self.config)
            .field("url", &self.url)
            .field("connected", &self.is_connected())
            .field("retry_count", &self.retry_count)
            .field("has_resource_manager", &self.resource_manager.is_some())
            .field("has_debug_router", &self.debug_router.is_some())
            .finish()
    }
}

impl Clone for BrpClient {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            http: self.http.clone(),
            url: self.url.clone(),
            connected: self.connected.clone(),
            retry_count: self.retry_count,
            resource_manager: self.resource_manager.clone(),
            request_id: self.request_id.clone(),
            command_registry: self.command_registry.clone(),
            debug_router: self.debug_router.clone(),
        }
    }
}

impl BrpClient {
    pub fn new(config: &Config) -> Self {
        let command_registry = Arc::new(CommandHandlerRegistry::new());
        let http = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("reqwest client builder with rustls must succeed");

        BrpClient {
            config: config.clone(),
            http,
            url: format!(
                "http://{}:{}",
                config.bevy_brp_host, config.bevy_brp_port
            ),
            connected: Arc::new(AtomicBool::new(false)),
            retry_count: 0,
            resource_manager: None,
            request_id: Arc::new(AtomicU64::new(1)),
            command_registry,
            debug_router: None,
        }
    }

    /// Initialize the client asynchronously with default handlers
    pub async fn init(&self) -> Result<()> {
        // Register core handler - safe async initialization
        let core_handler = Arc::new(CoreBrpHandler);
        self.command_registry.register(core_handler).await;
        Ok(())
    }

    pub fn with_resource_manager(mut self, resource_manager: Arc<RwLock<ResourceManager>>) -> Self {
        self.resource_manager = Some(resource_manager);
        self
    }

    /// Set the debug command router for handling debug commands
    pub fn with_debug_router(mut self, router: Arc<DebugCommandRouter>) -> Self {
        self.debug_router = Some(router);
        self
    }

    /// Register a custom command handler
    pub async fn register_handler(&self, handler: Arc<dyn BrpCommandHandler>) {
        self.command_registry.register(handler).await;
    }

    /// Get the command registry for external access
    pub fn command_registry(&self) -> Arc<CommandHandlerRegistry> {
        self.command_registry.clone()
    }

    pub async fn connect_with_retry(&mut self) -> Result<()> {
        const MAX_RETRIES: u32 = 5;
        const BASE_DELAY: Duration = Duration::from_millis(1000);

        while self.retry_count < MAX_RETRIES {
            match self.connect().await {
                Ok(()) => {
                    info!("Successfully connected to BRP at {}", self.url);
                    self.retry_count = 0;
                    return Ok(());
                }
                Err(e) => {
                    self.retry_count += 1;
                    let delay = BASE_DELAY * 2_u32.pow(self.retry_count.min(5));
                    warn!(
                        "Failed to connect to BRP (attempt {}/{}): {}. Retrying in {:?}",
                        self.retry_count, MAX_RETRIES, e, delay
                    );
                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err(Error::Connection(format!(
            "Failed to connect to BRP after {MAX_RETRIES} attempts"
        )))
    }

    /// HTTP "connect": probe the server with an `rpc.discover` request and
    /// accept any 2xx response as a healthy BRP endpoint.
    async fn connect(&mut self) -> Result<()> {
        debug!("Attempting BRP health check (rpc.discover) at {}", self.url);

        let discover = BrpRequest {
            method: builtin_methods::RPC_DISCOVER_METHOD.to_string(),
            id: Some(Value::from(self.request_id.fetch_add(1, Ordering::Relaxed))),
            params: None,
        };

        let response = self
            .http
            .post(&self.url)
            .json(&discover)
            .send()
            .await
            .map_err(|e| Error::Connection(format!("BRP health check failed: {e}")))?;

        if response.status().is_success() {
            self.connected.store(true, Ordering::Relaxed);
            Ok(())
        } else {
            self.connected.store(false, Ordering::Relaxed);
            Err(Error::Connection(format!(
                "BRP health check returned HTTP {}",
                response.status()
            )))
        }
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// Send a BRP request and return the response (with resource management)
    pub async fn send_request(&mut self, request: &BrpRequest) -> Result<BrpResponse> {
        // Check rate limiting if resource manager is available
        if let Some(ref rm) = self.resource_manager {
            let resource_manager = rm.read().await;
            if !resource_manager.check_brp_rate_limit().await {
                return Err(Error::Validation(
                    "BRP request rate limit exceeded".to_string(),
                ));
            }

            // Acquire operation permit
            let _permit = resource_manager.acquire_operation_permit().await?;

            // Check if we should sample this request
            if !resource_manager.should_sample().await {
                debug!("Skipping BRP request due to adaptive sampling");
                return Err(Error::Validation(
                    "Request skipped due to adaptive sampling".to_string(),
                ));
            }
        }

        let start_time = Instant::now();
        let result = self.send_request_internal(request).await;
        let duration = start_time.elapsed();

        // Record success/failure for circuit breaker
        if let Some(ref rm) = self.resource_manager {
            let resource_manager = rm.read().await;
            match &result {
                Ok(_) => {
                    resource_manager.record_operation_success().await;
                    debug!("Request completed in {:?}", duration);
                }
                Err(_) => {
                    resource_manager.record_operation_failure().await;
                    debug!("Request failed after {:?}", duration);
                }
            }
        }

        result
    }

    /// Internal send request without resource management.
    ///
    /// POSTs the JSON-RPC envelope and parses the `bevy_remote::BrpResponse`
    /// from the JSON-RPC response body.
    async fn send_request_internal(&mut self, request: &BrpRequest) -> Result<BrpResponse> {
        // Stamp a fresh JSON-RPC id unless the caller set one.
        let mut request = request.clone();
        if request.id.is_none() {
            request.id = Some(Value::from(self.request_id.fetch_add(1, Ordering::Relaxed)));
        }

        let response = self
            .http
            .post(&self.url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                self.connected.store(false, Ordering::Relaxed);
                if e.is_timeout() {
                    Error::Connection("Request timeout".to_string())
                } else {
                    Error::Connection(format!("BRP HTTP request failed: {e}"))
                }
            })?;

        let status = response.status();
        if !status.is_success() {
            self.connected.store(false, Ordering::Relaxed);
            return Err(Error::Connection(format!(
                "BRP server returned HTTP {status}"
            )));
        }

        self.connected.store(true, Ordering::Relaxed);

        response.json::<BrpResponse>().await.map_err(Error::from)
    }

    /// Send a batched request: executes immediately like `send_request`.
    ///
    /// BRP is request/response over HTTP, so batching has no transport to
    /// amortize; this preserves the interface for existing callers.
    pub async fn send_batched_request(&mut self, request: BrpRequest) -> Result<BrpResponse> {
        self.send_request(&request).await
    }

    /// Disconnect: with HTTP there is no persistent connection to drop, so
    /// this simply marks the client disconnected.
    pub async fn disconnect(&mut self) {
        self.connected.store(false, Ordering::Relaxed);
        info!("Disconnected from BRP");
    }
}

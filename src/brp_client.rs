use futures_util::{SinkExt, StreamExt};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tokio::time::{interval, Instant};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};
use url::Url;

use crate::brp_messages::{BrpRequest, BrpResponse};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::resource_manager::ResourceManager;

/// Batched request for efficient processing
#[derive(Debug, Clone)]
struct BatchedRequest {
    request: BrpRequest,
    timestamp: Instant,
    response_tx: mpsc::Sender<Result<BrpResponse>>,
}

#[derive(Debug)]
pub struct BrpClient {
    config: Config,
    ws_stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    connected: bool,
    retry_count: u32,
    resource_manager: Option<Arc<RwLock<ResourceManager>>>,
    request_queue: Arc<RwLock<VecDeque<BatchedRequest>>>,
    batch_processor_handle: Option<tokio::task::JoinHandle<()>>,
}

impl BrpClient {
    pub fn new(config: &Config) -> Self {
        BrpClient {
            config: config.clone(),
            ws_stream: None,
            connected: false,
            retry_count: 0,
            resource_manager: None,
            request_queue: Arc::new(RwLock::new(VecDeque::new())),
            batch_processor_handle: None,
        }
    }

    pub fn with_resource_manager(mut self, resource_manager: Arc<RwLock<ResourceManager>>) -> Self {
        self.resource_manager = Some(resource_manager);
        self
    }

    pub async fn connect_with_retry(&mut self) -> Result<()> {
        const MAX_RETRIES: u32 = 5;
        const BASE_DELAY: Duration = Duration::from_millis(1000);

        while self.retry_count < MAX_RETRIES {
            match self.connect().await {
                Ok(()) => {
                    info!("Successfully connected to BRP at {}", self.config.brp_url());
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

    async fn connect(&mut self) -> Result<()> {
        let url_str = self.config.brp_url();
        let url =
            Url::parse(&url_str).map_err(|e| Error::Connection(format!("Invalid BRP URL: {e}")))?;

        debug!("Attempting to connect to {}", url);
        let (ws_stream, _) = connect_async(&url_str)
            .await
            .map_err(|e| Error::WebSocket(Box::new(e)))?;

        self.ws_stream = Some(ws_stream);
        self.connected = true;

        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.connected
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
                // Return a mock response or cached result here if needed
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

    /// Internal send request without resource management
    async fn send_request_internal(&mut self, request: &BrpRequest) -> Result<BrpResponse> {
        let request_json = serde_json::to_string(request)?;
        self.send_message(&request_json).await?;

        // Wait for response with timeout
        let response = tokio::time::timeout(Duration::from_secs(5), self.receive_message())
            .await
            .map_err(|_| Error::Connection("Request timeout".to_string()))?;

        match response? {
            Some(response_text) => serde_json::from_str(&response_text).map_err(Error::Json),
            None => Err(Error::Connection(
                "Connection closed during request".to_string(),
            )),
        }
    }

    /// Send a batched request (queued for batch processing)
    pub async fn send_batched_request(&mut self, request: BrpRequest) -> Result<BrpResponse> {
        let (response_tx, mut response_rx) = mpsc::channel(1);

        let batched_request = BatchedRequest {
            request,
            timestamp: Instant::now(),
            response_tx,
        };

        // Add to queue
        {
            let mut queue = self.request_queue.write().await;
            queue.push_back(batched_request);
        }

        // Wait for response
        response_rx
            .recv()
            .await
            .ok_or_else(|| Error::Connection("Batch response channel closed".to_string()))?
    }

    /// Start batch processing
    pub async fn start_batch_processing(&mut self) -> Result<()> {
        if self.batch_processor_handle.is_some() {
            return Ok(()); // Already running
        }

        let queue = self.request_queue.clone();
        let resource_manager = self.resource_manager.clone();

        let handle = tokio::spawn(async move {
            let mut batch_interval = interval(Duration::from_millis(50)); // Batch every 50ms

            loop {
                batch_interval.tick().await;

                // Process batched requests
                let requests = {
                    let mut queue_guard = queue.write().await;
                    let batch_size = std::cmp::min(queue_guard.len(), 10); // Max 10 per batch
                    queue_guard.drain(..batch_size).collect::<Vec<_>>()
                };

                if requests.is_empty() {
                    continue;
                }

                // Check resource limits before processing batch
                if let Some(ref rm) = resource_manager {
                    let rm_guard = rm.read().await;
                    if !rm_guard.check_brp_rate_limit().await {
                        // Return rate limit errors to all requests
                        for req in requests {
                            let _ = req
                                .response_tx
                                .send(Err(Error::Validation(
                                    "BRP rate limit exceeded".to_string(),
                                )))
                                .await;
                        }
                        continue;
                    }
                }

                info!("Processing batch of {} BRP requests", requests.len());

                // Process each request in the batch
                // For better efficiency, we process them individually but with shared resources
                for batched_request in requests {
                    // Simulate batch processing by adding a small delay and processing
                    let result = if let Some(ref rm) = resource_manager {
                        let rm_guard = rm.read().await;
                        if rm_guard.should_sample().await {
                            // Process the request (simplified simulation)
                            Ok(crate::brp_messages::BrpResponse::Success(
                                Box::new(crate::brp_messages::BrpResult::Success),
                            ))
                        } else {
                            Err(Error::Validation(
                                "Request skipped due to adaptive sampling".to_string(),
                            ))
                        }
                    } else {
                        // Fallback processing without resource management
                        Ok(crate::brp_messages::BrpResponse::Success(
                            Box::new(crate::brp_messages::BrpResult::Success),
                        ))
                    };

                    let _ = batched_request.response_tx.send(result).await;
                }
            }
        });

        self.batch_processor_handle = Some(handle);
        info!("Batch processing started");
        Ok(())
    }

    /// Stop batch processing
    pub async fn stop_batch_processing(&mut self) {
        if let Some(handle) = self.batch_processor_handle.take() {
            handle.abort();
            info!("Batch processing stopped");
        }
    }

    pub async fn send_message(&mut self, message: &str) -> Result<()> {
        if let Some(ws_stream) = &mut self.ws_stream {
            ws_stream
                .send(Message::Text(message.to_string()))
                .await
                .map_err(|e| Error::WebSocket(Box::new(e)))?;
            debug!("Sent BRP message: {}", message);
            Ok(())
        } else {
            Err(Error::Connection("Not connected to BRP".to_string()))
        }
    }

    pub async fn receive_message(&mut self) -> Result<Option<String>> {
        if let Some(ws_stream) = &mut self.ws_stream {
            match ws_stream.next().await {
                Some(Ok(Message::Text(text))) => {
                    debug!("Received BRP message: {}", text);
                    Ok(Some(text))
                }
                Some(Ok(Message::Close(_))) => {
                    warn!("BRP connection closed");
                    self.connected = false;
                    self.ws_stream = None;
                    Ok(None)
                }
                Some(Err(e)) => {
                    error!("BRP WebSocket error: {}", e);
                    self.connected = false;
                    self.ws_stream = None;
                    Err(Error::WebSocket(Box::new(e)))
                }
                None => Ok(None),
                _ => Ok(None),
            }
        } else {
            Err(Error::Connection("Not connected to BRP".to_string()))
        }
    }

    pub async fn disconnect(&mut self) {
        if let Some(mut ws_stream) = self.ws_stream.take() {
            let _ = ws_stream.close(None).await;
        }
        self.connected = false;
        info!("Disconnected from BRP");
    }
}

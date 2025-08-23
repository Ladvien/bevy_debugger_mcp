/*
 * Bevy Debugger MCP Server - Rate Limiting
 * Copyright (C) 2025 ladvien
 */

use crate::error::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limiting configuration
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_minute: u32,
    pub requests_per_hour: u32,
    pub burst_size: u32,
    pub enable_per_operation_limits: bool,
    pub operation_limits: HashMap<String, u32>, // requests per minute for specific operations
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let mut operation_limits = HashMap::new();
        operation_limits.insert("stress_test".to_string(), 5);  // Limit stress tests
        operation_limits.insert("experiment".to_string(), 20);  // Limit experiments
        operation_limits.insert("observe".to_string(), 100);    // Higher limit for observations
        
        Self {
            requests_per_minute: 60,
            requests_per_hour: 1000,
            burst_size: 10,
            enable_per_operation_limits: true,
            operation_limits,
        }
    }
}

/// Rate limiting bucket for tracking user requests
#[derive(Debug, Clone)]
struct RateLimitBucket {
    requests_this_minute: u32,
    requests_this_hour: u32,
    minute_reset_time: Instant,
    hour_reset_time: Instant,
    burst_tokens: u32,
    last_refill: Instant,
}

impl RateLimitBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let now = Instant::now();
        Self {
            requests_this_minute: 0,
            requests_this_hour: 0,
            minute_reset_time: now + Duration::from_secs(60),
            hour_reset_time: now + Duration::from_secs(3600),
            burst_tokens: config.burst_size,
            last_refill: now,
        }
    }
    
    /// Check if a request can be allowed and update counters
    fn try_consume(&mut self, config: &RateLimitConfig) -> bool {
        let now = Instant::now();
        
        // Reset minute counter if needed
        if now >= self.minute_reset_time {
            self.requests_this_minute = 0;
            self.minute_reset_time = now + Duration::from_secs(60);
        }
        
        // Reset hour counter if needed
        if now >= self.hour_reset_time {
            self.requests_this_hour = 0;
            self.hour_reset_time = now + Duration::from_secs(3600);
        }
        
        // Refill burst tokens (token bucket algorithm)
        let time_since_refill = now.duration_since(self.last_refill);
        let tokens_to_add = (time_since_refill.as_secs_f64() * config.burst_size as f64 / 60.0) as u32;
        if tokens_to_add > 0 {
            self.burst_tokens = (self.burst_tokens + tokens_to_add).min(config.burst_size);
            self.last_refill = now;
        }
        
        // Check limits
        let minute_ok = self.requests_this_minute < config.requests_per_minute;
        let hour_ok = self.requests_this_hour < config.requests_per_hour;
        let burst_ok = self.burst_tokens > 0;
        
        if minute_ok && hour_ok && burst_ok {
            self.requests_this_minute += 1;
            self.requests_this_hour += 1;
            if self.burst_tokens > 0 {
                self.burst_tokens -= 1;
            }
            true
        } else {
            false
        }
    }
}

/// Operation-specific rate limiting bucket
#[derive(Debug, Clone)]
struct OperationBucket {
    requests_this_minute: u32,
    minute_reset_time: Instant,
}

impl OperationBucket {
    fn new() -> Self {
        Self {
            requests_this_minute: 0,
            minute_reset_time: Instant::now() + Duration::from_secs(60),
        }
    }
    
    fn try_consume(&mut self, limit: u32) -> bool {
        let now = Instant::now();
        
        // Reset counter if needed
        if now >= self.minute_reset_time {
            self.requests_this_minute = 0;
            self.minute_reset_time = now + Duration::from_secs(60);
        }
        
        if self.requests_this_minute < limit {
            self.requests_this_minute += 1;
            true
        } else {
            false
        }
    }
}

/// Rate limiter service
#[derive(Clone)]
pub struct RateLimiter {
    config: RateLimitConfig,
    user_buckets: Arc<RwLock<HashMap<String, RateLimitBucket>>>,
    operation_buckets: Arc<RwLock<HashMap<String, HashMap<String, OperationBucket>>>>, // user_id -> operation -> bucket
    violation_count: Arc<RwLock<u64>>,
}

impl RateLimiter {
    /// Create a new rate limiter
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            user_buckets: Arc::new(RwLock::new(HashMap::new())),
            operation_buckets: Arc::new(RwLock::new(HashMap::new())),
            violation_count: Arc::new(RwLock::new(0)),
        }
    }
    
    /// Check if a request should be rate limited
    pub async fn check_limit(&self, user_id: &str, operation: &str) -> Result<bool> {
        // Check general user rate limit
        let user_allowed = {
            let mut buckets = self.user_buckets.write().await;
            let bucket = buckets
                .entry(user_id.to_string())
                .or_insert_with(|| RateLimitBucket::new(&self.config));
            bucket.try_consume(&self.config)
        };
        
        if !user_allowed {
            warn!("Rate limit exceeded for user: {}", user_id);
            self.increment_violation_count().await;
            return Ok(false);
        }
        
        // Check operation-specific rate limit if enabled
        if self.config.enable_per_operation_limits {
            if let Some(&operation_limit) = self.config.operation_limits.get(operation) {
                let operation_allowed = {
                    let mut op_buckets = self.operation_buckets.write().await;
                    let user_operations = op_buckets
                        .entry(user_id.to_string())
                        .or_insert_with(HashMap::new);
                    let bucket = user_operations
                        .entry(operation.to_string())
                        .or_insert_with(OperationBucket::new);
                    bucket.try_consume(operation_limit)
                };
                
                if !operation_allowed {
                    warn!("Operation rate limit exceeded for user: {} operation: {}", user_id, operation);
                    self.increment_violation_count().await;
                    return Ok(false);
                }
            }
        }
        
        debug!("Rate limit check passed for user: {} operation: {}", user_id, operation);
        Ok(true)
    }
    
    /// Get current violation count
    pub async fn get_violation_count(&self) -> u64 {
        *self.violation_count.read().await
    }
    
    /// Reset rate limits for a user (admin operation)
    pub async fn reset_user_limits(&self, user_id: &str) -> Result<()> {
        {
            let mut buckets = self.user_buckets.write().await;
            buckets.remove(user_id);
        }
        
        {
            let mut op_buckets = self.operation_buckets.write().await;
            op_buckets.remove(user_id);
        }
        
        debug!("Reset rate limits for user: {}", user_id);
        Ok(())
    }
    
    /// Get rate limit status for a user
    pub async fn get_user_status(&self, user_id: &str) -> RateLimitStatus {
        let buckets = self.user_buckets.read().await;
        
        if let Some(bucket) = buckets.get(user_id) {
            RateLimitStatus {
                requests_remaining_minute: self.config.requests_per_minute.saturating_sub(bucket.requests_this_minute),
                requests_remaining_hour: self.config.requests_per_hour.saturating_sub(bucket.requests_this_hour),
                burst_tokens_remaining: bucket.burst_tokens,
                minute_reset_in_seconds: bucket.minute_reset_time.saturating_duration_since(Instant::now()).as_secs(),
                hour_reset_in_seconds: bucket.hour_reset_time.saturating_duration_since(Instant::now()).as_secs(),
            }
        } else {
            RateLimitStatus {
                requests_remaining_minute: self.config.requests_per_minute,
                requests_remaining_hour: self.config.requests_per_hour,
                burst_tokens_remaining: self.config.burst_size,
                minute_reset_in_seconds: 60,
                hour_reset_in_seconds: 3600,
            }
        }
    }
    
    /// Clean up expired buckets (should be called periodically)
    pub async fn cleanup_expired_buckets(&self) -> Result<usize> {
        let now = Instant::now();
        let mut cleaned = 0;
        
        // Clean up user buckets
        {
            let mut buckets = self.user_buckets.write().await;
            let expired_users: Vec<String> = buckets
                .iter()
                .filter(|(_, bucket)| now > bucket.hour_reset_time)
                .map(|(user_id, _)| user_id.clone())
                .collect();
            
            for user_id in expired_users {
                buckets.remove(&user_id);
                cleaned += 1;
            }
        }
        
        // Clean up operation buckets
        {
            let mut op_buckets = self.operation_buckets.write().await;
            let expired_users: Vec<String> = op_buckets
                .iter()
                .filter(|(_, operations)| {
                    operations.values().all(|bucket| now > bucket.minute_reset_time)
                })
                .map(|(user_id, _)| user_id.clone())
                .collect();
            
            for user_id in expired_users {
                op_buckets.remove(&user_id);
                cleaned += 1;
            }
        }
        
        if cleaned > 0 {
            debug!("Cleaned up {} expired rate limit buckets", cleaned);
        }
        
        Ok(cleaned)
    }
    
    async fn increment_violation_count(&self) {
        let mut count = self.violation_count.write().await;
        *count += 1;
    }
}

/// Rate limit status for a user
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub requests_remaining_minute: u32,
    pub requests_remaining_hour: u32,
    pub burst_tokens_remaining: u32,
    pub minute_reset_in_seconds: u64,
    pub hour_reset_in_seconds: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::test;

    #[test]
    async fn test_basic_rate_limiting() {
        let config = RateLimitConfig {
            requests_per_minute: 5,
            requests_per_hour: 20,
            burst_size: 3,
            enable_per_operation_limits: false,
            operation_limits: HashMap::new(),
        };
        
        let limiter = RateLimiter::new(config);
        let user_id = "test_user";
        
        // Should allow first 5 requests
        for i in 0..5 {
            assert!(limiter.check_limit(user_id, "test_op").await.unwrap(), 
                   "Request {} should be allowed", i);
        }
        
        // 6th request should be denied
        assert!(!limiter.check_limit(user_id, "test_op").await.unwrap(),
               "6th request should be denied");
    }
    
    #[test]
    async fn test_operation_specific_limits() {
        let mut operation_limits = HashMap::new();
        operation_limits.insert("stress_test".to_string(), 2);
        
        let config = RateLimitConfig {
            requests_per_minute: 10,
            requests_per_hour: 50,
            burst_size: 5,
            enable_per_operation_limits: true,
            operation_limits,
        };
        
        let limiter = RateLimiter::new(config);
        let user_id = "test_user";
        
        // Should allow 2 stress test requests
        assert!(limiter.check_limit(user_id, "stress_test").await.unwrap());
        assert!(limiter.check_limit(user_id, "stress_test").await.unwrap());
        
        // 3rd stress test should be denied
        assert!(!limiter.check_limit(user_id, "stress_test").await.unwrap());
        
        // But other operations should still work
        assert!(limiter.check_limit(user_id, "observe").await.unwrap());
    }
    
    #[test]
    async fn test_user_status() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiter::new(config);
        let user_id = "test_user";
        
        // Initial status
        let status = limiter.get_user_status(user_id).await;
        assert_eq!(status.requests_remaining_minute, 60);
        
        // After one request
        limiter.check_limit(user_id, "test_op").await.unwrap();
        let status = limiter.get_user_status(user_id).await;
        assert_eq!(status.requests_remaining_minute, 59);
    }
}
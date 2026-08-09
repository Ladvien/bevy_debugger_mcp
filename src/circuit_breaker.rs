/*
 * Production-Grade Circuit Breaker for BRP Connections
 * Copyright (C) 2025 ladvien
 *
 * Licensed under either of MIT (LICENSE-MIT) or Apache-2.0 (LICENSE-APACHE), at your option.
 *
 * Relicensed from GPL-3.0 when this crate was adopted into Ladvien/foundation_vs_slop: a GPL
 * crate in the Bevy ecosystem cannot be adopted, and being adoptable is why it is published.
 */

use crate::config::CircuitBreakerConfig;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CircuitState {
    Closed,    // Normal operation
    Open,      // Failing, rejecting requests
    HalfOpen,  // Testing if service recovered
}

/// Production-grade circuit breaker for BRP connections
#[derive(Debug)]
pub struct CircuitBreaker {
    config: CircuitBreakerConfig,
    state: AtomicU32, // Uses u32 to represent CircuitState
    failure_count: AtomicU32,
    success_count: AtomicU32,
    last_failure_time: AtomicU64,
    half_open_requests: AtomicU32,
}

impl CircuitBreaker {
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            config,
            state: AtomicU32::new(CircuitState::Closed as u32),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
            last_failure_time: AtomicU64::new(0),
            half_open_requests: AtomicU32::new(0),
        }
    }

    /// Check if request should be allowed through the circuit breaker
    pub fn can_execute(&self) -> bool {
        let current_state = self.get_state();
        
        match current_state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                // Check if we should transition to half-open
                if self.should_attempt_reset() {
                    self.transition_to_half_open();
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => {
                // Allow limited requests to test service recovery
                let current_requests = self.half_open_requests.load(Ordering::Relaxed);
                if current_requests < self.config.half_open_max_requests {
                    self.half_open_requests.fetch_add(1, Ordering::Relaxed);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Record successful operation
    pub fn record_success(&self) {
        let current_state = self.get_state();
        self.success_count.fetch_add(1, Ordering::Relaxed);
        
        match current_state {
            CircuitState::HalfOpen => {
                info!("Circuit breaker: Service recovered, transitioning to CLOSED");
                self.transition_to_closed();
            }
            CircuitState::Closed => {
                // Reset failure count on successful operation
                self.failure_count.store(0, Ordering::Relaxed);
            }
            CircuitState::Open => {
                // This shouldn't happen as requests should be rejected
                warn!("Circuit breaker: Received success while in OPEN state");
            }
        }
        
        debug!("Circuit breaker: Operation succeeded");
    }

    /// Record failed operation
    pub fn record_failure(&self) {
        let current_state = self.get_state();
        let failure_count = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        
        // Update last failure time
        if let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) {
            self.last_failure_time.store(duration.as_millis() as u64, Ordering::Relaxed);
        }

        match current_state {
            CircuitState::Closed => {
                if failure_count >= self.config.failure_threshold {
                    warn!(
                        "Circuit breaker: Failure threshold ({}) exceeded, transitioning to OPEN",
                        self.config.failure_threshold
                    );
                    self.transition_to_open();
                }
            }
            CircuitState::HalfOpen => {
                warn!("Circuit breaker: Half-open test failed, transitioning back to OPEN");
                self.transition_to_open();
            }
            CircuitState::Open => {
                // Already open, just update failure time
                debug!("Circuit breaker: Additional failure recorded in OPEN state");
            }
        }
        
        debug!("Circuit breaker: Operation failed (count: {})", failure_count);
    }

    /// Get current circuit breaker state
    pub fn get_state(&self) -> CircuitState {
        match self.state.load(Ordering::Relaxed) {
            0 => CircuitState::Closed,
            1 => CircuitState::Open,
            2 => CircuitState::HalfOpen,
            _ => CircuitState::Closed, // Default fallback
        }
    }

    /// Get circuit breaker metrics
    pub fn get_metrics(&self) -> CircuitBreakerMetrics {
        CircuitBreakerMetrics {
            state: self.get_state(),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            success_count: self.success_count.load(Ordering::Relaxed),
            half_open_requests: self.half_open_requests.load(Ordering::Relaxed),
        }
    }

    /// Reset circuit breaker to closed state (for testing/manual reset)
    pub fn reset(&self) {
        info!("Circuit breaker: Manual reset to CLOSED state");
        self.transition_to_closed();
    }

    /// Check if enough time has passed to attempt service recovery
    fn should_attempt_reset(&self) -> bool {
        let last_failure = self.last_failure_time.load(Ordering::Relaxed);
        if last_failure == 0 {
            return false;
        }

        if let Ok(current_time) = SystemTime::now().duration_since(UNIX_EPOCH) {
            let elapsed = Duration::from_millis(current_time.as_millis() as u64 - last_failure);
            elapsed >= self.config.reset_timeout
        } else {
            false
        }
    }

    /// Transition to closed state
    fn transition_to_closed(&self) {
        self.state.store(CircuitState::Closed as u32, Ordering::Relaxed);
        self.failure_count.store(0, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
    }

    /// Transition to open state
    fn transition_to_open(&self) {
        self.state.store(CircuitState::Open as u32, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
    }

    /// Transition to half-open state
    fn transition_to_half_open(&self) {
        info!("Circuit breaker: Testing service recovery, transitioning to HALF-OPEN");
        self.state.store(CircuitState::HalfOpen as u32, Ordering::Relaxed);
        self.half_open_requests.store(0, Ordering::Relaxed);
    }
}

/// Circuit breaker metrics for monitoring
#[derive(Debug, Clone)]
pub struct CircuitBreakerMetrics {
    pub state: CircuitState,
    pub failure_count: u32,
    pub success_count: u32,
    pub half_open_requests: u32,
}

impl CircuitBreakerMetrics {
    /// Calculate failure rate as a percentage
    pub fn failure_rate(&self) -> f64 {
        let total = self.failure_count + self.success_count;
        if total == 0 {
            0.0
        } else {
            (self.failure_count as f64 / total as f64) * 100.0
        }
    }
    
    /// Check if circuit breaker is healthy
    pub fn is_healthy(&self) -> bool {
        matches!(self.state, CircuitState::Closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_circuit_breaker_closed_state() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(60),
            half_open_max_requests: 2,
        };
        
        let breaker = CircuitBreaker::new(config);
        assert_eq!(breaker.get_state(), CircuitState::Closed);
        assert!(breaker.can_execute());
    }

    #[test]
    fn test_circuit_breaker_failure_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            reset_timeout: Duration::from_secs(60),
            half_open_max_requests: 2,
        };
        
        let breaker = CircuitBreaker::new(config);
        
        // First failure - should remain closed
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Closed);
        
        // Second failure - should transition to open
        breaker.record_failure();
        assert_eq!(breaker.get_state(), CircuitState::Open);
        assert!(!breaker.can_execute());
    }

    #[test]
    fn test_circuit_breaker_success_resets_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            reset_timeout: Duration::from_secs(60),
            half_open_max_requests: 2,
        };
        
        let breaker = CircuitBreaker::new(config);
        
        // Record some failures
        breaker.record_failure();
        breaker.record_failure();
        
        // Success should reset failure count
        breaker.record_success();
        let metrics = breaker.get_metrics();
        assert_eq!(metrics.failure_count, 0);
        assert_eq!(breaker.get_state(), CircuitState::Closed);
    }

    #[test]
    fn test_circuit_breaker_metrics() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            reset_timeout: Duration::from_secs(60),
            half_open_max_requests: 2,
        };
        
        let breaker = CircuitBreaker::new(config);
        
        breaker.record_success();
        breaker.record_success();
        breaker.record_failure();
        
        let metrics = breaker.get_metrics();
        assert_eq!(metrics.success_count, 2);
        assert_eq!(metrics.failure_count, 1);
        // 1/3 has no exact binary representation, so the literal a human writes down and the value the
        // division produces differ in the last ulp. Comparing floats for exact equality is the bug.
        assert!(
            (metrics.failure_rate() - 100.0 / 3.0).abs() < 1e-9,
            "failure rate should be ~33.33%, got {}",
            metrics.failure_rate()
        );
        assert!(metrics.is_healthy());
    }
}
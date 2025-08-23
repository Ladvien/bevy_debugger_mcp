//! Benchmark system to measure lock contention improvements
//! 
//! This module provides tools to measure the performance impact of our
//! Arc<RwLock<T>> reduction efforts.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use std::thread;
use tokio::sync::RwLock as TokioRwLock;
use tracing::{info, warn};

/// Results from lock contention benchmarks
#[derive(Debug, Clone)]
pub struct ContentionBenchmarkResults {
    pub scenario: String,
    pub thread_count: usize,
    pub operation_count: usize,
    pub total_duration: Duration,
    pub avg_lock_wait_time: Duration,
    pub max_lock_wait_time: Duration,
    pub contention_percentage: f64,
    pub throughput_ops_per_sec: f64,
    pub failed_operations: usize,
}

/// Benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    pub thread_count: usize,
    pub operations_per_thread: usize,
    pub read_write_ratio: f64, // 0.0 = all writes, 1.0 = all reads
    pub hold_duration_ms: u64,
    pub timeout_ms: u64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            thread_count: 8,
            operations_per_thread: 1000,
            read_write_ratio: 0.8, // 80% reads, 20% writes
            hold_duration_ms: 1,
            timeout_ms: 100,
        }
    }
}

/// Benchmark a std RwLock-based data structure
pub async fn benchmark_std_rwlock(config: BenchmarkConfig) -> ContentionBenchmarkResults {
    info!("Starting std::RwLock benchmark with {} threads", config.thread_count);
    
    let data = Arc::new(RwLock::new(HashMap::<String, u64>::new()));
    let start_time = Instant::now();
    let mut handles = Vec::new();
    
    let total_ops = config.thread_count * config.operations_per_thread;
    
    for thread_id in 0..config.thread_count {
        let data_clone = data.clone();
        let config_clone = config.clone();
        
        let handle = thread::spawn(move || {
            let mut lock_wait_times = Vec::new();
            let mut failed_ops = 0;
            
            for op_id in 0..config_clone.operations_per_thread {
                let key = format!("thread_{}_op_{}", thread_id, op_id);
                let is_read = (op_id as f64 / config_clone.operations_per_thread as f64) < config_clone.read_write_ratio;
                
                let lock_start = Instant::now();
                
                if is_read {
                    // Read operation
                    match data_clone.try_read() {
                        Ok(guard) => {
                            let wait_time = lock_start.elapsed();
                            lock_wait_times.push(wait_time);
                            
                            // Simulate work
                            let _ = guard.get(&key);
                            thread::sleep(Duration::from_millis(config_clone.hold_duration_ms));
                        }
                        Err(_) => {
                            failed_ops += 1;
                        }
                    }
                } else {
                    // Write operation
                    match data_clone.try_write() {
                        Ok(mut guard) => {
                            let wait_time = lock_start.elapsed();
                            lock_wait_times.push(wait_time);
                            
                            // Simulate work
                            guard.insert(key, thread_id as u64);
                            thread::sleep(Duration::from_millis(config_clone.hold_duration_ms));
                        }
                        Err(_) => {
                            failed_ops += 1;
                        }
                    }
                }
            }
            
            (lock_wait_times, failed_ops)
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut all_wait_times = Vec::new();
    let mut total_failed_ops = 0;
    
    for handle in handles {
        let (wait_times, failed_ops) = handle.join().unwrap();
        all_wait_times.extend(wait_times);
        total_failed_ops += failed_ops;
    }
    
    let total_duration = start_time.elapsed();
    
    // Calculate statistics
    let avg_wait_time = if all_wait_times.is_empty() {
        Duration::ZERO
    } else {
        let total_nanos: u128 = all_wait_times.iter().map(|d| d.as_nanos()).sum();
        Duration::from_nanos((total_nanos / all_wait_times.len() as u128) as u64)
    };
    
    let max_wait_time = all_wait_times.iter().max().cloned().unwrap_or(Duration::ZERO);
    let contention_percentage = (total_failed_ops as f64 / total_ops as f64) * 100.0;
    let throughput = (total_ops - total_failed_ops) as f64 / total_duration.as_secs_f64();
    
    ContentionBenchmarkResults {
        scenario: "std::RwLock".to_string(),
        thread_count: config.thread_count,
        operation_count: total_ops,
        total_duration,
        avg_lock_wait_time: avg_wait_time,
        max_lock_wait_time: max_wait_time,
        contention_percentage,
        throughput_ops_per_sec: throughput,
        failed_operations: total_failed_ops,
    }
}

/// Benchmark a tokio RwLock-based data structure
pub async fn benchmark_tokio_rwlock(config: BenchmarkConfig) -> ContentionBenchmarkResults {
    info!("Starting tokio::RwLock benchmark with {} threads", config.thread_count);
    
    let data = Arc::new(TokioRwLock::new(HashMap::<String, u64>::new()));
    let start_time = Instant::now();
    let mut handles = Vec::new();
    
    let total_ops = config.thread_count * config.operations_per_thread;
    
    for thread_id in 0..config.thread_count {
        let data_clone = data.clone();
        let config_clone = config.clone();
        
        let handle = tokio::spawn(async move {
            let mut lock_wait_times = Vec::new();
            let mut failed_ops = 0;
            
            for op_id in 0..config_clone.operations_per_thread {
                let key = format!("thread_{}_op_{}", thread_id, op_id);
                let is_read = (op_id as f64 / config_clone.operations_per_thread as f64) < config_clone.read_write_ratio;
                
                let lock_start = Instant::now();
                
                if is_read {
                    // Read operation
                    match tokio::time::timeout(
                        Duration::from_millis(config_clone.timeout_ms),
                        data_clone.read()
                    ).await {
                        Ok(guard) => {
                            let wait_time = lock_start.elapsed();
                            lock_wait_times.push(wait_time);
                            
                            // Simulate work
                            let _ = guard.get(&key);
                            tokio::time::sleep(Duration::from_millis(config_clone.hold_duration_ms)).await;
                        }
                        Err(_) => {
                            failed_ops += 1;
                        }
                    }
                } else {
                    // Write operation
                    match tokio::time::timeout(
                        Duration::from_millis(config_clone.timeout_ms),
                        data_clone.write()
                    ).await {
                        Ok(mut guard) => {
                            let wait_time = lock_start.elapsed();
                            lock_wait_times.push(wait_time);
                            
                            // Simulate work
                            guard.insert(key, thread_id as u64);
                            tokio::time::sleep(Duration::from_millis(config_clone.hold_duration_ms)).await;
                        }
                        Err(_) => {
                            failed_ops += 1;
                        }
                    }
                }
            }
            
            (lock_wait_times, failed_ops)
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut all_wait_times = Vec::new();
    let mut total_failed_ops = 0;
    
    for handle in handles {
        let (wait_times, failed_ops) = handle.await.unwrap();
        all_wait_times.extend(wait_times);
        total_failed_ops += failed_ops;
    }
    
    let total_duration = start_time.elapsed();
    
    // Calculate statistics
    let avg_wait_time = if all_wait_times.is_empty() {
        Duration::ZERO
    } else {
        let total_nanos: u128 = all_wait_times.iter().map(|d| d.as_nanos()).sum();
        Duration::from_nanos((total_nanos / all_wait_times.len() as u128) as u64)
    };
    
    let max_wait_time = all_wait_times.iter().max().cloned().unwrap_or(Duration::ZERO);
    let contention_percentage = (total_failed_ops as f64 / total_ops as f64) * 100.0;
    let throughput = (total_ops - total_failed_ops) as f64 / total_duration.as_secs_f64();
    
    ContentionBenchmarkResults {
        scenario: "tokio::RwLock".to_string(),
        thread_count: config.thread_count,
        operation_count: total_ops,
        total_duration,
        avg_lock_wait_time: avg_wait_time,
        max_lock_wait_time: max_wait_time,
        contention_percentage,
        throughput_ops_per_sec: throughput,
        failed_operations: total_failed_ops,
    }
}

/// Benchmark message-passing based data structure (actor model)
pub async fn benchmark_message_passing(config: BenchmarkConfig) -> ContentionBenchmarkResults {
    info!("Starting message-passing benchmark with {} threads", config.thread_count);
    
    use tokio::sync::mpsc;
    
    #[derive(Debug)]
    enum DataMessage {
        Read(String, tokio::sync::oneshot::Sender<Option<u64>>),
        Write(String, u64, tokio::sync::oneshot::Sender<()>),
    }
    
    // Create actor
    let (sender, mut receiver) = mpsc::unbounded_channel();
    
    // Spawn actor task
    let actor_handle = tokio::spawn(async move {
        let mut data = HashMap::<String, u64>::new();
        
        while let Some(message) = receiver.recv().await {
            match message {
                DataMessage::Read(key, respond_to) => {
                    let value = data.get(&key).cloned();
                    let _ = respond_to.send(value);
                    
                    // Simulate processing time
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
                DataMessage::Write(key, value, respond_to) => {
                    data.insert(key, value);
                    let _ = respond_to.send(());
                    
                    // Simulate processing time
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            }
        }
    });
    
    let start_time = Instant::now();
    let mut handles = Vec::new();
    let total_ops = config.thread_count * config.operations_per_thread;
    
    for thread_id in 0..config.thread_count {
        let sender_clone = sender.clone();
        let config_clone = config.clone();
        
        let handle = tokio::spawn(async move {
            let mut wait_times = Vec::new();
            let mut failed_ops = 0;
            
            for op_id in 0..config_clone.operations_per_thread {
                let key = format!("thread_{}_op_{}", thread_id, op_id);
                let is_read = (op_id as f64 / config_clone.operations_per_thread as f64) < config_clone.read_write_ratio;
                
                let operation_start = Instant::now();
                
                if is_read {
                    // Read operation
                    let (respond_to, response) = tokio::sync::oneshot::channel();
                    
                    if sender_clone.send(DataMessage::Read(key, respond_to)).is_err() {
                        failed_ops += 1;
                        continue;
                    }
                    
                    match tokio::time::timeout(
                        Duration::from_millis(config_clone.timeout_ms),
                        response
                    ).await {
                        Ok(_) => {
                            let wait_time = operation_start.elapsed();
                            wait_times.push(wait_time);
                        }
                        Err(_) => {
                            failed_ops += 1;
                        }
                    }
                } else {
                    // Write operation
                    let (respond_to, response) = tokio::sync::oneshot::channel();
                    
                    if sender_clone.send(DataMessage::Write(key, thread_id as u64, respond_to)).is_err() {
                        failed_ops += 1;
                        continue;
                    }
                    
                    match tokio::time::timeout(
                        Duration::from_millis(config_clone.timeout_ms),
                        response
                    ).await {
                        Ok(_) => {
                            let wait_time = operation_start.elapsed();
                            wait_times.push(wait_time);
                        }
                        Err(_) => {
                            failed_ops += 1;
                        }
                    }
                }
            }
            
            (wait_times, failed_ops)
        });
        
        handles.push(handle);
    }
    
    // Collect results
    let mut all_wait_times = Vec::new();
    let mut total_failed_ops = 0;
    
    for handle in handles {
        let (wait_times, failed_ops) = handle.await.unwrap();
        all_wait_times.extend(wait_times);
        total_failed_ops += failed_ops;
    }
    
    // Shutdown actor
    drop(sender);
    let _ = actor_handle.await;
    
    let total_duration = start_time.elapsed();
    
    // Calculate statistics
    let avg_wait_time = if all_wait_times.is_empty() {
        Duration::ZERO
    } else {
        let total_nanos: u128 = all_wait_times.iter().map(|d| d.as_nanos()).sum();
        Duration::from_nanos((total_nanos / all_wait_times.len() as u128) as u64)
    };
    
    let max_wait_time = all_wait_times.iter().max().cloned().unwrap_or(Duration::ZERO);
    let contention_percentage = (total_failed_ops as f64 / total_ops as f64) * 100.0;
    let throughput = (total_ops - total_failed_ops) as f64 / total_duration.as_secs_f64();
    
    ContentionBenchmarkResults {
        scenario: "Message Passing".to_string(),
        thread_count: config.thread_count,
        operation_count: total_ops,
        total_duration,
        avg_lock_wait_time: avg_wait_time,
        max_lock_wait_time: max_wait_time,
        contention_percentage,
        throughput_ops_per_sec: throughput,
        failed_operations: total_failed_ops,
    }
}

/// Run comprehensive lock contention benchmarks
pub async fn run_comprehensive_benchmark() -> Vec<ContentionBenchmarkResults> {
    info!("Starting comprehensive lock contention benchmark suite");
    
    let mut results = Vec::new();
    
    // Test with different configurations
    let configs = vec![
        BenchmarkConfig { thread_count: 2, ..Default::default() },
        BenchmarkConfig { thread_count: 4, ..Default::default() },
        BenchmarkConfig { thread_count: 8, ..Default::default() },
        BenchmarkConfig { thread_count: 16, ..Default::default() },
    ];
    
    for config in configs {
        info!("Running benchmarks with {} threads", config.thread_count);
        
        // Benchmark std RwLock
        let std_result = benchmark_std_rwlock(config.clone()).await;
        results.push(std_result);
        
        // Benchmark tokio RwLock  
        let tokio_result = benchmark_tokio_rwlock(config.clone()).await;
        results.push(tokio_result);
        
        // Benchmark message passing
        let message_result = benchmark_message_passing(config.clone()).await;
        results.push(message_result);
        
        // Small delay between test runs
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    
    info!("Comprehensive benchmark suite completed");
    results
}

/// Print benchmark results in a formatted table
pub fn print_benchmark_results(results: &[ContentionBenchmarkResults]) {
    println!("\n=== Lock Contention Benchmark Results ===");
    println!("{:<15} {:<8} {:<12} {:<15} {:<15} {:<12} {:<15}", 
        "Scenario", "Threads", "Ops/sec", "Avg Wait (μs)", "Max Wait (μs)", 
        "Contention %", "Failed Ops");
    println!("{:-<100}", "");
    
    for result in results {
        println!("{:<15} {:<8} {:<12.0} {:<15.0} {:<15.0} {:<12.2} {:<15}", 
            result.scenario,
            result.thread_count,
            result.throughput_ops_per_sec,
            result.avg_lock_wait_time.as_micros(),
            result.max_lock_wait_time.as_micros(),
            result.contention_percentage,
            result.failed_operations
        );
    }
    
    println!("\n=== Performance Analysis ===");
    
    // Find best performer
    if let Some(best) = results.iter().min_by(|a, b| 
        a.contention_percentage.partial_cmp(&b.contention_percentage).unwrap()
    ) {
        println!("✅ Best contention performance: {} ({:.2}% contention)", 
            best.scenario, best.contention_percentage);
    }
    
    if let Some(fastest) = results.iter().max_by(|a, b| 
        a.throughput_ops_per_sec.partial_cmp(&b.throughput_ops_per_sec).unwrap()
    ) {
        println!("⚡ Highest throughput: {} ({:.0} ops/sec)", 
            fastest.scenario, fastest.throughput_ops_per_sec);
    }
    
    // Check if we meet our <1% contention goal
    let low_contention: Vec<_> = results.iter()
        .filter(|r| r.contention_percentage < 1.0)
        .collect();
    
    if !low_contention.is_empty() {
        println!("🎯 Scenarios meeting <1% contention goal: {}", 
            low_contention.len());
        for result in low_contention {
            println!("   - {}: {:.2}%", result.scenario, result.contention_percentage);
        }
    } else {
        warn!("⚠️  No scenarios achieved <1% contention goal");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_std_rwlock_benchmark() {
        let config = BenchmarkConfig {
            thread_count: 2,
            operations_per_thread: 100,
            ..Default::default()
        };
        
        let result = benchmark_std_rwlock(config).await;
        assert_eq!(result.thread_count, 2);
        assert_eq!(result.operation_count, 200);
        assert!(!result.scenario.is_empty());
    }

    #[tokio::test]
    async fn test_message_passing_benchmark() {
        let config = BenchmarkConfig {
            thread_count: 2,
            operations_per_thread: 100,
            ..Default::default()
        };
        
        let result = benchmark_message_passing(config).await;
        assert_eq!(result.thread_count, 2);
        assert_eq!(result.operation_count, 200);
        assert_eq!(result.scenario, "Message Passing");
    }
}
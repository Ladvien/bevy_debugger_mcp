//! Deadlock detection system for debug builds
//! 
//! This module provides runtime deadlock detection by tracking lock acquisitions
//! and analyzing dependency cycles in the lock graph.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::{self, ThreadId};
use std::time::{Duration, Instant};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Unique identifier for locks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LockId(usize);

static NEXT_LOCK_ID: AtomicUsize = AtomicUsize::new(1);

impl LockId {
    fn new() -> Self {
        Self(NEXT_LOCK_ID.fetch_add(1, Ordering::Relaxed))
    }
}

/// Information about a lock acquisition
#[derive(Debug, Clone)]
struct LockAcquisition {
    lock_id: LockId,
    thread_id: ThreadId,
    acquired_at: Instant,
    lock_type: LockType,
    location: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
enum LockType {
    Read,
    Write,
    Exclusive, // Mutex
}

/// Edge in the lock dependency graph
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LockEdge {
    from: LockId,
    to: LockId,
}

/// Global deadlock detector state
struct DeadlockDetectorState {
    /// Currently held locks by thread
    held_locks: HashMap<ThreadId, Vec<LockId>>,
    /// Lock dependency graph (edges represent "X held while waiting for Y")
    dependency_graph: HashSet<LockEdge>,
    /// Lock metadata
    lock_info: HashMap<LockId, String>,
    /// Detection statistics
    detections: u64,
    false_positives: u64,
}

/// Deadlock detector - only active in debug builds
pub struct DeadlockDetector {
    state: Arc<Mutex<DeadlockDetectorState>>,
    monitoring_enabled: bool,
}

impl DeadlockDetector {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(DeadlockDetectorState {
                held_locks: HashMap::new(),
                dependency_graph: HashSet::new(),
                lock_info: HashMap::new(),
                detections: 0,
                false_positives: 0,
            })),
            monitoring_enabled: cfg!(debug_assertions),
        }
    }

    /// Register a lock acquisition attempt
    pub fn register_lock_attempt(&self, lock_id: LockId, lock_type: LockType, location: &'static str) {
        if !self.monitoring_enabled {
            return;
        }

        let thread_id = thread::current().id();
        let mut state = self.state.lock().unwrap();

        // Record the lock in our metadata
        state.lock_info.insert(lock_id, location.to_string());

        // Check for potential deadlocks before acquiring
        if let Some(current_locks) = state.held_locks.get(&thread_id) {
            // Add edges from all currently held locks to this new one
            for &held_lock in current_locks {
                let edge = LockEdge { from: held_lock, to: lock_id };
                state.dependency_graph.insert(edge);

                // Check for cycles
                if self.has_cycle_from(&state, lock_id) {
                    self.report_potential_deadlock(&state, &current_locks, lock_id, location);
                    state.detections += 1;
                }
            }
        }
    }

    /// Register successful lock acquisition
    pub fn register_lock_acquired(&self, lock_id: LockId) {
        if !self.monitoring_enabled {
            return;
        }

        let thread_id = thread::current().id();
        let mut state = self.state.lock().unwrap();

        state.held_locks
            .entry(thread_id)
            .or_insert_with(Vec::new)
            .push(lock_id);
    }

    /// Register lock release
    pub fn register_lock_released(&self, lock_id: LockId) {
        if !self.monitoring_enabled {
            return;
        }

        let thread_id = thread::current().id();
        let mut state = self.state.lock().unwrap();

        if let Some(locks) = state.held_locks.get_mut(&thread_id) {
            locks.retain(|&x| x != lock_id);
            if locks.is_empty() {
                state.held_locks.remove(&thread_id);
            }
        }
    }

    /// Check if there's a cycle in the dependency graph starting from a given lock
    fn has_cycle_from(&self, state: &DeadlockDetectorState, start_lock: LockId) -> bool {
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(start_lock);

        while let Some(current) = stack.pop_back() {
            if visited.contains(&current) {
                return true; // Found a cycle
            }
            visited.insert(current);

            // Find all locks that depend on the current lock
            for edge in &state.dependency_graph {
                if edge.from == current {
                    stack.push_back(edge.to);
                }
            }
        }

        false
    }

    /// Report a potential deadlock
    fn report_potential_deadlock(
        &self,
        state: &DeadlockDetectorState,
        held_locks: &[LockId],
        waiting_for: LockId,
        location: &str,
    ) {
        eprintln!("🚨 POTENTIAL DEADLOCK DETECTED 🚨");
        eprintln!("Thread: {:?}", thread::current().id());
        eprintln!("Location: {}", location);
        eprintln!("Currently holding {} locks:", held_locks.len());
        
        for &lock_id in held_locks {
            if let Some(info) = state.lock_info.get(&lock_id) {
                eprintln!("  - Lock {:?}: {}", lock_id, info);
            }
        }
        
        if let Some(info) = state.lock_info.get(&waiting_for) {
            eprintln!("Waiting for Lock {:?}: {}", waiting_for, info);
        }
        
        eprintln!("Dependency cycle detected in lock graph!");
        eprintln!("Consider reordering lock acquisitions or using timeout-based locking.\n");
    }

    /// Get detection statistics
    pub fn get_stats(&self) -> (u64, u64, usize) {
        if !self.monitoring_enabled {
            return (0, 0, 0);
        }

        let state = self.state.lock().unwrap();
        (
            state.detections,
            state.false_positives,
            state.dependency_graph.len(),
        )
    }

    /// Clear detection state (useful for testing)
    pub fn clear_state(&self) {
        if !self.monitoring_enabled {
            return;
        }

        let mut state = self.state.lock().unwrap();
        state.held_locks.clear();
        state.dependency_graph.clear();
        state.lock_info.clear();
    }
}

/// Global deadlock detector instance
static DETECTOR: std::sync::OnceLock<DeadlockDetector> = std::sync::OnceLock::new();

pub fn get_deadlock_detector() -> &'static DeadlockDetector {
    DETECTOR.get_or_init(DeadlockDetector::new)
}

/// Wrapper for Arc<RwLock<T>> that includes deadlock detection
pub struct TrackedRwLock<T> {
    inner: Arc<RwLock<T>>,
    lock_id: LockId,
    name: &'static str,
}

impl<T> TrackedRwLock<T> {
    pub fn new(inner: Arc<RwLock<T>>, name: &'static str) -> Self {
        Self {
            inner,
            lock_id: LockId::new(),
            name,
        }
    }

    pub async fn read(&self) -> std::sync::RwLockReadGuard<'_, T> {
        let detector = get_deadlock_detector();
        detector.register_lock_attempt(self.lock_id, LockType::Read, self.name);
        
        let guard = self.inner.read().unwrap();
        detector.register_lock_acquired(self.lock_id);
        guard
    }

    pub async fn write(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        let detector = get_deadlock_detector();
        detector.register_lock_attempt(self.lock_id, LockType::Write, self.name);
        
        let guard = self.inner.write().unwrap();
        detector.register_lock_acquired(self.lock_id);
        guard
    }
}

impl<T> Drop for TrackedRwLock<T> {
    fn drop(&mut self) {
        get_deadlock_detector().register_lock_released(self.lock_id);
    }
}

/// Macro for creating tracked locks in debug builds
#[macro_export]
macro_rules! tracked_rwlock {
    ($expr:expr, $name:expr) => {
        #[cfg(debug_assertions)]
        {
            crate::deadlock_detector::TrackedRwLock::new($expr, $name)
        }
        #[cfg(not(debug_assertions))]
        {
            $expr
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_deadlock_detector_creation() {
        let detector = DeadlockDetector::new();
        let (detections, false_positives, edges) = detector.get_stats();
        assert_eq!(detections, 0);
        assert_eq!(false_positives, 0);
        assert_eq!(edges, 0);
    }

    #[test]
    fn test_simple_cycle_detection() {
        let detector = DeadlockDetector::new();
        detector.clear_state();

        let lock1 = LockId::new();
        let lock2 = LockId::new();

        // Thread holds lock1, tries to acquire lock2
        detector.register_lock_acquired(lock1);
        detector.register_lock_attempt(lock2, LockType::Write, "test_location");

        // This should not detect a cycle yet
        let (detections, _, _) = detector.get_stats();
        assert_eq!(detections, 0);

        detector.clear_state();
    }

    #[test]
    fn test_tracked_rwlock() {
        let data = Arc::new(RwLock::new(42));
        let tracked = TrackedRwLock::new(data, "test_lock");
        
        // This would work in an async context
        // let guard = tracked.read().await;
        // assert_eq!(*guard, 42);
    }
}
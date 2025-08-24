use std::sync::{Arc, Weak};
use tokio::sync::{RwLock, Mutex, OnceCell};
use tracing::{debug, info};

use crate::brp_client::BrpClient;
use crate::config::Config;
use crate::debug_command_processor::{DebugCommandRouter, EntityInspectionProcessor};
use crate::entity_inspector::EntityInspector;
use crate::system_profiler::SystemProfiler;
use crate::system_profiler_processor::SystemProfilerProcessor;
use crate::visual_debug_overlay_processor::VisualDebugOverlayProcessor;
use crate::query_builder_processor::QueryBuilderProcessor;
use crate::memory_profiler_processor::MemoryProfilerProcessor;
use crate::session_processor::SessionProcessor;
use crate::issue_detector_processor::IssueDetectorProcessor;
use crate::performance_budget_processor::PerformanceBudgetProcessor;
use crate::pattern_learning::PatternLearningSystem;
use crate::suggestion_engine::SuggestionEngine;
use crate::workflow_automation::WorkflowAutomation;
use crate::hot_reload::{HotReloadSystem, HotReloadConfig};
use crate::error::Result;

/// Lazy initialization manager for performance optimization
/// 
/// This struct provides lazy initialization of expensive debug components
/// to reduce startup time when debugging features are not immediately needed.
pub struct LazyComponents {
    brp_client: Arc<RwLock<BrpClient>>,
    
    // Core components - lazily initialized
    entity_inspector: OnceCell<Arc<EntityInspector>>,
    system_profiler: OnceCell<Arc<SystemProfiler>>,
    
    // Processor components - lazily initialized
    entity_processor: OnceCell<Arc<EntityInspectionProcessor>>,
    profiler_processor: OnceCell<Arc<SystemProfilerProcessor>>,
    visual_overlay_processor: OnceCell<Arc<VisualDebugOverlayProcessor>>,
    query_builder_processor: OnceCell<Arc<QueryBuilderProcessor>>,
    memory_profiler_processor: OnceCell<Arc<MemoryProfilerProcessor>>,
    session_processor: OnceCell<Arc<SessionProcessor>>,
    issue_detector_processor: OnceCell<Arc<IssueDetectorProcessor>>,
    performance_budget_processor: OnceCell<Arc<PerformanceBudgetProcessor>>,
    
    // Debug command router - lazily initialized
    debug_command_router: OnceCell<Arc<DebugCommandRouter>>,
    
    // Machine learning components - lazily initialized
    pattern_learning_system: OnceCell<Arc<PatternLearningSystem>>,
    suggestion_engine: OnceCell<Arc<SuggestionEngine>>,
    workflow_automation: OnceCell<Arc<WorkflowAutomation>>,
    hot_reload_system: OnceCell<Arc<HotReloadSystem>>,
    
    // Initialization mutex to prevent race conditions
    init_mutex: Mutex<()>,
}

impl LazyComponents {
    /// Create new lazy components manager
    pub fn new(brp_client: Arc<RwLock<BrpClient>>) -> Self {
        debug!("Creating lazy components manager");
        Self {
            brp_client,
            entity_inspector: OnceCell::new(),
            system_profiler: OnceCell::new(),
            entity_processor: OnceCell::new(),
            profiler_processor: OnceCell::new(),
            visual_overlay_processor: OnceCell::new(),
            query_builder_processor: OnceCell::new(),
            memory_profiler_processor: OnceCell::new(),
            session_processor: OnceCell::new(),
            issue_detector_processor: OnceCell::new(),
            performance_budget_processor: OnceCell::new(),
            debug_command_router: OnceCell::new(),
            pattern_learning_system: OnceCell::new(),
            suggestion_engine: OnceCell::new(),
            workflow_automation: OnceCell::new(),
            hot_reload_system: OnceCell::new(),
            init_mutex: Mutex::new(()),
        }
    }
    
    /// Get or initialize entity inspector
    pub async fn get_entity_inspector(&self) -> Arc<EntityInspector> {
        // Try to get existing without cloning Arc immediately
        if let Some(inspector) = self.entity_inspector.get() {
            return Arc::clone(inspector);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock - still avoid unnecessary clone
        if let Some(inspector) = self.entity_inspector.get() {
            return Arc::clone(inspector);
        }
        
        debug!("Lazy initializing EntityInspector");
        let inspector = Arc::new(EntityInspector::new(Arc::clone(&self.brp_client)));
        
        // Store and return the same Arc instance
        let _ = self.entity_inspector.set(Arc::clone(&inspector));
        
        info!("EntityInspector initialized lazily");
        inspector
    }
    
    /// Get or initialize system profiler
    pub async fn get_system_profiler(&self) -> Arc<SystemProfiler> {
        if let Some(profiler) = self.system_profiler.get() {
            return Arc::clone(profiler);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(profiler) = self.system_profiler.get() {
            return Arc::clone(profiler);
        }
        
        debug!("Lazy initializing SystemProfiler");
        let profiler = Arc::new(SystemProfiler::new(Arc::clone(&self.brp_client)));
        
        let _ = self.system_profiler.set(Arc::clone(&profiler));
        
        info!("SystemProfiler initialized lazily");
        profiler
    }
    
    /// Get or initialize entity inspection processor
    pub async fn get_entity_processor(&self) -> Arc<EntityInspectionProcessor> {
        if let Some(processor) = self.entity_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.entity_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing EntityInspectionProcessor");
        let inspector = self.get_entity_inspector().await;
        let processor = Arc::new(EntityInspectionProcessor::new(inspector));
        
        let _ = self.entity_processor.set(Arc::clone(&processor));
        
        info!("EntityInspectionProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize system profiler processor
    pub async fn get_profiler_processor(&self) -> Arc<SystemProfilerProcessor> {
        if let Some(processor) = self.profiler_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.profiler_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing SystemProfilerProcessor");
        let profiler = self.get_system_profiler().await;
        let processor = Arc::new(SystemProfilerProcessor::new(profiler));
        
        let _ = self.profiler_processor.set(Arc::clone(&processor));
        
        info!("SystemProfilerProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize visual debug overlay processor
    pub async fn get_visual_overlay_processor(&self) -> Arc<VisualDebugOverlayProcessor> {
        if let Some(processor) = self.visual_overlay_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.visual_overlay_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing VisualDebugOverlayProcessor");
        let processor = Arc::new(VisualDebugOverlayProcessor::new(Arc::clone(&self.brp_client)));
        
        let _ = self.visual_overlay_processor.set(Arc::clone(&processor));
        
        info!("VisualDebugOverlayProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize query builder processor
    pub async fn get_query_builder_processor(&self) -> Arc<QueryBuilderProcessor> {
        if let Some(processor) = self.query_builder_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.query_builder_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing QueryBuilderProcessor");
        let processor = Arc::new(QueryBuilderProcessor::new(Arc::clone(&self.brp_client)));
        
        let _ = self.query_builder_processor.set(Arc::clone(&processor));
        
        info!("QueryBuilderProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize memory profiler processor
    pub async fn get_memory_profiler_processor(&self) -> Arc<MemoryProfilerProcessor> {
        if let Some(processor) = self.memory_profiler_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.memory_profiler_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing MemoryProfilerProcessor");
        let processor = Arc::new(MemoryProfilerProcessor::new(Arc::clone(&self.brp_client)));
        
        let _ = self.memory_profiler_processor.set(Arc::clone(&processor));
        
        info!("MemoryProfilerProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize session processor
    pub async fn get_session_processor(&self) -> Arc<SessionProcessor> {
        if let Some(processor) = self.session_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.session_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing SessionProcessor");
        let processor = Arc::new(SessionProcessor::new(Arc::clone(&self.brp_client)));
        
        // Start session processor for background tasks with proper error handling
        // Use Weak reference to avoid circular dependency in the task
        let processor_weak = Arc::downgrade(&processor);
        let task_handle = tokio::spawn(async move {
            if let Some(processor_strong) = processor_weak.upgrade() {
                if let Err(e) = processor_strong.start().await {
                    tracing::error!("Failed to start session processor: {}", e);
                    return Err(e);
                }
            }
            Ok(())
        });
        
        // TODO: Store task handle for proper lifecycle management
        // In a real implementation, we should track spawned tasks
        // and provide a way to shut them down gracefully
        
        let _ = self.session_processor.set(Arc::clone(&processor));
        
        info!("SessionProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize issue detector processor
    pub async fn get_issue_detector_processor(&self) -> Arc<IssueDetectorProcessor> {
        if let Some(processor) = self.issue_detector_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.issue_detector_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing IssueDetectorProcessor");
        let processor = Arc::new(IssueDetectorProcessor::new(Arc::clone(&self.brp_client)));
        
        let _ = self.issue_detector_processor.set(Arc::clone(&processor));
        
        info!("IssueDetectorProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize performance budget processor
    pub async fn get_performance_budget_processor(&self) -> Arc<PerformanceBudgetProcessor> {
        if let Some(processor) = self.performance_budget_processor.get() {
            return Arc::clone(processor);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(processor) = self.performance_budget_processor.get() {
            return Arc::clone(processor);
        }
        
        debug!("Lazy initializing PerformanceBudgetProcessor");
        let processor = Arc::new(PerformanceBudgetProcessor::new(Arc::clone(&self.brp_client)));
        
        let _ = self.performance_budget_processor.set(Arc::clone(&processor));
        
        info!("PerformanceBudgetProcessor initialized lazily");
        processor
    }
    
    /// Get or initialize debug command router with all processors
    pub async fn get_debug_command_router(&self) -> Arc<DebugCommandRouter> {
        if let Some(router) = self.debug_command_router.get() {
            return Arc::clone(router);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(router) = self.debug_command_router.get() {
            return Arc::clone(router);
        }
        
        debug!("Lazy initializing DebugCommandRouter");
        let router = Arc::new(DebugCommandRouter::new());
        
        // Initialize processors synchronously to avoid race conditions
        // This ensures the router is fully configured before being returned
        let entity_processor = self.get_entity_processor().await;
        let profiler_processor = self.get_profiler_processor().await;
        let visual_overlay_processor = self.get_visual_overlay_processor().await;
        let query_builder_processor = self.get_query_builder_processor().await;
        let memory_profiler_processor = self.get_memory_profiler_processor().await;
        let session_processor = self.get_session_processor().await;
        let issue_detector_processor = self.get_issue_detector_processor().await;
        let performance_budget_processor = self.get_performance_budget_processor().await;
        
        // Register all processors before storing the router
        router.register_processor("entity_inspection".to_string(), entity_processor).await;
        router.register_processor("system_profiling".to_string(), profiler_processor).await;
        router.register_processor("visual_debug_overlay".to_string(), visual_overlay_processor).await;
        router.register_processor("query_builder".to_string(), query_builder_processor).await;
        router.register_processor("memory_profiler".to_string(), memory_profiler_processor).await;
        router.register_processor("session_manager".to_string(), session_processor).await;
        router.register_processor("issue_detector".to_string(), issue_detector_processor).await;
        router.register_processor("performance_budget".to_string(), performance_budget_processor).await;
        
        info!("Debug command router processors registered lazily");
        
        let _ = self.debug_command_router.set(Arc::clone(&router));
        
        info!("DebugCommandRouter initialized lazily");
        router
    }
    
    /// Get or initialize pattern learning system
    pub async fn get_pattern_learning_system(&self) -> Arc<PatternLearningSystem> {
        if let Some(system) = self.pattern_learning_system.get() {
            return Arc::clone(system);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(system) = self.pattern_learning_system.get() {
            return Arc::clone(system);
        }
        
        debug!("Lazy initializing PatternLearningSystem");
        let system = Arc::new(PatternLearningSystem::new());
        
        let _ = self.pattern_learning_system.set(Arc::clone(&system));
        
        info!("PatternLearningSystem initialized lazily");
        system
    }
    
    /// Get or initialize suggestion engine
    pub async fn get_suggestion_engine(&self) -> Arc<SuggestionEngine> {
        if let Some(engine) = self.suggestion_engine.get() {
            return Arc::clone(engine);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(engine) = self.suggestion_engine.get() {
            return Arc::clone(engine);
        }
        
        debug!("Lazy initializing SuggestionEngine");
        let pattern_system = self.get_pattern_learning_system().await;
        let engine = Arc::new(SuggestionEngine::new(pattern_system));
        
        let _ = self.suggestion_engine.set(Arc::clone(&engine));
        
        info!("SuggestionEngine initialized lazily");
        engine
    }
    
    /// Get or initialize workflow automation
    pub async fn get_workflow_automation(&self) -> Arc<WorkflowAutomation> {
        if let Some(automation) = self.workflow_automation.get() {
            return Arc::clone(automation);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(automation) = self.workflow_automation.get() {
            return Arc::clone(automation);
        }
        
        debug!("Lazy initializing WorkflowAutomation");
        let pattern_system = self.get_pattern_learning_system().await;
        let suggestion_engine = self.get_suggestion_engine().await;
        let automation = Arc::new(WorkflowAutomation::new(pattern_system, suggestion_engine));
        
        let _ = self.workflow_automation.set(Arc::clone(&automation));
        
        info!("WorkflowAutomation initialized lazily");
        automation
    }
    
    /// Get or initialize hot reload system
    pub async fn get_hot_reload_system(&self) -> Arc<HotReloadSystem> {
        if let Some(system) = self.hot_reload_system.get() {
            return Arc::clone(system);
        }
        
        let _guard = self.init_mutex.lock().await;
        
        // Double-check after acquiring lock
        if let Some(system) = self.hot_reload_system.get() {
            return Arc::clone(system);
        }
        
        debug!("Lazy initializing HotReloadSystem");
        let pattern_system = self.get_pattern_learning_system().await;
        let suggestion_engine = self.get_suggestion_engine().await;
        let workflow_automation = self.get_workflow_automation().await;
        
        let config = HotReloadConfig::default();
        let system = Arc::new(HotReloadSystem::new(
            config,
            pattern_system,
            suggestion_engine,
            workflow_automation,
        ));
        
        // Start the hot reload system using weak reference
        let system_weak = Arc::downgrade(&system);
        tokio::spawn(async move {
            if let Some(system_strong) = system_weak.upgrade() {
                if let Err(e) = system_strong.start().await {
                    tracing::error!("Failed to start hot reload system: {}", e);
                }
            }
        });
        
        let _ = self.hot_reload_system.set(Arc::clone(&system));
        
        info!("HotReloadSystem initialized lazily");
        system
    }
    
    /// Check if any components have been initialized
    pub fn is_any_initialized(&self) -> bool {
        self.entity_inspector.get().is_some() ||
        self.system_profiler.get().is_some() ||
        self.debug_command_router.get().is_some()
    }
    
    /// Get initialization status for debugging
    pub fn get_initialization_status(&self) -> serde_json::Value {
        serde_json::json!({
            "entity_inspector": self.entity_inspector.get().is_some(),
            "system_profiler": self.system_profiler.get().is_some(),
            "entity_processor": self.entity_processor.get().is_some(),
            "profiler_processor": self.profiler_processor.get().is_some(),
            "visual_overlay_processor": self.visual_overlay_processor.get().is_some(),
            "query_builder_processor": self.query_builder_processor.get().is_some(),
            "memory_profiler_processor": self.memory_profiler_processor.get().is_some(),
            "session_processor": self.session_processor.get().is_some(),
            "issue_detector_processor": self.issue_detector_processor.get().is_some(),
            "performance_budget_processor": self.performance_budget_processor.get().is_some(),
            "debug_command_router": self.debug_command_router.get().is_some(),
            "pattern_learning_system": self.pattern_learning_system.get().is_some(),
            "suggestion_engine": self.suggestion_engine.get().is_some(),
            "workflow_automation": self.workflow_automation.get().is_some(),
            "hot_reload_system": self.hot_reload_system.get().is_some(),
        })
    }
}

/// Preload specific components that will likely be needed soon
/// This allows for selective eager initialization of critical components
pub async fn preload_critical_components(_components: &LazyComponents) -> Result<()> {
    debug!("Preloading critical debug components");
    
    // Only preload if feature flags indicate they're needed
    #[cfg(feature = "entity-inspection")]
    {
        let _ = _components.get_entity_inspector().await;
        let _ = _components.get_entity_processor().await;
    }
    
    #[cfg(feature = "performance-profiling")]
    {
        let _ = _components.get_system_profiler().await;
        let _ = _components.get_profiler_processor().await;
    }
    
    #[cfg(feature = "session-management")]
    {
        let _ = _components.get_session_processor().await;
    }
    
    info!("Critical components preloaded based on enabled features");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    
    #[tokio::test]
    async fn test_lazy_initialization() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let components = LazyComponents::new(brp_client);
        
        // Initially nothing should be initialized
        assert!(!components.is_any_initialized());
        
        // Initialize entity inspector
        let _ = components.get_entity_inspector().await;
        assert!(components.entity_inspector.get().is_some());
        
        // Initialize system profiler
        let _ = components.get_system_profiler().await;
        assert!(components.system_profiler.get().is_some());
        
        // Check status
        assert!(components.is_any_initialized());
    }
    
    #[tokio::test]
    async fn test_double_initialization() {
        let config = Config::default();
        let brp_client = Arc::new(RwLock::new(BrpClient::new(&config)));
        let components = LazyComponents::new(brp_client);
        
        // Get inspector twice - should return same instance
        let inspector1 = components.get_entity_inspector().await;
        let inspector2 = components.get_entity_inspector().await;
        
        assert!(Arc::ptr_eq(&inspector1, &inspector2));
    }
}
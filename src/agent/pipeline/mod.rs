//! Agent pipeline modules for task processing and lifecycle management
//!
//! This module provides focused components for agent task processing,
//! separating pure business logic from I/O operations.

pub mod nine_step_executor;
pub mod pipeline_orchestrator;

// Re-export public types for convenience
pub use nine_step_executor::NineStepExecutor;
// TaskProcessor is internal implementation detail, not exported
pub use pipeline_orchestrator::AgentPipeline;

// Re-export error types
pub use pipeline_orchestrator::PipelineError;

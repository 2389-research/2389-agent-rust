//! RFC-compliant 2389 Agent Protocol - Observability System
//!
//! This module implements comprehensive monitoring with structured logging,
//! metrics collection, and health check endpoints per the observability specification.

pub mod health;
pub mod logging;
pub mod metrics;

// Re-export for convenience
pub use health::HealthServer;
pub use logging::{LogFormat, init_default_logging, init_logging};
pub use metrics::{MetricsCollector, MetricsSnapshot, metrics};

// Span macros for structured logging
pub use logging::{lifecycle_span, mqtt_span, task_span, tool_span};

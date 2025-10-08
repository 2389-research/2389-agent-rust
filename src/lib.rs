//! Agent 2389 - Rust Implementation
//!
//! A production-ready Rust implementation of the 2389 Agent Protocol for interoperable AI agents.
//!
//! # Overview
//!
//! This crate provides a complete implementation of the 2389 Agent Protocol, including:
//! - Protocol message types and validation
//! - MQTT transport layer with QoS handling
//! - Tool system with JSON schema validation  
//! - LLM provider integrations
//! - Complete agent lifecycle management
//!
//! # Quick Start
//!
//! ```rust
//! use agent2389::protocol::{TaskEnvelope, TaskEnvelopeV2, WorkflowContext, WorkflowStep};
//! use uuid::Uuid;
//! use serde_json::json;
//!
//! // Create a v1.0 task envelope (backward compatibility)
//! let task_v1 = TaskEnvelope {
//!     task_id: Uuid::new_v4(),
//!     conversation_id: "example".to_string(),
//!     topic: "/control/agents/my-agent/input".to_string(),
//!     instruction: Some("Process this data".to_string()),
//!     input: json!({"key": "value"}),
//!     next: None,
//! };
//!
//! // Create a v2.0 task envelope with workflow context
//! let task_v2 = TaskEnvelopeV2 {
//!     task_id: Uuid::new_v4(),
//!     conversation_id: "example".to_string(),
//!     topic: "/control/agents/my-agent/input".to_string(),
//!     instruction: Some("Process this data".to_string()),
//!     input: json!({"urgency_score": 0.9}),
//!     next: None,
//!     version: "2.0".to_string(),
//!     context: Some(WorkflowContext {
//!         original_query: "Process urgent request".to_string(),
//!         steps_completed: vec![
//!             WorkflowStep {
//!                 agent_id: "analyzer".to_string(),
//!                 action: "Analyzed urgency".to_string(),
//!                 timestamp: "2024-01-01T12:00:00Z".to_string(),
//!             }
//!         ],
//!         iteration_count: 1,
//!     }),
//!     routing_trace: None,
//! };
//!
//! // Both serialize to JSON for MQTT transport
//! let v1_json = serde_json::to_string(&task_v1).unwrap();
//! let v2_json = serde_json::to_string(&task_v2).unwrap();
//! ```

pub mod agent;
pub mod config;
pub mod error;
pub mod health;
pub mod llm;
pub mod observability;
pub mod processing;
pub mod progress;
pub mod protocol;
pub mod routing;
pub mod testing;
pub mod tools;
pub mod transport;

// Re-export RFC-compliant types only
pub use agent::AgentLifecycle;
pub use config::*;
pub use error::{AgentError, AgentResult};
pub use progress::{
    MqttProgressReporter, Progress, ProgressCategory, ProgressEventType, ProgressMessage,
};
pub use protocol::*;
pub use tools::{Tool, ToolDescription, ToolError, ToolSystem};
pub use transport::mqtt::MqttClient;

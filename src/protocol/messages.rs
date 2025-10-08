//! Protocol message types for 2389 Agent Protocol
//!
//! This module defines all message structures used for agent communication,
//! including task envelopes, agent status, and error messages.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Task envelope containing all task information
///
/// This is the primary message type for agent communication.
/// See protocol section 6.1 for full specification.
///
/// # Examples
/// ```
/// use agent2389::protocol::TaskEnvelope;
/// use uuid::Uuid;
/// use serde_json::json;
///
/// let task = TaskEnvelope {
///     task_id: Uuid::new_v4(),
///     conversation_id: "test-conversation".to_string(),
///     topic: "/control/agents/my-agent/input".to_string(),
///     instruction: Some("Process this data".to_string()),
///     input: json!({"key": "value"}),
///     next: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskEnvelope {
    /// UUID v4 task identifier for idempotency
    pub task_id: Uuid,
    /// Conversation identifier for error routing  
    pub conversation_id: String,
    /// MQTT topic (must be canonicalized)
    pub topic: String,
    /// Instruction for this agent (optional)
    pub instruction: Option<String>,
    /// Input data - SHOULD be object for structured data
    pub input: Value,
    /// Next agent in pipeline (optional)
    pub next: Option<Box<NextTask>>,
}

/// TaskEnvelope v2.0 with workflow context and simplified routing
///
/// Extends the original TaskEnvelope with workflow context for multi-agent coordination.
///
/// # Examples
/// ```
/// use agent2389::protocol::{TaskEnvelopeV2, WorkflowContext, WorkflowStep};
/// use uuid::Uuid;
/// use serde_json::json;
///
/// let task = TaskEnvelopeV2 {
///     task_id: Uuid::new_v4(),
///     conversation_id: "test-conversation".to_string(),
///     topic: "/control/agents/my-agent/input".to_string(),
///     instruction: Some("Process this data".to_string()),
///     input: json!({"key": "value"}),
///     next: None,
///     version: "2.0".to_string(),
///     context: Some(WorkflowContext {
///         original_query: "User's original request".to_string(),
///         steps_completed: vec![
///             WorkflowStep {
///                 agent_id: "analyzer".to_string(),
///                 action: "Analyzed requirements".to_string(),
///                 timestamp: "2024-01-01T12:00:00Z".to_string(),
///             }
///         ],
///         iteration_count: 1,
///     }),
///     routing_trace: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskEnvelopeV2 {
    /// UUID v4 task identifier for idempotency
    pub task_id: Uuid,
    /// Conversation identifier for error routing
    pub conversation_id: String,
    /// MQTT topic (must be canonicalized)
    pub topic: String,
    /// Instruction for this agent (optional)
    pub instruction: Option<String>,
    /// Input data - SHOULD be object for structured data
    pub input: Value,
    /// Next agent in pipeline (optional)
    pub next: Option<Box<NextTask>>,
    /// Protocol version - "2.0" for this envelope type
    pub version: String,
    /// Workflow context for multi-agent coordination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<WorkflowContext>,
    /// Trace of routing decisions for debugging and observability
    pub routing_trace: Option<Vec<RoutingStep>>,
}

/// Context accumulated across multi-agent workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowContext {
    /// Original user query preserved from first agent
    pub original_query: String,
    /// Steps completed so far
    pub steps_completed: Vec<WorkflowStep>,
    /// Current iteration count (safety counter to prevent infinite loops)
    #[serde(default)]
    pub iteration_count: usize,
}

/// Single step in workflow history
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkflowStep {
    pub agent_id: String,
    pub action: String,
    pub timestamp: String,
}

/// Single step in routing trace for observability
///
/// Records routing decisions made during task processing for debugging and monitoring.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingStep {
    /// Agent that made the routing decision
    pub from_agent: String,
    /// Agent selected as routing target
    pub to_agent: String,
    /// Human-readable reason for routing decision
    pub reason: String,
    /// ISO 8601 timestamp of routing decision
    pub timestamp: String,
    /// Sequential step number within the trace
    pub step_number: u32,
}

/// Wrapper enum for version-aware TaskEnvelope deserialization
///
/// Automatically detects v1.0 vs v2.0 envelopes based on presence of version field.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum TaskEnvelopeWrapper {
    V2(TaskEnvelopeV2),
    V1(TaskEnvelope),
}

impl TaskEnvelopeWrapper {
    /// Get the task_id regardless of envelope version
    pub fn task_id(&self) -> Uuid {
        match self {
            TaskEnvelopeWrapper::V1(envelope) => envelope.task_id,
            TaskEnvelopeWrapper::V2(envelope) => envelope.task_id,
        }
    }

    /// Get the conversation_id regardless of envelope version
    pub fn conversation_id(&self) -> &str {
        match self {
            TaskEnvelopeWrapper::V1(envelope) => &envelope.conversation_id,
            TaskEnvelopeWrapper::V2(envelope) => &envelope.conversation_id,
        }
    }

    /// Get the topic regardless of envelope version
    pub fn topic(&self) -> &str {
        match self {
            TaskEnvelopeWrapper::V1(envelope) => &envelope.topic,
            TaskEnvelopeWrapper::V2(envelope) => &envelope.topic,
        }
    }

    /// Check if this is a v2.0 envelope
    pub fn is_v2(&self) -> bool {
        matches!(self, TaskEnvelopeWrapper::V2(_))
    }

    /// Convert v1.0 envelope to v2.0
    pub fn to_v2(self) -> TaskEnvelopeV2 {
        match self {
            TaskEnvelopeWrapper::V2(envelope) => envelope,
            TaskEnvelopeWrapper::V1(envelope) => TaskEnvelopeV2 {
                task_id: envelope.task_id,
                conversation_id: envelope.conversation_id,
                topic: envelope.topic,
                instruction: envelope.instruction,
                input: envelope.input,
                next: envelope.next,
                version: "2.0".to_string(),
                context: None,
                routing_trace: None,
            },
        }
    }

    /// Convert to v1.0 envelope (loses v2.0-specific fields)
    pub fn to_v1(self) -> TaskEnvelope {
        match self {
            TaskEnvelopeWrapper::V1(envelope) => envelope,
            TaskEnvelopeWrapper::V2(envelope) => TaskEnvelope {
                task_id: envelope.task_id,
                conversation_id: envelope.conversation_id,
                topic: envelope.topic,
                instruction: envelope.instruction,
                input: envelope.input,
                next: envelope.next,
            },
        }
    }
}

/// Next task in pipeline chain
///
/// Represents the continuation of a task pipeline to another agent.
///
/// # Examples
/// ```
/// use agent2389::protocol::NextTask;
/// use serde_json::json;
///
/// let next = NextTask {
///     topic: "/control/agents/next-agent/input".to_string(),
///     instruction: Some("Continue processing".to_string()),
///     input: Some(json!({"processed": true})),
///     next: None,
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NextTask {
    /// Next agent topic or final destination
    pub topic: String,
    /// Instruction for next agent
    pub instruction: Option<String>,
    /// Input will be set to previous agent's output
    pub input: Option<Value>,
    /// Continuation of pipeline
    pub next: Option<Box<NextTask>>,
}

/// Agent status message (retained)
///
/// Published to `/control/agents/{agent_id}/status` with retain flag.
/// RFC Section 6.2 - EXACT specification compliance.
///
/// # Examples
/// ```
/// use agent2389::protocol::{AgentStatus, AgentStatusType};
/// use chrono::Utc;
///
/// let status = AgentStatus {
///     agent_id: "my-agent".to_string(),
///     status: AgentStatusType::Available,
///     timestamp: Utc::now(),
///     capabilities: Some(vec!["research".to_string(), "writing".to_string()]),
///     description: Some("AI research and writing agent".to_string()),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub agent_id: String,
    pub status: AgentStatusType,
    /// RFC 3339 format with Z suffix
    pub timestamp: DateTime<Utc>,
    /// Agent capabilities (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    /// Agent description (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Agent status enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatusType {
    Available,
    Unavailable,
}

/// Error message format
///
/// Published to conversation topics when errors occur during processing.
///
/// # Examples
/// ```
/// use agent2389::protocol::{ErrorMessage, ErrorDetails, ErrorCode};
/// use uuid::Uuid;
///
/// let error = ErrorMessage {
///     error: ErrorDetails {
///         code: ErrorCode::ToolExecutionFailed,
///         message: "HTTP request timeout".to_string(),
///     },
///     task_id: Uuid::new_v4(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub error: ErrorDetails,
    pub task_id: Uuid,
}

/// Agent response message format
///
/// Published to conversation topics when tasks complete successfully.
///
/// # Examples
/// ```
/// use agent2389::protocol::ResponseMessage;
/// use uuid::Uuid;
/// use serde_json::json;
///
/// let response = ResponseMessage {
///     response: "Hello! I processed your request successfully.".to_string(),
///     task_id: Uuid::new_v4(),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    pub response: String,
    pub task_id: Uuid,
}

/// Error details structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetails {
    pub code: ErrorCode,
    /// Human-readable description (no sensitive data)
    pub message: String,
}

/// Protocol error codes
///
/// Maps to specific error conditions in the 2389 Agent Protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    ToolExecutionFailed,
    LlmError,
    InvalidInput,
    PipelineDepthExceeded,
    InternalError,
}

#[cfg(test)]
mod v2_tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_task_envelope_v2_serialization() {
        let task_id = Uuid::new_v4();
        let task = TaskEnvelopeV2 {
            task_id,
            conversation_id: "test-conversation".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: Some("Process this test".to_string()),
            input: json!({"test": "data"}),
            next: None,
            version: "2.0".to_string(),
            context: Some(WorkflowContext {
                original_query: "Test query".to_string(),
                steps_completed: vec![WorkflowStep {
                    agent_id: "agent1".to_string(),
                    action: "Analyzed request".to_string(),
                    timestamp: "2024-01-01T12:00:00Z".to_string(),
                }],
                iteration_count: 1,
            }),
            routing_trace: None,
        };

        // Should serialize and deserialize correctly
        let json = serde_json::to_string(&task).unwrap();
        let parsed: TaskEnvelopeV2 = serde_json::from_str(&json).unwrap();

        assert_eq!(task, parsed);
        assert_eq!(parsed.version, "2.0");
        assert!(parsed.context.is_some());

        let context = parsed.context.unwrap();
        assert_eq!(context.original_query, "Test query");
        assert_eq!(context.steps_completed.len(), 1);
        assert_eq!(context.steps_completed[0].agent_id, "agent1");
    }

    #[test]
    fn test_task_envelope_v2_with_trace() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conversation".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: Some("Process with trace".to_string()),
            input: json!({"test": "data"}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: Some(vec![
                RoutingStep {
                    from_agent: "agent1".to_string(),
                    to_agent: "agent2".to_string(),
                    reason: "Matched urgency rule".to_string(),
                    timestamp: "2024-01-01T12:00:00Z".to_string(),
                    step_number: 1,
                },
                RoutingStep {
                    from_agent: "agent2".to_string(),
                    to_agent: "agent3".to_string(),
                    reason: "Load balancing".to_string(),
                    timestamp: "2024-01-01T12:00:01Z".to_string(),
                    step_number: 2,
                },
            ]),
        };

        let json = serde_json::to_string(&task).unwrap();
        let parsed: TaskEnvelopeV2 = serde_json::from_str(&json).unwrap();

        assert_eq!(task, parsed);
        assert!(parsed.routing_trace.is_some());

        let trace = parsed.routing_trace.unwrap();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].from_agent, "agent1");
        assert_eq!(trace[0].to_agent, "agent2");
        assert_eq!(trace[0].reason, "Matched urgency rule");
        assert_eq!(trace[1].step_number, 2);
    }

    #[test]
    fn test_task_envelope_wrapper_v1_detection() {
        let v1_json = r#"{
            "task_id": "550e8400-e29b-41d4-a716-446655440000",
            "conversation_id": "test-conv",
            "topic": "/control/agents/test/input",
            "instruction": "test",
            "input": {"key": "value"},
            "next": null
        }"#;

        let wrapper: TaskEnvelopeWrapper = serde_json::from_str(v1_json).unwrap();

        assert!(!wrapper.is_v2());
        assert_eq!(
            wrapper.task_id().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(wrapper.conversation_id(), "test-conv");
        assert_eq!(wrapper.topic(), "/control/agents/test/input");

        // Should be able to convert to v1
        let v1_envelope = wrapper.to_v1();
        assert_eq!(v1_envelope.conversation_id, "test-conv");
    }

    #[test]
    fn test_task_envelope_wrapper_v2_detection() {
        let v2_json = r#"{
            "task_id": "550e8400-e29b-41d4-a716-446655440000",
            "conversation_id": "test-conv",
            "topic": "/control/agents/test/input",
            "instruction": "test",
            "input": {"key": "value"},
            "next": null,
            "version": "2.0",
            "context": null,
            "routing_trace": null
        }"#;

        let wrapper: TaskEnvelopeWrapper = serde_json::from_str(v2_json).unwrap();

        assert!(wrapper.is_v2());
        assert_eq!(
            wrapper.task_id().to_string(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
        assert_eq!(wrapper.conversation_id(), "test-conv");
        assert_eq!(wrapper.topic(), "/control/agents/test/input");

        // Should be able to convert to v2
        let v2_envelope = wrapper.to_v2();
        assert_eq!(v2_envelope.version, "2.0");
        assert!(v2_envelope.context.is_none());
    }

    #[test]
    fn test_v1_to_v2_conversion() {
        let v1_envelope = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/control/agents/test/input".to_string(),
            instruction: Some("test instruction".to_string()),
            input: json!({"key": "value"}),
            next: None,
        };

        let wrapper = TaskEnvelopeWrapper::V1(v1_envelope.clone());
        let v2_envelope = wrapper.to_v2();

        // Should preserve all v1 fields
        assert_eq!(v2_envelope.task_id, v1_envelope.task_id);
        assert_eq!(v2_envelope.conversation_id, v1_envelope.conversation_id);
        assert_eq!(v2_envelope.topic, v1_envelope.topic);
        assert_eq!(v2_envelope.instruction, v1_envelope.instruction);
        assert_eq!(v2_envelope.input, v1_envelope.input);
        assert_eq!(v2_envelope.next, v1_envelope.next);

        // Should add v2 fields with defaults
        assert_eq!(v2_envelope.version, "2.0");
        assert!(v2_envelope.context.is_none());
        assert!(v2_envelope.routing_trace.is_none());
    }

    #[test]
    fn test_v2_to_v1_conversion() {
        let v2_envelope = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/control/agents/test/input".to_string(),
            instruction: Some("test instruction".to_string()),
            input: json!({"key": "value"}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: Some(vec![]),
        };

        let wrapper = TaskEnvelopeWrapper::V2(v2_envelope.clone());
        let v1_envelope = wrapper.to_v1();

        // Should preserve common fields
        assert_eq!(v1_envelope.task_id, v2_envelope.task_id);
        assert_eq!(v1_envelope.conversation_id, v2_envelope.conversation_id);
        assert_eq!(v1_envelope.topic, v2_envelope.topic);
        assert_eq!(v1_envelope.instruction, v2_envelope.instruction);
        assert_eq!(v1_envelope.input, v2_envelope.input);
        assert_eq!(v1_envelope.next, v2_envelope.next);

        // v2-specific fields are lost (expected)
    }

    #[test]
    fn test_envelope_wrapper_serialization_roundtrip() {
        // Test that wrapper can serialize/deserialize both versions
        let v1_wrapper = TaskEnvelopeWrapper::V1(TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
        });

        let v1_json = serde_json::to_string(&v1_wrapper).unwrap();
        let v1_parsed: TaskEnvelopeWrapper = serde_json::from_str(&v1_json).unwrap();
        assert!(!v1_parsed.is_v2());

        let v2_wrapper = TaskEnvelopeWrapper::V2(TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        });

        let v2_json = serde_json::to_string(&v2_wrapper).unwrap();
        let v2_parsed: TaskEnvelopeWrapper = serde_json::from_str(&v2_json).unwrap();
        assert!(v2_parsed.is_v2());
    }

    #[test]
    fn test_minimal_v2_envelope() {
        // Test minimal v2 envelope with only required fields
        let minimal = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: None,
            routing_trace: None,
        };

        let json = serde_json::to_string(&minimal).unwrap();
        let parsed: TaskEnvelopeV2 = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.version, "2.0");
        assert!(parsed.context.is_none());
        assert!(parsed.routing_trace.is_none());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_task_envelope_serialization() {
        let task_id = Uuid::new_v4();
        let task = TaskEnvelope {
            task_id,
            conversation_id: "test-conversation".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: Some("Process this test".to_string()),
            input: json!({"test": "data"}),
            next: None,
        };

        // Should serialize and deserialize correctly
        let json = serde_json::to_string(&task).unwrap();
        let parsed: TaskEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(task, parsed);
        assert_eq!(parsed.task_id, task_id);
        assert_eq!(parsed.conversation_id, "test-conversation");
        assert_eq!(parsed.topic, "/control/agents/test-agent/input");
    }

    #[test]
    fn test_task_envelope_with_next() {
        let next_task = NextTask {
            topic: "/control/agents/next-agent/input".to_string(),
            instruction: Some("Continue processing".to_string()),
            input: Some(json!({"continued": true})),
            next: None,
        };

        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conversation".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: Some("Start processing".to_string()),
            input: json!({"start": "data"}),
            next: Some(Box::new(next_task)),
        };

        // Should handle nested structure
        let json = serde_json::to_string(&task).unwrap();
        let parsed: TaskEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(task, parsed);
        assert!(parsed.next.is_some());

        let next = parsed.next.unwrap();
        assert_eq!(next.topic, "/control/agents/next-agent/input");
        assert_eq!(next.instruction, Some("Continue processing".to_string()));
    }

    #[test]
    fn test_deeply_nested_pipeline() {
        // Create a pipeline with multiple nested tasks
        let deep_next = NextTask {
            topic: "/control/agents/final-agent/input".to_string(),
            instruction: Some("Final step".to_string()),
            input: None,
            next: None,
        };

        let middle_next = NextTask {
            topic: "/control/agents/middle-agent/input".to_string(),
            instruction: Some("Middle step".to_string()),
            input: None,
            next: Some(Box::new(deep_next)),
        };

        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conversation".to_string(),
            topic: "/control/agents/first-agent/input".to_string(),
            instruction: Some("First step".to_string()),
            input: json!({"pipeline": "test"}),
            next: Some(Box::new(middle_next)),
        };

        // Should handle deep nesting
        let json = serde_json::to_string(&task).unwrap();
        let parsed: TaskEnvelope = serde_json::from_str(&json).unwrap();

        assert_eq!(task, parsed);

        // Verify deep structure
        let level1 = parsed.next.unwrap();
        assert_eq!(level1.topic, "/control/agents/middle-agent/input");

        let level2 = level1.next.unwrap();
        assert_eq!(level2.topic, "/control/agents/final-agent/input");
        assert!(level2.next.is_none());
    }

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus {
            agent_id: "test-agent".to_string(),
            status: AgentStatusType::Available,
            timestamp: DateTime::from_timestamp(1609459200, 0).unwrap(), // Fixed timestamp for testing
            capabilities: None,
            description: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        let parsed: AgentStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.agent_id, "test-agent");
        assert_eq!(parsed.status, AgentStatusType::Available);

        // Verify JSON format includes lowercase status
        assert!(json.contains("\"available\""));
    }

    #[test]
    fn test_agent_status_unavailable() {
        let status = AgentStatus {
            agent_id: "test-agent".to_string(),
            status: AgentStatusType::Unavailable,
            timestamp: DateTime::from_timestamp(1609459200, 0).unwrap(),
            capabilities: None,
            description: None,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"unavailable\""));

        let parsed: AgentStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.status, AgentStatusType::Unavailable);
    }

    #[test]
    fn test_error_message_serialization() {
        let error = ErrorMessage {
            error: ErrorDetails {
                code: ErrorCode::ToolExecutionFailed,
                message: "HTTP request failed".to_string(),
            },
            task_id: Uuid::new_v4(),
        };

        let json = serde_json::to_string(&error).unwrap();
        let parsed: ErrorMessage = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.error.code, ErrorCode::ToolExecutionFailed);
        assert_eq!(parsed.error.message, "HTTP request failed");

        // Verify snake_case serialization
        assert!(json.contains("\"tool_execution_failed\""));
    }

    #[test]
    fn test_all_error_codes() {
        let error_codes = vec![
            ErrorCode::ToolExecutionFailed,
            ErrorCode::LlmError,
            ErrorCode::InvalidInput,
            ErrorCode::PipelineDepthExceeded,
            ErrorCode::InternalError,
        ];

        for code in error_codes {
            let error = ErrorMessage {
                error: ErrorDetails {
                    code: code.clone(),
                    message: "Test error".to_string(),
                },
                task_id: Uuid::new_v4(),
            };

            // Should serialize and deserialize correctly
            let json = serde_json::to_string(&error).unwrap();
            let parsed: ErrorMessage = serde_json::from_str(&json).unwrap();

            assert_eq!(parsed.error.code, code);
        }
    }

    #[test]
    fn test_protocol_compliance_json_format() {
        // Test exact JSON structure matches protocol specification
        let task = TaskEnvelope {
            task_id: Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            conversation_id: "conv-123".to_string(),
            topic: "/control/agents/test/input".to_string(),
            instruction: Some("test instruction".to_string()),
            input: json!({"key": "value"}),
            next: None,
        };

        let json = serde_json::to_string_pretty(&task).unwrap();

        // Verify required fields are present
        assert!(json.contains("\"task_id\""));
        assert!(json.contains("\"conversation_id\""));
        assert!(json.contains("\"topic\""));
        assert!(json.contains("\"instruction\""));
        assert!(json.contains("\"input\""));
        assert!(json.contains("\"next\""));
    }
}

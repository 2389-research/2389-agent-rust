//! Nine-step task processing algorithm implementation
//!
//! This module contains pure functions for the RFC-compliant 9-step task processing
//! algorithm with clear separation of business logic from I/O operations.

use crate::protocol::messages::{
    AgentStatus, AgentStatusType, ErrorMessage, NextTask, TaskEnvelope,
};
use crate::protocol::topics::canonicalize_topic;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;
use uuid::Uuid;

/// Pure nine-step algorithm execution logic
pub struct NineStepExecutor;

impl NineStepExecutor {
    /// Validate task topic against expected topic (Step 3)
    pub fn validate_task_topic(task_topic: &str, expected_agent_id: &str) -> Result<(), String> {
        let expected_topic = format!("/control/agents/{expected_agent_id}/input");
        let canonical_topic = canonicalize_topic(&expected_topic);
        let task_canonical_topic = canonicalize_topic(task_topic);

        if canonical_topic != task_canonical_topic {
            return Err(format!(
                "Topic mismatch: expected '{canonical_topic}', got '{task_canonical_topic}'"
            ));
        }

        Ok(())
    }

    /// Check if task ID is duplicate (Step 4)
    pub async fn check_task_idempotency(
        task_id: Uuid,
        processed_task_ids: &Arc<RwLock<HashSet<Uuid>>>,
    ) -> bool {
        let task_ids = processed_task_ids.read().await;
        task_ids.contains(&task_id)
    }

    /// Calculate pipeline depth (Step 5)
    pub fn calculate_pipeline_depth(task: &TaskEnvelope) -> usize {
        let mut depth = 0;

        // Count nested next tasks
        let mut current_next = task.next.as_ref();
        while let Some(next) = current_next {
            depth += 1;
            current_next = next.next.as_ref();
        }

        depth
    }

    /// Check if pipeline depth exceeds maximum (Step 5)
    pub fn validate_pipeline_depth(depth: usize, max_depth: usize) -> Result<(), usize> {
        if depth > max_depth {
            Err(depth)
        } else {
            Ok(())
        }
    }

    /// Create pipeline depth exceeded error message
    pub fn create_pipeline_depth_error(
        task_id: Uuid,
        depth: usize,
        max_depth: usize,
    ) -> ErrorMessage {
        ErrorMessage {
            error: crate::protocol::messages::ErrorDetails {
                code: crate::protocol::messages::ErrorCode::PipelineDepthExceeded,
                message: format!("Pipeline depth {depth} exceeds maximum {max_depth}"),
            },
            task_id,
        }
    }

    /// Create response message for conversation publishing
    pub fn create_response_message(
        task_id: Uuid,
        response: &str,
    ) -> crate::protocol::ResponseMessage {
        crate::protocol::ResponseMessage {
            response: response.to_string(),
            task_id,
        }
    }

    /// Extract target agent ID from topic (Step 8 helper)
    pub fn extract_target_agent_from_topic(topic: &str) -> Option<&str> {
        topic
            .strip_prefix("/control/agents/")
            .and_then(|s| s.strip_suffix("/input"))
    }

    /// Create next task envelope for pipeline forwarding (Step 8)
    pub fn create_next_task_envelope(
        original_task: &TaskEnvelope,
        next_task: &NextTask,
        processing_result: &str,
    ) -> TaskEnvelope {
        TaskEnvelope {
            task_id: uuid::Uuid::new_v4(), // New task ID for next agent
            conversation_id: original_task.conversation_id.clone(),
            topic: next_task.topic.clone(),
            instruction: next_task.instruction.clone(),
            input: serde_json::to_value(processing_result)
                .unwrap_or_else(|_| serde_json::Value::String(processing_result.to_string())),
            next: next_task.next.as_ref().map(|nested| {
                Box::new(NextTask {
                    topic: nested.topic.clone(),
                    instruction: nested.instruction.clone(),
                    input: nested.input.clone(),
                    next: nested.next.clone(),
                })
            }),
        }
    }

    /// Update processed task IDs with memory management (Step 9)
    pub async fn mark_task_completed(
        task_id: Uuid,
        processed_task_ids: &Arc<RwLock<HashSet<Uuid>>>,
        max_memory_tasks: usize,
    ) {
        let mut task_ids = processed_task_ids.write().await;
        task_ids.insert(task_id);

        // Prevent unbounded memory growth - keep last max_memory_tasks task IDs
        if task_ids.len() > max_memory_tasks {
            let oldest_ids: Vec<_> = task_ids
                .iter()
                .take(task_ids.len() - max_memory_tasks)
                .cloned()
                .collect();
            for id in oldest_ids {
                task_ids.remove(&id);
            }
        }
    }

    /// Create agent status message
    pub fn create_agent_status(agent_id: &str, status_type: AgentStatusType) -> AgentStatus {
        AgentStatus {
            agent_id: agent_id.to_string(),
            status: status_type,
            timestamp: chrono::Utc::now(),
            capabilities: None,
            description: None,
        }
    }

    /// Check if task is final (no next tasks)
    pub fn is_final_task(task: &TaskEnvelope) -> bool {
        task.next.is_none()
    }

    /// Log nine-step progress
    pub fn log_step_progress(step: u8, description: &str) {
        debug!("Step {}: {}", step, description);
    }

    /// Update health server timestamp
    pub async fn update_health_timestamp(
        health_server: Option<&Arc<crate::observability::health::HealthServer>>,
    ) {
        if let Some(health_server) = health_server {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            health_server.set_last_task_processed(now).await;
        }
    }
}

// TaskProcessor implementation is temporarily disabled while refactoring
// The pipeline orchestrator now calls AgentProcessor directly
// TODO: Consider whether TaskProcessor is needed or if AgentProcessor is sufficient

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_validate_task_topic() {
        // Valid topic should pass
        let result =
            NineStepExecutor::validate_task_topic("/control/agents/test-agent/input", "test-agent");
        assert!(result.is_ok());

        // Invalid topic should fail
        let result = NineStepExecutor::validate_task_topic("/wrong/topic", "test-agent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Topic mismatch"));
    }

    #[test]
    fn test_calculate_pipeline_depth() {
        // Test simple depth (no nested tasks)
        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: Some("Test".to_string()),
            input: serde_json::json!("Test"),
            next: None,
        };

        assert_eq!(NineStepExecutor::calculate_pipeline_depth(&task), 0);

        // Test nested pipeline depth
        let next_task = Box::new(NextTask {
            topic: "/control/agents/agent2/input".to_string(),
            instruction: Some("Continue".to_string()),
            input: None,
            next: Some(Box::new(NextTask {
                topic: "/control/agents/agent3/input".to_string(),
                instruction: Some("Final".to_string()),
                input: None,
                next: None,
            })),
        });

        let nested_task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: Some("Test".to_string()),
            input: serde_json::json!("Test"),
            next: Some(next_task),
        };

        // Should be 2 nested next tasks
        assert_eq!(NineStepExecutor::calculate_pipeline_depth(&nested_task), 2);
    }

    #[test]
    fn test_validate_pipeline_depth() {
        // Should pass when depth is within limit
        assert!(NineStepExecutor::validate_pipeline_depth(5, 10).is_ok());

        // Should fail when depth exceeds limit
        let result = NineStepExecutor::validate_pipeline_depth(15, 10);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), 15);
    }

    #[test]
    fn test_extract_target_agent_from_topic() {
        // Valid agent topic
        let result =
            NineStepExecutor::extract_target_agent_from_topic("/control/agents/test-agent/input");
        assert_eq!(result, Some("test-agent"));

        // Invalid topic
        let result = NineStepExecutor::extract_target_agent_from_topic("/wrong/topic");
        assert_eq!(result, None);
    }

    #[test]
    fn test_is_final_task() {
        // Task with no next should be final
        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: None,
            input: serde_json::Value::Null,
            next: None,
        };
        assert!(NineStepExecutor::is_final_task(&task));

        // Task with next should not be final
        let task_with_next = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction: None,
            input: serde_json::Value::Null,
            next: Some(Box::new(NextTask {
                topic: "/control/agents/next/input".to_string(),
                instruction: None,
                input: None,
                next: None,
            })),
        };
        assert!(!NineStepExecutor::is_final_task(&task_with_next));
    }

    #[test]
    fn test_create_pipeline_depth_error() {
        let task_id = Uuid::new_v4();
        let error = NineStepExecutor::create_pipeline_depth_error(task_id, 10, 5);

        assert_eq!(error.task_id, task_id);
        assert!(
            error
                .error
                .message
                .contains("Pipeline depth 10 exceeds maximum 5")
        );
    }

    #[test]
    fn test_create_response_message() {
        let task_id = Uuid::new_v4();
        let response = NineStepExecutor::create_response_message(task_id, "test response");

        assert_eq!(response.task_id, task_id);
        assert_eq!(response.response, "test response");
    }

    #[test]
    fn test_create_agent_status() {
        let status =
            NineStepExecutor::create_agent_status("test-agent", AgentStatusType::Available);

        assert_eq!(status.agent_id, "test-agent");
        assert_eq!(status.status, AgentStatusType::Available);
        // timestamp is set automatically
    }
}

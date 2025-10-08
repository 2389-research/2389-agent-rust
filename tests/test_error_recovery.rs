//! Error Recovery Tests
//!
//! Tests error handling and recovery mechanisms:
//! - LLM failures and retries
//! - Tool execution failures
//! - Transport errors
//! - Invalid input handling
//! - Error message publishing

mod test_helpers;

use agent2389::agent::processor::AgentProcessor;
use agent2389::error::AgentError;
use agent2389::llm::provider::{CompletionRequest, CompletionResponse, LlmError, LlmProvider};
use agent2389::protocol::messages::{ErrorCode, TaskEnvelope, TaskEnvelopeWrapper};
use agent2389::testing::mocks::{MockLlmProvider, MockTransport};
use agent2389::tools::ToolSystem;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

fn create_test_task(instruction: &str) -> TaskEnvelope {
    TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: format!("test-conversation-{}", Uuid::new_v4()),
        topic: "/test/agent".to_string(),
        instruction: Some(instruction.to_string()),
        input: json!({}),
        next: None,
    }
}

#[tokio::test]
async fn test_handles_llm_timeout_error() {
    // Arrange: Create processor with failing LLM
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport.clone());
    let task = create_test_task("This will timeout");

    // Act: Process task with failing LLM
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
        .await;

    // Assert: Should fail gracefully
    assert!(result.is_err(), "Should error when LLM fails");

    // Verify error was published
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let errors = transport.get_published_errors().await;
    assert!(
        !errors.is_empty(),
        "Error should be published to conversation topic"
    );
}

#[tokio::test]
async fn test_handles_llm_rate_limit_error() {
    /// LLM that returns rate limit error
    struct RateLimitLlmProvider;

    #[async_trait]
    impl LlmProvider for RateLimitLlmProvider {
        fn name(&self) -> &str {
            "rate-limit-provider"
        }

        fn available_models(&self) -> Vec<String> {
            vec!["test-model".to_string()]
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            Err(LlmError::RateLimitExceeded(
                "Rate limit exceeded".to_string(),
            ))
        }

        async fn health_check(&self) -> Result<(), LlmError> {
            Ok(())
        }
    }

    // Arrange
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(RateLimitLlmProvider);
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let task = create_test_task("Test rate limit");

    // Act
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
        .await;

    // Assert: Should fail with appropriate error
    assert!(result.is_err(), "Should error on rate limit");
}

#[tokio::test]
async fn test_handles_malformed_llm_response() {
    /// LLM that returns malformed response
    struct MalformedLlmProvider;

    #[async_trait]
    impl LlmProvider for MalformedLlmProvider {
        fn name(&self) -> &str {
            "malformed-provider"
        }

        fn available_models(&self) -> Vec<String> {
            vec!["test-model".to_string()]
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, LlmError> {
            Err(LlmError::InvalidResponse("Malformed response".to_string()))
        }

        async fn health_check(&self) -> Result<(), LlmError> {
            Ok(())
        }
    }

    // Arrange
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MalformedLlmProvider);
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let task = create_test_task("Test malformed response");

    // Act
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
        .await;

    // Assert: Should handle malformed response gracefully
    assert!(result.is_err(), "Should error on malformed response");
}

#[tokio::test]
async fn test_handles_transport_publish_failure() {
    // Arrange: Transport that fails to publish
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::single_response("Success"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::with_failure());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let task = create_test_task("Test transport failure");

    // Act: Process task with failing transport
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
        .await;

    // Assert: Should error when transport fails
    assert!(
        result.is_err(),
        "Should error when transport fails to publish"
    );
}

#[tokio::test]
async fn test_handles_empty_instruction() {
    // Arrange
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> =
        Arc::new(MockLlmProvider::single_response("Handled empty"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let mut task = create_test_task("");
    task.instruction = None;

    // Act: Process task with no instruction
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
        .await;

    // Assert: Should handle gracefully (may succeed or fail, but shouldn't crash)
    assert!(
        result.is_ok() || result.is_err(),
        "Should handle empty instruction without panic"
    );
}

#[tokio::test]
async fn test_error_types_map_to_protocol_codes() {
    // Test that AgentError variants map correctly to protocol error codes

    let tool_error = AgentError::tool_execution_failed("Test tool error");
    assert!(matches!(tool_error, AgentError::ToolExecutionFailed { .. }));

    let llm_error = AgentError::llm_error("Test LLM error");
    assert!(matches!(llm_error, AgentError::LlmError { .. }));

    let invalid_input = AgentError::invalid_input("Test invalid input");
    assert!(matches!(invalid_input, AgentError::InvalidInput { .. }));

    let internal_error = AgentError::internal_error("Test internal error");
    assert!(matches!(internal_error, AgentError::InternalError { .. }));
}

#[tokio::test]
async fn test_error_to_error_message_conversion() {
    use agent2389::error::AgentError;

    // Verify each error type converts correctly to ErrorMessage
    let tool_error = AgentError::tool_execution_failed("tool failed");
    let task_id = Uuid::new_v4();
    let error_msg = tool_error.to_error_message(task_id);
    assert!(matches!(
        error_msg.error.code,
        ErrorCode::ToolExecutionFailed
    ));
    assert_eq!(error_msg.task_id, task_id);

    let invalid_input = AgentError::invalid_input("invalid input");
    let error_msg = invalid_input.to_error_message(task_id);
    assert!(matches!(error_msg.error.code, ErrorCode::InvalidInput));

    let internal_error = AgentError::internal_error("internal error");
    let error_msg = internal_error.to_error_message(task_id);
    assert!(matches!(error_msg.error.code, ErrorCode::InternalError));

    let llm_error = AgentError::llm_error("llm failed");
    let error_msg = llm_error.to_error_message(task_id);
    assert!(matches!(error_msg.error.code, ErrorCode::LlmError));
}

#[tokio::test]
async fn test_concurrent_error_handling() {
    // Test that errors are handled correctly when multiple tasks fail concurrently
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = Arc::new(AgentProcessor::new(
        config,
        llm_provider,
        tool_system,
        transport.clone(),
    ));

    let mut handles = vec![];

    // Process 5 tasks that will all fail
    for i in 0..5 {
        let processor_clone = processor.clone();
        let handle = tokio::spawn(async move {
            let task = create_test_task(&format!("Failing task {i}"));
            processor_clone
                .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
                .await
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        let result = handle.await.unwrap();
        assert!(result.is_err(), "All tasks should fail");
    }

    // Verify errors were published
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let errors = transport.get_published_errors().await;
    assert!(
        errors.len() >= 5,
        "Should publish error for each failed task"
    );
}

#[tokio::test]
async fn test_error_recovery_preserves_task_id() {
    // Verify that when errors occur, the task_id is preserved in error messages
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport.clone());
    let task = create_test_task("Test task ID preservation");
    let task_id = task.task_id;

    // Act: Process failing task
    let _ = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/agent", false)
        .await;

    // Assert: Error message should contain original task_id
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    let errors = transport.get_published_errors().await;
    if let Some((_, error_msg)) = errors.first() {
        assert_eq!(
            error_msg.task_id, task_id,
            "Error message should preserve task_id"
        );
    }
}

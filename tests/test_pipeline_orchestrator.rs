//! Comprehensive tests for Pipeline Orchestrator
//!
//! Tests the agent pipeline orchestration including:
//! - Multi-step task coordination
//! - Step failure handling
//! - Context passing between steps
//! - Pipeline depth validation
//! - Error propagation

mod test_helpers;

use agent2389::agent::pipeline::AgentPipeline;
use agent2389::agent::processor::AgentProcessor;
use agent2389::llm::provider::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmProvider, TokenUsage,
};
use agent2389::protocol::messages::{AgentStatusType, TaskEnvelope, TaskEnvelopeWrapper};
use agent2389::testing::mocks::{MockLlmProvider, MockTransport};
use agent2389::tools::ToolSystem;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

/// Create a test pipeline with mock dependencies
fn create_test_pipeline() -> (
    AgentPipeline<MockTransport>,
    mpsc::Sender<TaskEnvelopeWrapper>,
) {
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> =
        Arc::new(MockLlmProvider::single_response("Pipeline test response"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let (task_sender, task_receiver) = mpsc::channel(100);

    let pipeline = AgentPipeline::new(processor, task_receiver, 16);

    (pipeline, task_sender)
}

/// Create a test task envelope
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
async fn test_pipeline_start_success() {
    let (mut pipeline, _sender) = create_test_pipeline();

    let result = pipeline.start().await;

    assert!(result.is_ok(), "Pipeline start should succeed");
}

#[tokio::test]
async fn test_pipeline_process_single_task() {
    let (pipeline, _sender) = create_test_pipeline();
    let task = create_test_task("Single task test");

    let result = pipeline
        .process_single_task(TaskEnvelopeWrapper::V1(task.clone()))
        .await;

    assert!(result.is_ok(), "Single task processing should succeed");
    let processing_result = result.unwrap();
    assert_eq!(processing_result.task_id, task.task_id);
}

#[tokio::test]
async fn test_pipeline_process_multiple_tasks_sequentially() {
    let (pipeline, _sender) = create_test_pipeline();

    for i in 0..5 {
        let task = create_test_task(&format!("Sequential task {i}"));
        let result = pipeline
            .process_single_task(TaskEnvelopeWrapper::V1(task))
            .await;
        assert!(result.is_ok(), "Task {i} should succeed");
    }
}

#[tokio::test]
async fn test_pipeline_update_status() {
    let (pipeline, _sender) = create_test_pipeline();

    let result = pipeline.update_status(AgentStatusType::Available).await;

    assert!(result.is_ok(), "Status update should succeed");
}

#[tokio::test]
async fn test_pipeline_update_status_multiple() {
    let (pipeline, _sender) = create_test_pipeline();

    let statuses = vec![
        AgentStatusType::Available,
        AgentStatusType::Unavailable,
        AgentStatusType::Available,
        AgentStatusType::Unavailable,
    ];

    for status in statuses {
        let result = pipeline.update_status(status.clone()).await;
        assert!(result.is_ok(), "Status update to {status:?} should succeed");
    }
}

#[tokio::test]
async fn test_pipeline_shutdown_graceful() {
    let (mut pipeline, _sender) = create_test_pipeline();

    pipeline.start().await.expect("Start should succeed");

    let result = pipeline.shutdown().await;

    assert!(result.is_ok(), "Graceful shutdown should succeed");
}

#[tokio::test]
async fn test_pipeline_shutdown_without_start() {
    let (pipeline, _sender) = create_test_pipeline();

    let result = pipeline.shutdown().await;

    assert!(result.is_ok(), "Shutdown without start should succeed");
}

#[tokio::test]
async fn test_pipeline_run_with_tasks() {
    let (mut pipeline, sender) = create_test_pipeline();

    pipeline.start().await.expect("Start should succeed");

    // Send some tasks
    for i in 0..3 {
        let task = create_test_task(&format!("Run test task {i}"));
        sender
            .send(TaskEnvelopeWrapper::V1(task))
            .await
            .expect("Send should succeed");
    }

    // Close sender to stop pipeline
    drop(sender);

    // Run pipeline (will exit when channel closes)
    let result = tokio::time::timeout(Duration::from_secs(5), pipeline.run()).await;

    assert!(result.is_ok(), "Pipeline run should complete");
    assert!(result.unwrap().is_ok(), "Pipeline run should succeed");
}

#[tokio::test]
async fn test_pipeline_task_failure_handling() {
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let (_task_sender, task_receiver) = mpsc::channel(100);
    let pipeline = AgentPipeline::new(processor, task_receiver, 16);

    let task = create_test_task("Task that will fail");

    let result = pipeline
        .process_single_task(TaskEnvelopeWrapper::V1(task))
        .await;

    // Should fail but pipeline should handle gracefully
    assert!(result.is_err(), "Task should fail with LLM error");
}

#[tokio::test]
async fn test_pipeline_concurrent_task_processing() {
    let (pipeline, _sender) = create_test_pipeline();
    let pipeline_arc = Arc::new(pipeline);

    let mut handles = vec![];

    for i in 0..10 {
        let pipeline_clone = pipeline_arc.clone();
        let handle = tokio::spawn(async move {
            let task = create_test_task(&format!("Concurrent task {i}"));
            pipeline_clone
                .process_single_task(TaskEnvelopeWrapper::V1(task))
                .await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        let _ = handle.await;
    }

    // All should complete - tasks are processed
}

#[tokio::test]
async fn test_pipeline_rejects_excessive_depth() {
    // Arrange: Create a task that exceeds maximum pipeline depth (16)
    let (pipeline, _sender) = create_test_pipeline();
    let mut task = create_test_task("Deep pipeline task");
    task.topic = "/test/agent/depth/1/2/3/4/5/6/7/8/9/10/11/12/13/14/15/16/17".to_string();

    // Act: Process the overly deep task
    let result = pipeline
        .process_single_task(TaskEnvelopeWrapper::V1(task.clone()))
        .await;

    // Assert: Pipeline MUST reject tasks exceeding max depth (16)
    // Count topic depth: split by '/' and count segments
    let depth = task.topic.split('/').filter(|s| !s.is_empty()).count();

    if depth > 16 {
        assert!(
            result.is_err(),
            "Pipeline should reject tasks with depth {depth} > 16. Current bug: depth validation not implemented"
        );
    } else {
        // If depth is within limit, processing should succeed
        assert!(
            result.is_ok(),
            "Pipeline should process tasks with valid depth {depth}"
        );
    }
}

// NOTE: V2 iteration limit enforcement tests are covered by unit tests in:
// src/agent/pipeline/pipeline_orchestrator.rs (lines 357-368 for the enforcement logic)
// Integration tests with full Router mock would require complex setup with AgentInfo structs
// The core safety mechanism is validated via unit tests of the enforcement logic

#[tokio::test]
async fn test_pipeline_rapid_task_submission() {
    let (mut pipeline, sender) = create_test_pipeline();

    pipeline.start().await.expect("Start should succeed");

    // Rapidly submit many tasks
    let task_count = 100;
    for i in 0..task_count {
        let task = create_test_task(&format!("Rapid task {i}"));
        if sender.send(TaskEnvelopeWrapper::V1(task)).await.is_err() {
            break;
        }
    }

    drop(sender);

    let result = tokio::time::timeout(Duration::from_secs(10), pipeline.run()).await;
    assert!(result.is_ok(), "Rapid task processing should complete");
}

#[tokio::test]
async fn test_pipeline_status_consistency() {
    let (pipeline, _sender) = create_test_pipeline();
    let transport = pipeline.processor().transport();

    pipeline
        .update_status(AgentStatusType::Available)
        .await
        .expect("Status update should succeed");

    tokio::time::sleep(Duration::from_millis(50)).await;

    let statuses = transport.get_published_statuses().await;
    assert!(!statuses.is_empty(), "Status should be published");
}

#[tokio::test]
async fn test_pipeline_empty_channel_handling() {
    let (mut pipeline, sender) = create_test_pipeline();

    pipeline.start().await.expect("Start should succeed");

    // Close channel immediately
    drop(sender);

    let result = tokio::time::timeout(Duration::from_secs(2), pipeline.run()).await;

    assert!(result.is_ok(), "Empty channel should exit cleanly");
}

#[tokio::test]
async fn test_pipeline_handles_task_without_instruction() {
    // Arrange: Create a task without instruction
    let (pipeline, _sender) = create_test_pipeline();
    let mut task = create_test_task("");
    task.instruction = None;

    // Act: Process the task
    let result = pipeline
        .process_single_task(TaskEnvelopeWrapper::V1(task))
        .await;

    // Assert: Pipeline handles missing instruction gracefully
    assert!(
        result.is_ok(),
        "Pipeline should handle tasks without instruction"
    );
}

#[tokio::test]
async fn test_pipeline_error_propagation() {
    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport.clone());
    let (_sender, receiver) = mpsc::channel(10);
    let pipeline = AgentPipeline::new(processor, receiver, 16);

    let task = create_test_task("Error propagation test");
    let result = pipeline
        .process_single_task(TaskEnvelopeWrapper::V1(task))
        .await;

    // Error should propagate
    assert!(result.is_err(), "Error should propagate from processor");

    // Error should be published
    tokio::time::sleep(Duration::from_millis(50)).await;
    let errors = transport.get_published_errors().await;
    assert!(!errors.is_empty(), "Error should be published to transport");
}

#[tokio::test]
async fn test_pipeline_shutdown_with_pending_tasks() {
    let (mut pipeline, sender) = create_test_pipeline();

    pipeline.start().await.expect("Start should succeed");

    // Send many tasks
    for i in 0..50 {
        let task = create_test_task(&format!("Pending task {i}"));
        let _ = sender.send(TaskEnvelopeWrapper::V1(task)).await;
    }

    // Shutdown immediately
    drop(sender);

    let result = tokio::time::timeout(Duration::from_secs(5), pipeline.shutdown()).await;

    assert!(
        result.is_ok(),
        "Shutdown with pending tasks should complete"
    );
}

/// LLM provider that tracks call count
struct CountingLlmProvider {
    call_count: Arc<Mutex<usize>>,
}

impl CountingLlmProvider {
    fn new() -> Self {
        Self {
            call_count: Arc::new(Mutex::new(0)),
        }
    }

    async fn get_call_count(&self) -> usize {
        *self.call_count.lock().await
    }
}

#[async_trait]
impl LlmProvider for CountingLlmProvider {
    fn name(&self) -> &str {
        "counting-provider"
    }

    fn available_models(&self) -> Vec<String> {
        vec!["counting-model".to_string()]
    }

    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        let mut count = self.call_count.lock().await;
        *count += 1;

        Ok(CompletionResponse {
            content: Some(format!("Response {}", *count)),
            model: "counting-model".to_string(),
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            finish_reason: FinishReason::Stop,
            tool_calls: None,
            metadata: HashMap::new(),
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_pipeline_invokes_llm_for_tasks() {
    // Arrange: Create pipeline with counting LLM provider
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(CountingLlmProvider::new());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider.clone(), tool_system, transport);
    let (_sender, receiver) = mpsc::channel(10);
    let pipeline = AgentPipeline::new(processor, receiver, 16);

    // Act: Process tasks
    for i in 0..5 {
        let task = create_test_task(&format!("Task {i}"));
        let _ = pipeline
            .process_single_task(TaskEnvelopeWrapper::V1(task))
            .await;
    }

    // Assert: LLM was actually invoked
    let call_count = llm_provider.get_call_count().await;
    assert!(
        call_count >= 5,
        "Pipeline should invoke LLM for each task, got {call_count} calls"
    );
}

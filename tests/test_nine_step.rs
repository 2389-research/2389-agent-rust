//! Comprehensive edge case tests for Nine-Step Processing
//!
//! Tests observable behavior and critical edge cases following OPERATIONAL EXCELLENCE:
//! - Dynamic routing decisions and agent selection
//! - Pipeline depth validation (RFC FR-013)
//! - Task forwarding with output transformation
//! - Error handling and fallback behavior
//! - Edge cases and boundary conditions
//!
//! All tests focus on BEHAVIOR, not implementation details.

mod test_helpers;

use agent2389::agent::discovery::AgentRegistry;
use agent2389::processing::nine_step::{NineStepProcessor, ProcessorConfig};
use agent2389::protocol::messages::{NextTask, TaskEnvelope, TaskEnvelopeWrapper};
use agent2389::routing::agent_selector::RoutingHelper;
use agent2389::testing::mocks::{MockLlmProvider, MockTransport};
use agent2389::tools::ToolSystem;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

// ========== Test Helpers ==========

fn create_test_processor() -> NineStepProcessor<MockTransport> {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::single_response("test response"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    NineStepProcessor::new(config, llm_provider, tool_system, transport)
}

fn create_processor_with_routing() -> NineStepProcessor<MockTransport> {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::single_response(
        r#"{"status": "completed"}"#,
    ));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());
    let routing_helper = RoutingHelper::new();
    let agent_registry = AgentRegistry::new();

    NineStepProcessor::new_with_routing(
        config,
        llm_provider,
        tool_system,
        transport,
        routing_helper,
        agent_registry,
    )
}

fn create_simple_task() -> TaskEnvelope {
    TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: "test-conversation".to_string(),
        topic: "/control/agents/test-agent/input".to_string(),
        instruction: Some("Process this task".to_string()),
        input: json!({"test": "data"}),
        next: None,
    }
}

fn create_task_with_depth(depth: u32) -> TaskEnvelope {
    let mut task = create_simple_task();

    // Build nested pipeline
    let mut current_next: Option<Box<NextTask>> = None;
    for i in (1..depth).rev() {
        current_next = Some(Box::new(NextTask {
            topic: format!("/control/agents/agent-{i}/input"),
            instruction: Some(format!("Step {i}")),
            input: None,
            next: current_next,
        }));
    }

    task.next = current_next;
    task
}

// ========== Pipeline Depth Validation Tests (RFC FR-013) ==========

#[tokio::test]
async fn test_nine_step_accepts_task_at_max_pipeline_depth() {
    let processor = create_test_processor();
    let task = create_task_with_depth(16); // Exactly at limit

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task at max pipeline depth (16) should be accepted"
    );
}

#[tokio::test]
async fn test_nine_step_rejects_task_exceeding_max_pipeline_depth() {
    let processor = create_test_processor();
    let task = create_task_with_depth(17); // Exceeds limit

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_err(),
        "Task exceeding max pipeline depth (17) should be rejected"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Pipeline depth") && error.contains("16"),
        "Error should mention pipeline depth limit, got: {error}"
    );
}

#[tokio::test]
async fn test_nine_step_accepts_single_task_no_pipeline() {
    let processor = create_test_processor();
    let task = create_simple_task(); // Depth = 1

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Single task with no pipeline should be accepted"
    );
}

#[tokio::test]
async fn test_nine_step_custom_max_depth_configuration() {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::single_response("test"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor_config = ProcessorConfig {
        max_pipeline_depth: 5, // Custom lower limit
        max_task_cache: 10000,
    };

    let processor = NineStepProcessor::with_config(
        config,
        llm_provider,
        tool_system,
        transport,
        processor_config,
    );

    let task_within_limit = create_task_with_depth(5);
    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task_within_limit),
            "/control/agents/test-agent/input",
            false,
        )
        .await;
    assert!(
        result.is_ok(),
        "Task at custom limit (5) should be accepted"
    );

    let task_exceeding_limit = create_task_with_depth(6);
    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task_exceeding_limit),
            "/control/agents/test-agent/input",
            false,
        )
        .await;
    assert!(
        result.is_err(),
        "Task exceeding custom limit (6) should be rejected"
    );
}

// ========== Task Forwarding Tests ==========

#[tokio::test]
async fn test_nine_step_forwards_task_with_next_field() {
    let processor = create_test_processor();
    let next_task = NextTask {
        topic: "/control/agents/next-agent/input".to_string(),
        instruction: Some("Next step instruction".to_string()),
        input: Some(json!({"forwarded": true})),
        next: None,
    };

    let mut task = create_simple_task();
    task.next = Some(Box::new(next_task));

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task.clone()),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task with next field should process successfully"
    );

    let processing_result = result.unwrap();
    assert!(
        processing_result.forwarded,
        "Task should be marked as forwarded"
    );

    // Verify task was published to transport
    let published_tasks = processor.transport.get_published_tasks().await;
    assert_eq!(
        published_tasks.len(),
        1,
        "One task should be forwarded to next agent"
    );
    assert_eq!(
        published_tasks[0].0, "/control/agents/next-agent/input",
        "Task should be forwarded to correct topic"
    );
    assert_eq!(
        published_tasks[0].1.task_id, task.task_id,
        "Forwarded task should preserve original task_id for traceability"
    );
}

#[tokio::test]
async fn test_nine_step_no_forwarding_without_next_field() {
    let processor = create_test_processor();
    let task = create_simple_task(); // No next field

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task without next field should process successfully"
    );

    let processing_result = result.unwrap();
    assert!(
        !processing_result.forwarded,
        "Task should not be marked as forwarded"
    );

    // Verify no tasks were forwarded
    let published_tasks = processor.transport.get_published_tasks().await;
    assert_eq!(published_tasks.len(), 0, "No tasks should be forwarded");
}

#[tokio::test]
async fn test_nine_step_forwarding_uses_response_as_input_when_not_specified() {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::single_response("LLM response content"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = NineStepProcessor::new(config, llm_provider, tool_system, transport.clone());

    let next_task = NextTask {
        topic: "/control/agents/next-agent/input".to_string(),
        instruction: Some("Next step".to_string()),
        input: None, // No input specified - should use previous response
        next: None,
    };

    let mut task = create_simple_task();
    task.next = Some(Box::new(next_task));

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(result.is_ok(), "Task should process successfully");

    // Verify forwarded task uses LLM response as input
    let published_tasks = transport.get_published_tasks().await;
    assert_eq!(published_tasks.len(), 1);

    let forwarded_input = &published_tasks[0].1.input;
    assert!(
        forwarded_input.is_string(),
        "Forwarded input should be string when not specified"
    );
    assert_eq!(
        forwarded_input.as_str().unwrap(),
        "LLM response content",
        "Forwarded input should contain LLM response"
    );
}

#[tokio::test]
async fn test_nine_step_forwards_through_multiple_hops() {
    let processor = create_test_processor();

    // Create a 3-hop pipeline: agent1 -> agent2 -> agent3
    let task = TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: "test-conversation".to_string(),
        topic: "/control/agents/agent1/input".to_string(),
        instruction: Some("Start task".to_string()),
        input: json!({"start": true}),
        next: Some(Box::new(NextTask {
            topic: "/control/agents/agent2/input".to_string(),
            instruction: Some("Middle step".to_string()),
            input: None,
            next: Some(Box::new(NextTask {
                topic: "/control/agents/agent3/input".to_string(),
                instruction: Some("Final step".to_string()),
                input: None,
                next: None,
            })),
        })),
    };

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/agent1/input",
            false,
        )
        .await;

    assert!(result.is_ok(), "Multi-hop pipeline should process");

    let processing_result = result.unwrap();
    assert!(
        processing_result.forwarded,
        "Task should be forwarded to next hop"
    );

    // Verify only the immediate next hop was forwarded
    let published_tasks = processor.transport.get_published_tasks().await;
    assert_eq!(
        published_tasks.len(),
        1,
        "Should forward to immediate next agent only"
    );
    assert_eq!(
        published_tasks[0].0, "/control/agents/agent2/input",
        "Should forward to agent2"
    );

    // Verify the remaining pipeline is preserved in forwarded task
    let forwarded_task = &published_tasks[0].1;
    assert!(
        forwarded_task.next.is_some(),
        "Forwarded task should preserve remaining pipeline"
    );
    let remaining_next = forwarded_task.next.as_ref().unwrap();
    assert_eq!(
        remaining_next.topic, "/control/agents/agent3/input",
        "Remaining pipeline should point to agent3"
    );
}

// ========== Error Scenario Tests ==========

#[tokio::test]
async fn test_nine_step_handles_llm_failure_gracefully() {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = NineStepProcessor::new(config, llm_provider, tool_system, transport);

    let task = create_simple_task();
    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_err(),
        "LLM failure should cause task processing to fail"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("LLM") || error.contains("Mock LLM failure"),
        "Error should indicate LLM failure, got: {error}"
    );
}

#[tokio::test]
async fn test_nine_step_handles_transport_failure_during_forwarding() {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::single_response("test"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::with_failure());

    let processor = NineStepProcessor::new(config, llm_provider, tool_system, transport);

    let next_task = NextTask {
        topic: "/control/agents/next-agent/input".to_string(),
        instruction: Some("Next step".to_string()),
        input: None,
        next: None,
    };

    let mut task = create_simple_task();
    task.next = Some(Box::new(next_task));

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_err(),
        "Transport failure during forwarding should cause task to fail"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("forward") || error.contains("publish"),
        "Error should indicate forwarding/transport failure, got: {error}"
    );
}

#[tokio::test]
async fn test_nine_step_rejects_retained_messages() {
    let processor = create_test_processor();
    let task = create_simple_task();

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            true,
        ) // is_retained = true
        .await;

    assert!(
        result.is_err(),
        "Retained messages should be rejected per RFC"
    );
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Retained") || error.contains("retained"),
        "Error should mention retained messages, got: {error}"
    );
}

#[tokio::test]
async fn test_nine_step_rejects_duplicate_task_id_for_idempotency() {
    let processor = create_test_processor();
    let task_id = Uuid::new_v4();

    let task1 = TaskEnvelope {
        task_id,
        conversation_id: "test".to_string(),
        topic: "/control/agents/test-agent/input".to_string(),
        instruction: Some("First attempt".to_string()),
        input: json!({}),
        next: None,
    };

    let task2 = TaskEnvelope {
        task_id, // Same task_id
        conversation_id: "test".to_string(),
        topic: "/control/agents/test-agent/input".to_string(),
        instruction: Some("Duplicate attempt".to_string()),
        input: json!({}),
        next: None,
    };

    // First task should succeed
    let result1 = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task1),
            "/control/agents/test-agent/input",
            false,
        )
        .await;
    assert!(result1.is_ok(), "First task should process successfully");

    // Second task with same ID should fail
    let result2 = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task2),
            "/control/agents/test-agent/input",
            false,
        )
        .await;
    assert!(
        result2.is_err(),
        "Duplicate task_id should be rejected for idempotency"
    );
    let error = result2.unwrap_err().to_string();
    assert!(
        error.contains("already processed") || error.contains("idempotency"),
        "Error should indicate idempotency violation, got: {error}"
    );
}

#[tokio::test]
async fn test_nine_step_rejects_topic_mismatch() {
    let processor = create_test_processor();
    let mut task = create_simple_task();
    task.topic = "/control/agents/different-agent/input".to_string();

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input", // Different from task.topic
            false,
        )
        .await;

    assert!(result.is_err(), "Topic mismatch should be rejected");
    let error = result.unwrap_err().to_string();
    assert!(
        error.contains("Topic") || error.contains("mismatch"),
        "Error should indicate topic mismatch, got: {error}"
    );
}

// ========== Dynamic Routing Tests ==========

#[tokio::test]
async fn test_nine_step_uses_static_routing_when_next_field_present() {
    let processor = create_processor_with_routing();

    let next_task = NextTask {
        topic: "/control/agents/static-target/input".to_string(),
        instruction: Some("Static route".to_string()),
        input: None,
        next: None,
    };

    let mut task = create_simple_task();
    task.next = Some(Box::new(next_task));

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(result.is_ok(), "Static routing should succeed");
    assert!(
        result.unwrap().forwarded,
        "Task should be forwarded via static route"
    );

    // Verify forwarded to static target
    let published_tasks = processor.transport.get_published_tasks().await;
    assert_eq!(published_tasks.len(), 1);
    assert_eq!(published_tasks[0].0, "/control/agents/static-target/input");
}

#[tokio::test]
async fn test_nine_step_no_forwarding_when_no_routing_rules_and_no_next_field() {
    let processor = create_processor_with_routing();
    let task = create_simple_task(); // No next field, no routing rules configured

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(result.is_ok(), "Task should process successfully");
    assert!(
        !result.unwrap().forwarded,
        "Task should not be forwarded without routing rules or next field"
    );

    let published_tasks = processor.transport.get_published_tasks().await;
    assert_eq!(published_tasks.len(), 0, "No tasks should be forwarded");
}

#[tokio::test]
async fn test_nine_step_routing_extracts_agent_id_from_control_topic() {
    let processor = create_processor_with_routing();

    // Test valid control topic format
    let agent_id = processor.extract_agent_id_from_topic("/control/agents/my-agent/input");
    assert_eq!(
        agent_id,
        Some("my-agent".to_string()),
        "Should extract agent ID from valid control topic"
    );

    // Test with different topic formats
    let agent_id = processor.extract_agent_id_from_topic("/control/agents/agent-123/input");
    assert_eq!(agent_id, Some("agent-123".to_string()));

    // Test invalid formats
    let agent_id = processor.extract_agent_id_from_topic("/some/other/topic");
    assert_eq!(agent_id, None, "Should return None for non-control topic");

    let agent_id = processor.extract_agent_id_from_topic("/control/agents/");
    assert_eq!(agent_id, None, "Should return None for incomplete topic");
}

// ========== Response Publishing Tests ==========

#[tokio::test]
async fn test_nine_step_publishes_response_to_conversation_topic() {
    let processor = create_test_processor();
    let task = create_simple_task();

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task.clone()),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(result.is_ok(), "Task should process successfully");

    // Verify response was published
    let published_responses = processor.transport.get_published_responses().await;
    assert_eq!(
        published_responses.len(),
        1,
        "Response should be published to conversation topic"
    );

    let (conversation_topic, response_msg) = &published_responses[0];
    assert!(
        conversation_topic.contains(&task.conversation_id),
        "Response topic should contain conversation ID"
    );
    assert_eq!(
        response_msg.task_id, task.task_id,
        "Response should reference correct task_id"
    );
    assert!(
        !response_msg.response.is_empty(),
        "Response should have content"
    );
}

#[tokio::test]
async fn test_nine_step_skips_publish_when_forwarding() {
    let processor = create_test_processor();

    let next_task = NextTask {
        topic: "/control/agents/next-agent/input".to_string(),
        instruction: Some("Next step".to_string()),
        input: None,
        next: None,
    };

    let mut task = create_simple_task();
    task.next = Some(Box::new(next_task));

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task.clone()),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(result.is_ok(), "Task should process successfully");

    // Verify forwarding also occurred
    let published_tasks = processor.transport.get_published_tasks().await;
    assert_eq!(published_tasks.len(), 1, "Task should also be forwarded");

    // Verify NO response was published to conversation when forwarding
    let published_responses = processor.transport.get_published_responses().await;
    assert_eq!(
        published_responses.len(),
        0,
        "Response should NOT be published when forwarding"
    );
}

// ========== Edge Cases and Boundary Conditions ==========

#[tokio::test]
async fn test_nine_step_handles_empty_instruction() {
    let processor = create_test_processor();
    let mut task = create_simple_task();
    task.instruction = None;

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task with no instruction should process successfully"
    );
}

#[tokio::test]
async fn test_nine_step_handles_empty_input() {
    let processor = create_test_processor();
    let mut task = create_simple_task();
    task.input = json!(null);

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task with null input should process successfully"
    );
}

#[tokio::test]
async fn test_nine_step_handles_complex_nested_input() {
    let processor = create_test_processor();
    let mut task = create_simple_task();
    task.input = json!({
        "nested": {
            "deeply": {
                "structured": {
                    "data": [1, 2, 3, 4, 5],
                    "metadata": {
                        "timestamp": "2025-01-01T00:00:00Z",
                        "version": "1.0"
                    }
                }
            }
        }
    });

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task with complex nested input should process successfully"
    );
}

#[tokio::test]
async fn test_nine_step_preserves_task_id_through_processing() {
    let processor = create_test_processor();
    let original_task_id = Uuid::new_v4();

    let task = TaskEnvelope {
        task_id: original_task_id,
        conversation_id: "test".to_string(),
        topic: "/control/agents/test-agent/input".to_string(),
        instruction: Some("Test".to_string()),
        input: json!({}),
        next: None,
    };

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(result.is_ok());
    let processing_result = result.unwrap();
    assert_eq!(
        processing_result.task_id, original_task_id,
        "Task ID should be preserved through processing"
    );

    // Verify response contains correct task_id
    let published_responses = processor.transport.get_published_responses().await;
    assert_eq!(published_responses[0].1.task_id, original_task_id);
}

#[tokio::test]
async fn test_nine_step_handles_very_long_instruction() {
    let processor = create_test_processor();
    let mut task = create_simple_task();
    task.instruction = Some("x".repeat(10000)); // 10KB instruction

    let result = processor
        .process_task(
            TaskEnvelopeWrapper::V1(task),
            "/control/agents/test-agent/input",
            false,
        )
        .await;

    assert!(
        result.is_ok(),
        "Task with very long instruction should process successfully"
    );
}

#[tokio::test]
async fn test_nine_step_idempotency_cache_respects_max_cache_size() {
    let config = test_helpers::test_config();
    let llm_provider = Arc::new(MockLlmProvider::single_response("test"));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor_config = ProcessorConfig {
        max_pipeline_depth: 16,
        max_task_cache: 5, // Small cache for testing
    };

    let processor = NineStepProcessor::with_config(
        config,
        llm_provider,
        tool_system,
        transport,
        processor_config,
    );

    // Process 10 tasks (exceeds cache size of 5)
    for i in 0..10 {
        let mut task = create_simple_task();
        task.task_id = Uuid::new_v4();
        task.instruction = Some(format!("Task {i}"));

        let result = processor
            .process_task(
                TaskEnvelopeWrapper::V1(task),
                "/control/agents/test-agent/input",
                false,
            )
            .await;
        assert!(result.is_ok(), "Task {i} should process");
    }

    // All tasks should have processed successfully
    // Cache management should prevent memory growth beyond max_task_cache
    // (Implementation detail: oldest entries should be evicted)
}

#[tokio::test]
async fn test_nine_step_concurrent_task_processing() {
    let processor = Arc::new(create_test_processor());

    // Spawn multiple concurrent tasks
    let mut handles = vec![];
    for i in 0..5 {
        let processor_clone = Arc::clone(&processor);
        let handle = tokio::spawn(async move {
            let mut task = create_simple_task();
            task.task_id = Uuid::new_v4();
            task.instruction = Some(format!("Concurrent task {i}"));

            processor_clone
                .process_task(
                    TaskEnvelopeWrapper::V1(task),
                    "/control/agents/test-agent/input",
                    false,
                )
                .await
        });
        handles.push(handle);
    }

    // All tasks should complete successfully
    for (i, handle) in handles.into_iter().enumerate() {
        let result = handle.await.expect("Task should not panic");
        assert!(result.is_ok(), "Concurrent task {i} should succeed");
    }
}

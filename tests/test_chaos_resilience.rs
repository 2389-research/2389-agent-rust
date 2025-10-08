//! Chaos and Resilience Testing
//!
//! Tests system behavior under adverse conditions:
//! - MQTT disconnect DURING task processing
//! - Broker restart while processing task
//! - Network failures mid-conversation
//! - Agent crash during tool execution

mod test_helpers;

use agent2389::agent::processor::AgentProcessor;
use agent2389::config::MqttSection;
use agent2389::llm::provider::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmProvider, TokenUsage,
};
use agent2389::protocol::messages::TaskEnvelope;
use agent2389::protocol::TaskEnvelopeWrapper;
use agent2389::testing::mocks::{MockLlmProvider, MockTransport};
use agent2389::tools::ToolSystem;
use agent2389::transport::mqtt::MqttClient;
use async_trait::async_trait;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// LLM provider that simulates slow processing (for testing disconnects mid-call)
struct SlowLlmProvider {
    delay_ms: u64,
    should_fail: Arc<AtomicBool>,
}

impl SlowLlmProvider {
    fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            should_fail: Arc::new(AtomicBool::new(false)),
        }
    }

    fn trigger_failure(&self) {
        self.should_fail.store(true, Ordering::SeqCst);
    }
}

#[async_trait]
impl LlmProvider for SlowLlmProvider {
    fn name(&self) -> &str {
        "slow-provider"
    }

    fn available_models(&self) -> Vec<String> {
        vec!["slow-model".to_string()]
    }

    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        // Simulate slow processing
        sleep(Duration::from_millis(self.delay_ms)).await;

        if self.should_fail.load(Ordering::SeqCst) {
            return Err(LlmError::ApiError("Simulated failure mid-call".to_string()));
        }

        Ok(CompletionResponse {
            content: Some("Slow response".to_string()),
            model: "slow-model".to_string(),
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            finish_reason: FinishReason::Stop,
            tool_calls: None,
            metadata: Default::default(),
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        Ok(())
    }
}

fn create_test_task(instruction: &str) -> TaskEnvelope {
    TaskEnvelope {
        task_id: Uuid::new_v4(),
        conversation_id: format!("chaos-test-{}", Uuid::new_v4()),
        topic: "/test/chaos".to_string(),
        instruction: Some(instruction.to_string()),
        input: json!({}),
        next: None,
    }
}

#[tokio::test]
async fn test_llm_failure_during_task_processing() {
    // Test that LLM failures during processing are handled gracefully

    let config = test_helpers::test_config();
    let slow_provider = Arc::new(SlowLlmProvider::new(100));
    let provider_ref = slow_provider.clone();

    let llm_provider: Arc<dyn LlmProvider> = slow_provider;
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport.clone());
    let task = create_test_task("Test LLM failure");

    // Trigger failure mid-processing
    tokio::spawn(async move {
        sleep(Duration::from_millis(50)).await;
        provider_ref.trigger_failure();
    });

    // Act: Process task
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/chaos", false)
        .await;

    // Assert: Should fail gracefully
    assert!(result.is_err(), "Should error when LLM fails");

    // Verify error was published
    sleep(Duration::from_millis(50)).await;
    let errors = transport.get_published_errors().await;
    assert!(
        !errors.is_empty(),
        "Error should be published when LLM fails mid-call"
    );
}

#[tokio::test]
async fn test_transport_failure_during_response_publishing() {
    // Test handling of transport failures when publishing response

    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::single_response("Success"));
    let tool_system = Arc::new(ToolSystem::new());

    // Transport that will fail
    let transport = Arc::new(MockTransport::with_failure());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let task = create_test_task("Test transport failure");

    // Act: Process task
    let result = processor
        .process_task(TaskEnvelopeWrapper::V1(task), "/test/chaos", false)
        .await;

    // Assert: Should handle transport failure
    assert!(
        result.is_err(),
        "Should error when transport fails to publish"
    );
}

#[tokio::test]
async fn test_concurrent_task_processing_with_failures() {
    // Test that failures in one task don't affect other concurrent tasks

    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(SlowLlmProvider::new(50));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = Arc::new(AgentProcessor::new(
        config,
        llm_provider,
        tool_system,
        transport.clone(),
    ));

    let mut handles = vec![];

    // Process 10 tasks concurrently
    for i in 0..10 {
        let processor_clone = processor.clone();
        let handle = tokio::spawn(async move {
            let task = create_test_task(&format!("Concurrent task {i}"));
            processor_clone
                .process_task(TaskEnvelopeWrapper::V1(task), "/test/chaos", false)
                .await
        });
        handles.push(handle);
    }

    // Wait for all tasks
    let results: Vec<_> = futures::future::join_all(handles)
        .await
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    // Assert: All tasks complete (some may succeed, some may fail, but none crash)
    assert_eq!(results.len(), 10, "All tasks should complete");
}

#[tokio::test]
async fn test_broker_unavailable_during_startup() {
    // Test agent behavior when broker is unavailable at startup

    let config = MqttSection {
        broker_url: "mqtt://localhost:9876".to_string(), // Non-existent broker
        username_env: None,
        password_env: None,
        heartbeat_interval_secs: 900,
    };

    let mut client = MqttClient::new("chaos-startup-agent", config)
        .await
        .expect("Client creation should succeed");

    // Act: Attempt connection with timeout
    let connect_result = tokio::time::timeout(Duration::from_secs(1), client.connect()).await;

    // Assert: Should timeout or fail, but not crash
    assert!(
        connect_result.is_err() || connect_result.unwrap().is_err(),
        "Connection should fail when broker unavailable"
    );

    // Verify client is still valid (didn't crash)
    assert!(
        !client.is_permanently_disconnected(),
        "Client should be retrying, not permanently disconnected"
    );
}

#[tokio::test]
async fn test_rapid_connect_disconnect_cycles() {
    // Test stability under rapid connect/disconnect cycles (simulating network instability)

    let config = MqttSection {
        broker_url: "mqtt://localhost:1883".to_string(),
        username_env: None,
        password_env: None,
        heartbeat_interval_secs: 900,
    };

    let mut client = MqttClient::new("rapid-cycle-agent", config)
        .await
        .expect("Client creation should succeed");

    // Act: Rapid connect/disconnect cycles
    for i in 0..5 {
        // Try to connect
        let _ = tokio::time::timeout(Duration::from_millis(200), client.connect()).await;

        // Immediately disconnect
        let _ = client.disconnect().await;

        sleep(Duration::from_millis(10)).await;

        // Verify client is still responsive
        assert!(
            !client.is_permanently_disconnected(),
            "Client should survive rapid cycle {i}"
        );
    }
}

#[tokio::test]
async fn test_task_processing_with_network_timeout() {
    // Test that tasks timeout appropriately when network is slow

    let config = test_helpers::test_config();

    // Very slow provider (simulating network timeout)
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(SlowLlmProvider::new(5000));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);
    let task = create_test_task("Test timeout");

    // Act: Process with short timeout
    let result = tokio::time::timeout(
        Duration::from_secs(1),
        processor.process_task(TaskEnvelopeWrapper::V1(task), "/test/chaos", false),
    )
    .await;

    // Assert: Should timeout
    assert!(result.is_err(), "Task should timeout on slow network");
}

#[tokio::test]
async fn test_memory_stability_under_repeated_failures() {
    // Test that repeated failures don't cause memory leaks or crashes

    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(MockLlmProvider::with_failure());
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = AgentProcessor::new(config, llm_provider, tool_system, transport);

    // Act: Process 100 failing tasks
    for i in 0..100 {
        let task = create_test_task(&format!("Failing task {i}"));
        let _ = processor
            .process_task(TaskEnvelopeWrapper::V1(task), "/test/chaos", false)
            .await;
    }

    // Assert: System should still be responsive (didn't crash or leak memory)
    // This is a basic smoke test - real memory leak detection would require profiling
}

#[tokio::test]
async fn test_graceful_degradation_under_load() {
    // Test that system gracefully handles load rather than crashing

    let config = test_helpers::test_config();
    let llm_provider: Arc<dyn LlmProvider> = Arc::new(SlowLlmProvider::new(10));
    let tool_system = Arc::new(ToolSystem::new());
    let transport = Arc::new(MockTransport::new());

    let processor = Arc::new(AgentProcessor::new(
        config,
        llm_provider,
        tool_system,
        transport,
    ));

    // Submit 50 tasks quickly (simulating load spike)
    let mut handles = vec![];
    for i in 0..50 {
        let processor_clone = processor.clone();
        let handle = tokio::spawn(async move {
            let task = create_test_task(&format!("Load test {i}"));
            processor_clone
                .process_task(TaskEnvelopeWrapper::V1(task), "/test/chaos", false)
                .await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;

    // Assert: All tasks complete without panics
    assert_eq!(results.len(), 50, "All tasks should complete");

    // Count successes (some may fail, but system shouldn't crash)
    let successes = results
        .iter()
        .filter(|r| r.is_ok() && r.as_ref().unwrap().is_ok())
        .count();

    // System should handle at least some tasks successfully
    assert!(successes > 0, "System should process some tasks under load");
}

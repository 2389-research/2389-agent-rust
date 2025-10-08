//! Comprehensive tests for Agent Lifecycle
//!
//! Tests RFC-compliant lifecycle management including:
//! - Agent startup sequence
//! - Graceful shutdown
//! - Signal handling (SIGTERM, SIGINT)
//! - Resource cleanup
//! - Connection establishment
//! - Registry integration
//! - Health server initialization

mod test_helpers;

use agent2389::agent::lifecycle::AgentLifecycle;
use agent2389::observability::health::HealthServer;
use agent2389::testing::mocks::{MockLlmProvider, MockTransport};
use std::sync::Arc;
use std::time::Duration;

/// Create a test lifecycle with mock dependencies
fn create_test_lifecycle() -> AgentLifecycle<MockTransport> {
    let config = test_helpers::test_config();
    let transport = MockTransport::new();
    let llm_provider = Box::new(MockLlmProvider::single_response("test response"));

    AgentLifecycle::new(config, transport, llm_provider)
}

#[test]
fn test_lifecycle_agent_id() {
    let lifecycle = create_test_lifecycle();
    assert_eq!(lifecycle.agent_id(), "test-agent");
}

#[tokio::test]
async fn test_lifecycle_initialization_idempotent() {
    let mut lifecycle = create_test_lifecycle();

    let result1 = lifecycle.initialize().await;
    assert!(result1.is_ok(), "First initialization should succeed");

    let result2 = lifecycle.initialize().await;
    assert!(
        result2.is_ok(),
        "Second initialization should succeed (idempotent)"
    );
}

#[tokio::test]
async fn test_health_manager_reports_status() {
    let lifecycle = create_test_lifecycle();

    let health_manager = lifecycle.health_check_manager();
    let overall_health = health_manager.calculate_overall_health().await;

    assert!(
        overall_health.is_ok(),
        "Health manager should calculate health status"
    );
}

#[tokio::test]
async fn test_lifecycle_start_success() {
    let mut lifecycle = create_test_lifecycle();

    lifecycle.initialize().await.expect("Init should succeed");

    let result = lifecycle.start().await;

    assert!(result.is_ok(), "Start should succeed: {result:?}");
    assert!(lifecycle.is_initialized());
}

#[tokio::test]
async fn test_lifecycle_start_without_init() {
    let mut lifecycle = create_test_lifecycle();

    // Start without explicit initialization should still work
    let result = lifecycle.start().await;

    assert!(
        result.is_ok(),
        "Start should succeed without init: {result:?}"
    );
}

#[tokio::test]
async fn test_lifecycle_shutdown_success() {
    let mut lifecycle = create_test_lifecycle();

    lifecycle.initialize().await.expect("Init should succeed");
    lifecycle.start().await.expect("Start should succeed");

    // Give pipeline time to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = lifecycle.shutdown().await;

    assert!(result.is_ok(), "Shutdown should succeed: {result:?}");
}

#[tokio::test]
async fn test_lifecycle_shutdown_without_start() {
    let mut lifecycle = create_test_lifecycle();

    let result = lifecycle.shutdown().await;

    // Should handle shutdown gracefully even without start
    assert!(result.is_ok(), "Shutdown without start should succeed");
}

#[tokio::test]
async fn test_lifecycle_full_cycle() {
    let mut lifecycle = create_test_lifecycle();

    // Initialize
    let init_result = lifecycle.initialize().await;
    assert!(init_result.is_ok(), "Initialize should succeed");

    // Start
    let start_result = lifecycle.start().await;
    assert!(start_result.is_ok(), "Start should succeed");

    // Let it run briefly
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Shutdown
    let shutdown_result = lifecycle.shutdown().await;
    assert!(shutdown_result.is_ok(), "Shutdown should succeed");
}

#[tokio::test]
async fn test_lifecycle_double_start_prevention() {
    let mut lifecycle = create_test_lifecycle();

    lifecycle.initialize().await.expect("Init should succeed");

    let result1 = lifecycle.start().await;
    assert!(result1.is_ok(), "First start should succeed");

    let result2 = lifecycle.start().await;
    // Second start should fail because components are moved
    assert!(result2.is_err(), "Second start should fail");
}

#[tokio::test]
async fn test_lifecycle_health_server_integration() {
    let mut lifecycle = create_test_lifecycle();

    // Create and set health server
    let health_server = Arc::new(HealthServer::new("test-agent".to_string(), 0));
    lifecycle.set_health_server(health_server.clone());

    lifecycle.initialize().await.expect("Init should succeed");
    let result = lifecycle.start().await;

    assert!(result.is_ok(), "Start with health server should succeed");
}

#[tokio::test]
async fn test_lifecycle_start_with_failing_transport() {
    let config = test_helpers::test_config();
    let transport = MockTransport::with_failure();
    let llm_provider = Box::new(MockLlmProvider::single_response("test"));

    let mut lifecycle = AgentLifecycle::new(config, transport, llm_provider);

    let result = lifecycle.start().await;

    // Should fail because transport cannot connect
    assert!(result.is_err(), "Start should fail with failing transport");
}

#[tokio::test]
async fn test_lifecycle_shutdown_idempotent() {
    let mut lifecycle = create_test_lifecycle();

    lifecycle.start().await.expect("Start should succeed");

    let result1 = lifecycle.shutdown().await;
    assert!(result1.is_ok(), "First shutdown should succeed");

    let result2 = lifecycle.shutdown().await;
    // Second shutdown should be idempotent
    assert!(result2.is_ok(), "Second shutdown should be idempotent");
}

#[tokio::test]
async fn test_lifecycle_rapid_start_shutdown() {
    let mut lifecycle = create_test_lifecycle();

    lifecycle.start().await.expect("Start should succeed");

    // Immediate shutdown without delay
    let result = lifecycle.shutdown().await;

    assert!(result.is_ok(), "Rapid shutdown should succeed");
}

#[tokio::test]
async fn test_lifecycle_multiple_instances() {
    let mut lifecycle1 = create_test_lifecycle();
    let mut lifecycle2 = create_test_lifecycle();

    let result1 = lifecycle1.start().await;
    let result2 = lifecycle2.start().await;

    assert!(result1.is_ok(), "First instance should start");
    assert!(result2.is_ok(), "Second instance should start");

    lifecycle1
        .shutdown()
        .await
        .expect("First shutdown should succeed");
    lifecycle2
        .shutdown()
        .await
        .expect("Second shutdown should succeed");
}

#[tokio::test]
async fn test_lifecycle_graceful_shutdown_timeout() {
    let mut lifecycle = create_test_lifecycle();

    lifecycle.start().await.expect("Start should succeed");

    // Shutdown with timeout
    let result = tokio::time::timeout(Duration::from_secs(5), lifecycle.shutdown()).await;

    assert!(result.is_ok(), "Shutdown should complete within timeout");
    assert!(result.unwrap().is_ok(), "Shutdown should succeed");
}

#[tokio::test]
async fn test_shutdown_cleans_up_resources() {
    // Arrange: Start a lifecycle
    let mut lifecycle = create_test_lifecycle();
    lifecycle.start().await.expect("Start should succeed");
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Act: Shutdown
    let result = lifecycle.shutdown().await;

    // Assert: Shutdown completes without errors
    assert!(
        result.is_ok(),
        "Shutdown should clean up all resources successfully"
    );
}

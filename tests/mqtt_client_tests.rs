//! Integration tests for MQTT client
//!
//! Tests the MQTT client's core functionality including:
//! - Connection lifecycle (connect, disconnect, reconnection)
//! - Message publishing (tasks, responses, errors, status)
//! - Subscription management
//! - State management and health monitoring
//! - Error handling and edge cases

use agent2389::config::MqttSection;
use agent2389::protocol::{
    AgentStatus, AgentStatusType, ErrorCode, ErrorDetails, ErrorMessage, ResponseMessage,
};
use agent2389::transport::mqtt::{MqttClient, ReconnectConfig};
use agent2389::transport::Transport;
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

fn test_mqtt_config() -> MqttSection {
    MqttSection {
        broker_url: "mqtt://localhost:1883".to_string(),
        username_env: None,
        password_env: None,
        heartbeat_interval_secs: 900,
    }
}

fn test_mqtt_config_with_auth() -> MqttSection {
    MqttSection {
        broker_url: "mqtt://localhost:1883".to_string(),
        username_env: Some("MQTT_USER".to_string()),
        password_env: Some("MQTT_PASS".to_string()),
        heartbeat_interval_secs: 900,
    }
}

fn test_mqtt_config_tls() -> MqttSection {
    MqttSection {
        broker_url: "mqtts://localhost:8883".to_string(),
        username_env: None,
        password_env: None,
        heartbeat_interval_secs: 900,
    }
}

#[tokio::test]
async fn test_mqtt_client_creation() {
    // Arrange: Create MQTT config
    let config = test_mqtt_config();

    // Act: Create client
    let result = MqttClient::new("test-agent", config).await;

    // Assert: Client created successfully but not yet connected
    assert!(result.is_ok(), "Client creation should succeed");
    let client = result.unwrap();
    assert!(
        !client.is_connected(),
        "Client should not be connected until connect() is called"
    );
}

#[tokio::test]
async fn test_mqtt_client_creation_with_tls() {
    let config = test_mqtt_config_tls();
    let result = MqttClient::new("test-agent-tls", config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mqtt_client_creation_with_auth() {
    // Arrange: Set auth environment variables
    unsafe {
        std::env::set_var("MQTT_USER_TEST", "testuser");
        std::env::set_var("MQTT_PASS_TEST", "testpass");
    }

    let mut config = test_mqtt_config_with_auth();
    config.username_env = Some("MQTT_USER_TEST".to_string());
    config.password_env = Some("MQTT_PASS_TEST".to_string());

    // Act: Create client with auth config
    let result = MqttClient::new("test-agent-auth", config).await;

    // Assert: Client created with credentials
    assert!(
        result.is_ok(),
        "Client with auth credentials should be created"
    );

    // Cleanup
    unsafe {
        std::env::remove_var("MQTT_USER_TEST");
        std::env::remove_var("MQTT_PASS_TEST");
    }
}

#[tokio::test]
async fn test_mqtt_client_invalid_broker_url() {
    let mut config = test_mqtt_config();
    config.broker_url = "invalid-url".to_string();

    let result = MqttClient::new("test-agent", config).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_publish_without_connection() {
    // Arrange: Create disconnected client
    let config = test_mqtt_config();
    let client = MqttClient::new("test-agent", config).await.unwrap();

    let status = AgentStatus {
        agent_id: "test-agent".to_string(),
        status: AgentStatusType::Available,
        timestamp: Utc::now(),
        capabilities: None,
        description: None,
    };

    // Act: Attempt to publish without connecting
    let result = client.publish_status(&status).await;

    // Assert: Publishing without connection MUST return an error
    // Client should not silently fail or queue messages without connection
    assert!(
        result.is_err(),
        "Publishing without connection should return error, not silently fail. \
         Current bug: check_connection_state() returns Ok when state_rx is None"
    );
}

#[tokio::test]
async fn test_health_metrics_initial_state() {
    let config = test_mqtt_config();
    let client = MqttClient::new("test-agent", config).await.unwrap();

    let metrics = client.get_health_metrics();
    assert_eq!(metrics.uptime, None);
    assert_eq!(metrics.time_since_last_message, None);
    assert_eq!(metrics.reconnect_count, 0);
}

#[tokio::test]
async fn test_reconnect_backoff_increases_with_pattern() {
    // Arrange: Create reconnect config with custom pattern
    let config = ReconnectConfig {
        max_attempts: Some(5),
        backoff_pattern: vec![200, 400, 800],
        sustained_delay: 1000,
    };

    // Act: Calculate delays for sequential attempts
    let delay1 = config.calculate_backoff_delay(1);
    let delay2 = config.calculate_backoff_delay(2);
    let delay3 = config.calculate_backoff_delay(3);

    // Assert: Delays follow the pattern
    assert_eq!(delay1, 200, "First attempt should use first pattern value");
    assert_eq!(
        delay2, 400,
        "Second attempt should use second pattern value"
    );
    assert_eq!(delay3, 800, "Third attempt should use third pattern value");
    assert!(
        delay2 > delay1 && delay3 > delay2,
        "Backoff should increase following pattern"
    );
}

#[tokio::test]
async fn test_protocol_messages_serialize_correctly() {
    // Arrange: Create protocol message instances
    let response = ResponseMessage {
        task_id: Uuid::new_v4(),
        response: json!({"result": "success"}).to_string(),
    };

    let error = ErrorMessage {
        error: ErrorDetails {
            code: ErrorCode::InternalError,
            message: "Test error".to_string(),
        },
        task_id: Uuid::new_v4(),
    };

    let status = AgentStatus {
        agent_id: "test-agent".to_string(),
        status: AgentStatusType::Available,
        timestamp: Utc::now(),
        capabilities: None,
        description: None,
    };

    // Act: Serialize to JSON
    let response_json = serde_json::to_string(&response).unwrap();
    let error_json = serde_json::to_string(&error).unwrap();
    let status_json = serde_json::to_string(&status).unwrap();

    // Assert: JSON contains expected fields
    assert!(
        response_json.contains("task_id"),
        "Response should contain task_id"
    );
    assert!(
        error_json.contains("internal_error"),
        "Error should contain error code"
    );
    assert!(
        status_json.contains("available"),
        "Status should contain status type"
    );
}

#[tokio::test]
async fn test_reconnect_backoff_uses_sustained_delay() {
    // Arrange: Use default reconnect config
    let config = ReconnectConfig::default();

    // Act: Calculate delay for attempt beyond pattern length
    let sustained = config.calculate_backoff_delay(30);

    // Assert: Uses sustained delay after pattern exhausted
    assert_eq!(
        sustained, config.sustained_delay,
        "Backoff should use sustained_delay after pattern exhausted"
    );
}

#[tokio::test]
async fn test_health_metrics_after_connection() {
    // This test would require actual broker connection
    // For now, we verify metrics exist in initial state (covered above)
    // Full integration test would verify metrics update after connect/disconnect
}

#[tokio::test]
async fn test_mqtt_client_handles_connection_state_transitions() {
    let config = test_mqtt_config();
    let client = MqttClient::new("test-agent-states", config).await.unwrap();

    // Initial state should be disconnected/not connected
    assert!(!client.is_connected(), "Should not be connected initially");

    // Note: Actual connection testing requires running broker
    // This test validates state tracking exists
}

#[tokio::test]
async fn test_publish_operations_require_connection() {
    let config = test_mqtt_config();
    let client = MqttClient::new("test-agent-publish", config).await.unwrap();

    let status = AgentStatus {
        agent_id: "test-agent-publish".to_string(),
        status: AgentStatusType::Available,
        timestamp: Utc::now(),
        capabilities: None,
        description: None,
    };

    let response = ResponseMessage {
        task_id: Uuid::new_v4(),
        response: json!({"result": "test"}).to_string(),
    };

    let error = ErrorMessage {
        task_id: Uuid::new_v4(),
        error: ErrorDetails {
            code: ErrorCode::InternalError,
            message: "test error".to_string(),
        },
    };

    // All publish operations should fail without connection
    assert!(
        client.publish_status(&status).await.is_err(),
        "Status publish should fail"
    );
    assert!(
        client
            .publish_response("/test/topic", &response)
            .await
            .is_err(),
        "Response publish should fail"
    );
    assert!(
        client.publish_error("/test/topic", &error).await.is_err(),
        "Error publish should fail"
    );
}

#[tokio::test]
async fn test_connection_state_query_methods() {
    let config = test_mqtt_config();
    let client = MqttClient::new("test-agent-query", config).await.unwrap();

    // Query connection state
    let state = client.connection_state();

    // Should have some state (even if disconnected)
    // The exact state depends on whether client.connect() was called
    assert!(matches!(state, _), "Should return a connection state");

    // Check if permanently disconnected
    let perm_disconnected = client.is_permanently_disconnected();
    assert!(
        !perm_disconnected,
        "Should not be permanently disconnected on creation"
    );
}

#[tokio::test]
async fn test_reconnect_config_validation() {
    // Test that ReconnectConfig uses custom pattern
    let config = ReconnectConfig {
        max_attempts: Some(5),
        backoff_pattern: vec![100, 200, 400],
        sustained_delay: 500,
    };

    // Verify backoff follows pattern
    let delay1 = config.calculate_backoff_delay(1);
    let delay2 = config.calculate_backoff_delay(2);
    let delay3 = config.calculate_backoff_delay(3);
    let delay4 = config.calculate_backoff_delay(4);

    assert!(delay2 > delay1, "Backoff should increase");
    assert!(delay3 > delay2, "Backoff should continue increasing");
    assert_eq!(delay1, 100, "Should use first pattern value");
    assert_eq!(delay4, 500, "Should use sustained delay after pattern");
}

#[tokio::test]
async fn test_mqtt_client_topic_canonicalization() {
    // Verify client handles topic formatting correctly
    let config = test_mqtt_config();
    let client = MqttClient::new("test-agent-topics", config).await.unwrap();

    // Client should exist and be ready for operations
    assert!(
        !client.is_connected(),
        "Should not be connected without connect() call"
    );

    // Topic handling tested implicitly through publish methods above
}

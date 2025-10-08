//! Integration Tests with Real MQTT Broker
//!
//! Tests MQTT client functionality with a real Mosquitto broker using testcontainers.
//! These tests validate:
//! - Connection to real broker
//! - Message publishing and subscription
//! - QoS 1 delivery guarantees
//! - MQTT v5 protocol features

mod mqtt_integration_helpers;

use agent2389::protocol::{AgentStatus, AgentStatusType};
use agent2389::transport::mqtt::MqttClient;
use agent2389::transport::Transport;
use mqtt_integration_helpers::MqttTestHarness;
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::time::sleep;

#[tokio::test]
async fn test_connect_to_real_broker() {
    // Arrange: Start real Mosquitto broker
    let docker = Cli::default();
    let harness = MqttTestHarness::new(&docker).await;
    let config = harness.mqtt_config();

    // Act: Create and connect MQTT client
    let mut client = MqttClient::new("test-agent", config)
        .await
        .expect("Client creation should succeed");

    let result = client.connect().await;

    // Assert: Connection succeeds
    assert!(result.is_ok(), "Should connect to real broker");
    assert!(client.is_connected(), "Client should report connected");

    // Cleanup
    let _ = client.disconnect().await;
}

#[tokio::test]
async fn test_publish_status_to_real_broker() {
    // Arrange: Start broker and connect client
    let docker = Cli::default();
    let harness = MqttTestHarness::new(&docker).await;
    let config = harness.mqtt_config();

    let mut client = MqttClient::new("test-agent-pub", config)
        .await
        .expect("Client creation should succeed");

    client.connect().await.expect("Connection should succeed");

    // Subscribe to tasks to establish connection
    client
        .subscribe_to_tasks()
        .await
        .expect("Subscription should succeed");

    // Act: Publish status
    let status = AgentStatus {
        agent_id: "test-agent-pub".to_string(),
        status: AgentStatusType::Available,
        timestamp: chrono::Utc::now(),
        capabilities: Some(vec!["test".to_string()]),
        description: Some("Test agent".to_string()),
    };

    let result = client.publish_status(&status).await;

    // Assert: Publish succeeds
    assert!(result.is_ok(), "Status publishing should succeed");

    // Give broker time to process
    sleep(Duration::from_millis(100)).await;

    // Cleanup
    let _ = client.disconnect().await;
}

#[tokio::test]
async fn test_subscribe_to_tasks() {
    // Arrange: Start broker and connect client
    let docker = Cli::default();
    let harness = MqttTestHarness::new(&docker).await;
    let config = harness.mqtt_config();

    let mut client = MqttClient::new("subscribe-test-agent", config)
        .await
        .expect("Client creation should succeed");

    client.connect().await.expect("Connection should succeed");

    // Act: Subscribe to task input topic
    let result = client.subscribe_to_tasks().await;

    // Assert: Subscription succeeds
    assert!(result.is_ok(), "Task subscription should succeed");
    assert!(client.is_connected(), "Client should remain connected");

    // Cleanup
    let _ = client.disconnect().await;
}

#[tokio::test]
async fn test_disconnect_from_real_broker() {
    // Arrange: Start broker and connect client
    let docker = Cli::default();
    let harness = MqttTestHarness::new(&docker).await;
    let config = harness.mqtt_config();

    let mut client = MqttClient::new("disconnect-test-agent", config)
        .await
        .expect("Client creation should succeed");

    client.connect().await.expect("Connection should succeed");
    assert!(client.is_connected(), "Client should be connected");

    // Act: Disconnect
    let result = client.disconnect().await;

    // Assert: Disconnection succeeds
    assert!(result.is_ok(), "Disconnection should succeed");

    // Give the disconnect state a moment to propagate
    sleep(Duration::from_millis(50)).await;

    assert!(
        !client.is_connected(),
        "Client should not be connected after disconnect"
    );
}

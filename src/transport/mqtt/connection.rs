//! Pure connection state management for MQTT client
//!
//! This module contains pure functions for connection state management,
//! configuration handling, and topic construction.

use crate::config::MqttSection;
use crate::protocol::{AgentStatus, canonicalize_topic};
use rumqttc::Transport as RumqttcTransport;
use rumqttc::v5::mqttbytes::v5::LastWill;
use rumqttc::v5::{MqttOptions, mqttbytes::QoS};
use std::time::Duration;
use thiserror::Error;
use url::Url;

/// Connection state for MQTT client
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Initial state - attempting to connect
    Connecting,
    /// Successfully connected and ready for operations
    Connected,
    /// Disconnected with reason
    Disconnected(String),
    /// Attempting to reconnect (attempt count)
    Reconnecting(u32),
    /// Permanently disconnected - max reconnection attempts exceeded
    PermanentlyDisconnected(String),
}

/// Reconnection configuration
#[derive(Debug, Clone)]
pub struct ReconnectConfig {
    /// Maximum number of reconnection attempts (None = unlimited)
    pub max_attempts: Option<u32>,
    /// Custom backoff pattern in milliseconds (if empty, uses exponential backoff)
    pub backoff_pattern: Vec<u64>,
    /// Delay to use after pattern is exhausted (for unlimited retries)
    pub sustained_delay: u64,
}

impl Default for ReconnectConfig {
    fn default() -> Self {
        Self {
            max_attempts: None,                      // Unlimited retries by default
            backoff_pattern: vec![25, 50, 100, 250], // 25ms, 50ms, 100ms, 250ms pattern
            sustained_delay: 250,                    // Stay at 250ms after pattern exhausted
        }
    }
}

impl ReconnectConfig {
    /// Calculate the maximum total time for all reconnection attempts
    /// Returns None if unlimited retries are configured
    pub fn calculate_max_total_time(&self) -> Option<u64> {
        self.max_attempts.map(|max_attempts| {
            let mut total_time = 0u64;
            for attempt in 1..=max_attempts {
                total_time += self.calculate_backoff_delay(attempt);
            }
            total_time
        })
    }

    /// Calculate backoff delay for given attempt using custom pattern
    /// Pattern: 25ms, 50ms, 100ms, 250ms, then sustain at 250ms forever
    pub fn calculate_backoff_delay(&self, attempt: u32) -> u64 {
        if self.backoff_pattern.is_empty() {
            // Fallback to sustained delay if no pattern
            self.sustained_delay
        } else {
            let index = (attempt.saturating_sub(1)) as usize;
            if index < self.backoff_pattern.len() {
                self.backoff_pattern[index]
            } else {
                // Pattern exhausted, use sustained delay
                self.sustained_delay
            }
        }
    }
}

/// RFC-only MQTT transport errors
#[derive(Debug, Error)]
pub enum MqttError {
    #[error("Connection failed")]
    ConnectionFailed(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Publishing failed")]
    PublishFailed(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Subscription failed")]
    SubscriptionFailed(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Serialization error")]
    SerializationError(#[source] serde_json::Error),
    #[error("Invalid broker URL: {0}")]
    InvalidBrokerUrl(String),
    #[error("Not connected - current state: {state:?}")]
    NotConnected { state: ConnectionState },
    #[error("Connection failed: {0}")]
    ConnectionFailedStr(String), // Keep for backwards compatibility where we need string errors
}

/// Pure function to configure MQTT options from config
/// This eliminates duplication between new() and create_connection()
pub fn configure_mqtt_options(
    agent_id: &str,
    config: &MqttSection,
) -> Result<MqttOptions, MqttError> {
    // Parse broker URL to extract host and port
    let url = Url::parse(&config.broker_url)
        .map_err(|_| MqttError::InvalidBrokerUrl(config.broker_url.clone()))?;

    let host = url
        .host_str()
        .ok_or_else(|| MqttError::InvalidBrokerUrl(config.broker_url.clone()))?;
    let port = url
        .port()
        .unwrap_or(if url.scheme() == "mqtts" { 8883 } else { 1883 });

    // Generate unique client ID for each connection attempt to prevent broker conflicts
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let client_id = format!("agent-{agent_id}-{timestamp}");
    let mut mqtt_options = MqttOptions::new(client_id, host, port);

    // Enable TLS for mqtts:// URLs per RFC Section 11 security requirements
    if url.scheme() == "mqtts" {
        let transport = RumqttcTransport::tls_with_default_config();
        mqtt_options.set_transport(transport);
    }

    // Set authentication from environment variables per RFC Section 9
    // Use consistent environment variable handling pattern
    if let Some(username_env) = &config.username_env {
        if let Ok(username) = std::env::var(username_env) {
            let password = config
                .password_env
                .as_ref()
                .and_then(|env_name| std::env::var(env_name).ok())
                .unwrap_or_default();
            mqtt_options.set_credentials(&username, &password);
        }
    }

    // RFC requires QoS 1 - set default keep alive
    mqtt_options.set_keep_alive(Duration::from_secs(60));

    // Set max packet size to 256KB to support large LLM responses
    // Default broker limit is 10KB which is too small for typical agent responses
    // MQTT v5 expects Option<u32> for max packet size
    mqtt_options.set_max_packet_size(Some(256 * 1024));

    // Configure Last Will Testament per RFC Section 7.3
    let status_topic = canonicalize_topic(&format!("/control/agents/{agent_id}/status"));
    let unavailable_status = AgentStatus {
        agent_id: agent_id.to_string(),
        status: crate::protocol::AgentStatusType::Unavailable,
        timestamp: chrono::Utc::now(),
        capabilities: None,
        description: None,
    };
    let lwt_payload =
        serde_json::to_string(&unavailable_status).map_err(MqttError::SerializationError)?;

    // MQTT v5 LastWill takes 5 parameters: topic, payload, qos, retain, properties
    let lwt = LastWill::new(&status_topic, lwt_payload, QoS::AtLeastOnce, true, None);
    mqtt_options.set_last_will(lwt);

    Ok(mqtt_options)
}

/// RFC Section 5.1 compliant topic construction functions
pub struct TopicBuilder;

impl TopicBuilder {
    /// Build agent status topic: `/control/agents/{agent_id}/status`
    pub fn build_status_topic(agent_id: &str) -> String {
        canonicalize_topic(&format!("/control/agents/{agent_id}/status"))
    }

    /// Build target agent input topic: `/control/agents/{target}/input`
    pub fn build_target_input_topic(target_agent: &str) -> String {
        canonicalize_topic(&format!("/control/agents/{target_agent}/input"))
    }

    /// Build conversation error topic: `/conversations/{conversation_id}/{agent_id}`
    pub fn build_error_topic(conversation_id: &str, agent_id: &str) -> String {
        canonicalize_topic(&format!("/conversations/{conversation_id}/{agent_id}"))
    }

    /// Build conversation response topic: `/conversations/{conversation_id}/{agent_id}`
    /// Note: Same topic pattern as errors - responses and errors both go to conversation topics
    pub fn build_response_topic(conversation_id: &str, agent_id: &str) -> String {
        canonicalize_topic(&format!("/conversations/{conversation_id}/{agent_id}"))
    }

    /// Build agent input topic: `/control/agents/{agent_id}/input`
    pub fn build_input_topic(agent_id: &str) -> String {
        canonicalize_topic(&format!("/control/agents/{agent_id}/input"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconnect_config_default() {
        let config = ReconnectConfig::default();
        assert_eq!(config.max_attempts, None); // Unlimited by default
        assert_eq!(config.backoff_pattern, vec![25, 50, 100, 250]);
        assert_eq!(config.sustained_delay, 250);
    }

    #[test]
    fn test_calculate_max_total_time() {
        // Test with limited attempts
        let config = ReconnectConfig {
            max_attempts: Some(10),
            backoff_pattern: vec![25, 50, 100, 250],
            sustained_delay: 250,
        };
        let total_time = config.calculate_max_total_time();
        assert!(total_time.is_some());
        assert!(total_time.unwrap() > 0);

        // Test with unlimited attempts
        let unlimited_config = ReconnectConfig::default();
        assert_eq!(unlimited_config.calculate_max_total_time(), None);
    }

    #[test]
    fn test_calculate_backoff_delay() {
        let config = ReconnectConfig::default();

        // Test custom pattern: 25ms, 50ms, 100ms, 250ms
        assert_eq!(config.calculate_backoff_delay(1), 25);
        assert_eq!(config.calculate_backoff_delay(2), 50);
        assert_eq!(config.calculate_backoff_delay(3), 100);
        assert_eq!(config.calculate_backoff_delay(4), 250);

        // Test sustained delay after pattern exhausted
        assert_eq!(config.calculate_backoff_delay(5), 250);
        assert_eq!(config.calculate_backoff_delay(10), 250);
        assert_eq!(config.calculate_backoff_delay(100), 250);
    }

    #[test]
    fn test_topic_construction() {
        // Test RFC Section 5.1 topic patterns
        assert_eq!(
            TopicBuilder::build_status_topic("my-agent"),
            "/control/agents/my-agent/status"
        );
        assert_eq!(
            TopicBuilder::build_target_input_topic("other-agent"),
            "/control/agents/other-agent/input"
        );
        assert_eq!(
            TopicBuilder::build_error_topic("conv-123", "my-agent"),
            "/conversations/conv-123/my-agent"
        );
    }

    #[test]
    fn test_topic_canonicalization() {
        // RFC Section 5.2: Topics must be canonicalized
        assert_eq!(
            TopicBuilder::build_target_input_topic("//agent//"),
            "/control/agents/agent/input"
        );
        assert_eq!(
            TopicBuilder::build_error_topic("//conv//123//", "test-agent"),
            "/conversations/conv/123/test-agent"
        );
    }

    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(
            ConnectionState::Disconnected("test".to_string()),
            ConnectionState::Disconnected("test".to_string())
        );
        assert_ne!(
            ConnectionState::Connected,
            ConnectionState::Disconnected("test".to_string())
        );
    }

    // Helper to create RFC-compliant MQTT configuration
    fn test_mqtt_config() -> MqttSection {
        MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        }
    }

    #[test]
    fn test_configure_mqtt_options() {
        let config = test_mqtt_config();
        let options = configure_mqtt_options("test-agent", &config);
        assert!(options.is_ok());
    }

    #[test]
    fn test_invalid_broker_url() {
        let mut config = test_mqtt_config();
        config.broker_url = "invalid-url".to_string();

        let result = configure_mqtt_options("test-agent", &config);
        assert!(matches!(result, Err(MqttError::InvalidBrokerUrl(_))));
    }

    #[test]
    fn test_mqtt_error_display() {
        let errors = vec![
            MqttError::ConnectionFailed("test".to_string().into()),
            MqttError::PublishFailed("test".to_string().into()),
            MqttError::SubscriptionFailed("test".to_string().into()),
            MqttError::InvalidBrokerUrl("test".to_string()),
            MqttError::NotConnected {
                state: ConnectionState::Disconnected("test".to_string()),
            },
            MqttError::ConnectionFailedStr("test".to_string()),
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
        }
    }
}

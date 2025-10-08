//! MQTT Integration for Agent Discovery
//!
//! Provides MQTT-based agent discovery by subscribing to agent status messages
//! and maintaining a live registry of available agents.

use super::discovery::{AgentRegistry, AgentStatusMessage};
use crate::error::{AgentError, AgentResult};
use crate::protocol::topics::canonicalize_topic;
use rumqttc::v5::mqttbytes::v5::Packet;
use rumqttc::v5::{AsyncClient, Event, mqttbytes::QoS};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// MQTT topic pattern for agent status messages
const AGENT_STATUS_TOPIC_PATTERN: &str = "/control/agents/+/status";

/// MQTT integration for agent discovery
#[derive(Debug)]
pub struct DiscoveryMqttIntegration {
    registry: AgentRegistry,
    client: Option<Arc<tokio::sync::Mutex<AsyncClient>>>,
    // Removed unused status_receiver field to prevent resource leak
}

/// Agent status update message from MQTT
#[derive(Debug, Clone)]
pub struct AgentStatusUpdate {
    pub agent_id: String,
    pub status_message: AgentStatusMessage,
    pub topic: String,
    pub retain: bool,
}

impl DiscoveryMqttIntegration {
    /// Create new discovery integration with shared registry
    pub fn new(registry: AgentRegistry) -> Self {
        Self {
            registry,
            client: None,
        }
    }

    /// Initialize MQTT subscription for agent discovery
    pub async fn initialize_mqtt_discovery(
        &mut self,
        mqtt_client: Arc<tokio::sync::Mutex<AsyncClient>>,
    ) -> AgentResult<()> {
        self.client = Some(mqtt_client.clone());

        // Subscribe to agent status messages
        {
            let client = mqtt_client.lock().await;
            client
                .subscribe(AGENT_STATUS_TOPIC_PATTERN, QoS::AtLeastOnce)
                .await
                .map_err(|e| {
                    AgentError::internal_error(format!("MQTT subscription failed: {e}"))
                })?;
        }

        info!(
            "Subscribed to agent status messages: {}",
            AGENT_STATUS_TOPIC_PATTERN
        );
        Ok(())
    }

    /// Process MQTT event for agent discovery
    /// Updated for MQTT v5 Event types
    pub async fn process_mqtt_event(&self, event: &Event) -> AgentResult<()> {
        if let Event::Incoming(Packet::Publish(publish)) = event {
            // Check if this is a status message
            let topic = String::from_utf8_lossy(&publish.topic).to_string();
            if self.is_status_message(&topic) {
                self.handle_status_message(&topic, &publish.payload, publish.retain)
                    .await?;
            }
        }
        Ok(())
    }

    /// Handle agent status message
    async fn handle_status_message(
        &self,
        topic: &str,
        payload: &[u8],
        retain: bool,
    ) -> AgentResult<()> {
        // Extract agent_id from topic: /control/agents/{agent_id}/status
        let agent_id = match self.extract_agent_id_from_topic(topic) {
            Some(id) => id,
            None => {
                warn!("Could not extract agent_id from topic: {}", topic);
                return Ok(());
            }
        };

        // Parse status message
        let status_message: AgentStatusMessage = match serde_json::from_slice(payload) {
            Ok(msg) => msg,
            Err(e) => {
                warn!(
                    "Failed to parse agent status message from {}: {}",
                    agent_id, e
                );
                return Ok(());
            }
        };

        // Convert to AgentInfo and register
        let agent_info = status_message.to_agent_info(agent_id.clone());

        debug!(
            "Processing status update for agent '{}': health={}, load={:.3}, retain={}",
            agent_id, agent_info.health, agent_info.load, retain
        );

        // Register agent (handles both new and updates)
        self.registry.register_agent(agent_info);

        // If this is a retained message, it's part of warm-up
        if retain {
            debug!("Processed retained status message for warm registry initialization");
        }

        Ok(())
    }

    /// Check if topic is a status message topic
    fn is_status_message(&self, topic: &str) -> bool {
        // Match pattern /control/agents/{agent_id}/status
        let canonical_topic = canonicalize_topic(topic);
        let parts: Vec<&str> = canonical_topic.trim_start_matches('/').split('/').collect();

        parts.len() == 4 && parts[0] == "control" && parts[1] == "agents" && parts[3] == "status"
    }

    /// Extract agent_id from status topic
    fn extract_agent_id_from_topic(&self, topic: &str) -> Option<String> {
        let canonical_topic = canonicalize_topic(topic);
        let parts: Vec<&str> = canonical_topic.trim_start_matches('/').split('/').collect();

        if parts.len() == 4 && parts[0] == "control" && parts[1] == "agents" && parts[3] == "status"
        {
            Some(parts[2].to_string())
        } else {
            None
        }
    }

    /// Get reference to the agent registry
    pub fn registry(&self) -> &AgentRegistry {
        &self.registry
    }

    /// Get agent count for monitoring
    pub fn get_discovery_stats(&self) -> DiscoveryStats {
        DiscoveryStats {
            total_agents: self.registry.agent_count(),
            healthy_agents: self.registry.healthy_agent_count(),
            agent_ids: self.registry.get_all_agent_ids(),
        }
    }

    /// Clean up MQTT resources
    pub async fn cleanup(&mut self) -> AgentResult<()> {
        if let Some(client) = &self.client {
            // Unsubscribe from agent status messages
            let mqtt_client = client.lock().await;
            if let Err(e) = mqtt_client.unsubscribe(AGENT_STATUS_TOPIC_PATTERN).await {
                warn!("Failed to unsubscribe from agent status messages: {}", e);
            }
            info!(
                "Unsubscribed from agent status messages: {}",
                AGENT_STATUS_TOPIC_PATTERN
            );
        }
        self.client = None;
        Ok(())
    }
}

/// Statistics about discovered agents
#[derive(Debug, Clone)]
pub struct DiscoveryStats {
    pub total_agents: usize,
    pub healthy_agents: usize,
    pub agent_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::discovery::AgentInfo;

    #[test]
    fn test_topic_pattern_matching() {
        let integration = DiscoveryMqttIntegration::new(AgentRegistry::new());

        // Valid status message topics
        assert!(integration.is_status_message("/control/agents/agent-001/status"));
        assert!(integration.is_status_message("/control/agents/email-processor/status"));

        // Invalid topics
        assert!(!integration.is_status_message("/control/agents/agent-001/input"));
        assert!(!integration.is_status_message("/conversations/test/agent-001"));
        assert!(!integration.is_status_message("/control/status"));
    }

    #[test]
    fn test_agent_id_extraction() {
        let integration = DiscoveryMqttIntegration::new(AgentRegistry::new());

        assert_eq!(
            integration.extract_agent_id_from_topic("/control/agents/my-agent/status"),
            Some("my-agent".to_string())
        );

        assert_eq!(
            integration.extract_agent_id_from_topic("/control/agents/email-processor/status"),
            Some("email-processor".to_string())
        );

        // Invalid topics
        assert_eq!(
            integration.extract_agent_id_from_topic("/control/agents/my-agent/input"),
            None
        );

        assert_eq!(
            integration.extract_agent_id_from_topic("/invalid/topic"),
            None
        );
    }

    #[tokio::test]
    async fn test_status_message_processing() {
        let integration = DiscoveryMqttIntegration::new(AgentRegistry::new());

        // Create a test status message
        let status_msg = AgentStatusMessage {
            health: "ok".to_string(),
            load: 0.3,
            last_updated: "2024-01-01T12:00:00Z".to_string(),
            description: Some("Test agent for email processing".to_string()),
            capabilities: Some(vec!["email".to_string()]),
            handles: Some(vec!["mail".to_string()]),
            metadata: None,
        };

        let payload = serde_json::to_vec(&status_msg).unwrap();

        // Process the status message
        integration
            .handle_status_message("/control/agents/email-agent/status", &payload, false)
            .await
            .unwrap();

        // Check that agent was registered
        let agent_info = integration.registry.get_agent("email-agent").unwrap();
        assert_eq!(agent_info.agent_id, "email-agent");
        assert_eq!(agent_info.health, "ok");
        assert_eq!(agent_info.load, 0.3);
        assert!(agent_info.can_handle("email"));
        assert!(agent_info.can_handle("mail"));
    }

    #[tokio::test]
    async fn test_invalid_status_message() {
        let integration = DiscoveryMqttIntegration::new(AgentRegistry::new());

        // Invalid JSON payload
        let invalid_payload = b"not json";

        // Should handle gracefully without panicking
        let result = integration
            .handle_status_message("/control/agents/test-agent/status", invalid_payload, false)
            .await;

        assert!(result.is_ok());

        // Agent should not be registered
        assert!(integration.registry.get_agent("test-agent").is_none());
    }

    #[tokio::test]
    async fn test_discovery_stats() {
        let registry = AgentRegistry::new();
        let integration = DiscoveryMqttIntegration::new(registry.clone());

        // Register some agents
        registry.register_agent(AgentInfo::new("agent1".to_string(), "ok".to_string(), 0.2));
        registry.register_agent(AgentInfo::new(
            "agent2".to_string(),
            "error".to_string(),
            0.8,
        ));
        registry.register_agent(AgentInfo::new("agent3".to_string(), "ok".to_string(), 0.5));

        let stats = integration.get_discovery_stats();

        assert_eq!(stats.total_agents, 3);
        assert_eq!(stats.healthy_agents, 2); // only "ok" agents
        assert_eq!(stats.agent_ids.len(), 3);
        assert!(stats.agent_ids.contains(&"agent1".to_string()));
        assert!(stats.agent_ids.contains(&"agent2".to_string()));
        assert!(stats.agent_ids.contains(&"agent3".to_string()));
    }

    #[test]
    fn test_topic_canonicalization() {
        let integration = DiscoveryMqttIntegration::new(AgentRegistry::new());

        // Test various topic formats
        assert!(integration.is_status_message("//control//agents//test//status//"));
        assert!(integration.is_status_message("/control/agents/test/status"));

        // Extract agent ID with different formats
        assert_eq!(
            integration.extract_agent_id_from_topic("//control//agents//test-agent//status//"),
            Some("test-agent".to_string())
        );
    }
}

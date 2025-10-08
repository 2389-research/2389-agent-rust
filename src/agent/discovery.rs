//! Agent Discovery System
//!
//! Provides dynamic agent discovery and capability matching through MQTT status messages.
//! Implements a thread-safe registry with TTL-based cleanup and load-aware agent selection.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime};
use tracing::{debug, info};

/// TTL for agent entries in the registry (15 seconds as per POC spec)
const AGENT_TTL_SECONDS: u64 = 15;

/// Information about a discovered agent
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentInfo {
    /// Agent identifier
    pub agent_id: String,
    /// Health status (required for POC)
    pub health: String,
    /// Current load factor 0.0-1.0 (required for POC)
    pub load: f64,
    /// Last update timestamp (ISO 8601)
    pub last_updated: String,
    /// Optional agent description
    pub description: Option<String>,
    /// Optional capabilities list
    pub capabilities: Option<Vec<String>>,
    /// Optional handles/keywords
    pub handles: Option<Vec<String>>,
    /// Optional metadata
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl AgentInfo {
    /// Create a new AgentInfo with minimal required fields
    pub fn new(agent_id: String, health: String, load: f64) -> Self {
        Self {
            agent_id,
            health,
            load,
            last_updated: Utc::now().to_rfc3339(),
            description: None,
            capabilities: None,
            handles: None,
            metadata: None,
        }
    }

    /// Check if agent is healthy and available
    pub fn is_healthy(&self) -> bool {
        self.health.to_lowercase() == "ok"
    }

    /// Check if agent is expired based on TTL
    pub fn is_expired(&self) -> bool {
        if let Ok(last_update) = DateTime::parse_from_rfc3339(&self.last_updated) {
            let age = Utc::now().signed_duration_since(last_update);
            age.num_seconds() > AGENT_TTL_SECONDS as i64
        } else {
            // If timestamp can't be parsed, consider it expired
            true
        }
    }

    /// Update timestamp to current time
    pub fn refresh_timestamp(&mut self) {
        self.last_updated = Utc::now().to_rfc3339();
    }

    /// Check if agent can handle a given capability (case-insensitive)
    pub fn can_handle(&self, capability: &str) -> bool {
        let capability_lower = capability.to_lowercase();

        // Check capabilities array
        if let Some(ref capabilities) = self.capabilities {
            if capabilities
                .iter()
                .any(|c| c.to_lowercase() == capability_lower)
            {
                return true;
            }
        }

        // Check handles array
        if let Some(ref handles) = self.handles {
            if handles.iter().any(|h| h.to_lowercase() == capability_lower) {
                return true;
            }
        }

        false
    }

    /// Builder method to set capabilities for fluent construction
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = Some(capabilities);
        self
    }
}

/// Thread-safe registry of discovered agents
#[derive(Debug, Clone)]
pub struct AgentRegistry {
    /// Map of agent_id to AgentInfo
    agents: Arc<RwLock<HashMap<String, AgentInfo>>>,
    /// Last cleanup time for TTL enforcement
    last_cleanup: Arc<RwLock<SystemTime>>,
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentRegistry {
    /// Create a new empty agent registry
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            last_cleanup: Arc::new(RwLock::new(SystemTime::now())),
        }
    }

    /// Register or update an agent in the registry
    pub fn register_agent(&self, mut agent_info: AgentInfo) {
        agent_info.refresh_timestamp();
        let agent_id = agent_info.agent_id.clone();

        {
            let mut agents = self.agents.write().unwrap();
            let is_new = !agents.contains_key(&agent_id);
            agents.insert(agent_id.clone(), agent_info);

            if is_new {
                info!("Registered new agent: {}", agent_id);
            } else {
                debug!("Updated agent info: {}", agent_id);
            }
        }

        // Trigger cleanup periodically
        self.cleanup_expired_agents();
    }

    /// Get agent information by ID
    pub fn get_agent(&self, agent_id: &str) -> Option<AgentInfo> {
        let agents = self.agents.read().unwrap();
        agents.get(agent_id).cloned()
    }

    /// Get all healthy agents
    pub fn get_healthy_agents(&self) -> Vec<AgentInfo> {
        let agents = self.agents.read().unwrap();
        agents
            .values()
            .filter(|agent| agent.is_healthy() && !agent.is_expired())
            .cloned()
            .collect()
    }

    /// Find agents that can handle a specific capability
    pub fn find_agents_with_capability(&self, capability: &str) -> Vec<AgentInfo> {
        self.get_healthy_agents()
            .into_iter()
            .filter(|agent| agent.can_handle(capability))
            .collect()
    }

    /// Find the best agent for a capability (lowest load, tie-break by agent_id)
    pub fn find_best_agent(&self, capability: &str) -> Option<AgentInfo> {
        let mut candidates = self.find_agents_with_capability(capability);

        if candidates.is_empty() {
            debug!("No agents found for capability: {}", capability);
            return None;
        }

        // Sort by load (ascending), then by agent_id for deterministic tie-breaking
        candidates.sort_by(|a, b| {
            a.load
                .partial_cmp(&b.load)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.agent_id.cmp(&b.agent_id))
        });

        let best_agent = candidates.into_iter().next()?;
        info!(
            "Selected agent '{}' for capability '{}' (load: {:.3})",
            best_agent.agent_id, capability, best_agent.load
        );

        Some(best_agent)
    }

    /// Get count of registered agents
    pub fn agent_count(&self) -> usize {
        let agents = self.agents.read().unwrap();
        agents.len()
    }

    /// Get count of healthy agents
    pub fn healthy_agent_count(&self) -> usize {
        self.get_healthy_agents().len()
    }

    /// Remove expired agents from registry (TTL cleanup)
    pub fn cleanup_expired_agents(&self) {
        let now = SystemTime::now();

        // Check and update cleanup timestamp atomically to prevent races
        let should_cleanup = {
            let mut last_cleanup = self.last_cleanup.write().unwrap();
            let time_since_last = now
                .duration_since(*last_cleanup)
                .unwrap_or(Duration::from_secs(0));

            if time_since_last >= Duration::from_secs(5) {
                *last_cleanup = now; // Update timestamp immediately
                true
            } else {
                false
            }
        }; // Release write lock on last_cleanup immediately

        if !should_cleanup {
            return;
        }

        // Perform cleanup with minimal lock time
        let (initial_count, removed_count) = {
            let mut agents = self.agents.write().unwrap();
            let initial_count = agents.len();
            let mut removed_count = 0;

            agents.retain(|agent_id, agent_info| {
                if agent_info.is_expired() {
                    debug!("Removing expired agent: {}", agent_id);
                    removed_count += 1;
                    false
                } else {
                    true
                }
            });

            (initial_count, removed_count)
        }; // Release write lock on agents immediately

        if removed_count > 0 {
            info!(
                "Cleaned up {} expired agents ({} -> {})",
                removed_count,
                initial_count,
                initial_count - removed_count
            );
        }
    }

    /// Remove all agents (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        let mut agents = self.agents.write().unwrap();
        agents.clear();
    }

    /// Register agent without refreshing timestamp (for testing TTL expiration only)
    ///
    /// WARNING: This method bypasses the normal timestamp refresh behavior and should
    /// ONLY be used in tests to verify TTL expiration logic. In production code, always
    /// use `register_agent()` which properly maintains timestamps.
    #[doc(hidden)]
    pub fn register_agent_without_refresh(&self, agent_info: AgentInfo) {
        let agent_id = agent_info.agent_id.clone();
        let mut agents = self.agents.write().unwrap();
        agents.insert(agent_id, agent_info);
    }

    /// Force cleanup of expired agents (for testing, bypasses rate limit)
    ///
    /// WARNING: This method bypasses the normal 5-second rate limit on cleanup
    /// and should ONLY be used in tests. In production code, use `cleanup_expired_agents()`
    /// which includes proper rate limiting.
    #[doc(hidden)]
    pub fn force_cleanup_for_test(&self) {
        let (initial_count, removed_count) = {
            let mut agents = self.agents.write().unwrap();
            let initial_count = agents.len();
            let mut removed_count = 0;

            agents.retain(|agent_id, agent_info| {
                if agent_info.is_expired() {
                    debug!("Removing expired agent: {}", agent_id);
                    removed_count += 1;
                    false
                } else {
                    true
                }
            });

            (initial_count, removed_count)
        };

        if removed_count > 0 {
            info!(
                "Cleaned up {} expired agents ({} -> {})",
                removed_count,
                initial_count,
                initial_count - removed_count
            );
        }
    }

    /// Get all agent IDs (for debugging)
    pub fn get_all_agent_ids(&self) -> Vec<String> {
        let agents = self.agents.read().unwrap();
        agents.keys().cloned().collect()
    }
}

/// Agent status message format for MQTT discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatusMessage {
    /// Required health field
    pub health: String,
    /// Required load field
    pub load: f64,
    /// Required last_updated field
    pub last_updated: String,
    /// Optional agent description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Vec<String>>,
    /// Optional handles
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handles: Option<Vec<String>>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

impl AgentStatusMessage {
    /// Convert to AgentInfo with agent_id
    pub fn to_agent_info(&self, agent_id: String) -> AgentInfo {
        AgentInfo {
            agent_id,
            health: self.health.clone(),
            load: self.load,
            last_updated: self.last_updated.clone(),
            description: self.description.clone(),
            capabilities: self.capabilities.clone(),
            handles: self.handles.clone(),
            metadata: self.metadata.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_agent_info_creation() {
        let agent = AgentInfo::new("test-agent".to_string(), "ok".to_string(), 0.5);

        assert_eq!(agent.agent_id, "test-agent");
        assert_eq!(agent.health, "ok");
        assert_eq!(agent.load, 0.5);
        assert!(agent.is_healthy());
        assert!(!agent.is_expired()); // Should not be expired immediately
    }

    #[test]
    fn test_agent_capability_matching() {
        let mut agent = AgentInfo::new("test-agent".to_string(), "ok".to_string(), 0.3);
        agent.capabilities = Some(vec!["email".to_string(), "calendar".to_string()]);
        agent.handles = Some(vec!["mail".to_string(), "scheduling".to_string()]);

        // Test capabilities (case-insensitive)
        assert!(agent.can_handle("email"));
        assert!(agent.can_handle("EMAIL"));
        assert!(agent.can_handle("calendar"));

        // Test handles (case-insensitive)
        assert!(agent.can_handle("mail"));
        assert!(agent.can_handle("MAIL"));
        assert!(agent.can_handle("scheduling"));

        // Test non-matching
        assert!(!agent.can_handle("database"));
        assert!(!agent.can_handle("unknown"));
    }

    #[test]
    fn test_agent_registry_registration() {
        let registry = AgentRegistry::new();
        let agent = AgentInfo::new("agent1".to_string(), "ok".to_string(), 0.2);

        assert_eq!(registry.agent_count(), 0);

        registry.register_agent(agent.clone());

        assert_eq!(registry.agent_count(), 1);
        assert_eq!(registry.healthy_agent_count(), 1);

        let retrieved = registry.get_agent("agent1").unwrap();
        assert_eq!(retrieved.agent_id, "agent1");
        assert_eq!(retrieved.health, "ok");
        assert_eq!(retrieved.load, 0.2);
    }

    #[test]
    fn test_agent_selection_by_load() {
        let registry = AgentRegistry::new();

        // Register agents with different loads
        let mut agent1 = AgentInfo::new("agent1".to_string(), "ok".to_string(), 0.8);
        agent1.capabilities = Some(vec!["email".to_string()]);

        let mut agent2 = AgentInfo::new("agent2".to_string(), "ok".to_string(), 0.2);
        agent2.capabilities = Some(vec!["email".to_string()]);

        let mut agent3 = AgentInfo::new("agent3".to_string(), "ok".to_string(), 0.5);
        agent3.capabilities = Some(vec!["email".to_string()]);

        registry.register_agent(agent1);
        registry.register_agent(agent2);
        registry.register_agent(agent3);

        // Should select agent2 (lowest load: 0.2)
        let best_agent = registry.find_best_agent("email").unwrap();
        assert_eq!(best_agent.agent_id, "agent2");
        assert_eq!(best_agent.load, 0.2);
    }

    #[test]
    fn test_agent_tie_breaking_by_id() {
        let registry = AgentRegistry::new();

        // Register agents with same load (should tie-break by agent_id)
        let mut agent_z = AgentInfo::new("z-agent".to_string(), "ok".to_string(), 0.5);
        agent_z.capabilities = Some(vec!["test".to_string()]);

        let mut agent_a = AgentInfo::new("a-agent".to_string(), "ok".to_string(), 0.5);
        agent_a.capabilities = Some(vec!["test".to_string()]);

        registry.register_agent(agent_z);
        registry.register_agent(agent_a);

        // Should select a-agent (alphabetically first)
        let best_agent = registry.find_best_agent("test").unwrap();
        assert_eq!(best_agent.agent_id, "a-agent");
    }

    #[test]
    fn test_agent_unhealthy_filtering() {
        let registry = AgentRegistry::new();

        let mut healthy_agent = AgentInfo::new("healthy".to_string(), "ok".to_string(), 0.3);
        healthy_agent.capabilities = Some(vec!["test".to_string()]);

        let mut unhealthy_agent = AgentInfo::new("unhealthy".to_string(), "error".to_string(), 0.1);
        unhealthy_agent.capabilities = Some(vec!["test".to_string()]);

        registry.register_agent(healthy_agent);
        registry.register_agent(unhealthy_agent);

        assert_eq!(registry.agent_count(), 2);
        assert_eq!(registry.healthy_agent_count(), 1);

        // Should only find healthy agent
        let candidates = registry.find_agents_with_capability("test");
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].agent_id, "healthy");

        // Best agent should be the healthy one despite higher load
        let best_agent = registry.find_best_agent("test").unwrap();
        assert_eq!(best_agent.agent_id, "healthy");
    }

    #[test]
    fn test_agent_status_message_conversion() {
        let status_msg = AgentStatusMessage {
            health: "ok".to_string(),
            load: 0.4,
            last_updated: "2024-01-01T12:00:00Z".to_string(),
            description: Some("Test email processing agent".to_string()),
            capabilities: Some(vec!["email".to_string(), "calendar".to_string()]),
            handles: Some(vec!["mail".to_string()]),
            metadata: Some({
                let mut map = HashMap::new();
                map.insert("version".to_string(), json!("1.0"));
                map
            }),
        };

        let agent_info = status_msg.to_agent_info("test-agent".to_string());

        assert_eq!(agent_info.agent_id, "test-agent");
        assert_eq!(agent_info.health, "ok");
        assert_eq!(agent_info.load, 0.4);
        assert_eq!(agent_info.last_updated, "2024-01-01T12:00:00Z");
        assert_eq!(agent_info.capabilities.unwrap(), vec!["email", "calendar"]);
        assert_eq!(agent_info.handles.unwrap(), vec!["mail"]);
        assert!(agent_info.metadata.is_some());
    }

    #[test]
    fn test_no_agents_for_capability() {
        let registry = AgentRegistry::new();

        let mut agent = AgentInfo::new("agent1".to_string(), "ok".to_string(), 0.2);
        agent.capabilities = Some(vec!["email".to_string()]);

        registry.register_agent(agent);

        // Should find nothing for non-existent capability
        assert!(registry.find_best_agent("database").is_none());

        let candidates = registry.find_agents_with_capability("database");
        assert!(candidates.is_empty());
    }
}

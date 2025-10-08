//! Agent Discovery and Selection Utilities
//!
//! Provides agent discovery and selection helpers using the AgentRegistry.
//! This is for finding available agents by capability or ID, NOT for workflow
//! routing decisions (see router.rs for V2 routing architecture).

use crate::agent::discovery::{AgentInfo, AgentRegistry};
use tracing::{debug, info, warn};

/// Agent selection decision result
///
/// Note: This is for agent DISCOVERY, not workflow routing.
/// For V2 workflow routing decisions, see `router::RoutingDecision`.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentSelectionDecision {
    /// Route to specific agent
    RouteToAgent {
        agent: Box<AgentInfo>,
        reason: String,
    },
    /// No routing available
    NoRoute { reason: String },
}

/// Simple routing helper
#[derive(Debug, Clone)]
pub struct RoutingHelper {
    // No state needed for now
}

impl Default for RoutingHelper {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutingHelper {
    /// Create a new routing helper
    pub fn new() -> Self {
        Self {}
    }

    /// Find best agent with a specific capability
    pub fn find_best_agent_for_capability(
        &self,
        capability: &str,
        registry: &AgentRegistry,
    ) -> AgentSelectionDecision {
        debug!("Finding best agent for capability: {}", capability);

        match registry.find_best_agent(capability) {
            Some(agent) => {
                let reason = format!(
                    "Selected best agent for capability '{}' (load: {:.3})",
                    capability, agent.load
                );

                info!(
                    "Found agent '{}' for capability '{}' with load {:.3}",
                    agent.agent_id, capability, agent.load
                );

                AgentSelectionDecision::RouteToAgent {
                    agent: Box::new(agent),
                    reason,
                }
            }
            None => {
                let reason = format!("No healthy agents found for capability '{capability}'");
                warn!("{}", reason);
                AgentSelectionDecision::NoRoute { reason }
            }
        }
    }

    /// Find agent by ID
    pub fn find_agent_by_id(
        &self,
        agent_id: &str,
        registry: &AgentRegistry,
    ) -> AgentSelectionDecision {
        debug!("Looking for agent with ID: {}", agent_id);

        if let Some(agent) = registry.get_agent(agent_id) {
            if agent.is_healthy() && !agent.is_expired() {
                let reason = format!("Found healthy agent '{agent_id}'");
                info!("{}", reason);

                AgentSelectionDecision::RouteToAgent {
                    agent: Box::new(agent),
                    reason,
                }
            } else {
                let reason = format!("Agent '{agent_id}' is unhealthy or expired");
                warn!("{}", reason);
                AgentSelectionDecision::NoRoute { reason }
            }
        } else {
            let reason = format!("Agent '{agent_id}' not found in registry");
            warn!("{}", reason);
            AgentSelectionDecision::NoRoute { reason }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::discovery::AgentInfo;

    fn create_test_registry() -> AgentRegistry {
        let registry = AgentRegistry::new();

        // Register test agents
        let mut email_agent = AgentInfo::new("email-processor".to_string(), "ok".to_string(), 0.5);
        email_agent.capabilities = Some(vec!["email".to_string()]);

        let mut calendar_agent =
            AgentInfo::new("calendar-processor".to_string(), "ok".to_string(), 0.2);
        calendar_agent.capabilities = Some(vec!["calendar".to_string(), "scheduling".to_string()]);

        let mut unhealthy_agent =
            AgentInfo::new("unhealthy-agent".to_string(), "error".to_string(), 0.1);
        unhealthy_agent.capabilities = Some(vec!["email".to_string()]);

        registry.register_agent(email_agent);
        registry.register_agent(calendar_agent);
        registry.register_agent(unhealthy_agent);

        registry
    }

    #[test]
    fn test_capability_based_routing() {
        let helper = RoutingHelper::new();
        let registry = create_test_registry();

        let decision = helper.find_best_agent_for_capability("calendar", &registry);

        match decision {
            AgentSelectionDecision::RouteToAgent { agent, .. } => {
                assert_eq!(agent.agent_id, "calendar-processor");
            }
            _ => panic!("Expected RouteToAgent decision"),
        }
    }

    #[test]
    fn test_capability_not_found() {
        let helper = RoutingHelper::new();
        let registry = create_test_registry();

        let decision = helper.find_best_agent_for_capability("nonexistent", &registry);
        assert!(matches!(decision, AgentSelectionDecision::NoRoute { .. }));
    }

    #[test]
    fn test_find_agent_by_id() {
        let helper = RoutingHelper::new();
        let registry = create_test_registry();

        let decision = helper.find_agent_by_id("email-processor", &registry);

        match decision {
            AgentSelectionDecision::RouteToAgent { agent, .. } => {
                assert_eq!(agent.agent_id, "email-processor");
            }
            _ => panic!("Expected RouteToAgent decision"),
        }
    }

    #[test]
    fn test_unhealthy_agent_not_selected() {
        let helper = RoutingHelper::new();
        let registry = create_test_registry();

        let decision = helper.find_agent_by_id("unhealthy-agent", &registry);
        assert!(matches!(decision, AgentSelectionDecision::NoRoute { .. }));
    }
}

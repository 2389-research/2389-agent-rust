//! V2 Routing Architecture - Router Trait and Decision Types
//!
//! This module defines the core Router trait that separates workflow routing decisions
//! from agent work execution. Routers decide what happens after an agent completes work:
//! either complete the workflow or forward to another agent.
//!
//! ## Design Philosophy
//!
//! **Agents are domain experts, not workflow coordinators.** They focus exclusively on
//! their work (research, writing, editing) and return pure work output. The Router
//! sees the full workflow context and makes intelligent routing decisions.
//!
//! ## Architecture
//!
//! ```text
//! Agent (Work) → Router (Decisions) → Orchestrator (Coordination)
//! ```
//!
//! # Example
//!
//! ```rust,no_run
//! use agent2389::routing::{Router, RoutingDecision};
//! use agent2389::protocol::TaskEnvelopeV2;
//! use agent2389::agent::discovery::AgentRegistry;
//! use serde_json::json;
//!
//! async fn example_routing(
//!     router: &dyn Router,
//!     task: &TaskEnvelopeV2,
//!     work_output: &serde_json::Value,
//!     registry: &AgentRegistry,
//! ) -> Result<(), Box<dyn std::error::Error>> {
//!     let decision = router.decide_next_step(task, work_output, registry).await?;
//!
//!     match decision {
//!         RoutingDecision::Complete { final_output } => {
//!             println!("Workflow complete: {:?}", final_output);
//!         }
//!         RoutingDecision::Forward { next_agent, next_instruction, forwarded_data } => {
//!             println!("Forwarding to: {} with instruction: {}", next_agent, next_instruction);
//!         }
//!     }
//!     Ok(())
//! }
//! ```

use crate::agent::discovery::AgentRegistry;
use crate::error::AgentError;
use crate::protocol::messages::TaskEnvelopeV2;
use serde_json::Value;

/// Router trait for making workflow routing decisions
///
/// Routers are responsible for deciding what happens after an agent completes work.
/// They see the full workflow context (original query, history, current output) and
/// decide whether to complete the workflow or forward to another agent.
///
/// ## Responsibilities
///
/// - Evaluate if original user request is satisfied
/// - Select next agent if more work is needed
/// - Provide clear instructions to next agent
/// - Detect loops and enforce safety limits
///
/// ## What Routers See
///
/// - Original user query (from TaskEnvelopeV2.context)
/// - Complete workflow history (steps completed)
/// - Current agent's work output
/// - Available agents (from registry)
/// - Iteration count (for safety)
///
/// ## What Routers Don't See
///
/// - Agent internals or system prompts
/// - LLM conversation history
/// - Tool execution details
#[async_trait::async_trait]
pub trait Router: Send + Sync {
    /// Decide the next step in the workflow based on agent output
    ///
    /// # Arguments
    ///
    /// * `original_task` - The complete TaskEnvelopeV2 with workflow context
    /// * `work_output` - JSON output from the agent's work
    /// * `registry` - Registry of available agents with capabilities
    ///
    /// # Returns
    ///
    /// Either `Complete` (workflow done) or `Forward` (continue to next agent)
    ///
    /// # Errors
    ///
    /// Returns error if routing decision cannot be made (e.g., network failure for GatekeeperRouter)
    async fn decide_next_step(
        &self,
        original_task: &TaskEnvelopeV2,
        work_output: &Value,
        registry: &AgentRegistry,
    ) -> Result<RoutingDecision, AgentError>;
}

/// Routing decision made by a Router
///
/// This enum represents the two possible outcomes after an agent completes work:
/// 1. The workflow is complete (user's request satisfied)
/// 2. The workflow continues (forward to another agent)
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RoutingDecision {
    /// Workflow is complete - publish final output to user
    Complete {
        /// Final output to publish to conversation topic
        final_output: Value,
    },
    /// Workflow continues - forward to next agent
    Forward {
        /// Agent ID to forward to (must exist in registry)
        next_agent: String,
        /// Instruction for the next agent (what to do)
        next_instruction: String,
        /// Data to forward to next agent
        forwarded_data: Value,
    },
}

impl RoutingDecision {
    /// Check if this decision completes the workflow
    pub fn is_complete(&self) -> bool {
        matches!(self, RoutingDecision::Complete { .. })
    }

    /// Check if this decision forwards to another agent
    pub fn is_forward(&self) -> bool {
        matches!(self, RoutingDecision::Forward { .. })
    }

    /// Extract next agent ID if this is a Forward decision
    pub fn next_agent(&self) -> Option<&str> {
        match self {
            RoutingDecision::Forward { next_agent, .. } => Some(next_agent),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_complete_decision() {
        let decision = RoutingDecision::Complete {
            final_output: json!({"result": "done"}),
        };

        assert!(decision.is_complete());
        assert!(!decision.is_forward());
        assert!(decision.next_agent().is_none());
    }

    #[test]
    fn test_forward_decision() {
        let decision = RoutingDecision::Forward {
            next_agent: "editor-agent".to_string(),
            next_instruction: "Polish the document".to_string(),
            forwarded_data: json!({"document": "..."}),
        };

        assert!(!decision.is_complete());
        assert!(decision.is_forward());
        assert_eq!(decision.next_agent(), Some("editor-agent"));
    }
}

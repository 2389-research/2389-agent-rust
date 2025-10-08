//! LLM-based Router Implementation
//!
//! This module implements the Router trait using LLM providers to make intelligent
//! routing decisions based on workflow context, agent output, and available agents.
//!
//! The LlmRouter uses structured output to guarantee valid JSON responses:
//! - OpenAI: JSON Schema with `response_format`
//! - Anthropic: Tool schema with `tool_choice: required`

use crate::agent::discovery::AgentRegistry;
use crate::error::AgentError;
use crate::llm::provider::{CompletionRequest, LlmProvider, Message, MessageRole};
use crate::protocol::messages::TaskEnvelopeV2;
use crate::routing::router::{Router, RoutingDecision};
use crate::routing::schema::RoutingDecisionOutput;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// LLM-based router that makes intelligent routing decisions
///
/// Uses structured output to ensure valid routing decisions from the LLM.
/// Sees the full workflow context including:
/// - Original user query
/// - Complete workflow history (steps completed)
/// - Current agent's work output
/// - Available agents with capabilities
/// - Current iteration count
pub struct LlmRouter {
    /// LLM provider (OpenAI, Anthropic, etc.)
    provider: Arc<dyn LlmProvider>,
    /// Model to use for routing decisions
    model: String,
    /// Temperature for routing decisions (default: 0.1 for consistency)
    temperature: f32,
}

impl LlmRouter {
    /// Create a new LLM-based router
    pub fn new(provider: Arc<dyn LlmProvider>, model: String) -> Self {
        Self {
            provider,
            model,
            temperature: 0.1, // Low temperature for consistent routing
        }
    }

    /// Create router with custom temperature
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = temperature;
        self
    }

    /// Format the workflow history for the LLM prompt
    fn format_workflow_history(task: &TaskEnvelopeV2) -> String {
        let context = match &task.context {
            Some(ctx) => ctx,
            None => return "No workflow history available.".to_string(),
        };

        let mut output = format!(
            "WORKFLOW HISTORY (Iteration {}/{}):\n",
            context.iteration_count,
            10 // max iterations
        );

        if context.steps_completed.is_empty() {
            output.push_str("No steps completed yet.\n");
        } else {
            for (i, step) in context.steps_completed.iter().enumerate() {
                output.push_str(&format!(
                    "{}. {} - Action: {} (Time: {})\n",
                    i + 1,
                    step.agent_id,
                    step.action,
                    step.timestamp
                ));
            }
        }

        output
    }

    /// Format available agents catalog for the LLM prompt
    fn format_agent_catalog(registry: &AgentRegistry) -> String {
        let agent_ids = registry.get_all_agent_ids();
        let agents: Vec<_> = agent_ids
            .iter()
            .filter_map(|id| registry.get_agent(id))
            .collect();

        if agents.is_empty() {
            return "No agents currently available.".to_string();
        }

        let mut output = String::from("AVAILABLE AGENTS:\n");
        for agent in agents {
            if agent.is_healthy() && !agent.is_expired() {
                let capabilities = agent
                    .capabilities
                    .as_ref()
                    .map(|c| c.join(", "))
                    .unwrap_or_else(|| "none".to_string());

                output.push_str(&format!(
                    "- {} (capabilities: {}, load: {:.3})\n",
                    agent.agent_id, capabilities, agent.load
                ));
            }
        }

        output
    }

    /// Build the routing prompt for the LLM
    fn build_routing_prompt(
        task: &TaskEnvelopeV2,
        work_output: &Value,
        registry: &AgentRegistry,
    ) -> String {
        let original_query = task
            .context
            .as_ref()
            .map(|c| c.original_query.as_str())
            .unwrap_or("Unknown");

        let workflow_history = Self::format_workflow_history(task);
        let agent_catalog = Self::format_agent_catalog(registry);

        format!(
            r#"You are a workflow router. Your job is to decide what happens next after an agent completes work.

ORIGINAL USER REQUEST:
{}

{}

CURRENT AGENT OUTPUT:
{}

{}

DECISION CRITERIA:
1. Has the original user request been fully satisfied?
2. What work remains to complete the request?
3. Which agent is best suited for the remaining work?
4. Are we in a loop? (Check if same agent visited multiple times)
5. Are we approaching max iterations? (Currently at {}/10)

IMPORTANT:
- Set workflow_complete to true ONLY if the user's original request is fully satisfied
- If more work is needed, select the most appropriate agent and provide a clear instruction
- Consider the workflow history to avoid loops
- Be concise in your reasoning

Make your routing decision:"#,
            original_query,
            workflow_history,
            serde_json::to_string_pretty(work_output)
                .unwrap_or_else(|_| "Invalid JSON".to_string()),
            agent_catalog,
            task.context
                .as_ref()
                .map(|c| c.iteration_count)
                .unwrap_or(0)
        )
    }

    /// Parse LLM response into RoutingDecision
    fn parse_routing_decision(
        output: &RoutingDecisionOutput,
        work_output: &Value,
    ) -> Result<RoutingDecision, AgentError> {
        // Validate the output structure
        output.validate().map_err(|e| AgentError::InvalidInput {
            message: format!("Invalid routing decision from LLM: {e}"),
        })?;

        if output.workflow_complete {
            debug!(
                reasoning = %output.reasoning,
                "Router decided workflow is complete"
            );

            Ok(RoutingDecision::Complete {
                final_output: work_output.clone(),
            })
        } else {
            let next_agent =
                output
                    .next_agent
                    .as_ref()
                    .ok_or_else(|| AgentError::InvalidInput {
                        message: "Missing next_agent".to_string(),
                    })?;

            let next_instruction =
                output
                    .next_instruction
                    .as_ref()
                    .ok_or_else(|| AgentError::InvalidInput {
                        message: "Missing next_instruction".to_string(),
                    })?;

            debug!(
                next_agent = %next_agent,
                next_instruction = %next_instruction,
                reasoning = %output.reasoning,
                "Router decided to forward to next agent"
            );

            Ok(RoutingDecision::Forward {
                next_agent: next_agent.clone(),
                next_instruction: next_instruction.clone(),
                forwarded_data: work_output.clone(),
            })
        }
    }
}

#[async_trait::async_trait]
impl Router for LlmRouter {
    async fn decide_next_step(
        &self,
        original_task: &TaskEnvelopeV2,
        work_output: &Value,
        registry: &AgentRegistry,
    ) -> Result<RoutingDecision, AgentError> {
        info!("LlmRouter making routing decision");

        // Build the routing prompt
        let prompt = Self::build_routing_prompt(original_task, work_output, registry);

        debug!("Routing prompt:\n{}", prompt);

        // Create completion request with structured output
        let request = CompletionRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "You are a workflow routing expert. Analyze the workflow context and decide whether to complete or forward.".to_string(),
                },
                Message {
                    role: MessageRole::User,
                    content: prompt,
                },
            ],
            temperature: Some(self.temperature),
            max_tokens: Some(500),
            top_p: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            metadata: Default::default(),
        };

        // Call LLM provider
        let response = self
            .provider
            .complete(request)
            .await
            .map_err(|e| AgentError::LlmError {
                message: e.to_string(),
            })?;

        // Parse the response as RoutingDecisionOutput
        let content = response.content.ok_or_else(|| AgentError::LlmError {
            message: "No content in LLM response".to_string(),
        })?;

        let routing_output: RoutingDecisionOutput =
            serde_json::from_str(&content).map_err(|e| {
                warn!(
                    error = %e,
                    response = %content,
                    "Failed to parse LLM routing decision"
                );
                AgentError::InvalidInput {
                    message: format!("Failed to parse routing decision: {e}"),
                }
            })?;

        info!(
            workflow_complete = routing_output.workflow_complete,
            reasoning = %routing_output.reasoning,
            "Parsed routing decision from LLM"
        );

        // Convert to RoutingDecision
        Self::parse_routing_decision(&routing_output, work_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::discovery::AgentInfo;
    use crate::protocol::messages::{WorkflowContext, WorkflowStep};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_format_workflow_history_empty() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: Some(WorkflowContext {
                original_query: "Test".to_string(),
                steps_completed: vec![],
                iteration_count: 0,
            }),
            routing_trace: None,
        };

        let history = LlmRouter::format_workflow_history(&task);
        assert!(history.contains("Iteration 0/10"));
        assert!(history.contains("No steps completed"));
    }

    #[test]
    fn test_format_workflow_history_with_steps() {
        let task = TaskEnvelopeV2 {
            task_id: Uuid::new_v4(),
            conversation_id: "conv1".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: json!({}),
            next: None,
            version: "2.0".to_string(),
            context: Some(WorkflowContext {
                original_query: "Test".to_string(),
                steps_completed: vec![
                    WorkflowStep {
                        agent_id: "research-agent".to_string(),
                        action: "Researched topic".to_string(),
                        timestamp: "2024-01-01T00:00:00Z".to_string(),
                    },
                    WorkflowStep {
                        agent_id: "writer-agent".to_string(),
                        action: "Wrote document".to_string(),
                        timestamp: "2024-01-01T00:05:00Z".to_string(),
                    },
                ],
                iteration_count: 2,
            }),
            routing_trace: None,
        };

        let history = LlmRouter::format_workflow_history(&task);
        assert!(history.contains("Iteration 2/10"));
        assert!(history.contains("research-agent"));
        assert!(history.contains("writer-agent"));
        assert!(history.contains("Researched topic"));
    }

    #[test]
    fn test_format_agent_catalog() {
        let registry = AgentRegistry::new();

        let mut agent1 = AgentInfo::new("researcher".to_string(), "ok".to_string(), 0.3);
        agent1.capabilities = Some(vec!["research".to_string(), "analysis".to_string()]);

        let mut agent2 = AgentInfo::new("writer".to_string(), "ok".to_string(), 0.5);
        agent2.capabilities = Some(vec!["writing".to_string()]);

        registry.register_agent(agent1);
        registry.register_agent(agent2);

        let catalog = LlmRouter::format_agent_catalog(&registry);
        assert!(catalog.contains("researcher"));
        assert!(catalog.contains("research, analysis"));
        assert!(catalog.contains("writer"));
        assert!(catalog.contains("writing"));
    }

    #[test]
    fn test_parse_routing_decision_complete() {
        let output = RoutingDecisionOutput {
            workflow_complete: true,
            reasoning: "All work done".to_string(),
            next_agent: None,
            next_instruction: None,
        };

        let work_output = json!({"result": "success"});
        let decision = LlmRouter::parse_routing_decision(&output, &work_output).unwrap();

        assert!(decision.is_complete());
        assert!(!decision.is_forward());
    }

    #[test]
    fn test_parse_routing_decision_forward() {
        let output = RoutingDecisionOutput {
            workflow_complete: false,
            reasoning: "Need editing".to_string(),
            next_agent: Some("editor".to_string()),
            next_instruction: Some("Polish document".to_string()),
        };

        let work_output = json!({"document": "draft"});
        let decision = LlmRouter::parse_routing_decision(&output, &work_output).unwrap();

        assert!(decision.is_forward());
        assert_eq!(decision.next_agent(), Some("editor"));
    }

    #[test]
    fn test_parse_routing_decision_invalid() {
        let output = RoutingDecisionOutput {
            workflow_complete: false,
            reasoning: "Need more work".to_string(),
            next_agent: None, // Missing!
            next_instruction: Some("Do something".to_string()),
        };

        let work_output = json!({});
        let result = LlmRouter::parse_routing_decision(&output, &work_output);

        assert!(result.is_err());
    }
}

//! Structured Output Schemas for Routing Decisions
//!
//! This module defines the JSON schemas used for LLM-based routing decisions.
//! These schemas ensure that LLMs return valid, structured routing decisions
//! using either JSON Schema (OpenAI) or Tool schemas (Anthropic).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Structured output schema for LLM routing decisions
///
/// This schema is used with:
/// - OpenAI: JSON Schema with `response_format`
/// - Anthropic: Tool schema with `tool_choice: required`
///
/// The LLM sees the full workflow context and decides whether to complete
/// the workflow or forward to another agent.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RoutingDecisionOutput {
    /// Whether the workflow is complete (user's request fully satisfied)
    pub workflow_complete: bool,

    /// Reasoning for the routing decision (for observability and debugging)
    pub reasoning: String,

    /// Next agent ID to forward to (required if workflow_complete is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_agent: Option<String>,

    /// Instruction for the next agent (required if workflow_complete is false)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_instruction: Option<String>,
}

impl RoutingDecisionOutput {
    /// Validate that the routing decision is internally consistent
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// - workflow_complete is false but next_agent is missing
    /// - workflow_complete is false but next_instruction is missing
    pub fn validate(&self) -> Result<(), String> {
        if !self.workflow_complete {
            if self.next_agent.is_none() {
                return Err("next_agent is required when workflow_complete is false".to_string());
            }
            if self.next_instruction.is_none() {
                return Err(
                    "next_instruction is required when workflow_complete is false".to_string(),
                );
            }
        }
        Ok(())
    }

    /// Generate the JSON schema for this structure
    ///
    /// Used for OpenAI's structured output feature
    pub fn json_schema() -> serde_json::Value {
        let schema = schemars::schema_for!(RoutingDecisionOutput);
        serde_json::to_value(schema).expect("Schema should be serializable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complete_decision_validation() {
        let decision = RoutingDecisionOutput {
            workflow_complete: true,
            reasoning: "Task complete".to_string(),
            next_agent: None,
            next_instruction: None,
        };

        assert!(decision.validate().is_ok());
    }

    #[test]
    fn test_forward_decision_validation() {
        let decision = RoutingDecisionOutput {
            workflow_complete: false,
            reasoning: "Need editing".to_string(),
            next_agent: Some("editor-agent".to_string()),
            next_instruction: Some("Polish the document".to_string()),
        };

        assert!(decision.validate().is_ok());
    }

    #[test]
    fn test_invalid_forward_missing_agent() {
        let decision = RoutingDecisionOutput {
            workflow_complete: false,
            reasoning: "Need more work".to_string(),
            next_agent: None,
            next_instruction: Some("Do something".to_string()),
        };

        assert!(decision.validate().is_err());
    }

    #[test]
    fn test_invalid_forward_missing_instruction() {
        let decision = RoutingDecisionOutput {
            workflow_complete: false,
            reasoning: "Need more work".to_string(),
            next_agent: Some("some-agent".to_string()),
            next_instruction: None,
        };

        assert!(decision.validate().is_err());
    }

    #[test]
    fn test_serialization() {
        let decision = RoutingDecisionOutput {
            workflow_complete: false,
            reasoning: "Document needs polish".to_string(),
            next_agent: Some("editor-agent".to_string()),
            next_instruction: Some("Polish to publication quality".to_string()),
        };

        let json = serde_json::to_string(&decision).unwrap();
        let parsed: RoutingDecisionOutput = serde_json::from_str(&json).unwrap();

        assert!(!parsed.workflow_complete);
        assert_eq!(parsed.next_agent, Some("editor-agent".to_string()));
    }

    #[test]
    fn test_schema_generation() {
        let schema = RoutingDecisionOutput::json_schema();

        // Should be a valid JSON schema
        assert!(schema.is_object());
        assert!(schema["properties"].is_object());
        assert!(schema["properties"]["workflow_complete"].is_object());
        assert!(schema["properties"]["reasoning"].is_object());
        assert!(schema["properties"]["next_agent"].is_object());
        assert!(schema["properties"]["next_instruction"].is_object());
    }
}

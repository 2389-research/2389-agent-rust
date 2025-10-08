//! Route decision schema for v2 workflow
//!
//! Defines the structured output format for agent routing decisions
//! and provides the JSON schema for LLM structured outputs.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Agent routing decision from LLM response (v2 workflow)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteDecision {
    /// Schema version for evolution tracking
    pub schema_version: String,

    /// Agent's work output (freeform string)
    pub result: String,

    /// Next agent to forward to (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_agent: Option<String>,

    /// Instructions for next agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_instruction: Option<String>,

    /// True if workflow is complete
    pub workflow_complete: bool,
}

impl RouteDecision {
    /// Get the JSON Schema for RouteDecision (for structured outputs)
    pub fn json_schema() -> Value {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "title": "RouteDecision",
            "type": "object",
            "additionalProperties": false,
            "required": ["schema_version", "result", "workflow_complete"],
            "properties": {
                "schema_version": {
                    "type": "string",
                    "const": "1.0",
                    "description": "Schema version for evolution tracking"
                },
                "result": {
                    "type": "string",
                    "description": "Freeform textual result of the agent's work"
                },
                "next_agent": {
                    "type": "string",
                    "description": "ID of the next agent to run",
                    "minLength": 1
                },
                "next_instruction": {
                    "type": "string",
                    "description": "Instruction to pass to the next agent"
                },
                "workflow_complete": {
                    "type": "boolean",
                    "description": "Whether the workflow is complete"
                }
            }
        })
    }

    /// Create a RouteDecision from AgentDecision (fallback compatibility)
    pub fn from_agent_decision(decision: &crate::agent::response::AgentDecision) -> Self {
        Self {
            schema_version: "1.0".to_string(),
            result: decision.result.to_string(),
            next_agent: decision.next_agent.clone(),
            next_instruction: decision.next_instruction.clone(),
            workflow_complete: decision.workflow_complete,
        }
    }
}

impl Default for RouteDecision {
    fn default() -> Self {
        Self {
            schema_version: "1.0".to_string(),
            result: String::new(),
            next_agent: None,
            next_instruction: None,
            workflow_complete: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_decision_serialization() {
        let decision = RouteDecision {
            schema_version: "1.0".to_string(),
            result: "Research completed".to_string(),
            next_agent: Some("writer-agent".to_string()),
            next_instruction: Some("Write article based on research".to_string()),
            workflow_complete: false,
        };

        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("\"schema_version\":\"1.0\""));
        assert!(json.contains("\"result\":\"Research completed\""));
        assert!(json.contains("\"next_agent\":\"writer-agent\""));
        assert!(json.contains("\"workflow_complete\":false"));
    }

    #[test]
    fn test_route_decision_deserialization() {
        let json = r#"{
            "schema_version": "1.0",
            "result": "Analysis complete",
            "next_agent": "reviewer",
            "next_instruction": "Review the analysis",
            "workflow_complete": false
        }"#;

        let decision: RouteDecision = serde_json::from_str(json).unwrap();
        assert_eq!(decision.schema_version, "1.0");
        assert_eq!(decision.result, "Analysis complete");
        assert_eq!(decision.next_agent, Some("reviewer".to_string()));
        assert_eq!(
            decision.next_instruction,
            Some("Review the analysis".to_string())
        );
        assert!(!decision.workflow_complete);
    }

    #[test]
    fn test_route_decision_workflow_complete() {
        let decision = RouteDecision {
            schema_version: "1.0".to_string(),
            result: "All done!".to_string(),
            next_agent: None,
            next_instruction: None,
            workflow_complete: true,
        };

        let json = serde_json::to_string(&decision).unwrap();
        assert!(json.contains("\"workflow_complete\":true"));
        assert!(!json.contains("next_agent"));
        assert!(!json.contains("next_instruction"));
    }

    #[test]
    fn test_json_schema_generation() {
        let schema = RouteDecision::json_schema();

        assert_eq!(schema["title"], "RouteDecision");
        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["schema_version"]["const"], "1.0");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("result")));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("workflow_complete")));
    }

    #[test]
    fn test_default_route_decision() {
        let decision = RouteDecision::default();

        assert_eq!(decision.schema_version, "1.0");
        assert!(decision.result.is_empty());
        assert!(decision.next_agent.is_none());
        assert!(decision.next_instruction.is_none());
        assert!(!decision.workflow_complete);
    }
}

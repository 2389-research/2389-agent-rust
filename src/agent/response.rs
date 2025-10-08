//! Agent response and decision handling
//!
//! Provides structures and utilities for parsing agent decisions about routing.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Agent's routing decision from LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDecision {
    /// Schema version (optional, for backwards compatibility)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema_version: Option<String>,

    /// Agent's work output
    pub result: Value,

    /// Next agent to forward to (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_agent: Option<String>,

    /// Instructions for next agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_instruction: Option<String>,

    /// True if workflow is complete
    #[serde(default)]
    pub workflow_complete: bool,
}

/// Parse agent decision from response string
pub fn parse_agent_decision(response: &str) -> Result<AgentDecision, String> {
    // First try to parse as raw JSON
    if let Ok(decision) = serde_json::from_str::<AgentDecision>(response) {
        return Ok(decision);
    }

    // Try to extract JSON from markdown blocks
    if let Some(json_str) = extract_json_from_markdown(response) {
        if let Ok(decision) = serde_json::from_str::<AgentDecision>(&json_str) {
            return Ok(decision);
        }
    }

    // Try to find JSON object in the response
    if let Some(json_str) = find_json_object(response) {
        if let Ok(decision) = serde_json::from_str::<AgentDecision>(&json_str) {
            return Ok(decision);
        }
    }

    Err("Failed to parse agent decision from response".to_string())
}

/// Extract JSON from markdown code blocks
fn extract_json_from_markdown(text: &str) -> Option<String> {
    // Look for ```json blocks
    if let Some(start) = text.find("```json") {
        let content = &text[start + 7..];
        if let Some(end) = content.find("```") {
            return Some(content[..end].trim().to_string());
        }
    }

    // Look for ``` blocks without language specifier
    if let Some(start) = text.find("```") {
        let content = &text[start + 3..];
        if let Some(end) = content.find("```") {
            let potential_json = content[..end].trim();
            // Check if it looks like JSON
            if potential_json.starts_with('{') && potential_json.ends_with('}') {
                return Some(potential_json.to_string());
            }
        }
    }

    None
}

/// Find JSON object in text
fn find_json_object(text: &str) -> Option<String> {
    // Find the first { and try to match it with }
    let mut brace_count = 0;
    let mut start_pos = None;

    for (i, ch) in text.char_indices() {
        match ch {
            '{' => {
                if start_pos.is_none() {
                    start_pos = Some(i);
                }
                brace_count += 1;
            }
            '}' => {
                brace_count -= 1;
                if brace_count == 0 && start_pos.is_some() {
                    let json_str = &text[start_pos.unwrap()..=i];
                    // Verify it's valid JSON
                    if serde_json::from_str::<Value>(json_str).is_ok() {
                        return Some(json_str.to_string());
                    }
                    // Reset and continue looking
                    start_pos = None;
                }
            }
            _ => {}
        }
    }

    None
}

impl Default for AgentDecision {
    fn default() -> Self {
        Self {
            schema_version: None,
            result: Value::Object(serde_json::Map::new()),
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
    fn test_parse_raw_json() {
        let response = r#"{
            "result": {"status": "analyzed"},
            "next_agent": "processor",
            "next_instruction": "Process the analysis",
            "workflow_complete": false
        }"#;

        let decision = parse_agent_decision(response).unwrap();
        assert_eq!(decision.next_agent, Some("processor".to_string()));
        assert_eq!(
            decision.next_instruction,
            Some("Process the analysis".to_string())
        );
        assert!(!decision.workflow_complete);
    }

    #[test]
    fn test_parse_markdown_json() {
        let response = r#"Here is my analysis:

        ```json
        {
            "result": {"analysis": "complete"},
            "next_agent": "reviewer",
            "workflow_complete": false
        }
        ```

        The analysis is done."#;

        let decision = parse_agent_decision(response).unwrap();
        assert_eq!(decision.next_agent, Some("reviewer".to_string()));
        assert!(!decision.workflow_complete);
    }

    #[test]
    fn test_parse_embedded_json() {
        let response = r#"The result is: {"result": {"done": true}, "workflow_complete": true} and that's it."#;

        let decision = parse_agent_decision(response).unwrap();
        assert!(decision.workflow_complete);
        assert_eq!(decision.next_agent, None);
    }

    #[test]
    fn test_workflow_complete() {
        let response = r#"{
            "result": {"final": "output"},
            "workflow_complete": true
        }"#;

        let decision = parse_agent_decision(response).unwrap();
        assert!(decision.workflow_complete);
        assert_eq!(decision.next_agent, None);
    }

    #[test]
    fn test_minimal_decision() {
        let response = r#"{
            "result": {}
        }"#;

        let decision = parse_agent_decision(response).unwrap();
        assert!(!decision.workflow_complete);
        assert_eq!(decision.next_agent, None);
        assert_eq!(decision.next_instruction, None);
    }

    #[test]
    fn test_invalid_json() {
        let response = "This is not JSON at all";
        let result = parse_agent_decision(response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_with_schema_version() {
        let response = r#"```json
{
  "schema_version": "1.0",
  "result": "Article content here",
  "workflow_complete": true
}
```"#;

        let decision = parse_agent_decision(response).unwrap();
        assert_eq!(decision.schema_version, Some("1.0".to_string()));
        assert!(decision.workflow_complete);
        
        // Verify the result is extracted as a string
        if let Value::String(content) = &decision.result {
            assert_eq!(content, "Article content here");
        } else {
            panic!("Expected result to be a string");
        }
    }
}

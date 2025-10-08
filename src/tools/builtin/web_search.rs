//! Web search tool implementation
//!
//! This module implements the web search builtin tool using the Serper API
//! for searching current information on the web.

use crate::tools::{Tool, ToolDescription, ToolError};
use async_trait::async_trait;
use serde_json::{Value, json};

/// Web search tool using Serper API - builtin implementation
pub struct WebSearchTool {
    client: Option<reqwest::Client>,
    api_key: Option<String>,
    max_results: usize,
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self {
            client: None,
            api_key: None,
            max_results: 10,
        }
    }
}

impl WebSearchTool {
    /// Create new web search tool
    pub fn new() -> Self {
        Self::default()
    }

    /// Build search payload (pure function)
    fn build_search_payload(query: &str, num_results: usize, max_results: usize) -> Value {
        json!({
            "q": query,
            "num": std::cmp::min(num_results, max_results),
            "gl": "us",
            "hl": "en"
        })
    }

    /// Parse search response (pure function)
    fn parse_search_response(search_result: &Value, num_results: usize) -> Vec<Value> {
        let mut formatted_results = Vec::new();

        if let Some(organic) = search_result.get("organic").and_then(|o| o.as_array()) {
            for result in organic.iter().take(num_results) {
                if let (Some(title), Some(link)) = (
                    result.get("title").and_then(|t| t.as_str()),
                    result.get("link").and_then(|l| l.as_str()),
                ) {
                    let snippet = result.get("snippet").and_then(|s| s.as_str()).unwrap_or("");

                    formatted_results.push(json!({
                        "title": title,
                        "url": link,
                        "snippet": snippet
                    }));
                }
            }
        }

        formatted_results
    }

    /// Format final search response (pure function)
    fn format_search_response(query: &str, results: Vec<Value>) -> Value {
        json!({
            "query": query,
            "results": results
        })
    }

    /// Validate search parameters (pure function)
    fn validate_search_params(query: Option<&str>) -> Result<&str, String> {
        query.ok_or_else(|| "Query parameter is required".to_string())
    }

    /// Extract number of results from parameters (pure function)
    fn extract_num_results(parameters: &Value) -> usize {
        parameters
            .get("num_results")
            .and_then(|n| n.as_u64())
            .unwrap_or(10) as usize
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn describe(&self) -> ToolDescription {
        ToolDescription {
            name: "web_search".to_string(),
            description: "Search web for current information".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "num_results": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 20,
                        "default": 10
                    }
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        }
    }

    async fn initialize(&mut self, config: Option<&Value>) -> Result<(), ToolError> {
        // Get API key from environment variable
        self.api_key = std::env::var("SERPER_API_KEY").ok();
        if self.api_key.is_none() {
            return Err(ToolError::InitializationError(
                "SERPER_API_KEY environment variable not set".to_string(),
            ));
        }

        // Configure from config if provided
        if let Some(config) = config {
            if let Some(max_results) = config.get("max_results").and_then(|v| v.as_u64()) {
                self.max_results = max_results as usize;
            }
        }

        // Initialize HTTP client
        self.client = Some(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| ToolError::InitializationError(e.to_string()))?,
        );

        Ok(())
    }

    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionError("Tool not initialized".to_string()))?;

        let api_key = self
            .api_key
            .as_ref()
            .ok_or_else(|| ToolError::ExecutionError("API key not configured".to_string()))?;

        // Validate and extract parameters using pure functions
        let query = Self::validate_search_params(parameters["query"].as_str())
            .map_err(ToolError::ExecutionError)?;
        let num_results = Self::extract_num_results(parameters);

        // Build request payload using pure function
        let payload = Self::build_search_payload(query, num_results, self.max_results);

        // Make request to Serper API (impure I/O)
        let response = client
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", api_key)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Request failed: {e}")))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ToolError::ExecutionError(format!(
                "Serper API error ({}): {}",
                status.as_u16(),
                error_text
            )));
        }

        let search_result: Value = response
            .json()
            .await
            .map_err(|e| ToolError::ExecutionError(format!("Failed to parse response: {e}")))?;

        // Parse and format response using pure functions
        let results = Self::parse_search_response(&search_result, num_results);
        Ok(Self::format_search_response(query, results))
    }

    async fn shutdown(&mut self) -> Result<(), ToolError> {
        self.client = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_search_tool_creation() {
        let tool = WebSearchTool::new();
        assert!(tool.client.is_none());
        assert!(tool.api_key.is_none());
        assert_eq!(tool.max_results, 10);
    }

    #[test]
    fn test_build_search_payload() {
        let payload = WebSearchTool::build_search_payload("test query", 5, 10);

        assert_eq!(payload["q"], "test query");
        assert_eq!(payload["num"], 5);
        assert_eq!(payload["gl"], "us");
        assert_eq!(payload["hl"], "en");
    }

    #[test]
    fn test_build_search_payload_respects_max_results() {
        let payload = WebSearchTool::build_search_payload("test query", 15, 10);

        // Should limit to max_results
        assert_eq!(payload["num"], 10);
    }

    #[test]
    fn test_validate_search_params() {
        // Valid query should pass
        assert!(WebSearchTool::validate_search_params(Some("test")).is_ok());

        // None query should fail
        assert!(WebSearchTool::validate_search_params(None).is_err());
    }

    #[test]
    fn test_extract_num_results() {
        let params_with_num = json!({"num_results": 7});
        assert_eq!(WebSearchTool::extract_num_results(&params_with_num), 7);

        let params_without_num = json!({"query": "test"});
        assert_eq!(WebSearchTool::extract_num_results(&params_without_num), 10);
    }

    #[test]
    fn test_parse_search_response_empty() {
        let empty_response = json!({});
        let results = WebSearchTool::parse_search_response(&empty_response, 5);

        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_response_with_results() {
        let response = json!({
            "organic": [
                {
                    "title": "Test Title",
                    "link": "https://example.com",
                    "snippet": "Test snippet"
                }
            ]
        });

        let results = WebSearchTool::parse_search_response(&response, 5);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["title"], "Test Title");
        assert_eq!(results[0]["url"], "https://example.com");
        assert_eq!(results[0]["snippet"], "Test snippet");
    }

    #[test]
    fn test_format_search_response() {
        let results = vec![json!({
            "title": "Test",
            "url": "https://example.com",
            "snippet": "Test snippet"
        })];

        let response = WebSearchTool::format_search_response("test query", results);

        assert_eq!(response["query"], "test query");
        assert_eq!(response["results"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_tool_description() {
        let tool = WebSearchTool::new();
        let description = tool.describe();

        assert_eq!(description.name, "web_search");
        assert!(!description.description.is_empty());
        assert!(description.parameters.is_object());
    }
}

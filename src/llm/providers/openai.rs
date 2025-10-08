//! OpenAI provider implementation
//!
//! This module provides OpenAI API integration for the LLM provider system.

use crate::llm::provider::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmProvider, Message,
    MessageRole, TokenUsage, ToolCall as ProviderToolCall,
};
use crate::tools::ToolDescription;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, error, warn};

/// OpenAI provider configuration
#[derive(Debug, Clone)]
pub struct OpenAiConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout: Duration,
}

impl Default for OpenAiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.openai.com/v1".to_string(),
            timeout: Duration::from_secs(60),
        }
    }
}

/// OpenAI provider implementation
pub struct OpenAiProvider {
    config: OpenAiConfig,
    client: Client,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider
    pub fn new(config: OpenAiConfig) -> Result<Self, LlmError> {
        if config.api_key.is_empty() {
            return Err(LlmError::NotConfigured(
                "OpenAI API key is required".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        Ok(Self { config, client })
    }

    /// Estimate token count for messages (pure function)
    fn estimate_token_count(messages: &[OpenAiMessage]) -> usize {
        messages
            .iter()
            .map(|m| m.content.as_ref().map(|c| c.len()).unwrap_or(0) / 4)
            .sum()
    }

    /// Convert completion request to OpenAI format (pure function)
    fn convert_to_openai_request(
        request: &CompletionRequest,
        messages: Vec<OpenAiMessage>,
        tools: Option<Vec<OpenAiTool>>,
    ) -> OpenAiCompletionRequest {
        use crate::llm::provider::ResponseFormat;

        // Convert response_format if present
        let response_format = request.response_format.as_ref().map(|rf| match rf {
            ResponseFormat::Text => OpenAiResponseFormat::Simple {
                format_type: "text".to_string(),
            },
            ResponseFormat::Json => OpenAiResponseFormat::Simple {
                format_type: "json_object".to_string(),
            },
            ResponseFormat::JsonSchema { json_schema } => OpenAiResponseFormat::JsonSchema {
                format_type: "json_schema".to_string(),
                json_schema: OpenAiJsonSchema {
                    name: json_schema.name.clone(),
                    strict: json_schema.strict,
                    schema: json_schema.schema.clone(),
                },
            },
        });

        OpenAiCompletionRequest {
            model: request.model.clone(),
            messages,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            top_p: request.top_p,
            stop: request.stop_sequences.clone(),
            tools,
            tool_choice: request.tool_choice.clone(),
            response_format,
        }
    }

    /// Parse OpenAI completion response (pure function)
    fn parse_completion_response(
        openai_response: OpenAiCompletionResponse,
        request_metadata: std::collections::HashMap<String, String>,
    ) -> Result<CompletionResponse, LlmError> {
        if openai_response.choices.is_empty() {
            return Err(LlmError::ApiError(
                "No choices returned from OpenAI".to_string(),
            ));
        }

        let choice = &openai_response.choices[0];
        let usage = TokenUsage {
            prompt_tokens: openai_response.usage.prompt_tokens,
            completion_tokens: openai_response.usage.completion_tokens,
            total_tokens: openai_response.usage.total_tokens,
        };

        let tool_calls = choice
            .message
            .tool_calls
            .as_ref()
            .map(|calls| Self::extract_tool_calls(calls));

        let finish_reason = Self::convert_finish_reason_pure(choice.finish_reason.clone());

        Ok(CompletionResponse {
            content: choice.message.content.clone(),
            model: openai_response.model,
            usage,
            finish_reason,
            tool_calls,
            metadata: request_metadata,
        })
    }

    /// Extract tool calls from OpenAI format (pure function)
    fn extract_tool_calls(calls: &[OpenAiToolCall]) -> Vec<ProviderToolCall> {
        calls
            .iter()
            .filter_map(|call| {
                match serde_json::from_str::<serde_json::Value>(&call.function.arguments) {
                    Ok(args) => Some(ProviderToolCall {
                        id: call.id.clone(),
                        name: call.function.name.clone(),
                        arguments: args,
                    }),
                    Err(e) => {
                        error!("Failed to parse tool call arguments: {}", e);
                        None
                    }
                }
            })
            .collect()
    }

    /// Convert OpenAI finish reason to internal format (pure function)
    fn convert_finish_reason_pure(reason: Option<String>) -> FinishReason {
        match reason.as_deref() {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Error,
        }
    }

    /// Convert internal message to OpenAI format
    fn convert_message(&self, message: &Message) -> OpenAiMessage {
        OpenAiMessage {
            role: match message.role {
                MessageRole::System => "system".to_string(),
                MessageRole::User => "user".to_string(),
                MessageRole::Assistant => "assistant".to_string(),
            },
            content: Some(message.content.clone()),
            tool_calls: None,
        }
    }

    /// Convert tool description to OpenAI tool format
    fn convert_tool(&self, tool_desc: &ToolDescription) -> OpenAiTool {
        OpenAiTool {
            tool_type: "function".to_string(),
            function: OpenAiFunction {
                name: tool_desc.name.clone(),
                description: tool_desc.description.clone(),
                parameters: tool_desc.parameters.clone(),
            },
        }
    }
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn available_models(&self) -> Vec<String> {
        vec![
            "gpt-4".to_string(),
            "gpt-4-turbo".to_string(),
            "gpt-3.5-turbo".to_string(),
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
        ]
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        // Convert messages and tools using existing methods
        let openai_messages: Vec<OpenAiMessage> = request
            .messages
            .iter()
            .map(|m| self.convert_message(m))
            .collect();

        let tools = request.tools.as_ref().map(|tool_descriptions| {
            tool_descriptions
                .iter()
                .map(|tool_desc| self.convert_tool(tool_desc))
                .collect()
        });

        // Use pure functions for preprocessing
        let estimated_tokens = Self::estimate_token_count(&openai_messages);
        self.log_request_info(&openai_messages, estimated_tokens);

        let openai_request = Self::convert_to_openai_request(&request, openai_messages, tools);

        // Delegate to retry orchestrator
        self.complete_with_retry(openai_request, request.metadata)
            .await
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        let response = self
            .client
            .get(format!("{}/models", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .send()
            .await
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(LlmError::AuthenticationFailed(
                "OpenAI API authentication failed".to_string(),
            ))
        }
    }
}

impl OpenAiProvider {
    /// Log request information (impure)
    fn log_request_info(&self, messages: &[OpenAiMessage], estimated_tokens: usize) {
        debug!(
            "OpenAI request: {} messages, estimated ~{} tokens",
            messages.len(),
            estimated_tokens
        );

        if estimated_tokens > 120000 {
            warn!(
                "Large request detected: estimated {} tokens, may exceed model limits",
                estimated_tokens
            );
        }
    }

    /// Retry orchestrator - handles only I/O and retry logic (impure)
    async fn complete_with_retry(
        &self,
        openai_request: OpenAiCompletionRequest,
        metadata: std::collections::HashMap<String, String>,
    ) -> Result<CompletionResponse, LlmError> {
        let backoff_delays = [100u64, 200, 300];
        let mut last_error = None;

        for (attempt, &delay_ms) in std::iter::once(&0u64)
            .chain(backoff_delays.iter())
            .enumerate()
        {
            if attempt > 0 {
                debug!(
                    "OpenAI retry attempt {} after {}ms delay",
                    attempt, delay_ms
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            match self.make_api_request(&openai_request).await {
                Ok(openai_response) => {
                    if attempt > 0 {
                        debug!("OpenAI request succeeded after {} retries", attempt);
                    }

                    let response = Self::parse_completion_response(openai_response, metadata)?;
                    self.log_response_info(&response);
                    return Ok(response);
                }
                Err(e) => {
                    warn!("OpenAI request attempt {} failed: {}", attempt + 1, e);
                    last_error = Some(e.clone());
                    if matches!(e, LlmError::ApiError(_)) && !self.should_retry(&e) {
                        error!("Non-retryable API error, aborting: {}", e);
                        return Err(e);
                    }
                    if attempt < backoff_delays.len() {
                        debug!("Error is retryable, will retry");
                    }
                }
            }
        }

        error!("OpenAI request failed after all retries");
        Err(last_error
            .unwrap_or_else(|| LlmError::NetworkError("All retry attempts failed".to_string())))
    }

    /// Make single API request (impure I/O)
    async fn make_api_request(
        &self,
        openai_request: &OpenAiCompletionRequest,
    ) -> Result<OpenAiCompletionResponse, LlmError> {
        let response = self
            .client
            .post(format!("{}/chat/completions", self.config.base_url))
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .header("Content-Type", "application/json")
            .json(openai_request)
            .send()
            .await
            .map_err(|e| {
                let error_msg = format!(
                    "HTTP request failed: {} (is_connect: {}, is_timeout: {}, is_request: {})",
                    e,
                    e.is_connect(),
                    e.is_timeout(),
                    e.is_request()
                );
                warn!("OpenAI network error details: {}", error_msg);
                LlmError::NetworkError(error_msg)
            })?;

        let status = response.status();

        if status.is_server_error() {
            let error_text = response.text().await.unwrap_or_default();
            let error_msg = format!("OpenAI API server error: {status} - {error_text}");
            warn!("OpenAI server error: {}", error_msg);
            return Err(LlmError::ApiError(error_msg));
        }

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            error!(
                "OpenAI API client error - Status: {}, Response: {}",
                status, error_text
            );

            if error_text.contains("maximum context length")
                || error_text.contains("too many tokens")
            {
                warn!("Token limit exceeded - conversation may be too long");
            }

            return Err(LlmError::ApiError(format!(
                "OpenAI API error: {status} - {error_text}"
            )));
        }

        response
            .json()
            .await
            .map_err(|e| LlmError::RequestFailed(e.to_string()))
    }

    /// Check if error should trigger retry (pure)
    fn should_retry(&self, error: &LlmError) -> bool {
        match error {
            LlmError::NetworkError(_) => true,
            LlmError::ApiError(msg) => msg.contains("server error"),
            _ => false,
        }
    }

    /// Log response information (impure)
    fn log_response_info(&self, response: &CompletionResponse) {
        debug!(
            "OpenAI response: {} tokens used (prompt: {}, completion: {}), finish_reason: {:?}, tool_calls: {}",
            response.usage.total_tokens,
            response.usage.prompt_tokens,
            response.usage.completion_tokens,
            response.finish_reason,
            response.tool_calls.as_ref().map(|tc| tc.len()).unwrap_or(0)
        );
    }
}

#[derive(Debug, Serialize)]
struct OpenAiCompletionRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<OpenAiResponseFormat>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OpenAiCompletionResponse {
    model: String,
    choices: Vec<OpenAiChoice>,
    usage: OpenAiUsage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

/// OpenAI response format
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAiResponseFormat {
    /// Simple type string
    Simple {
        #[serde(rename = "type")]
        format_type: String,
    },
    /// JSON schema with strict validation
    JsonSchema {
        #[serde(rename = "type")]
        format_type: String,
        json_schema: OpenAiJsonSchema,
    },
}

/// OpenAI JSON Schema format
#[derive(Debug, Serialize, Deserialize)]
struct OpenAiJsonSchema {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    strict: Option<bool>,
    schema: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_config_default() {
        let config = OpenAiConfig::default();
        assert_eq!(config.base_url, "https://api.openai.com/v1");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert!(config.api_key.is_empty());
    }

    #[test]
    fn test_openai_provider_creation_without_api_key() {
        let config = OpenAiConfig::default();
        let result = OpenAiProvider::new(config);
        assert!(matches!(result, Err(LlmError::NotConfigured(_))));
    }

    #[test]
    fn test_openai_provider_creation_with_api_key() {
        let config = OpenAiConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let result = OpenAiProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_openai_provider_name() {
        let config = OpenAiConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = OpenAiProvider::new(config).unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_openai_provider_available_models() {
        let config = OpenAiConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = OpenAiProvider::new(config).unwrap();
        let models = provider.available_models();

        assert!(!models.is_empty());
        assert!(models.contains(&"gpt-4".to_string()));
        assert!(models.contains(&"gpt-3.5-turbo".to_string()));
    }

    #[test]
    fn test_message_conversion() {
        let config = OpenAiConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = OpenAiProvider::new(config).unwrap();

        let message = Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        };

        let openai_message = provider.convert_message(&message);
        assert_eq!(openai_message.role, "user");
        assert_eq!(openai_message.content, Some("Hello".to_string()));
    }

    #[test]
    fn test_finish_reason_conversion() {
        assert!(matches!(
            OpenAiProvider::convert_finish_reason_pure(Some("stop".to_string())),
            FinishReason::Stop
        ));
        assert!(matches!(
            OpenAiProvider::convert_finish_reason_pure(Some("length".to_string())),
            FinishReason::Length
        ));
        assert!(matches!(
            OpenAiProvider::convert_finish_reason_pure(Some("content_filter".to_string())),
            FinishReason::ContentFilter
        ));
        assert!(matches!(
            OpenAiProvider::convert_finish_reason_pure(None),
            FinishReason::Error
        ));
    }

    #[test]
    fn test_openai_request_serialization() {
        let request = OpenAiCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![OpenAiMessage {
                role: "user".to_string(),
                content: Some("Hello".to_string()),
                tool_calls: None,
            }],
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            stop: None,
            tools: None,
            tool_choice: None,
            response_format: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"gpt-4\""));
        assert!(json.contains("\"max_tokens\":100"));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(!json.contains("top_p"));
        assert!(!json.contains("stop"));
    }
}

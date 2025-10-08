//! Anthropic provider implementation
//!
//! This module provides Anthropic API integration for the LLM provider system.

use crate::llm::provider::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmProvider, Message,
    MessageRole, TokenUsage,
};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Anthropic provider configuration
#[derive(Debug, Clone)]
pub struct AnthropicConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout: Duration,
    pub version: String,
}

impl Default for AnthropicConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            timeout: Duration::from_secs(60),
            version: "2023-06-01".to_string(),
        }
    }
}

/// Anthropic provider implementation
pub struct AnthropicProvider {
    config: AnthropicConfig,
    client: Client,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider
    pub fn new(config: AnthropicConfig) -> Result<Self, LlmError> {
        if config.api_key.is_empty() {
            return Err(LlmError::NotConfigured(
                "Anthropic API key is required".to_string(),
            ));
        }

        let client = Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        Ok(Self { config, client })
    }

    /// Convert internal messages to Anthropic format
    fn convert_messages(&self, messages: &[Message]) -> (Option<String>, Vec<AnthropicMessage>) {
        let mut system_message = None;
        let mut anthropic_messages = Vec::new();

        for message in messages {
            match message.role {
                MessageRole::System => {
                    system_message = Some(message.content.clone());
                }
                MessageRole::User | MessageRole::Assistant => {
                    anthropic_messages.push(AnthropicMessage {
                        role: match message.role {
                            MessageRole::User => "user".to_string(),
                            MessageRole::Assistant => "assistant".to_string(),
                            MessageRole::System => unreachable!(),
                        },
                        content: message.content.clone(),
                    });
                }
            }
        }

        (system_message, anthropic_messages)
    }

    /// Convert Anthropic finish reason to internal format
    fn convert_finish_reason(&self, reason: Option<String>) -> FinishReason {
        match reason.as_deref() {
            Some("end_turn") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("stop_sequence") => FinishReason::Stop,
            _ => FinishReason::Error,
        }
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn available_models(&self) -> Vec<String> {
        vec![
            "claude-3-5-sonnet-20241022".to_string(),
            "claude-3-5-haiku-20241022".to_string(),
            "claude-3-opus-20240229".to_string(),
            "claude-3-sonnet-20240229".to_string(),
            "claude-3-haiku-20240307".to_string(),
        ]
    }

    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        use crate::llm::provider::ResponseFormat;

        let (system_message, messages) = self.convert_messages(&request.messages);

        // Convert response_format if present
        // Anthropic only supports simple {"type": "json"} format, not full JSON schema
        let response_format = request.response_format.as_ref().and_then(|rf| match rf {
            ResponseFormat::Json | ResponseFormat::JsonSchema { .. } => {
                Some(AnthropicResponseFormat {
                    format_type: "json".to_string(),
                })
            }
            ResponseFormat::Text => None,
        });

        let anthropic_request = AnthropicCompletionRequest {
            model: request.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(4096),
            messages,
            system: system_message,
            temperature: request.temperature,
            top_p: request.top_p,
            stop_sequences: request.stop_sequences,
            response_format,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.version)
            .header("Content-Type", "application/json")
            .json(&anthropic_request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(LlmError::ApiError(format!(
                "Anthropic API error: {status} - {error_text}"
            )));
        }

        let anthropic_response: AnthropicCompletionResponse = response
            .json()
            .await
            .map_err(|e| LlmError::RequestFailed(e.to_string()))?;

        if anthropic_response.content.is_empty() {
            return Err(LlmError::ApiError(
                "No content returned from Anthropic".to_string(),
            ));
        }

        let content = anthropic_response
            .content
            .into_iter()
            .filter_map(|c| match c.content_type.as_str() {
                "text" => Some(c.text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        let usage = TokenUsage {
            prompt_tokens: anthropic_response.usage.input_tokens,
            completion_tokens: anthropic_response.usage.output_tokens,
            total_tokens: anthropic_response.usage.input_tokens
                + anthropic_response.usage.output_tokens,
        };

        Ok(CompletionResponse {
            content: Some(content),
            model: anthropic_response.model,
            usage,
            finish_reason: self.convert_finish_reason(anthropic_response.stop_reason),
            tool_calls: None, // Anthropic doesn't support structured tool calling yet
            metadata: request.metadata,
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        // Anthropic doesn't have a simple health check endpoint, so we make a minimal request
        let test_request = AnthropicCompletionRequest {
            model: "claude-3-haiku-20240307".to_string(),
            max_tokens: 1,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hi".to_string(),
            }],
            system: None,
            temperature: None,
            top_p: None,
            stop_sequences: None,
            response_format: None,
        };

        let response = self
            .client
            .post(format!("{}/messages", self.config.base_url))
            .header("x-api-key", &self.config.api_key)
            .header("anthropic-version", &self.config.version)
            .header("Content-Type", "application/json")
            .json(&test_request)
            .send()
            .await
            .map_err(|e| LlmError::NetworkError(e.to_string()))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(LlmError::AuthenticationFailed(
                "Anthropic API authentication failed".to_string(),
            ))
        }
    }
}

#[derive(Debug, Serialize)]
struct AnthropicCompletionRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<AnthropicResponseFormat>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicCompletionResponse {
    content: Vec<AnthropicContent>,
    model: String,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    content_type: String,
    text: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

/// Anthropic response format
/// Note: Anthropic uses {"type": "json"} for simple JSON mode
#[derive(Debug, Serialize, Deserialize)]
struct AnthropicResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_config_default() {
        let config = AnthropicConfig::default();
        assert_eq!(config.base_url, "https://api.anthropic.com/v1");
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.version, "2023-06-01");
        assert!(config.api_key.is_empty());
    }

    #[test]
    fn test_anthropic_provider_creation_without_api_key() {
        let config = AnthropicConfig::default();
        let result = AnthropicProvider::new(config);
        assert!(matches!(result, Err(LlmError::NotConfigured(_))));
    }

    #[test]
    fn test_anthropic_provider_creation_with_api_key() {
        let config = AnthropicConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let result = AnthropicProvider::new(config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_anthropic_provider_name() {
        let config = AnthropicConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = AnthropicProvider::new(config).unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_anthropic_provider_available_models() {
        let config = AnthropicConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = AnthropicProvider::new(config).unwrap();
        let models = provider.available_models();

        assert!(!models.is_empty());
        assert!(models.contains(&"claude-3-5-sonnet-20241022".to_string()));
        assert!(models.contains(&"claude-3-haiku-20240307".to_string()));
    }

    #[test]
    fn test_message_conversion() {
        let config = AnthropicConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = AnthropicProvider::new(config).unwrap();

        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are helpful".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
            },
        ];

        let (system, anthropic_messages) = provider.convert_messages(&messages);
        assert_eq!(system, Some("You are helpful".to_string()));
        assert_eq!(anthropic_messages.len(), 1);
        assert_eq!(anthropic_messages[0].role, "user");
        assert_eq!(anthropic_messages[0].content, "Hello");
    }

    #[test]
    fn test_finish_reason_conversion() {
        let config = AnthropicConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let provider = AnthropicProvider::new(config).unwrap();

        assert!(matches!(
            provider.convert_finish_reason(Some("end_turn".to_string())),
            FinishReason::Stop
        ));
        assert!(matches!(
            provider.convert_finish_reason(Some("max_tokens".to_string())),
            FinishReason::Length
        ));
        assert!(matches!(
            provider.convert_finish_reason(Some("stop_sequence".to_string())),
            FinishReason::Stop
        ));
        assert!(matches!(
            provider.convert_finish_reason(None),
            FinishReason::Error
        ));
    }

    #[test]
    fn test_anthropic_request_serialization() {
        let request = AnthropicCompletionRequest {
            model: "claude-3-haiku-20240307".to_string(),
            max_tokens: 100,
            messages: vec![AnthropicMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            system: Some("You are helpful".to_string()),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            response_format: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"model\":\"claude-3-haiku-20240307\""));
        assert!(json.contains("\"max_tokens\":100"));
        assert!(json.contains("\"system\":\"You are helpful\""));
        assert!(json.contains("\"temperature\":0.7"));
        assert!(!json.contains("top_p"));
        assert!(!json.contains("stop_sequences"));
    }
}

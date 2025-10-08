//! LLM provider abstraction and trait definitions
//!
//! This module defines the core traits and types for LLM provider interactions,
//! enabling multiple provider backends with a unified interface.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// A single message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// Message roles in a conversation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// LLM completion request parameters
#[derive(Debug, Clone)]
pub struct CompletionRequest {
    pub messages: Vec<Message>,
    pub model: String,
    pub max_tokens: Option<u32>,
    pub temperature: Option<f32>,
    pub top_p: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
    pub tools: Option<Vec<crate::tools::ToolDescription>>,
    pub tool_choice: Option<String>,
    pub response_format: Option<ResponseFormat>,
    pub metadata: HashMap<String, String>,
}

/// Tool call information from LLM response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// LLM completion response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub content: Option<String>,
    pub model: String,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub metadata: HashMap<String, String>,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Reason why completion finished
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FinishReason {
    Stop,
    Length,
    ContentFilter,
    Error,
}

/// Response format for structured outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseFormat {
    /// Plain text response
    Text,
    /// JSON object without schema validation
    Json,
    /// JSON with strict schema validation
    JsonSchema { json_schema: JsonSchemaDefinition },
}

/// JSON Schema definition for structured outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchemaDefinition {
    /// Schema name
    pub name: String,
    /// Whether to use strict mode (OpenAI only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
    /// The JSON Schema object
    pub schema: serde_json::Value,
}

impl Default for ResponseFormat {
    fn default() -> Self {
        Self::Text
    }
}

/// LLM provider trait for dependency injection and testing
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name (e.g., "openai", "anthropic")
    fn name(&self) -> &str;

    /// Get list of available models for this provider
    fn available_models(&self) -> Vec<String>;

    /// Generate a completion from the given request
    async fn complete(&self, request: CompletionRequest) -> Result<CompletionResponse, LlmError>;

    /// Check if the provider is configured and ready
    async fn health_check(&self) -> Result<(), LlmError>;
}

/// LLM provider errors
#[derive(Debug, Clone, Error)]
pub enum LlmError {
    #[error("Provider not configured: {0}")]
    NotConfigured(String),
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("Network error: {0}")]
    NetworkError(String),
    #[error("API error: {0}")]
    ApiError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let message = Message {
            role: MessageRole::User,
            content: "Hello, world!".to_string(),
        };

        assert_eq!(message.role, MessageRole::User);
        assert_eq!(message.content, "Hello, world!");
    }

    #[test]
    fn test_completion_request_creation() {
        let messages = vec![
            Message {
                role: MessageRole::System,
                content: "You are a helpful assistant.".to_string(),
            },
            Message {
                role: MessageRole::User,
                content: "Hello!".to_string(),
            },
        ];

        let request = CompletionRequest {
            messages,
            model: "gpt-4".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            metadata: HashMap::new(),
            response_format: None,
        };

        assert_eq!(request.messages.len(), 2);
        assert_eq!(request.model, "gpt-4");
        assert_eq!(request.max_tokens, Some(100));
        assert_eq!(request.temperature, Some(0.7));
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.prompt_tokens, 0);
        assert_eq!(usage.completion_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_llm_error_display() {
        let errors = vec![
            LlmError::NotConfigured("test".to_string()),
            LlmError::AuthenticationFailed("test".to_string()),
            LlmError::ModelNotFound("test".to_string()),
            LlmError::RateLimitExceeded("test".to_string()),
            LlmError::RequestFailed("test".to_string()),
            LlmError::InvalidRequest("test".to_string()),
            LlmError::NetworkError("test".to_string()),
            LlmError::ApiError("test".to_string()),
        ];

        for error in errors {
            let error_string = error.to_string();
            assert!(!error_string.is_empty());
        }
    }

    #[test]
    fn test_message_role_serialization() {
        let system_role = MessageRole::System;
        let user_role = MessageRole::User;
        let assistant_role = MessageRole::Assistant;

        // Test that roles can be serialized (will be used in JSON requests)
        let system_json = serde_json::to_string(&system_role).unwrap();
        let user_json = serde_json::to_string(&user_role).unwrap();
        let assistant_json = serde_json::to_string(&assistant_role).unwrap();

        assert_eq!(system_json, "\"system\"");
        assert_eq!(user_json, "\"user\"");
        assert_eq!(assistant_json, "\"assistant\"");
    }

    #[test]
    fn test_message_serialization() {
        let message = Message {
            role: MessageRole::User,
            content: "Test message".to_string(),
        };

        let json = serde_json::to_string(&message).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.content, message.content);
        assert_eq!(deserialized.role, MessageRole::User);
    }
}

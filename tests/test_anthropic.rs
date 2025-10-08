//! Integration tests for Anthropic provider
//!
//! Tests behavioral contracts without testing implementation details:
//! - API request/response handling
//! - Error scenarios (rate limits, auth failures, network errors)
//! - Token usage tracking
//! - Message format conversions
//! - Finish reason handling

use agent2389::llm::provider::{
    CompletionRequest, FinishReason, LlmError, LlmProvider, Message, MessageRole,
};
use agent2389::llm::providers::anthropic::{AnthropicConfig, AnthropicProvider};
use std::collections::HashMap;
use std::time::Duration;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config(base_url: &str) -> AnthropicConfig {
    AnthropicConfig {
        api_key: "test-api-key".to_string(),
        base_url: base_url.to_string(),
        timeout: Duration::from_secs(5),
        version: "2023-06-01".to_string(),
    }
}

fn test_request(model: &str) -> CompletionRequest {
    CompletionRequest {
        messages: vec![Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
        }],
        model: model.to_string(),
        max_tokens: Some(100),
        temperature: Some(0.7),
        top_p: None,
        stop_sequences: None,
        tools: None,
        tool_choice: None,
        response_format: None,
        metadata: HashMap::new(),
    }
}

#[tokio::test]
async fn test_anthropic_provider_returns_successful_completion_with_valid_response() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "id": "msg_123",
        "type": "message",
        "role": "assistant",
        "content": [
            {
                "type": "text",
                "text": "Hello! How can I help you?"
            }
        ],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 15
        }
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .and(header("x-api-key", "test-api-key"))
        .and(header("anthropic-version", "2023-06-01"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
        response.content,
        Some("Hello! How can I help you?".to_string())
    );
    assert_eq!(response.model, "claude-3-haiku-20240307");
    assert_eq!(response.usage.prompt_tokens, 10);
    assert_eq!(response.usage.completion_tokens, 15);
    assert_eq!(response.usage.total_tokens, 25);
    assert!(matches!(response.finish_reason, FinishReason::Stop));
}

#[tokio::test]
async fn test_anthropic_provider_handles_multiple_content_blocks() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [
            {
                "type": "text",
                "text": "First part. "
            },
            {
                "type": "text",
                "text": "Second part."
            }
        ],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "end_turn",
        "usage": {
            "input_tokens": 10,
            "output_tokens": 20
        }
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
        response.content,
        Some("First part. Second part.".to_string())
    );
}

#[tokio::test]
async fn test_anthropic_provider_converts_system_message_to_system_field() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [{"type": "text", "text": "Response"}],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let mut request = test_request("claude-3-haiku-20240307");
    request.messages.insert(
        0,
        Message {
            role: MessageRole::System,
            content: "You are helpful".to_string(),
        },
    );

    let result = provider.complete(request).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_anthropic_provider_returns_error_when_api_responds_with_401() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid API key"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ApiError(msg) => {
            assert!(msg.contains("401"));
            assert!(msg.contains("Invalid API key"));
        }
        other => panic!("Expected ApiError, got {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_provider_returns_error_when_api_responds_with_429() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(429).set_body_string("Rate limit exceeded"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ApiError(msg) => {
            assert!(msg.contains("429"));
            assert!(msg.contains("Rate limit exceeded"));
        }
        other => panic!("Expected ApiError, got {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_provider_returns_error_when_content_is_empty() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 0}
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ApiError(msg) => {
            assert!(msg.contains("No content"));
        }
        other => panic!("Expected ApiError for empty content, got {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_provider_converts_max_tokens_finish_reason() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [{"type": "text", "text": "Truncated"}],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "max_tokens",
        "usage": {"input_tokens": 10, "output_tokens": 100}
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(matches!(response.finish_reason, FinishReason::Length));
}

#[tokio::test]
async fn test_anthropic_provider_converts_stop_sequence_finish_reason() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [{"type": "text", "text": "Response"}],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "stop_sequence",
        "usage": {"input_tokens": 10, "output_tokens": 20}
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(matches!(response.finish_reason, FinishReason::Stop));
}

#[tokio::test]
async fn test_anthropic_provider_returns_error_when_json_parsing_fails() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Invalid JSON"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider
        .complete(test_request("claude-3-haiku-20240307"))
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::RequestFailed(_) => {}
        other => panic!("Expected RequestFailed for JSON parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_health_check_succeeds_when_api_available() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [{"type": "text", "text": "Hi"}],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 1, "output_tokens": 1}
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_anthropic_health_check_fails_when_auth_invalid() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Invalid API key"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let result = provider.health_check().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::AuthenticationFailed(_) => {}
        other => panic!("Expected AuthenticationFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn test_anthropic_provider_preserves_request_metadata() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "content": [{"type": "text", "text": "Response"}],
        "model": "claude-3-haiku-20240307",
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 10, "output_tokens": 5}
    });

    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = AnthropicProvider::new(config).unwrap();

    let mut request = test_request("claude-3-haiku-20240307");
    request
        .metadata
        .insert("request_id".to_string(), "test-123".to_string());

    let result = provider.complete(request).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
        response.metadata.get("request_id"),
        Some(&"test-123".to_string())
    );
}

#[test]
fn test_anthropic_provider_creation_requires_api_key() {
    let config = AnthropicConfig::default();
    let result = AnthropicProvider::new(config);

    assert!(result.is_err());
    if let Err(LlmError::NotConfigured(msg)) = result {
        assert!(msg.contains("API key"));
    } else {
        panic!("Expected NotConfigured error");
    }
}

#[test]
fn test_anthropic_provider_reports_correct_name() {
    let config = AnthropicConfig {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = AnthropicProvider::new(config).unwrap();

    assert_eq!(provider.name(), "anthropic");
}

#[test]
fn test_anthropic_provider_lists_available_models() {
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

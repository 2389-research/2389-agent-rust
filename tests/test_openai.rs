//! Integration tests for OpenAI provider
//!
//! Tests behavioral contracts without testing implementation details:
//! - API request/response handling
//! - Error scenarios (rate limits, auth failures, network errors, token limits)
//! - Retry logic and exponential backoff
//! - Tool use integration
//! - Token usage tracking
//! - Model selection

use agent2389::llm::provider::{
    CompletionRequest, FinishReason, LlmError, LlmProvider, Message, MessageRole,
};
use agent2389::llm::providers::openai::{OpenAiConfig, OpenAiProvider};
use std::collections::HashMap;
use std::time::Duration;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn test_config(base_url: &str) -> OpenAiConfig {
    OpenAiConfig {
        api_key: "test-api-key".to_string(),
        base_url: base_url.to_string(),
        timeout: Duration::from_secs(5),
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
async fn test_openai_provider_returns_successful_completion_with_valid_response() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "id": "chatcmpl-123",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "gpt-4",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Hello! How can I assist you today?"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 15,
            "total_tokens": 25
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .and(header("Authorization", "Bearer test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
        response.content,
        Some("Hello! How can I assist you today?".to_string())
    );
    assert_eq!(response.model, "gpt-4");
    assert_eq!(response.usage.prompt_tokens, 10);
    assert_eq!(response.usage.completion_tokens, 15);
    assert_eq!(response.usage.total_tokens, 25);
    assert!(matches!(response.finish_reason, FinishReason::Stop));
}

#[tokio::test]
async fn test_openai_provider_handles_tool_calls_in_response() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_123",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "{\"location\": \"San Francisco\"}"
                            }
                        }
                    ]
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 10,
            "total_tokens": 30
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.tool_calls.is_some());
    let tool_calls = response.tool_calls.unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0].id, "call_123");
    assert_eq!(tool_calls[0].name, "get_weather");
    assert_eq!(
        tool_calls[0].arguments["location"],
        serde_json::json!("San Francisco")
    );
}

#[tokio::test]
async fn test_openai_provider_returns_error_when_api_responds_with_401() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(
            ResponseTemplate::new(401).set_body_string(
                r#"{"error": {"message": "Incorrect API key provided", "type": "invalid_request_error"}}"#,
            ),
        )
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ApiError(msg) => {
            assert!(msg.contains("401"));
            assert!(msg.contains("Incorrect API key"));
        }
        other => panic!("Expected ApiError, got {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_provider_returns_error_when_api_responds_with_429() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(429).set_body_string(
            r#"{"error": {"message": "Rate limit exceeded", "type": "rate_limit_error"}}"#,
        ))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

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
async fn test_openai_provider_detects_token_limit_errors() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"error": {"message": "This model's maximum context length is 8192 tokens"}}"#,
        ))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ApiError(msg) => {
            assert!(msg.contains("maximum context length"));
        }
        other => panic!("Expected ApiError for token limit, got {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_provider_retries_on_server_errors() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service temporarily unavailable"))
        .up_to_n_times(2)
        .mount(&mock_server)
        .await;

    let success_response = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Success after retry"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(success_response))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response.content, Some("Success after retry".to_string()));
}

#[tokio::test]
async fn test_openai_provider_fails_after_all_retries_exhausted() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(503).set_body_string("Service unavailable"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_err());
}

#[tokio::test]
async fn test_openai_provider_converts_length_finish_reason() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Truncated response"
                },
                "finish_reason": "length"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 100,
            "total_tokens": 110
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(matches!(response.finish_reason, FinishReason::Length));
}

#[tokio::test]
async fn test_openai_provider_converts_content_filter_finish_reason() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": null
                },
                "finish_reason": "content_filter"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 0,
            "total_tokens": 10
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(matches!(
        response.finish_reason,
        FinishReason::ContentFilter
    ));
}

#[tokio::test]
async fn test_openai_provider_returns_error_when_choices_empty() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 0,
            "total_tokens": 10
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::ApiError(msg) => {
            assert!(msg.contains("No choices"));
        }
        other => panic!("Expected ApiError for empty choices, got {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_provider_returns_error_when_json_parsing_fails() {
    let mock_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_string("Invalid JSON"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::RequestFailed(_) => {}
        other => panic!("Expected RequestFailed for JSON parse error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_provider_handles_invalid_tool_call_arguments() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [
                        {
                            "id": "call_123",
                            "type": "function",
                            "function": {
                                "name": "get_weather",
                                "arguments": "invalid json {{"
                            }
                        }
                    ]
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 20,
            "completion_tokens": 10,
            "total_tokens": 30
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.complete(test_request("gpt-4")).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.tool_calls.is_some());
    let tool_calls = response.tool_calls.unwrap();
    assert_eq!(tool_calls.len(), 0);
}

#[tokio::test]
async fn test_openai_health_check_succeeds_when_models_endpoint_available() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "object": "list",
        "data": [
            {"id": "gpt-4", "object": "model"},
            {"id": "gpt-3.5-turbo", "object": "model"}
        ]
    });

    Mock::given(method("GET"))
        .and(path("/models"))
        .and(header("Authorization", "Bearer test-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.health_check().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_openai_health_check_fails_when_auth_invalid() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/models"))
        .respond_with(ResponseTemplate::new(401).set_body_string("Unauthorized"))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let result = provider.health_check().await;
    assert!(result.is_err());
    match result.unwrap_err() {
        LlmError::AuthenticationFailed(_) => {}
        other => panic!("Expected AuthenticationFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_provider_preserves_request_metadata() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Response"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 10,
            "completion_tokens": 5,
            "total_tokens": 15
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let mut request = test_request("gpt-4");
    request
        .metadata
        .insert("request_id".to_string(), "test-456".to_string());

    let result = provider.complete(request).await;
    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(
        response.metadata.get("request_id"),
        Some(&"test-456".to_string())
    );
}

#[tokio::test]
async fn test_openai_provider_handles_multiple_message_roles() {
    let mock_server = MockServer::start().await;

    let response_body = serde_json::json!({
        "model": "gpt-4",
        "choices": [
            {
                "message": {
                    "role": "assistant",
                    "content": "Response"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 30,
            "completion_tokens": 5,
            "total_tokens": 35
        }
    });

    Mock::given(method("POST"))
        .and(path("/chat/completions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(response_body))
        .mount(&mock_server)
        .await;

    let config = test_config(&mock_server.uri());
    let provider = OpenAiProvider::new(config).unwrap();

    let mut request = test_request("gpt-4");
    request.messages = vec![
        Message {
            role: MessageRole::System,
            content: "You are helpful".to_string(),
        },
        Message {
            role: MessageRole::User,
            content: "Question".to_string(),
        },
        Message {
            role: MessageRole::Assistant,
            content: "Previous answer".to_string(),
        },
        Message {
            role: MessageRole::User,
            content: "Follow-up".to_string(),
        },
    ];

    let result = provider.complete(request).await;
    assert!(result.is_ok());
}

#[test]
fn test_openai_provider_creation_requires_api_key() {
    let config = OpenAiConfig::default();
    let result = OpenAiProvider::new(config);

    assert!(result.is_err());
    if let Err(LlmError::NotConfigured(msg)) = result {
        assert!(msg.contains("API key"));
    } else {
        panic!("Expected NotConfigured error");
    }
}

#[test]
fn test_openai_provider_reports_correct_name() {
    let config = OpenAiConfig {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = OpenAiProvider::new(config).unwrap();

    assert_eq!(provider.name(), "openai");
}

#[test]
fn test_openai_provider_lists_available_models() {
    let config = OpenAiConfig {
        api_key: "test-key".to_string(),
        ..Default::default()
    };
    let provider = OpenAiProvider::new(config).unwrap();

    let models = provider.available_models();
    assert!(!models.is_empty());
    assert!(models.contains(&"gpt-4".to_string()));
    assert!(models.contains(&"gpt-3.5-turbo".to_string()));
    assert!(models.contains(&"gpt-4o".to_string()));
}

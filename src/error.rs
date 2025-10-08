//! RFC-compliant error types for 2389 Agent Protocol
//!
//! This module implements ONLY the error types and codes specified in the RFC.
//! Maps internal errors to protocol-defined error codes for MQTT publishing.

use crate::protocol::messages::{ErrorCode, ErrorDetails, ErrorMessage};
use thiserror::Error;
use uuid::Uuid;

/// Main error type for 2389 Agent Protocol operations
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Tool execution failed: {message}")]
    ToolExecutionFailed { message: String },

    #[error("LLM provider error: {message}")]
    LlmError { message: String },

    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Pipeline depth exceeded: current depth {current}, max depth {max}")]
    PipelineDepthExceeded { current: u32, max: u32 },

    #[error("Internal error: {message}")]
    InternalError { message: String },

    #[error("Transport error: {0}")]
    TransportError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("Configuration error: {0}")]
    ConfigError(#[from] crate::config::ConfigError),

    #[error("Tool error: {0}")]
    ToolError(#[from] crate::tools::ToolError),

    #[error("Routing error: {message}")]
    RoutingError { message: String },
}

impl AgentError {
    /// Convert AgentError to protocol-compliant ErrorMessage for MQTT publishing
    pub fn to_error_message(&self, task_id: Uuid) -> ErrorMessage {
        let (code, message) = match self {
            AgentError::ToolExecutionFailed { message } => {
                (ErrorCode::ToolExecutionFailed, message.clone())
            }
            AgentError::LlmError { message } => (ErrorCode::LlmError, message.clone()),
            AgentError::InvalidInput { message } => (ErrorCode::InvalidInput, message.clone()),
            AgentError::PipelineDepthExceeded { current, max } => (
                ErrorCode::PipelineDepthExceeded,
                format!("Pipeline depth {current} exceeds maximum {max}"),
            ),
            AgentError::InternalError { message } => (ErrorCode::InternalError, message.clone()),
            AgentError::TransportError(e) => {
                (ErrorCode::InternalError, format!("Transport error: {e}"))
            }
            AgentError::ConfigError(e) => (
                ErrorCode::InternalError,
                format!("Configuration error: {e}"),
            ),
            AgentError::ToolError(e) => {
                (ErrorCode::ToolExecutionFailed, format!("Tool error: {e}"))
            }
            AgentError::RoutingError { message } => (ErrorCode::InternalError, message.clone()),
        };

        ErrorMessage {
            error: ErrorDetails {
                code,
                message: sanitize_error_message(&message),
            },
            task_id,
        }
    }

    /// Create tool execution error
    pub fn tool_execution_failed<S: Into<String>>(message: S) -> Self {
        Self::ToolExecutionFailed {
            message: message.into(),
        }
    }

    /// Create LLM error
    pub fn llm_error<S: Into<String>>(message: S) -> Self {
        Self::LlmError {
            message: message.into(),
        }
    }

    /// Create invalid input error
    pub fn invalid_input<S: Into<String>>(message: S) -> Self {
        Self::InvalidInput {
            message: message.into(),
        }
    }

    /// Create pipeline depth exceeded error
    pub fn pipeline_depth_exceeded(current: u32, max: u32) -> Self {
        Self::PipelineDepthExceeded { current, max }
    }

    /// Create internal error
    pub fn internal_error<S: Into<String>>(message: S) -> Self {
        Self::InternalError {
            message: message.into(),
        }
    }
}

/// Sanitize error messages to prevent sensitive data leakage per RFC requirements
fn sanitize_error_message(message: &str) -> String {
    // Remove potential sensitive patterns
    let mut sanitized = message.to_string();

    // Remove common secret patterns
    sanitized = regex::Regex::new(r"(?i)(password|token|key|secret)[=:]\s*\S+")
        .unwrap()
        .replace_all(&sanitized, "${1}=***")
        .to_string();

    // Remove potential file paths that might contain sensitive info
    sanitized =
        regex::Regex::new(r"/[a-zA-Z0-9._/-]+/(secrets?|\.ssh|\.aws|\.config)/[a-zA-Z0-9._/-]+")
            .unwrap()
            .replace_all(&sanitized, "/***REDACTED***/")
            .to_string();

    // Truncate very long messages - ensure total length is <= 500
    if sanitized.len() > 500 {
        let truncate_suffix = "...[truncated]";
        let max_content_len = 500 - truncate_suffix.len();
        sanitized = format!("{}{}", &sanitized[..max_content_len], truncate_suffix);
    }

    sanitized
}

/// Result type for Agent operations
pub type AgentResult<T> = Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_message_creation() {
        let task_id = Uuid::new_v4();
        let error = AgentError::tool_execution_failed("HTTP request failed");

        let error_msg = error.to_error_message(task_id);

        assert_eq!(error_msg.task_id, task_id);
        assert_eq!(error_msg.error.code, ErrorCode::ToolExecutionFailed);
        assert_eq!(error_msg.error.message, "HTTP request failed");
    }

    #[test]
    fn test_pipeline_depth_error() {
        let task_id = Uuid::new_v4();
        let error = AgentError::pipeline_depth_exceeded(17, 16);

        let error_msg = error.to_error_message(task_id);

        assert_eq!(error_msg.error.code, ErrorCode::PipelineDepthExceeded);
        assert!(error_msg.error.message.contains("17"));
        assert!(error_msg.error.message.contains("16"));
    }

    #[test]
    fn test_error_message_sanitization() {
        let task_id = Uuid::new_v4();
        let error =
            AgentError::internal_error("Failed to authenticate: password=secret123 token=abc456");

        let error_msg = error.to_error_message(task_id);

        // Should redact sensitive information
        assert!(!error_msg.error.message.contains("secret123"));
        assert!(!error_msg.error.message.contains("abc456"));
        assert!(error_msg.error.message.contains("password=***"));
        assert!(error_msg.error.message.contains("token=***"));
    }

    #[test]
    fn test_long_message_truncation() {
        let long_message = "x".repeat(600);
        let sanitized = sanitize_error_message(&long_message);

        assert!(sanitized.len() <= 500);
        assert!(sanitized.ends_with("...[truncated]"));
    }

    #[test]
    fn test_file_path_redaction() {
        let message = "Failed to read /home/user/.ssh/id_rsa and /etc/secrets/api.key";
        let sanitized = sanitize_error_message(message);

        assert!(sanitized.contains("/***REDACTED***/"));
        assert!(!sanitized.contains("/home/user/.ssh/id_rsa"));
    }

    // ========== Tests for Error Constructor Functions ==========

    #[test]
    fn test_tool_execution_failed_constructor() {
        let error = AgentError::tool_execution_failed("test error");
        assert!(matches!(error, AgentError::ToolExecutionFailed { .. }));
        assert_eq!(error.to_string(), "Tool execution failed: test error");
    }

    #[test]
    fn test_llm_error_constructor() {
        let error = AgentError::llm_error("model timeout");
        assert!(matches!(error, AgentError::LlmError { .. }));
        assert_eq!(error.to_string(), "LLM provider error: model timeout");
    }

    #[test]
    fn test_invalid_input_constructor() {
        let error = AgentError::invalid_input("missing field");
        assert!(matches!(error, AgentError::InvalidInput { .. }));
        assert_eq!(error.to_string(), "Invalid input: missing field");
    }

    #[test]
    fn test_pipeline_depth_exceeded_constructor() {
        let error = AgentError::pipeline_depth_exceeded(20, 16);
        assert!(matches!(error, AgentError::PipelineDepthExceeded { .. }));
        assert!(error.to_string().contains("20"));
        assert!(error.to_string().contains("16"));
    }

    #[test]
    fn test_internal_error_constructor() {
        let error = AgentError::internal_error("unexpected state");
        assert!(matches!(error, AgentError::InternalError { .. }));
        assert_eq!(error.to_string(), "Internal error: unexpected state");
    }

    // ========== Tests for Error Code Mapping ==========

    #[test]
    fn test_all_error_variants_map_to_protocol_codes() {
        let task_id = Uuid::new_v4();

        // Test each variant maps to correct protocol code
        let tool_error = AgentError::tool_execution_failed("test");
        assert_eq!(
            tool_error.to_error_message(task_id).error.code,
            ErrorCode::ToolExecutionFailed
        );

        let llm_error = AgentError::llm_error("test");
        assert_eq!(
            llm_error.to_error_message(task_id).error.code,
            ErrorCode::LlmError
        );

        let invalid_error = AgentError::invalid_input("test");
        assert_eq!(
            invalid_error.to_error_message(task_id).error.code,
            ErrorCode::InvalidInput
        );

        let depth_error = AgentError::pipeline_depth_exceeded(17, 16);
        assert_eq!(
            depth_error.to_error_message(task_id).error.code,
            ErrorCode::PipelineDepthExceeded
        );

        let internal_error = AgentError::internal_error("test");
        assert_eq!(
            internal_error.to_error_message(task_id).error.code,
            ErrorCode::InternalError
        );
    }

    // ========== Tests for Sanitization Edge Cases ==========

    #[test]
    fn test_sanitize_multiple_secrets() {
        let message = "Auth failed: password=pass1 api_key=key123 secret=hidden token=tok456";
        let sanitized = sanitize_error_message(message);

        assert!(!sanitized.contains("pass1"));
        assert!(!sanitized.contains("key123"));
        assert!(!sanitized.contains("hidden"));
        assert!(!sanitized.contains("tok456"));
        assert!(sanitized.contains("password=***"));
        assert!(sanitized.contains("key=***"));
    }

    #[test]
    fn test_sanitize_case_insensitive() {
        let message = "PASSWORD=secret123 Token=abc Key=xyz";
        let sanitized = sanitize_error_message(message);

        assert!(!sanitized.contains("secret123"));
        assert!(!sanitized.contains("abc"));
        assert!(!sanitized.contains("xyz"));
    }

    #[test]
    fn test_sanitize_with_colons() {
        let message = "password: secret123 token: abc456";
        let sanitized = sanitize_error_message(message);

        assert!(!sanitized.contains("secret123"));
        assert!(!sanitized.contains("abc456"));
    }

    #[test]
    fn test_sanitize_empty_message() {
        let sanitized = sanitize_error_message("");
        assert_eq!(sanitized, "");
    }

    #[test]
    fn test_sanitize_exactly_500_chars() {
        let message = "x".repeat(500);
        let sanitized = sanitize_error_message(&message);
        assert_eq!(sanitized.len(), 500);
        assert!(!sanitized.contains("truncated"));
    }

    #[test]
    fn test_sanitize_aws_config_paths() {
        let message = "Failed to read /home/user/.aws/credentials";
        let sanitized = sanitize_error_message(message);

        assert!(sanitized.contains("/***REDACTED***/"));
        assert!(!sanitized.contains(".aws/credentials"));
    }

    #[test]
    fn test_routing_error_maps_to_internal_error() {
        let task_id = Uuid::new_v4();
        let error = AgentError::RoutingError {
            message: "No route found".to_string(),
        };

        let error_msg = error.to_error_message(task_id);
        assert_eq!(error_msg.error.code, ErrorCode::InternalError);
        assert_eq!(error_msg.error.message, "No route found");
    }
}

//! Topic canonicalization and agent ID validation for 2389 Agent Protocol
//!
//! This module implements the exact topic canonicalization rules and agent ID
//! validation as specified in the 2389 Agent Protocol specification.

use thiserror::Error;

pub fn canonicalize_topic(topic: &str) -> String {
    if topic.is_empty() {
        return "/".to_string();
    }

    // Rule 1: Ensure single leading slash
    let mut result = if topic.starts_with('/') {
        topic.to_string()
    } else {
        format!("/{topic}")
    };

    // Rule 3: Collapse multiple consecutive slashes
    while result.contains("//") {
        result = result.replace("//", "/");
    }

    // Rule 2: Remove trailing slashes (except for root "/")
    if result.len() > 1 && result.ends_with('/') {
        result.pop();
    }

    result
}

pub fn validate_agent_id(agent_id: &str) -> Result<(), ValidationError> {
    if agent_id.is_empty() {
        return Err(ValidationError::EmptyAgentId);
    }

    for ch in agent_id.chars() {
        if !ch.is_ascii_alphanumeric() && ch != '.' && ch != '_' && ch != '-' {
            return Err(ValidationError::InvalidAgentIdChar(ch));
        }
    }

    Ok(())
}

/// Validation errors for agent protocol
#[derive(Debug, Error, PartialEq)]
pub enum ValidationError {
    #[error("Agent ID cannot be empty")]
    EmptyAgentId,
    #[error("Agent ID contains invalid character: '{0}'")]
    InvalidAgentIdChar(char),
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Property-based tests for canonicalization rules
    // These tests will FAIL initially - that's the TDD approach!

    proptest! {
        #[test]
        fn canonicalize_topic_is_idempotent(topic in ".*") {
            // Property: Canonicalizing a topic twice should give the same result
            let first = canonicalize_topic(&topic);
            let second = canonicalize_topic(&first);
            prop_assert_eq!(first, second, "canonicalize_topic should be idempotent");
        }

        #[test]
        fn canonicalize_topic_starts_with_slash(topic in ".*") {
            // Property: All canonicalized topics should start with exactly one slash
            let result = canonicalize_topic(&topic);
            prop_assert!(result.starts_with('/'), "Topic should start with /: {}", result);
            prop_assert!(!result.starts_with("//"), "Topic should not start with //: {}", result);
        }

        #[test]
        fn canonicalize_topic_no_consecutive_slashes(topic in ".*") {
            // Property: Canonicalized topics should never have consecutive slashes
            let result = canonicalize_topic(&topic);
            prop_assert!(!result.contains("//"), "No consecutive slashes allowed: {}", result);
        }

        #[test]
        fn canonicalize_topic_no_trailing_slash(topic in ".*") {
            // Property: Canonicalized topics should not end with slash (except root "/")
            let result = canonicalize_topic(&topic);
            if result.len() > 1 {
                prop_assert!(!result.ends_with('/'), "No trailing slash (except root): {}", result);
            }
        }
    }

    // Concrete test cases from the protocol specification
    #[test]
    fn test_protocol_examples() {
        // These are the exact examples from the technical requirements
        assert_eq!(
            canonicalize_topic("//control//agents/foo/"),
            "/control/agents/foo"
        );
        assert_eq!(
            canonicalize_topic("control/agents/bar"),
            "/control/agents/bar"
        );
        assert_eq!(
            canonicalize_topic("/control/agents/baz"),
            "/control/agents/baz"
        );
    }

    #[test]
    fn test_edge_cases() {
        // Empty string
        assert_eq!(canonicalize_topic(""), "/");

        // Just slashes
        assert_eq!(canonicalize_topic("/"), "/");
        assert_eq!(canonicalize_topic("//"), "/");
        assert_eq!(canonicalize_topic("///"), "/");

        // Single segments
        assert_eq!(canonicalize_topic("test"), "/test");
        assert_eq!(canonicalize_topic("/test"), "/test");
        assert_eq!(canonicalize_topic("/test/"), "/test");
        assert_eq!(canonicalize_topic("//test//"), "/test");

        // Multiple segments
        assert_eq!(canonicalize_topic("a/b/c"), "/a/b/c");
        assert_eq!(canonicalize_topic("/a/b/c"), "/a/b/c");
        assert_eq!(canonicalize_topic("a/b/c/"), "/a/b/c");
        assert_eq!(canonicalize_topic("//a//b//c//"), "/a/b/c");
    }

    #[test]
    fn test_complex_canonicalization() {
        // Complex cases with multiple rule applications
        assert_eq!(
            canonicalize_topic("///control///agents////foo///"),
            "/control/agents/foo"
        );
        assert_eq!(
            canonicalize_topic("control//agents//foo"),
            "/control/agents/foo"
        );
        assert_eq!(
            canonicalize_topic("/control/agents/foo/bar/"),
            "/control/agents/foo/bar"
        );
    }

    // Property-based tests for agent ID validation
    proptest! {
        #[test]
        fn test_valid_agent_id_format(
            // Generate valid agent IDs using the exact character set
            id in "[a-zA-Z0-9._-]{1,64}"
        ) {
            prop_assert!(validate_agent_id(&id).is_ok(), "Valid agent ID should pass: {}", id);
        }

        #[test]
        fn test_invalid_agent_id_chars(
            // Generate strings with at least one invalid character
            id in "[^a-zA-Z0-9._-]{1}[a-zA-Z0-9._-]*"
        ) {
            prop_assert!(validate_agent_id(&id).is_err(), "Invalid agent ID should fail: {}", id);
        }
    }

    #[test]
    fn test_agent_id_validation_examples() {
        // Valid agent IDs
        assert!(validate_agent_id("my-agent").is_ok());
        assert!(validate_agent_id("agent_123").is_ok());
        assert!(validate_agent_id("agent.test").is_ok());
        assert!(validate_agent_id("Agent-1").is_ok());
        assert!(validate_agent_id("a").is_ok());
        assert!(validate_agent_id("123").is_ok());
        assert!(validate_agent_id("test.agent-123_foo").is_ok());

        // Invalid agent IDs
        assert_eq!(validate_agent_id(""), Err(ValidationError::EmptyAgentId));
        assert!(validate_agent_id("agent@host").is_err());
        assert!(validate_agent_id("agent host").is_err()); // space
        assert!(validate_agent_id("agent/path").is_err()); // slash
        assert!(validate_agent_id("agent:port").is_err()); // colon
        assert!(validate_agent_id("agent#tag").is_err()); // hash
        assert!(validate_agent_id("agent$var").is_err()); // dollar
    }

    #[test]
    fn test_agent_id_validation_specific_errors() {
        // Test specific error types
        assert_eq!(validate_agent_id(""), Err(ValidationError::EmptyAgentId));

        // Test that we get the right character in the error
        if let Err(ValidationError::InvalidAgentIdChar(ch)) = validate_agent_id("test@host") {
            assert_eq!(ch, '@');
        } else {
            panic!("Expected InvalidAgentIdChar error");
        }

        if let Err(ValidationError::InvalidAgentIdChar(ch)) = validate_agent_id("test host") {
            assert_eq!(ch, ' ');
        } else {
            panic!("Expected InvalidAgentIdChar error");
        }
    }
}

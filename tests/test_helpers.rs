//! Test helpers and utilities for integration tests

use agent2389::config::{AgentConfig, AgentSection, BudgetConfig, LlmSection, MqttSection};
use std::collections::HashMap;

/// Create a test configuration for integration tests
#[allow(dead_code)]
pub fn test_config() -> AgentConfig {
    AgentConfig {
        agent: AgentSection {
            id: "test-agent".to_string(),
            description: "Test agent for integration tests".to_string(),
            capabilities: vec!["testing".to_string(), "mock-responses".to_string()],
        },
        mqtt: MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        },
        llm: LlmSection {
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            system_prompt: "You are a helpful AI agent.".to_string(),
            temperature: Some(0.7),
            max_tokens: Some(4000),
        },
        tools: HashMap::new(),
        budget: BudgetConfig::default(),
        routing: None, // V2 routing disabled by default in tests
    }
}

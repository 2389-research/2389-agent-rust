//! RFC-compliant configuration system for 2389 Agent Protocol
//!
//! This module implements ONLY the configuration fields specified in RFC Section 9.
//! No additional fields beyond the RFC specification are allowed.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Main agent configuration structure - RFC Section 9 compliant ONLY
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentConfig {
    pub agent: AgentSection,
    pub mqtt: MqttSection,
    pub llm: LlmSection,
    #[serde(default)]
    pub tools: std::collections::HashMap<String, ToolConfig>,
    #[serde(default)]
    pub budget: BudgetConfig,
    /// V2 routing configuration (optional)
    pub routing: Option<RoutingConfig>,
}

/// Agent section - RFC Section 9 fields only
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentSection {
    /// Agent identifier (must match [a-zA-Z0-9._-]+)
    pub id: String,
    /// Description of what this agent does
    pub description: String,
    /// List of agent capabilities for routing and discovery
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// MQTT section - RFC Section 9 fields only
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MqttSection {
    /// MQTT broker URL with protocol and port
    pub broker_url: String,
    /// Environment variable containing username
    pub username_env: Option<String>,
    /// Environment variable containing password
    pub password_env: Option<String>,
    /// Status heartbeat interval in seconds (default: 900 = 15 minutes)
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval_secs: u64,
}

fn default_heartbeat_interval() -> u64 {
    900 // 15 minutes
}

/// LLM section - RFC Section 9 fields only
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmSection {
    /// Provider name (e.g., "anthropic", "openai")
    pub provider: String,
    /// Model identifier
    pub model: String,
    /// Environment variable containing API key
    pub api_key_env: String,
    /// System prompt
    pub system_prompt: String,
    /// Optional temperature (0.0 to 2.0)
    pub temperature: Option<f32>,
    /// Optional max tokens
    pub max_tokens: Option<u32>,
}

/// Tool configuration - RFC Section 9 compliant
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ToolConfig {
    /// Simple form: tool_name = "identifier"
    Simple(String),
    /// Complex form: tool_name = { impl = "identifier", config = { ... } }
    Complex {
        #[serde(rename = "impl")]
        implementation: String,
        #[serde(default)]
        config: std::collections::HashMap<String, serde_json::Value>,
    },
}
/// Budget configuration for tool calls and iterations
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetConfig {
    /// Maximum number of tool calls per task
    pub max_tool_calls: u32,
    /// Maximum number of iterations per task
    pub max_iterations: u32,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            max_tool_calls: 15,
            max_iterations: 8,
        }
    }
}

/// Routing configuration for V2 dynamic routing
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingConfig {
    /// Routing strategy: "llm" or "gatekeeper"
    pub strategy: RoutingStrategy,

    /// Maximum workflow iterations before forced completion
    #[serde(default = "default_max_routing_iterations")]
    pub max_iterations: usize,

    /// LLM router configuration (required if strategy = "llm")
    pub llm: Option<LlmRouterConfig>,

    /// Gatekeeper router configuration (required if strategy = "gatekeeper")
    pub gatekeeper: Option<GatekeeperRouterConfig>,
}

/// Routing strategy selection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RoutingStrategy {
    Llm,
    Gatekeeper,
}

/// LLM router configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmRouterConfig {
    /// LLM provider: "openai" or "anthropic"
    pub provider: String,
    /// Model identifier
    pub model: String,
    /// Temperature for routing decisions (default: 0.1)
    #[serde(default = "default_routing_temperature")]
    pub temperature: f32,
}

/// Gatekeeper router configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatekeeperRouterConfig {
    /// External routing service URL
    pub url: String,
    /// Timeout in milliseconds (default: 5000)
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    /// Retry attempts (default: 3)
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: usize,
}

fn default_max_routing_iterations() -> usize {
    10
}

fn default_routing_temperature() -> f32 {
    0.1
}

fn default_timeout_ms() -> u64 {
    5000
}

fn default_retry_attempts() -> usize {
    3
}

impl RoutingConfig {
    /// Validate routing configuration consistency
    pub fn validate(&self) -> Result<(), ConfigError> {
        match self.strategy {
            RoutingStrategy::Llm => {
                if self.llm.is_none() {
                    return Err(ConfigError::InvalidConfig(
                        "LLM routing strategy requires [routing.llm] configuration".to_string(),
                    ));
                }
            }
            RoutingStrategy::Gatekeeper => {
                if self.gatekeeper.is_none() {
                    return Err(ConfigError::InvalidConfig(
                        "Gatekeeper routing strategy requires [routing.gatekeeper] configuration"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Configuration loading errors
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    FileRead(#[from] std::io::Error),
    #[error("Failed to parse TOML: {0}")]
    TomlParse(#[from] toml::de::Error),
    #[error("Environment variable not found: {0}")]
    EnvVarNotFound(String),
    #[error("Invalid agent ID format: {0}")]
    InvalidAgentId(String),
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl AgentConfig {
    /// Load configuration from TOML file with environment variable resolution
    pub fn load_from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let mut config: AgentConfig = toml::from_str(&content)?;

        // Validate agent ID format per RFC
        validate_agent_id(&config.agent.id)?;

        // Validate routing configuration if present
        if let Some(ref routing) = config.routing {
            routing.validate()?;
        }

        // Resolve environment variables
        config.resolve_env_vars()?;

        Ok(config)
    }

    /// Resolve environment variables in configuration
    fn resolve_env_vars(&mut self) -> Result<(), ConfigError> {
        // Resolve MQTT credentials
        if let Some(_username_env) = &self.mqtt.username_env {
            // Environment variables are resolved at runtime, not config load time
        }
        if let Some(_password_env) = &self.mqtt.password_env {
            // Environment variables are resolved at runtime, not config load time
        }

        // LLM API key is resolved at runtime

        Ok(())
    }

    /// Helper method to get environment variable with consistent error handling
    fn get_env_var_optional(env_var_name: Option<&String>) -> Option<String> {
        env_var_name.and_then(|name| std::env::var(name).ok())
    }

    /// Helper method to get environment variable with error propagation
    fn get_env_var_required(env_var_name: &str) -> Result<String, ConfigError> {
        std::env::var(env_var_name)
            .map_err(|_| ConfigError::EnvVarNotFound(env_var_name.to_string()))
    }

    /// Get MQTT username from environment variable
    pub fn get_mqtt_username(&self) -> Option<String> {
        Self::get_env_var_optional(self.mqtt.username_env.as_ref())
    }

    /// Get MQTT password from environment variable
    pub fn get_mqtt_password(&self) -> Option<String> {
        Self::get_env_var_optional(self.mqtt.password_env.as_ref())
    }

    /// Get LLM API key from environment variable
    pub fn get_llm_api_key(&self) -> Result<String, ConfigError> {
        Self::get_env_var_required(&self.llm.api_key_env)
    }

    /// Create a test configuration for unit testing
    #[cfg(test)]
    pub fn test_config() -> Self {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "A test agent"
capabilities = ["testing", "mock-responses", "validation"]

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful AI agent."
temperature = 0.7
max_tokens = 4000

[tools]
"#;
        toml::from_str(toml_content).expect("Test config should parse")
    }
}

/// Validate agent ID format per RFC Section 5.1
fn validate_agent_id(agent_id: &str) -> Result<(), ConfigError> {
    let valid_chars = agent_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-');

    if agent_id.is_empty() || !valid_chars {
        return Err(ConfigError::InvalidAgentId(format!(
            "Agent ID '{agent_id}' must match pattern [a-zA-Z0-9._-]+"
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rfc_compliant_config() {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "A test agent for RFC compliance"

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "MQTT_USERNAME"
password_env = "MQTT_PASSWORD"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are a helpful AI agent."
temperature = 0.7
max_tokens = 4000

[tools]
http_request = "builtin"
file_read = { impl = "builtin", config = { max_size = 1048576 } }
"#;

        let config: AgentConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.agent.id, "test-agent");
        assert_eq!(config.agent.description, "A test agent for RFC compliance");
        assert_eq!(config.mqtt.broker_url, "mqtt://localhost:1883");
        assert_eq!(config.llm.provider, "anthropic");
        assert_eq!(config.llm.temperature, Some(0.7));
        assert_eq!(config.tools.len(), 2);
    }

    #[test]
    fn test_invalid_agent_id() {
        let result = validate_agent_id("invalid@agent");
        assert!(result.is_err());

        let result = validate_agent_id("valid-agent_123.test");
        assert!(result.is_ok());
    }

    #[test]
    fn test_minimal_config() {
        let toml_content = r#"
[agent]
id = "minimal"
description = "Minimal agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."
"#;

        let config: AgentConfig = toml::from_str(toml_content).unwrap();
        assert_eq!(config.agent.id, "minimal");
        assert_eq!(config.llm.temperature, None);
        assert_eq!(config.llm.max_tokens, None);
        assert_eq!(config.tools.len(), 0);
    }

    #[test]
    fn test_routing_config_llm_strategy() {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "Test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."

[routing]
strategy = "llm"
max_iterations = 10

[routing.llm]
provider = "openai"
model = "gpt-4o-mini"
temperature = 0.1
"#;

        let config: AgentConfig = toml::from_str(toml_content).unwrap();
        let routing = config.routing.expect("Routing config should be present");
        assert_eq!(routing.strategy, RoutingStrategy::Llm);
        assert_eq!(routing.max_iterations, 10);

        let llm_config = routing.llm.expect("LLM routing config should be present");
        assert_eq!(llm_config.provider, "openai");
        assert_eq!(llm_config.model, "gpt-4o-mini");
        assert_eq!(llm_config.temperature, 0.1);
    }

    #[test]
    fn test_routing_config_gatekeeper_strategy() {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "Test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."

[routing]
strategy = "gatekeeper"
max_iterations = 15

[routing.gatekeeper]
url = "http://localhost:8080/route"
timeout_ms = 3000
retry_attempts = 5
"#;

        let config: AgentConfig = toml::from_str(toml_content).unwrap();
        let routing = config.routing.expect("Routing config should be present");
        assert_eq!(routing.strategy, RoutingStrategy::Gatekeeper);
        assert_eq!(routing.max_iterations, 15);

        let gk_config = routing
            .gatekeeper
            .expect("Gatekeeper routing config should be present");
        assert_eq!(gk_config.url, "http://localhost:8080/route");
        assert_eq!(gk_config.timeout_ms, 3000);
        assert_eq!(gk_config.retry_attempts, 5);
    }

    #[test]
    fn test_routing_config_defaults() {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "Test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."

[routing]
strategy = "llm"

[routing.llm]
provider = "openai"
model = "gpt-4o-mini"
"#;

        let config: AgentConfig = toml::from_str(toml_content).unwrap();
        let routing = config.routing.expect("Routing config should be present");

        // Test default values
        assert_eq!(routing.max_iterations, 10); // default

        let llm_config = routing.llm.expect("LLM config should be present");
        assert_eq!(llm_config.temperature, 0.1); // default
    }

    #[test]
    fn test_routing_config_missing_llm_when_strategy_llm() {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "Test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."

[routing]
strategy = "llm"
# Missing [routing.llm] section!
"#;

        let result: Result<AgentConfig, _> = toml::from_str(toml_content);
        // Should parse fine - validation happens separately
        assert!(result.is_ok());

        // But routing config should be invalid
        let config = result.unwrap();
        let routing = config.routing.expect("Routing config should be present");
        assert!(routing.llm.is_none(), "LLM config should be None");
    }

    #[test]
    fn test_routing_config_missing_gatekeeper_when_strategy_gatekeeper() {
        let toml_content = r#"
[agent]
id = "test-agent"
description = "Test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."

[routing]
strategy = "gatekeeper"
# Missing [routing.gatekeeper] section!
"#;

        let result: Result<AgentConfig, _> = toml::from_str(toml_content);
        // Should parse fine - validation happens separately
        assert!(result.is_ok());

        // But routing config should be invalid
        let config = result.unwrap();
        let routing = config.routing.expect("Routing config should be present");
        assert!(
            routing.gatekeeper.is_none(),
            "Gatekeeper config should be None"
        );
    }
}

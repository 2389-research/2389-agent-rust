//! Configuration loading and validation tests
//!
//! Tests focus on BEHAVIOR of configuration loading, validation, and error handling.
//! We test observable outcomes, not implementation details of TOML parsing.

use agent2389::config::{AgentConfig, ConfigError};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_config_loads_successfully_from_valid_toml() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"
capabilities = ["testing", "validation"]

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.agent.id, "test-agent");
    assert_eq!(config.agent.description, "A test agent");
    assert_eq!(config.agent.capabilities, vec!["testing", "validation"]);
    assert_eq!(config.mqtt.broker_url, "mqtt://localhost:1883");
    assert_eq!(config.llm.provider, "anthropic");
    assert_eq!(config.llm.model, "claude-sonnet-4-20250514");
}

#[test]
fn test_config_loads_with_optional_fields() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "MQTT_USER"
password_env = "MQTT_PASS"

[llm]
provider = "openai"
model = "gpt-4"
api_key_env = "OPENAI_API_KEY"
system_prompt = "You are helpful."
temperature = 0.5
max_tokens = 2000

[budget]
max_tool_calls = 20
max_iterations = 10
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.mqtt.username_env, Some("MQTT_USER".to_string()));
    assert_eq!(config.mqtt.password_env, Some("MQTT_PASS".to_string()));
    assert_eq!(config.llm.temperature, Some(0.5));
    assert_eq!(config.llm.max_tokens, Some(2000));
    assert_eq!(config.budget.max_tool_calls, 20);
    assert_eq!(config.budget.max_iterations, 10);
}

#[test]
fn test_config_applies_default_budget_when_not_specified() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.budget.max_tool_calls, 15);
    assert_eq!(config.budget.max_iterations, 8);
}

#[test]
fn test_config_loads_with_simple_tool() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."

[tools]
http_request = "builtin"
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.tools.len(), 1);
    assert!(config.tools.contains_key("http_request"));
}

#[test]
fn test_config_loads_with_complex_tool() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."

[tools.file_read]
impl = "builtin"
config = {{ max_size = 1048576 }}
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.tools.len(), 1);
    assert!(config.tools.contains_key("file_read"));
}

#[test]
fn test_config_returns_error_when_agent_section_missing() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for missing agent section"),
    }
}

#[test]
fn test_config_returns_error_when_mqtt_section_missing() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for missing mqtt section"),
    }
}

#[test]
fn test_config_returns_error_when_llm_section_missing() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for missing llm section"),
    }
}

#[test]
fn test_config_returns_error_for_invalid_toml_syntax() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent
id = "test-agent"
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for invalid TOML syntax"),
    }
}

#[test]
fn test_config_returns_error_for_empty_file() {
    let temp_file = NamedTempFile::new().unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
}

#[test]
fn test_config_returns_error_when_agent_id_missing() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for missing agent ID"),
    }
}

#[test]
fn test_config_returns_error_when_broker_url_missing() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for missing broker URL"),
    }
}

#[test]
fn test_config_returns_error_when_llm_provider_missing() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::TomlParse(_)) => {}
        _ => panic!("Expected TomlParse error for missing provider"),
    }
}

#[test]
fn test_config_returns_error_for_invalid_agent_id_with_special_chars() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "invalid@agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::InvalidAgentId(_)) => {}
        _ => panic!("Expected InvalidAgentId error for invalid characters"),
    }
}

#[test]
fn test_config_returns_error_for_empty_agent_id() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = ""
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let result = AgentConfig::load_from_file(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(ConfigError::InvalidAgentId(_)) => {}
        _ => panic!("Expected InvalidAgentId error for empty agent ID"),
    }
}

#[test]
fn test_config_accepts_valid_agent_id_with_allowed_chars() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "valid-agent_123.test"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.agent.id, "valid-agent_123.test");
}

#[test]
fn test_config_returns_error_when_file_not_found() {
    use std::path::Path;

    let result = AgentConfig::load_from_file(Path::new("/nonexistent/config.toml"));

    assert!(result.is_err());
    match result {
        Err(ConfigError::FileRead(_)) => {}
        _ => panic!("Expected FileRead error for nonexistent file"),
    }
}

#[test]
fn test_get_mqtt_username_returns_none_when_not_configured() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.get_mqtt_username(), None);
}

#[test]
fn test_get_mqtt_password_returns_none_when_not_configured() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.get_mqtt_password(), None);
}

#[test]
fn test_get_mqtt_username_retrieves_from_environment() {
    unsafe {
        std::env::set_var("TEST_MQTT_USER", "test_user");
    }

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "TEST_MQTT_USER"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.get_mqtt_username(), Some("test_user".to_string()));

    unsafe {
        std::env::remove_var("TEST_MQTT_USER");
    }
}

#[test]
fn test_get_mqtt_password_retrieves_from_environment() {
    unsafe {
        std::env::set_var("TEST_MQTT_PASS", "test_pass");
    }

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"
password_env = "TEST_MQTT_PASS"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.get_mqtt_password(), Some("test_pass".to_string()));

    unsafe {
        std::env::remove_var("TEST_MQTT_PASS");
    }
}

#[test]
fn test_get_mqtt_username_returns_none_when_env_var_not_set() {
    unsafe {
        std::env::remove_var("NONEXISTENT_USER_VAR");
    }

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"
username_env = "NONEXISTENT_USER_VAR"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.get_mqtt_username(), None);
}

#[test]
fn test_get_llm_api_key_retrieves_from_environment() {
    unsafe {
        std::env::set_var("TEST_API_KEY", "sk-test123");
    }

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "TEST_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.get_llm_api_key().unwrap(), "sk-test123");

    unsafe {
        std::env::remove_var("TEST_API_KEY");
    }
}

#[test]
fn test_get_llm_api_key_returns_error_when_env_var_not_set() {
    unsafe {
        std::env::remove_var("NONEXISTENT_API_KEY");
    }

    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "NONEXISTENT_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    let result = config.get_llm_api_key();

    assert!(result.is_err());
    match result {
        Err(ConfigError::EnvVarNotFound(var)) => {
            assert_eq!(var, "NONEXISTENT_API_KEY");
        }
        _ => panic!("Expected EnvVarNotFound error"),
    }
}

#[test]
fn test_config_handles_multiple_capabilities() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"
capabilities = ["cap1", "cap2", "cap3"]

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.agent.capabilities.len(), 3);
    assert_eq!(config.agent.capabilities[0], "cap1");
    assert_eq!(config.agent.capabilities[1], "cap2");
    assert_eq!(config.agent.capabilities[2], "cap3");
}

#[test]
fn test_config_handles_empty_capabilities() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"
capabilities = []

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.agent.capabilities.len(), 0);
}

#[test]
fn test_config_defaults_empty_capabilities_when_not_specified() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert_eq!(config.agent.capabilities.len(), 0);
}

#[test]
fn test_config_accepts_different_mqtt_broker_url_formats() {
    let test_cases = vec![
        ("mqtt://localhost:1883", "mqtt://localhost:1883"),
        (
            "mqtts://broker.example.com:8883",
            "mqtts://broker.example.com:8883",
        ),
        ("tcp://192.168.1.1:1883", "tcp://192.168.1.1:1883"),
    ];

    for (broker_url, expected) in test_cases {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "{broker_url}"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = "You are helpful."
"#
        )
        .unwrap();

        let config = AgentConfig::load_from_file(temp_file.path()).unwrap();
        assert_eq!(config.mqtt.broker_url, expected);
    }
}

#[test]
fn test_config_accepts_different_llm_providers() {
    let providers = vec!["anthropic", "openai", "custom-provider"];

    for provider in providers {
        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(
            temp_file,
            r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "{provider}"
model = "test-model"
api_key_env = "API_KEY"
system_prompt = "You are helpful."
"#
        )
        .unwrap();

        let config = AgentConfig::load_from_file(temp_file.path()).unwrap();
        assert_eq!(config.llm.provider, provider);
    }
}

#[test]
fn test_config_handles_multiline_system_prompt() {
    let mut temp_file = NamedTempFile::new().unwrap();
    writeln!(
        temp_file,
        r#"
[agent]
id = "test-agent"
description = "A test agent"

[mqtt]
broker_url = "mqtt://localhost:1883"

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20250514"
api_key_env = "ANTHROPIC_API_KEY"
system_prompt = """
You are a helpful AI agent.
You provide clear and concise responses.
Always be professional.
"""
"#
    )
    .unwrap();

    let config = AgentConfig::load_from_file(temp_file.path()).unwrap();

    assert!(config.llm.system_prompt.contains("helpful AI agent"));
    assert!(config.llm.system_prompt.contains("clear and concise"));
    assert!(config.llm.system_prompt.contains("professional"));
}

//! RFC-compliant tool system for 2389 Agent Protocol
//!
//! This module implements ONLY the tool interface specified in RFC Section 8.
//! No additional functionality beyond the RFC specification is allowed.

use crate::config::ToolConfig;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;

pub mod builtin;

/// RFC Section 8: Tool interface specification
#[async_trait]
pub trait Tool: Send + Sync {
    /// RFC Section 8.1: describe() Method
    /// Returns JSON-serializable structure conforming to JSON Schema Draft 2020-12 subset
    fn describe(&self) -> ToolDescription;

    /// RFC Section 8.2: initialize(config) Method
    /// Receives configuration dictionary from agent.toml
    /// Called once at agent startup
    async fn initialize(&mut self, config: Option<&Value>) -> Result<(), ToolError>;

    /// RFC Section 8.3: execute(parameters) Method
    /// Receives parameters matching schema from describe()
    /// Parameters MUST be validated against schema before execution
    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError>;

    /// RFC Section 8.4: shutdown() Method [OPTIONAL]
    /// Performs cleanup (close connections, release resources)
    async fn shutdown(&mut self) -> Result<(), ToolError> {
        Ok(())
    }
}

/// Tool description per RFC Section 8.1
#[derive(Debug, Clone)]
pub struct ToolDescription {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

/// Tool system for managing and executing RFC-compliant tools
pub struct ToolSystem {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolSystem {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Initialize tool system with configuration from agent.toml
    pub async fn initialize(
        &mut self,
        tool_configs: &HashMap<String, ToolConfig>,
    ) -> Result<(), ToolError> {
        for (tool_name, tool_config) in tool_configs {
            let mut tool = self.create_tool(tool_name, tool_config)?;

            // Extract config for initialize() method
            let config = match tool_config {
                ToolConfig::Simple(_) => None,
                ToolConfig::Complex { config, .. } => Some(serde_json::to_value(config).unwrap()),
            };

            // RFC Section 8.2: initialize(config) method
            tool.initialize(config.as_ref()).await?;

            self.tools.insert(tool_name.clone(), tool);
        }

        Ok(())
    }

    /// Create tool instance based on configuration
    fn create_tool(
        &self,
        tool_name: &str,
        config: &ToolConfig,
    ) -> Result<Box<dyn Tool>, ToolError> {
        let impl_name = match config {
            ToolConfig::Simple(impl_name) => impl_name,
            ToolConfig::Complex { implementation, .. } => implementation,
        };

        match impl_name.as_str() {
            "builtin" => Ok(self.create_builtin_tool(tool_name)?),
            _ => Err(ToolError::UnknownImplementation(impl_name.clone())),
        }
    }

    /// Create builtin tool instances
    fn create_builtin_tool(&self, tool_name: &str) -> Result<Box<dyn Tool>, ToolError> {
        match tool_name {
            "http_request" => Ok(Box::new(builtin::HttpRequestTool::new())),
            "file_read" => Ok(Box::new(builtin::FileReadTool::new())),
            "file_write" => Ok(Box::new(builtin::FileWriteTool::new())),
            "web_search" => Ok(Box::new(builtin::WebSearchTool::new())),
            _ => Err(ToolError::UnknownTool(tool_name.to_string())),
        }
    }

    /// Get tool description
    pub fn describe_tool(&self, tool_name: &str) -> Option<ToolDescription> {
        self.tools.get(tool_name).map(|tool| tool.describe())
    }

    /// Execute tool with validated parameters
    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: &Value,
    ) -> Result<Value, ToolError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ToolError::UnknownTool(tool_name.to_string()))?;

        // RFC Section 8.3: Parameters MUST be validated against schema before execution
        self.validate_parameters(tool_name, parameters)?;

        tool.execute(parameters).await
    }

    /// Validate parameters against tool schema per RFC Section 8.3
    fn validate_parameters(&self, tool_name: &str, parameters: &Value) -> Result<(), ToolError> {
        let tool = self
            .tools
            .get(tool_name)
            .ok_or_else(|| ToolError::UnknownTool(tool_name.to_string()))?;

        let description = tool.describe();
        let validator = jsonschema::validator_for(&description.parameters)
            .map_err(|e| ToolError::SchemaError(format!("Schema compilation error: {e}")))?;

        validator.validate(parameters).map_err(|errors| {
            let error_messages: Vec<String> = errors
                .map(|e| format!("At '{}': {}", e.instance_path, e))
                .collect();
            ToolError::ValidationError(error_messages.join("; "))
        })
    }

    /// Get list of available tools
    pub fn list_tools(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Shutdown all tools
    pub async fn shutdown(&mut self) -> Result<(), ToolError> {
        for tool in self.tools.values_mut() {
            tool.shutdown().await?;
        }
        Ok(())
    }
}

impl Default for ToolSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// RFC-compliant tool system errors
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Unknown tool: {0}")]
    UnknownTool(String),
    #[error("Unknown tool implementation: {0}")]
    UnknownImplementation(String),
    #[error("Tool initialization failed: {0}")]
    InitializationError(String),
    #[error("Parameter validation failed: {0}")]
    ValidationError(String),
    #[error("Schema error: {0}")]
    SchemaError(String),
    #[error("Tool execution failed: {0}")]
    ExecutionError(String),
    #[error("Tool shutdown failed: {0}")]
    ShutdownError(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_tool_system_creation() {
        let tool_system = ToolSystem::new();
        assert_eq!(tool_system.list_tools().len(), 0);
    }

    #[tokio::test]
    async fn test_tool_system_initialization() {
        let mut tool_system = ToolSystem::new();
        let mut tool_configs = HashMap::new();

        tool_configs.insert(
            "http_request".to_string(),
            ToolConfig::Simple("builtin".to_string()),
        );

        let result = tool_system.initialize(&tool_configs).await;
        assert!(result.is_ok());
        assert_eq!(tool_system.list_tools().len(), 1);
        assert!(
            tool_system
                .list_tools()
                .contains(&"http_request".to_string())
        );
    }

    #[tokio::test]
    async fn test_unknown_tool_implementation() {
        let mut tool_system = ToolSystem::new();
        let mut tool_configs = HashMap::new();

        tool_configs.insert(
            "test_tool".to_string(),
            ToolConfig::Simple("unknown".to_string()),
        );

        let result = tool_system.initialize(&tool_configs).await;
        assert!(matches!(result, Err(ToolError::UnknownImplementation(_))));
    }

    #[tokio::test]
    async fn test_unknown_builtin_tool() {
        let mut tool_system = ToolSystem::new();
        let mut tool_configs = HashMap::new();

        tool_configs.insert(
            "unknown_tool".to_string(),
            ToolConfig::Simple("builtin".to_string()),
        );

        let result = tool_system.initialize(&tool_configs).await;
        assert!(matches!(result, Err(ToolError::UnknownTool(_))));
    }

    #[tokio::test]
    async fn test_tool_execution_unknown_tool() {
        let tool_system = ToolSystem::new();
        let params = json!({"test": "value"});

        let result = tool_system.execute_tool("unknown", &params).await;
        assert!(matches!(result, Err(ToolError::UnknownTool(_))));
    }
}

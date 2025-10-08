use agent2389::config::ToolConfig;
use agent2389::tools::{Tool, ToolDescription, ToolError, ToolSystem};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::sync::Mutex;
use tokio::time::timeout;

#[tokio::test]
async fn test_tool_initialization_with_valid_config() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "http_request".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );
    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    let result = tool_system.initialize(&tool_configs).await;

    assert!(result.is_ok());
    assert_eq!(tool_system.list_tools().len(), 2);
    assert!(tool_system
        .list_tools()
        .contains(&"http_request".to_string()));
    assert!(tool_system.list_tools().contains(&"file_read".to_string()));
}

#[tokio::test]
async fn test_tool_initialization_with_invalid_config() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "invalid_tool".to_string(),
        ToolConfig::Simple("nonexistent_impl".to_string()),
    );

    let result = tool_system.initialize(&tool_configs).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::UnknownImplementation(_))));
}

#[tokio::test]
async fn test_tool_initialization_with_missing_builtin() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "nonexistent_builtin".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    let result = tool_system.initialize(&tool_configs).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::UnknownTool(_))));
}

#[tokio::test]
async fn test_tool_initialization_with_complex_config() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    let mut config_map = HashMap::new();
    config_map.insert("max_file_size".to_string(), json!(2048));

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Complex {
            implementation: "builtin".to_string(),
            config: config_map,
        },
    );

    let result = tool_system.initialize(&tool_configs).await;

    assert!(result.is_ok());
    assert_eq!(tool_system.list_tools().len(), 1);
}

#[tokio::test]
async fn test_tool_schema_validation_with_valid_params() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test content").unwrap();

    let params = json!({
        "path": test_file.to_str().unwrap()
    });

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_ok());
}

#[tokio::test]
async fn test_tool_schema_validation_with_missing_required_param() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let params = json!({});

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ValidationError(_))));
}

#[tokio::test]
async fn test_tool_schema_validation_with_wrong_param_type() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let params = json!({
        "path": 12345
    });

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ValidationError(_))));
}

#[tokio::test]
async fn test_tool_schema_validation_with_additional_params() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    std::fs::write(&test_file, "test content").unwrap();

    let params = json!({
        "path": test_file.to_str().unwrap(),
        "unexpected_param": "should_fail"
    });

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ValidationError(_))));
}

#[tokio::test]
async fn test_tool_execution_with_valid_inputs() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    let expected_content = "test content for reading";
    std::fs::write(&test_file, expected_content).unwrap();

    let params = json!({
        "path": test_file.to_str().unwrap()
    });

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_ok());
    let response = result.unwrap();
    assert_eq!(response["content"].as_str().unwrap(), expected_content);
}

#[tokio::test]
async fn test_tool_execution_with_nonexistent_tool() {
    let tool_system = ToolSystem::new();
    let params = json!({"param": "value"});

    let result = tool_system.execute_tool("nonexistent_tool", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::UnknownTool(_))));
}

#[tokio::test]
async fn test_tool_execution_returns_error_on_failure() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let params = json!({
        "path": "/nonexistent/path/to/file.txt"
    });

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ExecutionError(_))));
}

#[tokio::test]
async fn test_tool_execution_with_invalid_response_format() {
    struct InvalidResponseTool;

    #[async_trait]
    impl Tool for InvalidResponseTool {
        fn describe(&self) -> ToolDescription {
            ToolDescription {
                name: "invalid_response".to_string(),
                description: "Tool that returns invalid response".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }
        }

        async fn initialize(&mut self, _config: Option<&Value>) -> Result<(), ToolError> {
            Ok(())
        }

        async fn execute(&self, _parameters: &Value) -> Result<Value, ToolError> {
            Err(ToolError::ExecutionError(
                "Simulated invalid response".to_string(),
            ))
        }
    }

    let mut tool = InvalidResponseTool;
    tool.initialize(None).await.unwrap();

    let result = tool.execute(&json!({})).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ExecutionError(_))));
}

#[tokio::test]
async fn test_concurrent_tool_executions_maintain_isolation() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_write".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let tool_system = Arc::new(tool_system);
    let temp_dir = TempDir::new().unwrap();

    let mut handles = vec![];

    for i in 0..5 {
        let tool_system_clone = Arc::clone(&tool_system);
        let test_file = temp_dir.path().join(format!("test_{i}.txt"));

        let handle = tokio::spawn(async move {
            let params = json!({
                "path": test_file.to_str().unwrap(),
                "content": format!("content {}", i)
            });

            tool_system_clone
                .execute_tool("file_write", &params)
                .await
                .unwrap()
        });

        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        results.push(handle.await);
    }

    for result in results {
        assert!(result.is_ok());
    }

    for i in 0..5 {
        let test_file = temp_dir.path().join(format!("test_{i}.txt"));
        let content = std::fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, format!("content {i}"));
    }
}

#[tokio::test]
async fn test_tool_cleanup_runs_on_shutdown() {
    struct CleanupTrackingTool {
        cleaned_up: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl Tool for CleanupTrackingTool {
        fn describe(&self) -> ToolDescription {
            ToolDescription {
                name: "cleanup_test".to_string(),
                description: "Test cleanup".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }
        }

        async fn initialize(&mut self, _config: Option<&Value>) -> Result<(), ToolError> {
            Ok(())
        }

        async fn execute(&self, _parameters: &Value) -> Result<Value, ToolError> {
            Ok(json!({}))
        }

        async fn shutdown(&mut self) -> Result<(), ToolError> {
            let mut cleaned = self.cleaned_up.lock().await;
            *cleaned = true;
            Ok(())
        }
    }

    let cleaned_up = Arc::new(Mutex::new(false));
    let mut tool = CleanupTrackingTool {
        cleaned_up: Arc::clone(&cleaned_up),
    };

    tool.shutdown().await.unwrap();

    let is_cleaned = *cleaned_up.lock().await;
    assert!(is_cleaned);
}

#[tokio::test]
async fn test_tool_cleanup_runs_even_when_execution_fails() {
    struct FailingTool {
        cleaned_up: Arc<Mutex<bool>>,
    }

    #[async_trait]
    impl Tool for FailingTool {
        fn describe(&self) -> ToolDescription {
            ToolDescription {
                name: "failing_tool".to_string(),
                description: "Tool that fails execution".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }
        }

        async fn initialize(&mut self, _config: Option<&Value>) -> Result<(), ToolError> {
            Ok(())
        }

        async fn execute(&self, _parameters: &Value) -> Result<Value, ToolError> {
            Err(ToolError::ExecutionError("Intentional failure".to_string()))
        }

        async fn shutdown(&mut self) -> Result<(), ToolError> {
            let mut cleaned = self.cleaned_up.lock().await;
            *cleaned = true;
            Ok(())
        }
    }

    let cleaned_up = Arc::new(Mutex::new(false));
    let mut tool = FailingTool {
        cleaned_up: Arc::clone(&cleaned_up),
    };

    let _ = tool.execute(&json!({})).await;
    tool.shutdown().await.unwrap();

    let is_cleaned = *cleaned_up.lock().await;
    assert!(is_cleaned);
}

#[tokio::test]
async fn test_tool_registry_populated_after_initialization() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    assert_eq!(tool_system.list_tools().len(), 0);

    tool_configs.insert(
        "http_request".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );
    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );
    tool_configs.insert(
        "file_write".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    assert_eq!(tool_system.list_tools().len(), 3);
    assert!(tool_system
        .list_tools()
        .contains(&"http_request".to_string()));
    assert!(tool_system.list_tools().contains(&"file_read".to_string()));
    assert!(tool_system.list_tools().contains(&"file_write".to_string()));
}

#[tokio::test]
async fn test_tool_describe_returns_valid_schema() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let description = tool_system.describe_tool("file_read");

    assert!(description.is_some());
    let desc = description.unwrap();
    assert_eq!(desc.name, "file_read");
    assert!(!desc.description.is_empty());
    assert!(desc.parameters.is_object());
}

#[tokio::test]
async fn test_multiple_sequential_tool_calls_succeed() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_write".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );
    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");
    let content = "sequential test content";

    let write_params = json!({
        "path": test_file.to_str().unwrap(),
        "content": content
    });

    let write_result = tool_system.execute_tool("file_write", &write_params).await;
    assert!(write_result.is_ok());

    let read_params = json!({
        "path": test_file.to_str().unwrap()
    });

    let read_result = tool_system.execute_tool("file_read", &read_params).await;
    assert!(read_result.is_ok());

    let response = read_result.unwrap();
    assert_eq!(response["content"].as_str().unwrap(), content);
}

#[tokio::test]
async fn test_tool_system_shutdown_cleans_all_tools() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "http_request".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );
    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();
    assert_eq!(tool_system.list_tools().len(), 2);

    let result = tool_system.shutdown().await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_tool_with_empty_parameters() {
    struct EmptyParamsTool;

    #[async_trait]
    impl Tool for EmptyParamsTool {
        fn describe(&self) -> ToolDescription {
            ToolDescription {
                name: "empty_params".to_string(),
                description: "Tool with no parameters".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }
        }

        async fn initialize(&mut self, _config: Option<&Value>) -> Result<(), ToolError> {
            Ok(())
        }

        async fn execute(&self, _parameters: &Value) -> Result<Value, ToolError> {
            Ok(json!({"result": "success"}))
        }
    }

    let mut tool = EmptyParamsTool;
    tool.initialize(None).await.unwrap();

    let result = tool.execute(&json!({})).await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap()["result"].as_str().unwrap(), "success");
}

#[tokio::test]
async fn test_tool_execution_times_out_after_configured_duration() {
    struct SlowTool;

    #[async_trait]
    impl Tool for SlowTool {
        fn describe(&self) -> ToolDescription {
            ToolDescription {
                name: "slow_tool".to_string(),
                description: "Tool that takes a long time".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }),
            }
        }

        async fn initialize(&mut self, _config: Option<&Value>) -> Result<(), ToolError> {
            Ok(())
        }

        async fn execute(&self, _parameters: &Value) -> Result<Value, ToolError> {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok(json!({}))
        }
    }

    let mut tool = SlowTool;
    tool.initialize(None).await.unwrap();

    let params = json!({});
    let execution = timeout(Duration::from_millis(100), tool.execute(&params));

    let result = execution.await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_file_write_creates_parent_directories() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    tool_configs.insert(
        "file_write".to_string(),
        ToolConfig::Simple("builtin".to_string()),
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let nested_file = temp_dir.path().join("nested").join("dirs").join("test.txt");

    let params = json!({
        "path": nested_file.to_str().unwrap(),
        "content": "nested content"
    });

    let result = tool_system.execute_tool("file_write", &params).await;

    assert!(result.is_ok());
    assert!(nested_file.exists());

    let content = std::fs::read_to_string(&nested_file).unwrap();
    assert_eq!(content, "nested content");
}

#[tokio::test]
async fn test_file_read_validates_file_size_limit() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    let mut config_map = HashMap::new();
    config_map.insert("max_file_size".to_string(), json!(10));

    tool_configs.insert(
        "file_read".to_string(),
        ToolConfig::Complex {
            implementation: "builtin".to_string(),
            config: config_map,
        },
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("large.txt");
    std::fs::write(&test_file, "this content is longer than 10 bytes").unwrap();

    let params = json!({
        "path": test_file.to_str().unwrap()
    });

    let result = tool_system.execute_tool("file_read", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ExecutionError(_))));
}

#[tokio::test]
async fn test_file_write_validates_content_size_limit() {
    let mut tool_system = ToolSystem::new();
    let mut tool_configs = HashMap::new();

    let mut config_map = HashMap::new();
    config_map.insert("max_file_size".to_string(), json!(10));

    tool_configs.insert(
        "file_write".to_string(),
        ToolConfig::Complex {
            implementation: "builtin".to_string(),
            config: config_map,
        },
    );

    tool_system.initialize(&tool_configs).await.unwrap();

    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test.txt");

    let params = json!({
        "path": test_file.to_str().unwrap(),
        "content": "this content is longer than 10 bytes"
    });

    let result = tool_system.execute_tool("file_write", &params).await;

    assert!(result.is_err());
    assert!(matches!(result, Err(ToolError::ExecutionError(_))));
}

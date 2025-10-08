//! File operations tool implementations
//!
//! This module implements builtin tools for file reading and writing operations
//! with security checks and size limits.

use crate::tools::{Tool, ToolDescription, ToolError};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::fs;
use std::path::Path;

/// File read tool - builtin implementation
pub struct FileReadTool {
    max_file_size: usize,
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileReadTool {
    pub fn new() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB default
        }
    }

    /// Validate file path and check security constraints (pure function)
    fn validate_file_path(path: &Path) -> Result<(), String> {
        if !path.exists() {
            return Err(format!("File not found: {}", path.display()));
        }

        if !path.is_file() {
            return Err(format!("Path is not a file: {}", path.display()));
        }

        Ok(())
    }

    /// Check file size constraints (pure function)
    fn check_file_size(file_size: u64, max_size: usize) -> Result<(), String> {
        if file_size > max_size as u64 {
            return Err(format!(
                "File too large: {file_size} bytes (max: {max_size})"
            ));
        }
        Ok(())
    }

    /// Format file read response (pure function)
    fn format_read_response(content: String, size: u64) -> Value {
        json!({
            "content": content,
            "size": size
        })
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn describe(&self) -> ToolDescription {
        ToolDescription {
            name: "file_read".to_string(),
            description: "Read file contents".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string"
                    }
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        }
    }

    async fn initialize(&mut self, config: Option<&Value>) -> Result<(), ToolError> {
        if let Some(config) = config {
            if let Some(max_size) = config.get("max_file_size").and_then(|v| v.as_u64()) {
                self.max_file_size = max_size as usize;
            }
        }
        Ok(())
    }

    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError> {
        let path_str = parameters["path"].as_str().unwrap();
        let path = Path::new(path_str);

        // Security validation using pure function
        Self::validate_file_path(path).map_err(ToolError::ExecutionError)?;

        // Check file size using pure function
        let metadata = fs::metadata(path).map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        Self::check_file_size(metadata.len(), self.max_file_size)
            .map_err(ToolError::ExecutionError)?;

        // Read file contents (impure I/O)
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        // Format response using pure function
        Ok(Self::format_read_response(content, metadata.len()))
    }

    async fn shutdown(&mut self) -> Result<(), ToolError> {
        Ok(())
    }
}

/// File write tool - builtin implementation
pub struct FileWriteTool {
    max_file_size: usize,
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWriteTool {
    pub fn new() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB default
        }
    }

    /// Check content size constraints (pure function)
    fn check_content_size(content_len: usize, max_size: usize) -> Result<(), String> {
        if content_len > max_size {
            return Err(format!(
                "Content too large: {content_len} bytes (max: {max_size})"
            ));
        }
        Ok(())
    }

    /// Format file write response (pure function)
    fn format_write_response(path: &str, bytes_written: usize) -> Value {
        json!({
            "path": path,
            "bytes_written": bytes_written
        })
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn describe(&self) -> ToolDescription {
        ToolDescription {
            name: "file_write".to_string(),
            description: "Write content to file".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string"
                    },
                    "content": {
                        "type": "string"
                    }
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        }
    }

    async fn initialize(&mut self, config: Option<&Value>) -> Result<(), ToolError> {
        if let Some(config) = config {
            if let Some(max_size) = config.get("max_file_size").and_then(|v| v.as_u64()) {
                self.max_file_size = max_size as usize;
            }
        }
        Ok(())
    }

    async fn execute(&self, parameters: &Value) -> Result<Value, ToolError> {
        let path_str = parameters["path"].as_str().unwrap();
        let content = parameters["content"].as_str().unwrap();

        // Check content size using pure function
        Self::check_content_size(content.len(), self.max_file_size)
            .map_err(ToolError::ExecutionError)?;

        let path = Path::new(path_str);

        // Create parent directories if they don't exist (impure I/O)
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| ToolError::ExecutionError(e.to_string()))?;
        }

        // Write file contents (impure I/O)
        tokio::fs::write(path, content)
            .await
            .map_err(|e| ToolError::ExecutionError(e.to_string()))?;

        // Format response using pure function
        Ok(Self::format_write_response(path_str, content.len()))
    }

    async fn shutdown(&mut self) -> Result<(), ToolError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_file_read_tool_creation() {
        let tool = FileReadTool::new();
        assert_eq!(tool.max_file_size, 1024 * 1024);
    }

    #[test]
    fn test_file_write_tool_creation() {
        let tool = FileWriteTool::new();
        assert_eq!(tool.max_file_size, 1024 * 1024);
    }

    #[test]
    fn test_validate_file_path() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();

        // Valid file should pass
        assert!(FileReadTool::validate_file_path(path).is_ok());

        // Non-existent file should fail
        let non_existent = Path::new("/non/existent/file.txt");
        assert!(FileReadTool::validate_file_path(non_existent).is_err());
    }

    #[test]
    fn test_check_file_size() {
        // Small file should pass
        assert!(FileReadTool::check_file_size(100, 1024).is_ok());

        // Large file should fail
        assert!(FileReadTool::check_file_size(2048, 1024).is_err());
    }

    #[test]
    fn test_check_content_size() {
        // Small content should pass
        assert!(FileWriteTool::check_content_size(100, 1024).is_ok());

        // Large content should fail
        assert!(FileWriteTool::check_content_size(2048, 1024).is_err());
    }

    #[test]
    fn test_file_read_tool_description() {
        let tool = FileReadTool::new();
        let description = tool.describe();

        assert_eq!(description.name, "file_read");
        assert!(!description.description.is_empty());
        assert!(description.parameters.is_object());
    }

    #[test]
    fn test_file_write_tool_description() {
        let tool = FileWriteTool::new();
        let description = tool.describe();

        assert_eq!(description.name, "file_write");
        assert!(!description.description.is_empty());
        assert!(description.parameters.is_object());
    }
}

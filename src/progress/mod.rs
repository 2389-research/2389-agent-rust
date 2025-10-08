use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub mod mqtt_reporter;
pub use mqtt_reporter::MqttProgressReporter;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressMessage {
    pub agent_id: String,
    pub task_id: Option<String>,
    pub conversation_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub category: ProgressCategory,
    pub event_type: ProgressEventType,
    pub message: String,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProgressCategory {
    General,
    Tool,
    LLM,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProgressEventType {
    TaskStart,
    TaskComplete,
    TaskError,
    StepStart,
    StepComplete,
    ToolCall,
    ToolComplete,
    ToolError,
    LlmRequest,
    LlmResponse,
    LlmError,
    ValidationStart,
    ValidationComplete,
    ValidationError,
    Processing,
    Custom,
}

impl ProgressMessage {
    pub fn new(
        agent_id: String,
        category: ProgressCategory,
        event_type: ProgressEventType,
        message: String,
    ) -> Self {
        Self {
            agent_id,
            task_id: None,
            conversation_id: None,
            timestamp: Utc::now(),
            category,
            event_type,
            message,
            metadata: None,
        }
    }

    pub fn with_task_context(
        mut self,
        task_id: Option<String>,
        conversation_id: Option<String>,
    ) -> Self {
        self.task_id = task_id;
        self.conversation_id = conversation_id;
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    pub fn topic(&self) -> String {
        match self.category {
            ProgressCategory::General => format!("/control/agents/{}/progress", self.agent_id),
            ProgressCategory::Tool => format!("/control/agents/{}/progress/tools", self.agent_id),
            ProgressCategory::LLM => format!("/control/agents/{}/progress/llm", self.agent_id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressConfig {
    pub enabled: bool,
    pub verbosity: ProgressVerbosity,
    pub throttle_ms: u64,
    pub batch_size: usize,
    pub categories: Vec<ProgressCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProgressVerbosity {
    Minimal,
    Normal,
    Verbose,
}

impl Default for ProgressConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            verbosity: ProgressVerbosity::Normal,
            throttle_ms: 100,
            batch_size: 10,
            categories: vec![
                ProgressCategory::General,
                ProgressCategory::Tool,
                ProgressCategory::LLM,
            ],
        }
    }
}

#[async_trait]
pub trait Progress: Send + Sync {
    async fn report_task_start(&self, task_id: &str, conversation_id: &str, message: &str);
    async fn report_task_complete(&self, task_id: &str, conversation_id: &str, message: &str);
    async fn report_task_error(
        &self,
        task_id: Option<&str>,
        conversation_id: Option<&str>,
        message: &str,
    );

    async fn report_step_start(
        &self,
        task_id: &str,
        conversation_id: &str,
        step: u8,
        message: &str,
    );
    async fn report_step_complete(
        &self,
        task_id: &str,
        conversation_id: &str,
        step: u8,
        message: &str,
    );

    async fn report_tool_call(
        &self,
        task_id: &str,
        conversation_id: &str,
        tool_name: &str,
        message: &str,
    );
    async fn report_tool_complete(
        &self,
        task_id: &str,
        conversation_id: &str,
        tool_name: &str,
        message: &str,
    );
    async fn report_tool_error(
        &self,
        task_id: &str,
        conversation_id: &str,
        tool_name: &str,
        message: &str,
    );

    async fn report_llm_request(&self, task_id: &str, conversation_id: &str, message: &str);
    async fn report_llm_response(&self, task_id: &str, conversation_id: &str, message: &str);
    async fn report_llm_error(&self, task_id: &str, conversation_id: &str, message: &str);

    async fn report_validation_start(&self, task_id: &str, conversation_id: &str, message: &str);
    async fn report_validation_complete(&self, task_id: &str, conversation_id: &str, message: &str);
    async fn report_validation_error(&self, task_id: &str, conversation_id: &str, message: &str);

    async fn report_processing(&self, task_id: &str, conversation_id: &str, message: &str);

    async fn report_custom(
        &self,
        category: ProgressCategory,
        event_type: ProgressEventType,
        task_id: Option<&str>,
        conversation_id: Option<&str>,
        message: &str,
        metadata: Option<serde_json::Value>,
    );
}

pub struct NoOpProgress;

#[async_trait]
impl Progress for NoOpProgress {
    async fn report_task_start(&self, _task_id: &str, _conversation_id: &str, _message: &str) {}
    async fn report_task_complete(&self, _task_id: &str, _conversation_id: &str, _message: &str) {}
    async fn report_task_error(
        &self,
        _task_id: Option<&str>,
        _conversation_id: Option<&str>,
        _message: &str,
    ) {
    }

    async fn report_step_start(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _step: u8,
        _message: &str,
    ) {
    }
    async fn report_step_complete(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _step: u8,
        _message: &str,
    ) {
    }

    async fn report_tool_call(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _tool_name: &str,
        _message: &str,
    ) {
    }
    async fn report_tool_complete(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _tool_name: &str,
        _message: &str,
    ) {
    }
    async fn report_tool_error(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _tool_name: &str,
        _message: &str,
    ) {
    }

    async fn report_llm_request(&self, _task_id: &str, _conversation_id: &str, _message: &str) {}
    async fn report_llm_response(&self, _task_id: &str, _conversation_id: &str, _message: &str) {}
    async fn report_llm_error(&self, _task_id: &str, _conversation_id: &str, _message: &str) {}

    async fn report_validation_start(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _message: &str,
    ) {
    }
    async fn report_validation_complete(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _message: &str,
    ) {
    }
    async fn report_validation_error(
        &self,
        _task_id: &str,
        _conversation_id: &str,
        _message: &str,
    ) {
    }

    async fn report_processing(&self, _task_id: &str, _conversation_id: &str, _message: &str) {}

    async fn report_custom(
        &self,
        _category: ProgressCategory,
        _event_type: ProgressEventType,
        _task_id: Option<&str>,
        _conversation_id: Option<&str>,
        _message: &str,
        _metadata: Option<serde_json::Value>,
    ) {
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_message_creation() {
        let msg = ProgressMessage::new(
            "test-agent".to_string(),
            ProgressCategory::General,
            ProgressEventType::TaskStart,
            "Starting task processing".to_string(),
        );

        assert_eq!(msg.agent_id, "test-agent");
        assert_eq!(msg.category, ProgressCategory::General);
        assert_eq!(msg.event_type, ProgressEventType::TaskStart);
        assert_eq!(msg.message, "Starting task processing");
        assert!(msg.task_id.is_none());
        assert!(msg.conversation_id.is_none());
    }

    #[test]
    fn test_progress_message_with_context() {
        let msg = ProgressMessage::new(
            "test-agent".to_string(),
            ProgressCategory::Tool,
            ProgressEventType::ToolCall,
            "Calling web search tool".to_string(),
        )
        .with_task_context(Some("task-123".to_string()), Some("conv-456".to_string()));

        assert_eq!(msg.task_id, Some("task-123".to_string()));
        assert_eq!(msg.conversation_id, Some("conv-456".to_string()));
    }

    #[test]
    fn test_progress_message_topic_routing() {
        let general_msg = ProgressMessage::new(
            "agent-1".to_string(),
            ProgressCategory::General,
            ProgressEventType::TaskStart,
            "Starting".to_string(),
        );
        assert_eq!(general_msg.topic(), "/control/agents/agent-1/progress");

        let tool_msg = ProgressMessage::new(
            "agent-1".to_string(),
            ProgressCategory::Tool,
            ProgressEventType::ToolCall,
            "Tool call".to_string(),
        );
        assert_eq!(tool_msg.topic(), "/control/agents/agent-1/progress/tools");

        let llm_msg = ProgressMessage::new(
            "agent-1".to_string(),
            ProgressCategory::LLM,
            ProgressEventType::LlmRequest,
            "LLM request".to_string(),
        );
        assert_eq!(llm_msg.topic(), "/control/agents/agent-1/progress/llm");
    }

    #[test]
    fn test_progress_config_default() {
        let config = ProgressConfig::default();
        assert!(config.enabled);
        assert_eq!(config.verbosity, ProgressVerbosity::Normal);
        assert_eq!(config.throttle_ms, 100);
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.categories.len(), 3);
    }
}

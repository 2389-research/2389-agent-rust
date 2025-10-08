use super::{
    Progress, ProgressCategory, ProgressConfig, ProgressEventType, ProgressMessage,
    ProgressVerbosity,
};
use crate::transport::Transport;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, trace};

pub struct MqttProgressReporter<T: Transport + 'static> {
    agent_id: String,
    transport: Arc<T>,
    config: Arc<RwLock<ProgressConfig>>,
    message_buffer: Arc<Mutex<VecDeque<ProgressMessage>>>,
}

impl<T: Transport + 'static> MqttProgressReporter<T> {
    pub fn new(agent_id: String, transport: Arc<T>, config: ProgressConfig) -> Self {
        Self {
            agent_id,
            transport,
            config: Arc::new(RwLock::new(config)),
            message_buffer: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    async fn should_report(&self, category: &ProgressCategory) -> bool {
        let config = self.config.read().await;
        config.enabled && config.categories.contains(category)
    }

    async fn buffer_message(&self, mut message: ProgressMessage) {
        let config = self.config.read().await;

        // Filter by verbosity
        match config.verbosity {
            ProgressVerbosity::Minimal => {
                // Only task start/complete/error and critical events
                if !matches!(
                    message.event_type,
                    ProgressEventType::TaskStart
                        | ProgressEventType::TaskComplete
                        | ProgressEventType::TaskError
                        | ProgressEventType::ToolError
                        | ProgressEventType::LlmError
                        | ProgressEventType::ValidationError
                ) {
                    return;
                }
            }
            ProgressVerbosity::Normal => {
                // Skip verbose step-by-step details
                if matches!(
                    message.event_type,
                    ProgressEventType::StepStart
                        | ProgressEventType::StepComplete
                        | ProgressEventType::ValidationStart
                        | ProgressEventType::ValidationComplete
                ) {
                    return;
                }
            }
            ProgressVerbosity::Verbose => {
                // Report everything
            }
        }

        // Set agent_id if not already set
        if message.agent_id.is_empty() {
            message.agent_id = self.agent_id.clone();
        }

        let mut buffer = self.message_buffer.lock().await;
        buffer.push_back(message);

        // Flush immediately for real-time progress updates
        drop(buffer);
        drop(config);
        self.flush_buffer().await;
    }

    async fn flush_buffer(&self) {
        let mut buffer = self.message_buffer.lock().await;
        if buffer.is_empty() {
            return;
        }

        let messages: Vec<ProgressMessage> = buffer.drain(..).collect();
        drop(buffer);

        for message in messages {
            if let Err(e) = self.publish_message(&message).await {
                error!("Failed to publish progress message: {}", e);
            }
        }
    }

    async fn publish_message(
        &self,
        message: &ProgressMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let topic = message.topic();
        let payload = serde_json::to_vec(message)?;

        trace!("Publishing progress: {} -> {}", topic, message.message);

        self.transport.publish(&topic, payload, false).await?;
        Ok(())
    }

    pub async fn update_config(&self, config: ProgressConfig) {
        let mut current_config = self.config.write().await;
        *current_config = config;
        debug!("Progress reporter config updated");
    }

    pub async fn get_config(&self) -> ProgressConfig {
        self.config.read().await.clone()
    }

    async fn create_message(
        &self,
        category: ProgressCategory,
        event_type: ProgressEventType,
        task_id: Option<&str>,
        conversation_id: Option<&str>,
        message: &str,
        metadata: Option<serde_json::Value>,
    ) -> ProgressMessage {
        ProgressMessage::new(
            self.agent_id.clone(),
            category,
            event_type,
            message.to_string(),
        )
        .with_task_context(
            task_id.map(|s| s.to_string()),
            conversation_id.map(|s| s.to_string()),
        )
        .with_metadata(metadata.unwrap_or_default())
    }

    pub fn start_background_flush(self: Arc<Self>) {
        let reporter_clone = Arc::clone(&self);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(500));
            loop {
                interval.tick().await;
                reporter_clone.flush_buffer().await;
            }
        });
    }
}

#[async_trait]
impl<T: Transport + 'static> Progress for MqttProgressReporter<T> {
    async fn report_task_start(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::TaskStart,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_task_complete(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::TaskComplete,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_task_error(
        &self,
        task_id: Option<&str>,
        conversation_id: Option<&str>,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::TaskError,
                task_id,
                conversation_id,
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_step_start(
        &self,
        task_id: &str,
        conversation_id: &str,
        step: u8,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let metadata = serde_json::json!({ "step": step });
        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::StepStart,
                Some(task_id),
                Some(conversation_id),
                message,
                Some(metadata),
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_step_complete(
        &self,
        task_id: &str,
        conversation_id: &str,
        step: u8,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let metadata = serde_json::json!({ "step": step });
        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::StepComplete,
                Some(task_id),
                Some(conversation_id),
                message,
                Some(metadata),
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_tool_call(
        &self,
        task_id: &str,
        conversation_id: &str,
        tool_name: &str,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::Tool).await {
            return;
        }

        let metadata = serde_json::json!({ "tool_name": tool_name });
        let progress_msg = self
            .create_message(
                ProgressCategory::Tool,
                ProgressEventType::ToolCall,
                Some(task_id),
                Some(conversation_id),
                message,
                Some(metadata),
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_tool_complete(
        &self,
        task_id: &str,
        conversation_id: &str,
        tool_name: &str,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::Tool).await {
            return;
        }

        let metadata = serde_json::json!({ "tool_name": tool_name });
        let progress_msg = self
            .create_message(
                ProgressCategory::Tool,
                ProgressEventType::ToolComplete,
                Some(task_id),
                Some(conversation_id),
                message,
                Some(metadata),
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_tool_error(
        &self,
        task_id: &str,
        conversation_id: &str,
        tool_name: &str,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::Tool).await {
            return;
        }

        let metadata = serde_json::json!({ "tool_name": tool_name });
        let progress_msg = self
            .create_message(
                ProgressCategory::Tool,
                ProgressEventType::ToolError,
                Some(task_id),
                Some(conversation_id),
                message,
                Some(metadata),
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_llm_request(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::LLM).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::LLM,
                ProgressEventType::LlmRequest,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_llm_response(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::LLM).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::LLM,
                ProgressEventType::LlmResponse,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_llm_error(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::LLM).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::LLM,
                ProgressEventType::LlmError,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_validation_start(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::ValidationStart,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_validation_complete(
        &self,
        task_id: &str,
        conversation_id: &str,
        message: &str,
    ) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::ValidationComplete,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_validation_error(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::ValidationError,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_processing(&self, task_id: &str, conversation_id: &str, message: &str) {
        if !self.should_report(&ProgressCategory::General).await {
            return;
        }

        let progress_msg = self
            .create_message(
                ProgressCategory::General,
                ProgressEventType::Processing,
                Some(task_id),
                Some(conversation_id),
                message,
                None,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }

    async fn report_custom(
        &self,
        category: ProgressCategory,
        event_type: ProgressEventType,
        task_id: Option<&str>,
        conversation_id: Option<&str>,
        message: &str,
        metadata: Option<serde_json::Value>,
    ) {
        if !self.should_report(&category).await {
            return;
        }

        let progress_msg = self
            .create_message(
                category,
                event_type,
                task_id,
                conversation_id,
                message,
                metadata,
            )
            .await;

        self.buffer_message(progress_msg).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::MockTransport;

    #[tokio::test]
    async fn test_mqtt_progress_reporter_creation() {
        let transport = Arc::new(MockTransport::new());
        let config = ProgressConfig::default();

        let reporter = MqttProgressReporter::new("test-agent".to_string(), transport, config);

        assert_eq!(reporter.agent_id, "test-agent");
    }

    #[tokio::test]
    async fn test_progress_reporting_disabled() {
        let transport = Arc::new(MockTransport::new());
        let config = ProgressConfig {
            enabled: false,
            ..Default::default()
        };

        let reporter =
            MqttProgressReporter::new("test-agent".to_string(), transport.clone(), config);

        reporter
            .report_task_start("task-1", "conv-1", "Starting task")
            .await;

        // Should not publish anything
        let messages = transport.get_published_messages().await;
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_verbosity_filtering() {
        let transport = Arc::new(MockTransport::new());
        let config = ProgressConfig {
            verbosity: ProgressVerbosity::Minimal,
            ..Default::default()
        };

        let reporter =
            MqttProgressReporter::new("test-agent".to_string(), transport.clone(), config);

        // Should report task start (minimal verbosity allows this)
        reporter
            .report_task_start("task-1", "conv-1", "Starting task")
            .await;

        // Should NOT report step start (minimal verbosity filters this out)
        reporter
            .report_step_start("task-1", "conv-1", 1, "Starting step 1")
            .await;

        // Force flush
        reporter.flush_buffer().await;

        let messages = transport.get_published_messages().await;
        assert_eq!(messages.len(), 1);

        let (topic, _) = &messages[0];
        assert_eq!(topic, "/control/agents/test-agent/progress");
    }

    #[tokio::test]
    async fn test_category_filtering() {
        let transport = Arc::new(MockTransport::new());
        let config = ProgressConfig {
            categories: vec![ProgressCategory::General], // Only general, no tools or LLM
            ..Default::default()
        };

        let reporter =
            MqttProgressReporter::new("test-agent".to_string(), transport.clone(), config);

        reporter
            .report_task_start("task-1", "conv-1", "Starting task")
            .await;
        reporter
            .report_tool_call("task-1", "conv-1", "web_search", "Searching web")
            .await;
        reporter
            .report_llm_request("task-1", "conv-1", "Requesting LLM")
            .await;

        reporter.flush_buffer().await;

        let messages = transport.get_published_messages().await;
        assert_eq!(messages.len(), 1); // Only the task start should be reported
    }

    #[tokio::test]
    async fn test_topic_routing() {
        let transport = Arc::new(MockTransport::new());
        let config = ProgressConfig::default();

        let reporter =
            MqttProgressReporter::new("test-agent".to_string(), transport.clone(), config);

        reporter
            .report_task_start("task-1", "conv-1", "Starting task")
            .await;
        reporter
            .report_tool_call("task-1", "conv-1", "web_search", "Searching web")
            .await;
        reporter
            .report_llm_request("task-1", "conv-1", "Requesting LLM")
            .await;

        reporter.flush_buffer().await;

        let messages = transport.get_published_messages().await;
        assert_eq!(messages.len(), 3);

        let topics: Vec<&String> = messages.iter().map(|(topic, _)| topic).collect();
        assert!(topics.contains(&&"/control/agents/test-agent/progress".to_string()));
        assert!(topics.contains(&&"/control/agents/test-agent/progress/tools".to_string()));
        assert!(topics.contains(&&"/control/agents/test-agent/progress/llm".to_string()));
    }
}

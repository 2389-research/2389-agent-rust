//! Mock implementations for testing
//!
//! Provides mock Transport, LlmProvider, ToolSystem, and AgentRegistry implementations
//! to enable comprehensive testing without external dependencies.

use crate::agent::discovery::{AgentInfo, AgentRegistry};
use crate::error::AgentError;
use crate::llm::provider::{
    CompletionRequest, CompletionResponse, FinishReason, LlmError, LlmProvider, TokenUsage,
};
use crate::protocol::messages::{
    AgentStatus, ErrorMessage, ResponseMessage, TaskEnvelope, TaskEnvelopeWrapper,
};
use crate::tools::ToolError;
use crate::transport::{mqtt::ConnectionState, Transport};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub type PublishedMessage = (String, Vec<u8>);

/// Mock transport for testing
#[derive(Debug, Default)]
pub struct MockTransport {
    pub published_tasks: Arc<Mutex<Vec<(String, TaskEnvelope)>>>,
    pub published_responses: Arc<Mutex<Vec<(String, ResponseMessage)>>>,
    pub published_statuses: Arc<Mutex<Vec<AgentStatus>>>,
    pub published_errors: Arc<Mutex<Vec<(String, ErrorMessage)>>>,
    pub published_messages: Arc<Mutex<Vec<PublishedMessage>>>,
    pub should_fail: bool,
    pub task_sender: Arc<Mutex<Option<mpsc::Sender<TaskEnvelopeWrapper>>>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_failure() -> Self {
        Self {
            should_fail: true,
            ..Default::default()
        }
    }

    pub async fn get_published_tasks(&self) -> Vec<(String, TaskEnvelope)> {
        self.published_tasks.lock().await.clone()
    }

    pub async fn get_published_responses(&self) -> Vec<(String, ResponseMessage)> {
        self.published_responses.lock().await.clone()
    }

    pub async fn get_published_statuses(&self) -> Vec<AgentStatus> {
        self.published_statuses.lock().await.clone()
    }

    pub async fn get_published_errors(&self) -> Vec<(String, ErrorMessage)> {
        self.published_errors.lock().await.clone()
    }

    pub async fn get_published_messages(&self) -> Vec<(String, Vec<u8>)> {
        self.published_messages.lock().await.clone()
    }

    pub async fn clear_history(&self) {
        self.published_tasks.lock().await.clear();
        self.published_responses.lock().await.clear();
        self.published_statuses.lock().await.clear();
        self.published_errors.lock().await.clear();
        self.published_messages.lock().await.clear();
    }
}

#[async_trait]
impl Transport for MockTransport {
    type Error = AgentError;

    async fn connect(&mut self) -> Result<(), Self::Error> {
        if self.should_fail {
            Err(AgentError::internal_error("Mock connection failure"))
        } else {
            Ok(())
        }
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn publish_status(&self, status: &AgentStatus) -> Result<(), Self::Error> {
        if self.should_fail {
            return Err(AgentError::internal_error("Mock publish failure"));
        }

        let mut statuses = self.published_statuses.lock().await;
        statuses.push(status.clone());
        Ok(())
    }

    async fn publish_task(
        &self,
        target_agent: &str,
        envelope: &TaskEnvelope,
    ) -> Result<(), Self::Error> {
        if self.should_fail {
            return Err(AgentError::internal_error("Mock publish failure"));
        }

        // Build full topic path like real MQTT transport does
        // If target_agent already looks like a full topic path (starts with /), use it as-is
        let topic = if target_agent.starts_with('/') {
            target_agent.to_string()
        } else {
            format!("/control/agents/{target_agent}/input")
        };
        let mut tasks = self.published_tasks.lock().await;
        tasks.push((topic, envelope.clone()));
        Ok(())
    }

    async fn publish_error(
        &self,
        conversation_id: &str,
        error: &ErrorMessage,
    ) -> Result<(), Self::Error> {
        if self.should_fail {
            return Err(AgentError::internal_error("Mock publish failure"));
        }

        let mut errors = self.published_errors.lock().await;
        errors.push((conversation_id.to_string(), error.clone()));
        Ok(())
    }

    async fn publish_response(
        &self,
        conversation_id: &str,
        response: &ResponseMessage,
    ) -> Result<(), Self::Error> {
        if self.should_fail {
            return Err(AgentError::internal_error("Mock publish failure"));
        }

        let mut responses = self.published_responses.lock().await;
        responses.push((conversation_id.to_string(), response.clone()));
        Ok(())
    }

    async fn subscribe_to_tasks(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn is_connected(&self) -> bool {
        !self.should_fail
    }

    fn connection_state(&self) -> Option<ConnectionState> {
        if self.should_fail {
            Some(ConnectionState::Disconnected(
                "Mock disconnection".to_string(),
            ))
        } else {
            Some(ConnectionState::Connected)
        }
    }

    fn is_permanently_disconnected(&self) -> bool {
        false
    }

    async fn publish(
        &self,
        topic: &str,
        payload: Vec<u8>,
        _retain: bool,
    ) -> Result<(), Self::Error> {
        if self.should_fail {
            return Err(AgentError::internal_error("Mock publish failure"));
        }

        if let Ok(mut published) = self.published_messages.try_lock() {
            published.push((topic.to_string(), payload));
        }
        Ok(())
    }

    fn set_task_sender(&self, sender: mpsc::Sender<TaskEnvelopeWrapper>) {
        if let Ok(mut task_sender) = self.task_sender.try_lock() {
            *task_sender = Some(sender);
        }
    }
}

/// Mock LLM provider for testing
#[derive(Debug)]
pub struct MockLlmProvider {
    pub responses: Vec<String>,
    pub current_response: Arc<Mutex<usize>>,
    pub should_fail: bool,
}

impl MockLlmProvider {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses,
            current_response: Arc::new(Mutex::new(0)),
            should_fail: false,
        }
    }

    pub fn with_failure() -> Self {
        Self {
            responses: vec![],
            current_response: Arc::new(Mutex::new(0)),
            should_fail: true,
        }
    }

    pub fn single_response(response: impl Into<String>) -> Self {
        Self::new(vec![response.into()])
    }
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    fn name(&self) -> &str {
        "mock"
    }

    fn available_models(&self) -> Vec<String> {
        vec!["mock-model".to_string()]
    }

    async fn complete(&self, _request: CompletionRequest) -> Result<CompletionResponse, LlmError> {
        if self.should_fail {
            return Err(LlmError::RequestFailed("Mock LLM failure".to_string()));
        }

        let mut current = self.current_response.lock().await;
        let response_idx = *current % self.responses.len().max(1);
        *current += 1;

        let content = if self.responses.is_empty() {
            "Mock response".to_string()
        } else {
            self.responses[response_idx].clone()
        };

        Ok(CompletionResponse {
            content: Some(content),
            model: "mock-model".to_string(),
            usage: TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            },
            finish_reason: FinishReason::Stop,
            tool_calls: None,
            metadata: HashMap::new(),
        })
    }

    async fn health_check(&self) -> Result<(), LlmError> {
        if self.should_fail {
            Err(LlmError::RequestFailed(
                "Mock health check failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

/// Mock tool system for testing
#[derive(Debug, Default)]
pub struct MockToolSystem {
    pub executed_tools: Arc<Mutex<Vec<(String, Value)>>>,
    pub tool_responses: HashMap<String, Value>,
    pub should_fail: bool,
}

impl MockToolSystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_tool_response(tool_name: impl Into<String>, result: Value) -> Self {
        let mut responses = HashMap::new();
        responses.insert(tool_name.into(), result);

        Self {
            tool_responses: responses,
            ..Default::default()
        }
    }

    pub fn with_failure() -> Self {
        Self {
            should_fail: true,
            ..Default::default()
        }
    }

    pub async fn get_executed_tools(&self) -> Vec<(String, Value)> {
        self.executed_tools.lock().await.clone()
    }

    pub async fn clear_history(&self) {
        self.executed_tools.lock().await.clear();
    }

    pub async fn execute_tool(
        &self,
        tool_name: &str,
        parameters: &Value,
    ) -> Result<Value, ToolError> {
        if self.should_fail {
            return Err(ToolError::ExecutionError(format!(
                "Mock tool failure: {tool_name}"
            )));
        }

        // Record the tool execution
        if let Ok(mut executed) = self.executed_tools.try_lock() {
            executed.push((tool_name.to_string(), parameters.clone()));
        }

        // Return predefined response or default
        Ok(self
            .tool_responses
            .get(tool_name)
            .cloned()
            .unwrap_or_else(|| serde_json::json!({"result": "Mock tool execution"})))
    }

    pub fn list_tools(&self) -> Vec<String> {
        self.tool_responses.keys().cloned().collect()
    }
}

// ========== V2 TESTING SUPPORT ==========

/// Agent decision structure for V2 dynamic routing tests
#[derive(Debug, Clone)]
pub struct AgentDecision {
    /// Whether the workflow is complete
    pub workflow_complete: bool,
    /// Next agent ID to route to (None if workflow complete)
    pub next_agent: Option<String>,
    /// Instruction for the next agent
    pub next_instruction: Option<String>,
    /// Result data to pass to next agent
    pub result: Value,
}

impl AgentDecision {
    /// Create a decision to complete the workflow
    pub fn complete(result: Value) -> Self {
        Self {
            workflow_complete: true,
            next_agent: None,
            next_instruction: None,
            result,
        }
    }

    /// Create a decision to route to another agent
    pub fn route_to(
        agent_id: impl Into<String>,
        instruction: impl Into<String>,
        result: Value,
    ) -> Self {
        Self {
            workflow_complete: false,
            next_agent: Some(agent_id.into()),
            next_instruction: Some(instruction.into()),
            result,
        }
    }

    /// Convert to JSON format expected by agent decision parser
    pub fn to_json(&self) -> Value {
        json!({
            "workflow_complete": self.workflow_complete,
            "next_agent": self.next_agent,
            "next_instruction": self.next_instruction,
            "result": self.result,
        })
    }
}

/// Mock agent registry for V2 testing
#[derive(Debug, Clone)]
pub struct MockAgentRegistry {
    registry: AgentRegistry,
    unavailable_agents: Arc<Mutex<Vec<String>>>,
}

impl MockAgentRegistry {
    /// Create a new mock registry
    pub fn new() -> Self {
        Self {
            registry: AgentRegistry::new(),
            unavailable_agents: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register an agent with ID and capabilities
    pub fn register_agent(
        &self,
        agent_id: impl Into<String>,
        capabilities: Vec<impl Into<String>>,
    ) {
        let agent_id = agent_id.into();
        let capabilities: Vec<String> = capabilities.into_iter().map(|c| c.into()).collect();

        let agent_info = AgentInfo {
            agent_id: agent_id.clone(),
            health: "ok".to_string(), // Must be "ok" for is_healthy() to return true
            load: 0.0,
            last_updated: chrono::Utc::now().to_rfc3339(),
            description: Some(format!("Mock agent {agent_id}")),
            capabilities: Some(capabilities),
            handles: None,
            metadata: None,
        };

        self.registry.register_agent(agent_info);
    }

    /// Mark an agent as unavailable for testing routing fallback
    pub async fn set_agent_unavailable(&self, agent_id: impl Into<String>) {
        let mut unavailable = self.unavailable_agents.lock().await;
        unavailable.push(agent_id.into());
    }

    /// Mark an agent as available again
    pub async fn set_agent_available(&self, agent_id: &str) {
        let mut unavailable = self.unavailable_agents.lock().await;
        unavailable.retain(|id| id != agent_id);
    }

    /// Get the underlying AgentRegistry for use in processors
    pub fn registry(&self) -> &AgentRegistry {
        &self.registry
    }

    /// Check if an agent is marked unavailable
    pub async fn is_agent_unavailable(&self, agent_id: &str) -> bool {
        let unavailable = self.unavailable_agents.lock().await;
        unavailable.contains(&agent_id.to_string())
    }

    /// Get all registered agent IDs
    pub fn get_agent_ids(&self) -> Vec<String> {
        self.registry
            .get_healthy_agents()
            .into_iter()
            .map(|agent| agent.agent_id)
            .collect()
    }
}

impl Default for MockAgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Enhanced MockLlmProvider with V2 agent decision support
impl MockLlmProvider {
    /// Create a provider that returns agent decisions in sequence
    pub fn with_agent_decisions(decisions: Vec<AgentDecision>) -> Self {
        let responses: Vec<String> = decisions
            .into_iter()
            .map(|decision| decision.to_json().to_string())
            .collect();

        Self::new(responses)
    }

    /// Create a provider that always completes the workflow
    pub fn always_complete(result: Value) -> Self {
        Self::with_agent_decisions(vec![AgentDecision::complete(result)])
    }

    /// Create a provider that routes to a specific agent
    pub fn route_to_agent(
        agent_id: impl Into<String>,
        instruction: impl Into<String>,
        result: Value,
    ) -> Self {
        Self::with_agent_decisions(vec![AgentDecision::route_to(agent_id, instruction, result)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::messages::TaskEnvelope;
    use serde_json::json;
    use uuid::Uuid;

    #[tokio::test]
    async fn test_mock_transport() {
        let transport = MockTransport::new();

        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/test".to_string(),
            instruction: Some("test instruction".to_string()),
            input: json!({}),
            next: None,
        };

        transport.publish_task("/test", &task).await.unwrap();

        let published = transport.get_published_tasks().await;
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, "/test");
        assert_eq!(published[0].1.task_id, task.task_id);
    }

    #[tokio::test]
    async fn test_mock_llm_provider() {
        let provider = MockLlmProvider::single_response("Test response");

        let request = CompletionRequest {
            messages: vec![],
            model: "test".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            metadata: HashMap::new(),
        };

        let response = provider.complete(request).await.unwrap();
        assert_eq!(response.content, Some("Test response".to_string()));
    }

    #[tokio::test]
    async fn test_mock_tool_system() {
        let tool_system = MockToolSystem::new();

        let result = tool_system
            .execute_tool("test_tool", &json!({"param": "value"}))
            .await
            .unwrap();

        assert_eq!(result, json!({"result": "Mock tool execution"}));

        let executed = tool_system.get_executed_tools().await;
        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0].0, "test_tool");
    }

    // ========== V2 Mock Tests ==========

    #[test]
    fn test_agent_decision_complete() {
        let decision = AgentDecision::complete(json!({"status": "done"}));

        assert!(decision.workflow_complete);
        assert!(decision.next_agent.is_none());
        assert!(decision.next_instruction.is_none());
        assert_eq!(decision.result, json!({"status": "done"}));

        let json_output = decision.to_json();
        assert_eq!(json_output["workflow_complete"], true);
    }

    #[test]
    fn test_agent_decision_route_to() {
        let decision =
            AgentDecision::route_to("processor", "Process the data", json!({"data": "analyzed"}));

        assert!(!decision.workflow_complete);
        assert_eq!(decision.next_agent, Some("processor".to_string()));
        assert_eq!(
            decision.next_instruction,
            Some("Process the data".to_string())
        );
        assert_eq!(decision.result, json!({"data": "analyzed"}));

        let json_output = decision.to_json();
        assert_eq!(json_output["workflow_complete"], false);
        assert_eq!(json_output["next_agent"], "processor");
    }

    #[test]
    fn test_mock_agent_registry_creation() {
        let registry = MockAgentRegistry::new();
        assert_eq!(registry.get_agent_ids().len(), 0);
    }

    #[test]
    fn test_mock_agent_registry_register_agent() {
        let registry = MockAgentRegistry::new();

        registry.register_agent("analyzer", vec!["analysis", "data-processing"]);
        registry.register_agent("processor", vec!["processing"]);

        let agent_ids = registry.get_agent_ids();
        assert_eq!(agent_ids.len(), 2);
        assert!(agent_ids.contains(&"analyzer".to_string()));
        assert!(agent_ids.contains(&"processor".to_string()));
    }

    #[tokio::test]
    async fn test_mock_agent_registry_availability() {
        let registry = MockAgentRegistry::new();
        registry.register_agent("agent1", vec!["capability1"]);

        // Agent should be available initially
        assert!(!registry.is_agent_unavailable("agent1").await);

        // Mark as unavailable
        registry.set_agent_unavailable("agent1").await;
        assert!(registry.is_agent_unavailable("agent1").await);

        // Mark as available again
        registry.set_agent_available("agent1").await;
        assert!(!registry.is_agent_unavailable("agent1").await);
    }

    #[test]
    fn test_mock_agent_registry_underlying_registry() {
        let mock_registry = MockAgentRegistry::new();
        mock_registry.register_agent("test-agent", vec!["test-capability"]);

        let underlying = mock_registry.registry();
        let agent = underlying.get_agent("test-agent");

        assert!(agent.is_some());
        assert_eq!(agent.unwrap().agent_id, "test-agent");
    }

    #[tokio::test]
    async fn test_mock_llm_provider_with_agent_decisions() {
        let decisions = vec![
            AgentDecision::route_to("agent2", "Continue", json!({"step": 1})),
            AgentDecision::complete(json!({"final": "result"})),
        ];

        let provider = MockLlmProvider::with_agent_decisions(decisions);

        let request = CompletionRequest {
            messages: vec![],
            model: "test".to_string(),
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: None,
            stop_sequences: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            metadata: HashMap::new(),
        };

        // First call should return routing decision
        let response1 = provider.complete(request.clone()).await.unwrap();
        let content1: Value = serde_json::from_str(&response1.content.unwrap()).unwrap();
        assert_eq!(content1["workflow_complete"], false);
        assert_eq!(content1["next_agent"], "agent2");

        // Second call should return complete decision
        let response2 = provider.complete(request).await.unwrap();
        let content2: Value = serde_json::from_str(&response2.content.unwrap()).unwrap();
        assert_eq!(content2["workflow_complete"], true);
    }

    #[test]
    fn test_mock_llm_provider_always_complete() {
        let provider = MockLlmProvider::always_complete(json!({"status": "done"}));

        assert_eq!(provider.responses.len(), 1);
        let decision: Value = serde_json::from_str(&provider.responses[0]).unwrap();
        assert_eq!(decision["workflow_complete"], true);
    }

    #[test]
    fn test_mock_llm_provider_route_to_agent() {
        let provider =
            MockLlmProvider::route_to_agent("processor", "Process it", json!({"data": "test"}));

        assert_eq!(provider.responses.len(), 1);
        let decision: Value = serde_json::from_str(&provider.responses[0]).unwrap();
        assert_eq!(decision["workflow_complete"], false);
        assert_eq!(decision["next_agent"], "processor");
        assert_eq!(decision["next_instruction"], "Process it");
    }
}

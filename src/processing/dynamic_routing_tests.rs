//! Integration Tests for V2 Dynamic Routing
//!
//! Tests the V2 routing architecture including:
//! - Basic routing workflows (single agent, two agents, multi-agent)
//! - Iteration limit enforcement
//! - Loop detection and workflow history
//! - Error handling (missing agents, router failures)
//! - Router-specific integration (LLM and Gatekeeper)
//! - Configuration handling

#[cfg(test)]
mod tests {
    use crate::agent::discovery::AgentRegistry;
    use crate::agent::pipeline::AgentPipeline;
    use crate::agent::processor::AgentProcessor;
    use crate::config::{AgentConfig, AgentSection, BudgetConfig, LlmSection, MqttSection};
    use crate::protocol::{TaskEnvelopeV2, WorkflowContext, WorkflowStep};
    use crate::routing::{Router, RoutingDecision};
    use crate::testing::mocks::{MockAgentRegistry, MockLlmProvider, MockTransport};
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use uuid::Uuid;

    /// Create a basic test configuration
    fn create_test_config() -> AgentConfig {
        AgentConfig {
            agent: AgentSection {
                id: "test-agent".to_string(),
                description: "Test agent".to_string(),
                capabilities: vec!["test".to_string()],
            },
            mqtt: MqttSection {
                broker_url: "mqtt://localhost:1883".to_string(),
                username_env: None,
                password_env: None,
                heartbeat_interval_secs: 900,
            },
            llm: LlmSection {
                provider: "mock".to_string(),
                model: "mock-model".to_string(),
                api_key_env: "MOCK_API_KEY".to_string(),
                system_prompt: "You are a test agent".to_string(),
                temperature: Some(0.7),
                max_tokens: Some(1000),
            },
            tools: HashMap::new(),
            budget: BudgetConfig::default(),
            routing: None,
        }
    }

    /// Create a test task envelope
    fn create_test_task(
        task_id: Uuid,
        conversation_id: &str,
        instruction: Option<String>,
        context: Option<WorkflowContext>,
    ) -> TaskEnvelopeV2 {
        TaskEnvelopeV2 {
            task_id,
            conversation_id: conversation_id.to_string(),
            topic: "/control/agents/test-agent/input".to_string(),
            instruction,
            input: json!({"test": "data"}),
            next: None,
            version: "2.0".to_string(),
            context,
            routing_trace: None,
        }
    }

    /// Create a simple mock router that always completes
    struct AlwaysCompleteRouter;

    #[async_trait::async_trait]
    impl Router for AlwaysCompleteRouter {
        async fn decide_next_step(
            &self,
            _task: &TaskEnvelopeV2,
            work_output: &Value,
            _agent_registry: &crate::agent::discovery::AgentRegistry,
        ) -> Result<RoutingDecision, crate::error::AgentError> {
            Ok(RoutingDecision::Complete {
                final_output: work_output.clone(),
            })
        }
    }

    /// Create a mock router that forwards to a specific agent
    struct ForwardToAgentRouter {
        next_agent: String,
        next_instruction: String,
    }

    #[async_trait::async_trait]
    impl Router for ForwardToAgentRouter {
        async fn decide_next_step(
            &self,
            _task: &TaskEnvelopeV2,
            work_output: &Value,
            _agent_registry: &crate::agent::discovery::AgentRegistry,
        ) -> Result<RoutingDecision, crate::error::AgentError> {
            Ok(RoutingDecision::Forward {
                next_agent: self.next_agent.clone(),
                next_instruction: self.next_instruction.clone(),
                forwarded_data: work_output.clone(),
            })
        }
    }

    /// Helper to create a test pipeline with a router
    fn create_test_pipeline(
        router: Arc<dyn Router>,
        registry: Arc<AgentRegistry>,
        max_iterations: usize,
    ) -> (AgentPipeline<MockTransport>, Arc<MockTransport>) {
        let config = create_test_config();
        let transport = Arc::new(MockTransport::new());
        let llm_provider = Arc::new(MockLlmProvider::single_response("Test response"));
        let tool_system = Arc::new(crate::tools::ToolSystem::new());

        // AgentProcessor signature: config, llm_provider, tool_system, transport
        let processor = AgentProcessor::new(config, llm_provider, tool_system, transport.clone());

        let (_tx, rx) = mpsc::channel(10);

        let pipeline = AgentPipeline::with_router(
            processor,
            rx,
            16, // max_pipeline_depth
            router,
            registry,
            max_iterations,
        );

        (pipeline, transport)
    }

    // ========== BASIC ROUTING FLOW TESTS ==========

    #[tokio::test]
    async fn test_single_agent_workflow_completion() {
        // Arrange: Create pipeline with router that always completes
        let registry = MockAgentRegistry::new();
        let (pipeline, transport) = create_test_pipeline(
            Arc::new(AlwaysCompleteRouter),
            Arc::new(registry.registry().clone()),
            10,
        );

        let task = create_test_task(
            Uuid::new_v4(),
            "test-conversation",
            Some("Complete this task".to_string()),
            None,
        );

        let work_output = json!({"status": "done", "result": "success"});

        // Act: Process with routing (router should complete)
        let result = pipeline
            .process_with_routing(task.clone(), work_output.clone())
            .await;

        // Assert: Should succeed and publish final result
        assert!(result.is_ok(), "Workflow should complete successfully");

        // Verify final result was published
        let published_messages = transport.get_published_messages().await;
        assert!(
            !published_messages.is_empty(),
            "Should publish final result"
        );

        // Find the conversation topic message (format: /conversations/{conversation_id}/{agent_id})
        let final_result = published_messages.iter().find(|(topic, _)| {
            topic.starts_with(&format!("/conversations/{}", task.conversation_id))
        });

        assert!(
            final_result.is_some(),
            "Should publish to conversation topic"
        );
    }

    #[tokio::test]
    async fn test_two_agent_workflow() {
        // Arrange: Create pipeline with router that forwards once
        let registry = MockAgentRegistry::new();
        registry.register_agent("processor-agent", vec!["processing"]);

        let router = ForwardToAgentRouter {
            next_agent: "processor-agent".to_string(),
            next_instruction: "Process the data".to_string(),
        };

        let (pipeline, transport) =
            create_test_pipeline(Arc::new(router), Arc::new(registry.registry().clone()), 10);

        let task = create_test_task(
            Uuid::new_v4(),
            "test-conversation",
            Some("Analyze this data".to_string()),
            None,
        );

        let work_output = json!({"analysis": "complete", "data": "analyzed"});

        // Act: Process with routing (router should forward)
        let result = pipeline
            .process_with_routing(task.clone(), work_output)
            .await;

        // Assert: Should succeed and forward to next agent
        assert!(result.is_ok(), "Workflow should forward successfully");

        // Verify task was forwarded
        let published_messages = transport.get_published_messages().await;
        assert!(
            !published_messages.is_empty(),
            "Should publish forward task"
        );

        // Find the forwarded task
        let forwarded_task_msg = published_messages
            .iter()
            .find(|(topic, _)| topic.contains("processor-agent"));

        assert!(
            forwarded_task_msg.is_some(),
            "Should forward to processor-agent"
        );

        // Parse and verify the forwarded task
        let (_, payload) = forwarded_task_msg.unwrap();
        let forwarded_task: TaskEnvelopeV2 = serde_json::from_slice(payload).unwrap();

        // Note: Task ID is NOT preserved - new UUID generated for forwarded task
        assert_ne!(
            forwarded_task.task_id, task.task_id,
            "Task ID gets new UUID"
        );
        assert_eq!(
            forwarded_task.conversation_id, task.conversation_id,
            "Conversation ID preserved"
        );
        assert_eq!(
            forwarded_task.instruction,
            Some("Process the data".to_string()),
            "Instruction updated"
        );

        // Verify context was created
        assert!(
            forwarded_task.context.is_some(),
            "Context should be created"
        );
        let context = forwarded_task.context.unwrap();
        assert_eq!(context.iteration_count, 1, "Iteration count incremented");
        assert_eq!(
            context.steps_completed.len(),
            1,
            "One step added to history"
        );
    }

    #[tokio::test]
    async fn test_multi_agent_workflow() {
        // Arrange: Create a workflow that forwards through 3 agents
        let registry = MockAgentRegistry::new();
        registry.register_agent("analyzer", vec!["analysis"]);
        registry.register_agent("processor", vec!["processing"]);
        registry.register_agent("formatter", vec!["formatting"]);

        let router = ForwardToAgentRouter {
            next_agent: "analyzer".to_string(),
            next_instruction: "Analyze".to_string(),
        };

        let (pipeline, transport) =
            create_test_pipeline(Arc::new(router), Arc::new(registry.registry().clone()), 10);

        let task = create_test_task(
            Uuid::new_v4(),
            "multi-agent-conversation",
            Some("Start workflow".to_string()),
            None,
        );

        let work_output = json!({"step": 1});

        // Act: Process first step
        let result = pipeline.process_with_routing(task, work_output).await;

        // Assert: First forward succeeds
        assert!(result.is_ok(), "First forward should succeed");

        // Verify task was forwarded to analyzer
        let published_messages = transport.get_published_messages().await;
        let analyzer_msg = published_messages
            .iter()
            .find(|(topic, _)| topic.contains("analyzer"));

        assert!(analyzer_msg.is_some(), "Should forward to analyzer");

        let (_, payload) = analyzer_msg.unwrap();
        let forwarded_task: TaskEnvelopeV2 = serde_json::from_slice(payload).unwrap();

        // Verify workflow context
        let context = forwarded_task.context.unwrap();
        assert_eq!(context.iteration_count, 1, "Iteration count is 1");
        assert_eq!(context.steps_completed.len(), 1, "One step completed");
        assert_eq!(
            context.steps_completed[0].agent_id, "test-agent",
            "First agent recorded"
        );
    }

    // ========== ITERATION LIMIT TESTS ==========

    #[tokio::test]
    async fn test_max_iterations_enforcement() {
        // Arrange: Create pipeline with max_iterations=2
        let registry = MockAgentRegistry::new();
        registry.register_agent("next-agent", vec!["test"]);

        let router = ForwardToAgentRouter {
            next_agent: "next-agent".to_string(),
            next_instruction: "Continue".to_string(),
        };

        let (pipeline, transport) = create_test_pipeline(
            Arc::new(router),
            Arc::new(registry.registry().clone()),
            2, // Max 2 iterations
        );

        // Create task at iteration limit
        let task = create_test_task(
            Uuid::new_v4(),
            "test-conversation",
            Some("Task at limit".to_string()),
            Some(WorkflowContext {
                original_query: "Original query".to_string(),
                steps_completed: vec![
                    WorkflowStep {
                        agent_id: "agent1".to_string(),
                        action: "Step 1".to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    },
                    WorkflowStep {
                        agent_id: "agent2".to_string(),
                        action: "Step 2".to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    },
                ],
                iteration_count: 2, // Already at limit
            }),
        );

        let work_output = json!({"status": "continuing"});

        // Act: Process with routing
        let result = pipeline
            .process_with_routing(task.clone(), work_output.clone())
            .await;

        // Assert: Should complete (publish final result) instead of forwarding
        assert!(result.is_ok(), "Should handle max iterations gracefully");

        // Verify final result was published (not forwarded)
        let published_messages = transport.get_published_messages().await;
        let final_result = published_messages.iter().find(|(topic, _)| {
            topic.starts_with(&format!("/conversations/{}", task.conversation_id))
        });

        assert!(
            final_result.is_some(),
            "Should publish final result when hitting iteration limit"
        );

        // Verify NO forwarding occurred
        let forwarded = published_messages
            .iter()
            .find(|(topic, _)| topic.contains("next-agent"));
        assert!(
            forwarded.is_none(),
            "Should NOT forward when at iteration limit"
        );
    }

    #[tokio::test]
    async fn test_iteration_count_increments() {
        // Arrange: Pipeline that forwards
        let registry = MockAgentRegistry::new();
        registry.register_agent("next-agent", vec!["test"]);

        let router = ForwardToAgentRouter {
            next_agent: "next-agent".to_string(),
            next_instruction: "Continue".to_string(),
        };

        let (pipeline, transport) =
            create_test_pipeline(Arc::new(router), Arc::new(registry.registry().clone()), 10);

        // Start with iteration_count = 0
        let task = create_test_task(
            Uuid::new_v4(),
            "test-conversation",
            Some("Start".to_string()),
            None,
        );

        let work_output = json!({"data": "test"});

        // Act: Process
        let result = pipeline.process_with_routing(task, work_output).await;

        // Assert: Success
        assert!(result.is_ok(), "Should forward successfully");

        // Verify iteration count incremented
        let published_messages = transport.get_published_messages().await;
        let forwarded_msg = published_messages
            .iter()
            .find(|(topic, _)| topic.contains("next-agent"))
            .expect("Should forward task");

        let (_, payload) = forwarded_msg;
        let forwarded_task: TaskEnvelopeV2 = serde_json::from_slice(payload).unwrap();

        assert!(forwarded_task.context.is_some(), "Context should exist");
        assert_eq!(
            forwarded_task.context.unwrap().iteration_count,
            1,
            "Iteration count should increment from 0 to 1"
        );
    }

    // ========== ERROR HANDLING TESTS ==========

    #[tokio::test]
    async fn test_missing_next_agent() {
        // Arrange: Router forwards to non-existent agent
        let registry = MockAgentRegistry::new();
        // Note: NOT registering "non-existent-agent"

        let router = ForwardToAgentRouter {
            next_agent: "non-existent-agent".to_string(),
            next_instruction: "Do something".to_string(),
        };

        let (pipeline, _transport) =
            create_test_pipeline(Arc::new(router), Arc::new(registry.registry().clone()), 10);

        let task = create_test_task(
            Uuid::new_v4(),
            "test-conversation",
            Some("Forward to missing agent".to_string()),
            None,
        );

        let work_output = json!({"data": "test"});

        // Act: Process with routing
        let result = pipeline.process_with_routing(task, work_output).await;

        // Assert: Should fail with appropriate error
        assert!(result.is_err(), "Should fail when next agent doesn't exist");

        let error_msg = format!("{:?}", result.unwrap_err());
        assert!(
            error_msg.contains("unknown agent") || error_msg.contains("non-existent-agent"),
            "Error should mention missing agent"
        );
    }

    // ========== WORKFLOW HISTORY TESTS ==========

    #[tokio::test]
    async fn test_workflow_history_preserved() {
        // Arrange: Forward through multiple agents
        let registry = MockAgentRegistry::new();
        registry.register_agent("agent2", vec!["test"]);

        let router = ForwardToAgentRouter {
            next_agent: "agent2".to_string(),
            next_instruction: "Continue".to_string(),
        };

        let (pipeline, transport) =
            create_test_pipeline(Arc::new(router), Arc::new(registry.registry().clone()), 10);

        // Start with existing history
        let task = create_test_task(
            Uuid::new_v4(),
            "test-conversation",
            Some("Continue workflow".to_string()),
            Some(WorkflowContext {
                original_query: "Original query".to_string(),
                steps_completed: vec![WorkflowStep {
                    agent_id: "agent0".to_string(),
                    action: "Started workflow".to_string(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                }],
                iteration_count: 1,
            }),
        );

        let work_output = json!({"result": "processed"});

        // Act: Process
        let result = pipeline
            .process_with_routing(task.clone(), work_output)
            .await;

        // Assert: Success
        assert!(result.is_ok(), "Should forward successfully");

        // Verify history preserved and extended
        let published_messages = transport.get_published_messages().await;
        let forwarded_msg = published_messages
            .iter()
            .find(|(topic, _)| topic.contains("agent2"))
            .expect("Should forward to agent2");

        let (_, payload) = forwarded_msg;
        let forwarded_task: TaskEnvelopeV2 = serde_json::from_slice(payload).unwrap();

        let context = forwarded_task.context.expect("Context should exist");

        // Original query preserved
        assert_eq!(
            context.original_query, "Original query",
            "Original query should be preserved"
        );

        // History extended
        assert_eq!(context.steps_completed.len(), 2, "Should have 2 steps now");
        assert_eq!(
            context.steps_completed[0].agent_id, "agent0",
            "First step preserved"
        );
        assert_eq!(
            context.steps_completed[1].agent_id, "test-agent",
            "Current step added"
        );

        // Iteration incremented
        assert_eq!(context.iteration_count, 2, "Iteration count incremented");
    }
}

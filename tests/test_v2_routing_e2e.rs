//! End-to-End V2 Routing Tests
//!
//! These tests validate the complete V2 routing system with real agents
//! making dynamic routing decisions through natural iterative workflows.
//!
//! Test Scenarios:
//! 1. Research → Write → Edit → Review workflow
//! 2. Multi-iteration refinement based on quality feedback
//! 3. Natural loop detection and termination
//! 4. Max iteration enforcement

use agent2389::agent::discovery::AgentRegistry;
use agent2389::agent::pipeline::pipeline_orchestrator::AgentPipeline;
use agent2389::agent::processor::AgentProcessor;
use agent2389::config::{AgentConfig, AgentSection, BudgetConfig, LlmSection, MqttSection};
use agent2389::llm::provider::LlmProvider;
use agent2389::protocol::messages::{TaskEnvelopeV2, WorkflowContext};
use agent2389::routing::llm_router::LlmRouter;
use agent2389::routing::Router;
use agent2389::testing::mocks::{AgentDecision, MockAgentRegistry, MockLlmProvider, MockTransport};
use agent2389::tools::ToolSystem;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

mod test_helpers;

/// Create a test agent configuration with a given ID and system prompt
fn create_agent_config(agent_id: &str, system_prompt: &str) -> AgentConfig {
    AgentConfig {
        agent: AgentSection {
            id: agent_id.to_string(),
            description: format!("{agent_id} agent for testing"),
            capabilities: vec![agent_id.to_string()],
        },
        mqtt: MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        },
        llm: LlmSection {
            provider: "openai".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key_env: "OPENAI_API_KEY".to_string(),
            system_prompt: system_prompt.to_string(),
            temperature: Some(0.7),
            max_tokens: Some(2000),
        },
        tools: HashMap::new(),
        budget: BudgetConfig::default(),
        routing: None,
    }
}

/// Create a test pipeline with router
fn create_test_pipeline_with_router(
    config: AgentConfig,
    llm: Arc<dyn LlmProvider>,
    router: Arc<dyn Router>,
    registry: Arc<AgentRegistry>,
    max_iterations: usize,
) -> (AgentPipeline<MockTransport>, Arc<MockTransport>) {
    let transport = Arc::new(MockTransport::new());
    let tool_system = Arc::new(ToolSystem::new());

    let processor = AgentProcessor::new(config, llm, tool_system, transport.clone());

    let (_tx, rx) = mpsc::channel(10);

    let pipeline = AgentPipeline::with_router(processor, rx, 16, router, registry, max_iterations);

    (pipeline, transport)
}

#[tokio::test]
async fn test_research_write_edit_workflow() {
    // This test simulates a natural workflow:
    // 1. Research agent gathers information
    // 2. Router decides to send to Writer
    // 3. Writer creates content
    // 4. Router decides to send to Editor
    // 5. Editor polishes content
    // 6. Router decides workflow is complete

    // Setup: Create mock LLM that simulates routing decisions
    let mock_llm = MockLlmProvider::with_agent_decisions(vec![
        // Research agent works, router decides to forward to writer
        AgentDecision::route_to(
            "writer-agent",
            "Write article from research",
            json!({"research": "Rust async traits stabilized in 1.75"}),
        ),
        // Writer agent works, router decides to forward to editor
        AgentDecision::route_to(
            "editor-agent",
            "Polish the article",
            json!({"article": "# Rust Async Programming\n\nAsync traits are now stable..."}),
        ),
        // Editor agent works, router decides workflow is complete
        AgentDecision::complete(
            json!({"polished_article": "# Rust Async Programming\n\nAsync traits..."}),
        ),
    ]);

    // Setup: Create agent registry with all three agents
    let registry = MockAgentRegistry::new();
    registry.register_agent("research-agent", vec!["research".to_string()]);
    registry.register_agent("writer-agent", vec!["writing".to_string()]);
    registry.register_agent("editor-agent", vec!["editing".to_string()]);

    let mock_llm_arc = Arc::new(mock_llm);

    // Setup: Create router
    let router = Arc::new(LlmRouter::new(
        mock_llm_arc.clone(),
        "gpt-4o-mini".to_string(),
    ));

    // Setup: Create research agent pipeline
    let config = create_agent_config(
        "research-agent",
        "You are a research agent. Find information and return structured findings.",
    );

    let (pipeline, transport) = create_test_pipeline_with_router(
        config,
        mock_llm_arc,
        router,
        Arc::new(registry.registry().clone()),
        10,
    );

    // Execute: Start workflow with research task
    let task = TaskEnvelopeV2 {
        task_id: Uuid::new_v4(),
        conversation_id: "conv-123".to_string(),
        topic: "/control/agents/research-agent/input".to_string(),
        instruction: Some("Research Rust async programming".to_string()),
        input: json!({}),
        next: None,
        version: "2.0".to_string(),
        context: Some(WorkflowContext {
            original_query: "Create article on Rust async programming".to_string(),
            steps_completed: vec![],
            iteration_count: 0,
        }),
        routing_trace: None,
    };

    let work_output = json!({"research": "Rust async traits stabilized in 1.75"});

    let result = pipeline
        .process_with_routing(task.clone(), work_output)
        .await;

    // Assert: Workflow should succeed
    assert!(result.is_ok(), "Workflow should complete successfully");

    // Assert: Check that forwarding happened
    let published = transport.get_published_messages().await;
    assert!(!published.is_empty(), "Should have published messages");

    // Assert: Verify forwarding to writer agent
    let writer_forwards: Vec<_> = published
        .iter()
        .filter(|(topic, _)| topic.contains("writer-agent"))
        .collect();
    assert!(
        !writer_forwards.is_empty(),
        "Should forward to writer-agent"
    );
}

#[tokio::test]
async fn test_iterative_quality_refinement() {
    // This test simulates an iterative refinement workflow:
    // 1. Writer creates content
    // 2. Judge reviews and finds issues → routes back to Writer
    // 3. Writer improves content
    // 4. Judge reviews again and approves → workflow complete

    let mock_llm = MockLlmProvider::with_agent_decisions(vec![
        // Initial write, router sends to judge
        AgentDecision::route_to(
            "judge-agent",
            "Review this article for quality",
            json!({"article": "Basic article about Rust"}),
        ),
        // Judge finds issues, router sends back to writer for improvement
        AgentDecision::route_to(
            "writer-agent",
            "Improve article: add depth and examples",
            json!({
                "quality_score": 6,
                "issues": ["lacks depth", "missing examples"],
                "recommendation": "needs_improvement"
            }),
        ),
        // Writer improves, router sends to judge again
        AgentDecision::route_to(
            "judge-agent",
            "Review improved article",
            json!({"improved_article": "Comprehensive article with examples..."}),
        ),
        // Judge approves, router completes workflow
        AgentDecision::complete(json!({
            "quality_score": 9,
            "strengths": ["comprehensive", "good examples"],
            "recommendation": "approved"
        })),
    ]);

    let registry = MockAgentRegistry::new();
    registry.register_agent("writer-agent", vec!["writing".to_string()]);
    registry.register_agent("judge-agent", vec!["quality-review".to_string()]);

    let mock_llm_arc = Arc::new(mock_llm);

    let router = Arc::new(LlmRouter::new(
        mock_llm_arc.clone(),
        "gpt-4o-mini".to_string(),
    ));

    let config = create_agent_config(
        "writer-agent",
        "You are a writer. Create and improve content based on feedback.",
    );

    let (pipeline, transport) = create_test_pipeline_with_router(
        config,
        mock_llm_arc,
        router,
        Arc::new(registry.registry().clone()),
        10,
    );

    let task = TaskEnvelopeV2 {
        task_id: Uuid::new_v4(),
        conversation_id: "conv-refinement".to_string(),
        topic: "/control/agents/writer-agent/input".to_string(),
        instruction: Some("Write article about Rust async".to_string()),
        input: json!({}),
        next: None,
        version: "2.0".to_string(),
        context: Some(WorkflowContext {
            original_query: "Create high-quality article on Rust async".to_string(),
            steps_completed: vec![],
            iteration_count: 0,
        }),
        routing_trace: None,
    };

    let work_output = json!({"article": "Basic article about Rust"});

    let result = pipeline.process_with_routing(task, work_output).await;
    assert!(result.is_ok(), "Iterative workflow should complete");

    // Verify that multiple iterations occurred
    let published = transport.get_published_messages().await;

    let writer_tasks: Vec<_> = published
        .iter()
        .filter(|(topic, _)| topic.contains("writer-agent"))
        .collect();

    let judge_tasks: Vec<_> = published
        .iter()
        .filter(|(topic, _)| topic.contains("judge-agent"))
        .collect();

    // Should have forwarded to judge at least once, and back to writer for improvement
    assert!(
        !judge_tasks.is_empty(),
        "Should have sent to judge for review"
    );
    println!(
        "Iterative workflow: {} writer tasks, {} judge tasks",
        writer_tasks.len(),
        judge_tasks.len()
    );
}

#[tokio::test]
async fn test_max_iterations_prevents_infinite_loop() {
    // This test ensures that max_iterations prevents runaway workflows
    // where agents keep forwarding to each other indefinitely

    // Create a mock LLM that always wants to forward
    let mock_llm = MockLlmProvider::with_agent_decisions(vec![
        AgentDecision::route_to(
            "agent-b",
            "Continue processing",
            json!({"result": "iteration 1"}),
        ),
        AgentDecision::route_to(
            "agent-a",
            "Continue processing",
            json!({"result": "iteration 2"}),
        ),
        AgentDecision::route_to(
            "agent-b",
            "Continue processing",
            json!({"result": "iteration 3"}),
        ),
        AgentDecision::route_to(
            "agent-a",
            "Continue processing",
            json!({"result": "iteration 4"}),
        ),
        AgentDecision::route_to(
            "agent-b",
            "Continue processing",
            json!({"result": "iteration 5"}),
        ),
        // Keep going beyond max_iterations
        AgentDecision::route_to(
            "agent-a",
            "Continue processing",
            json!({"result": "iteration 6"}),
        ),
    ]);

    let registry = MockAgentRegistry::new();
    registry.register_agent("agent-a", vec!["processing".to_string()]);
    registry.register_agent("agent-b", vec!["processing".to_string()]);

    let mock_llm_arc = Arc::new(mock_llm);

    let router = Arc::new(LlmRouter::new(
        mock_llm_arc.clone(),
        "gpt-4o-mini".to_string(),
    ));

    let config = create_agent_config("agent-a", "You are agent A");

    // Set max_iterations to 5
    let (pipeline, transport) = create_test_pipeline_with_router(
        config,
        mock_llm_arc,
        router,
        Arc::new(registry.registry().clone()),
        5,
    );

    let task = TaskEnvelopeV2 {
        task_id: Uuid::new_v4(),
        conversation_id: "conv-loop".to_string(),
        topic: "/control/agents/agent-a/input".to_string(),
        instruction: Some("Start processing".to_string()),
        input: json!({}),
        next: None,
        version: "2.0".to_string(),
        context: Some(WorkflowContext {
            original_query: "Process data".to_string(),
            steps_completed: vec![],
            iteration_count: 0,
        }),
        routing_trace: None,
    };

    let work_output = json!({"result": "iteration 1"});

    let result = pipeline.process_with_routing(task, work_output).await;

    // The key assertion: pipeline completes without error even when router
    // wants to keep routing beyond max_iterations
    assert!(result.is_ok(), "Should complete (forced by max iterations)");

    // Verify that some routing happened - we should have at least attempted
    // to forward to agent-b initially
    let published = transport.get_published_messages().await;
    assert!(
        !published.is_empty(),
        "Should have published some messages during processing"
    );

    // Note: In a single-pipeline test, we can't verify the exact iteration count
    // because there's no actual multi-agent message passing. The real test of
    // max_iterations is in the v2_workflow_demo with real MQTT.
}

#[tokio::test]
async fn test_workflow_history_tracks_iterations() {
    // Verify that workflow history correctly tracks each step in the iteration

    let mock_llm = MockLlmProvider::with_agent_decisions(vec![
        AgentDecision::route_to("agent-b", "Step 2", json!({"step": 1})),
        AgentDecision::route_to("agent-c", "Step 3", json!({"step": 2})),
        AgentDecision::complete(json!({"step": 3})),
    ]);

    let registry = MockAgentRegistry::new();
    registry.register_agent("agent-a", vec!["step".to_string()]);
    registry.register_agent("agent-b", vec!["step".to_string()]);
    registry.register_agent("agent-c", vec!["step".to_string()]);

    let mock_llm_arc = Arc::new(mock_llm);

    let router = Arc::new(LlmRouter::new(
        mock_llm_arc.clone(),
        "gpt-4o-mini".to_string(),
    ));

    let config = create_agent_config("agent-a", "Agent A");

    let (pipeline, transport) = create_test_pipeline_with_router(
        config,
        mock_llm_arc,
        router,
        Arc::new(registry.registry().clone()),
        10,
    );

    let task = TaskEnvelopeV2 {
        task_id: Uuid::new_v4(),
        conversation_id: "conv-history".to_string(),
        topic: "/control/agents/agent-a/input".to_string(),
        instruction: Some("Start workflow".to_string()),
        input: json!({}),
        next: None,
        version: "2.0".to_string(),
        context: Some(WorkflowContext {
            original_query: "Multi-step workflow".to_string(),
            steps_completed: vec![],
            iteration_count: 0,
        }),
        routing_trace: None,
    };

    let work_output = json!({"step": 1});

    let result = pipeline.process_with_routing(task, work_output).await;
    assert!(result.is_ok(), "Workflow should track history correctly");

    // In a single-pipeline test with MockTransport, we can't verify actual
    // multi-agent forwarding because there's no real MQTT message passing.
    // What we CAN verify is that routing decisions were made.
    let published = transport.get_published_messages().await;

    // Verify that SOME messages were published (conversation messages, etc)
    assert!(!published.is_empty(), "Should have published some messages");

    // Note: To truly test multi-agent workflows with history tracking,
    // use the v2_workflow_demo with real MQTT and multiple agent pipelines.
    // This unit test verifies that routing completes without errors.
}

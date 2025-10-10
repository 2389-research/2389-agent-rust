//! V2 Routing Workflow Demonstration
//!
//! This example demonstrates real V2 routing workflows with multiple agents
//! collaborating through dynamic routing decisions.
//!
//! Usage:
//!   cargo run --example v2_workflow_demo --workflow research-write-edit
//!   cargo run --example v2_workflow_demo --workflow iterative --real-llm
//!   cargo run --example v2_workflow_demo --list

use agent2389::agent::discovery::{AgentInfo, AgentRegistry};
use agent2389::agent::pipeline::pipeline_orchestrator::AgentPipeline;
use agent2389::agent::processor::AgentProcessor;
use agent2389::config::AgentConfig;
use agent2389::llm::provider::LlmProvider;
use agent2389::protocol::messages::{TaskEnvelopeV2, WorkflowContext};
use agent2389::routing::llm_router::LlmRouter;
use agent2389::testing::mocks::{AgentDecision, MockLlmProvider};
use agent2389::tools::ToolSystem;
use agent2389::transport::{MqttTransport, Transport};
use clap::{Parser, ValueEnum};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// MQTT broker URL - always at localhost:1883
const MQTT_BROKER_URL: &str = "mqtt://localhost:1883";

#[derive(Parser)]
#[command(name = "v2-workflow-demo")]
#[command(about = "Demonstrates V2 routing with real agent workflows")]
struct Cli {
    /// Workflow type to run
    #[arg(short, long, value_enum)]
    workflow: Option<WorkflowType>,

    /// List available workflows
    #[arg(short, long)]
    list: bool,

    /// Use real LLM instead of mock (requires API keys)
    #[arg(long)]
    real_llm: bool,

    /// Timeout in seconds
    #[arg(short, long, default_value = "60")]
    timeout: u64,
}

#[derive(Clone, ValueEnum)]
enum WorkflowType {
    /// Linear workflow: Research → Writer → Editor
    ResearchWriteEdit,
    /// Iterative refinement: Write → Judge → Write (improve) → Judge
    Iterative,
    /// Infinite loop prevention test
    PingPong,
}

impl WorkflowType {
    fn name(&self) -> &str {
        match self {
            Self::ResearchWriteEdit => "Research → Write → Edit",
            Self::Iterative => "Iterative Write → Judge → Refine",
            Self::PingPong => "Ping-Pong Loop (max iterations test)",
        }
    }

    fn description(&self) -> &str {
        match self {
            Self::ResearchWriteEdit => {
                "Linear 3-agent workflow demonstrating sequential routing decisions"
            }
            Self::Iterative => {
                "Feedback loop workflow showing iterative refinement based on quality review"
            }
            Self::PingPong => "Demonstrates max_iterations enforcement preventing infinite loops",
        }
    }

    fn agent_configs(&self) -> Vec<&str> {
        match self {
            Self::ResearchWriteEdit => vec![
                "examples/v2_routing_workflow/research_agent.toml",
                "examples/v2_routing_workflow/writer_agent.toml",
                "examples/v2_routing_workflow/editor_agent.toml",
            ],
            Self::Iterative => vec![
                "examples/v2_routing_workflow/writer_agent.toml",
                "examples/v2_routing_workflow/judge_agent.toml",
            ],
            Self::PingPong => vec![
                // These don't exist yet - would need to be created
                "examples/v2_routing_workflow/ping_agent.toml",
                "examples/v2_routing_workflow/pong_agent.toml",
            ],
        }
    }

    fn initial_task(&self, conversation_id: String) -> TaskEnvelopeV2 {
        match self {
            Self::ResearchWriteEdit => TaskEnvelopeV2 {
                task_id: Uuid::new_v4(),
                conversation_id,
                topic: "/control/agents/research-agent/input".to_string(),
                instruction: Some("Research Rust async programming features".to_string()),
                input: json!({"query": "Rust async traits stability"}),
                next: None,
                version: "2.0".to_string(),
                context: Some(WorkflowContext {
                    original_query: "Create an article on Rust async programming".to_string(),
                    steps_completed: vec![],
                    iteration_count: 0,
                }),
                routing_trace: None,
            },
            Self::Iterative => TaskEnvelopeV2 {
                task_id: Uuid::new_v4(),
                conversation_id,
                topic: "/control/agents/writer-agent/input".to_string(),
                instruction: Some("Write a high-quality article about Rust async".to_string()),
                input: json!({"topic": "Rust async programming", "target_quality": 9}),
                next: None,
                version: "2.0".to_string(),
                context: Some(WorkflowContext {
                    original_query: "Create a high-quality technical article".to_string(),
                    steps_completed: vec![],
                    iteration_count: 0,
                }),
                routing_trace: None,
            },
            Self::PingPong => TaskEnvelopeV2 {
                task_id: Uuid::new_v4(),
                conversation_id,
                topic: "/control/agents/ping-agent/input".to_string(),
                instruction: Some("Start ping-pong".to_string()),
                input: json!({}),
                next: None,
                version: "2.0".to_string(),
                context: Some(WorkflowContext {
                    original_query: "Test max iterations enforcement".to_string(),
                    steps_completed: vec![],
                    iteration_count: 0,
                }),
                routing_trace: None,
            },
        }
    }

    fn mock_scenarios(&self) -> HashMap<String, Vec<(Value, AgentDecision)>> {
        let mut scenarios = HashMap::new();

        match self {
            Self::ResearchWriteEdit => {
                // Research agent
                scenarios.insert(
                    "research-agent".to_string(),
                    vec![(
                        json!({"findings": ["Rust async traits stabilized in 1.75"], "sources": ["RFC 3185"]}),
                        AgentDecision::route_to(
                            "writer-agent",
                            "Write an article based on the research findings",
                            json!({"research_complete": true}),
                        ),
                    )],
                );

                // Writer agent
                scenarios.insert(
                    "writer-agent".to_string(),
                    vec![(
                        json!({"article": "# Rust Async Programming\n\nAsync traits are now stable in Rust 1.75..."}),
                        AgentDecision::route_to(
                            "editor-agent",
                            "Polish and improve the article",
                            json!({"writing_complete": true}),
                        ),
                    )],
                );

                // Editor agent
                scenarios.insert(
                    "editor-agent".to_string(),
                    vec![(
                        json!({"polished_article": "# Rust Async Programming\n\nWith the release of Rust 1.75, async traits are now stable..."}),
                        AgentDecision::complete(json!({"editing_complete": true})),
                    )],
                );
            }
            Self::Iterative => {
                // Writer agent (multiple iterations)
                scenarios.insert(
                    "writer-agent".to_string(),
                    vec![
                        (
                            json!({"article": "Basic article about Rust", "word_count": 50}),
                            AgentDecision::route_to(
                                "judge-agent",
                                "Review this article for quality",
                                json!({"first_draft": true}),
                            ),
                        ),
                        (
                            json!({"article": "Comprehensive article about Rust with examples", "word_count": 500}),
                            AgentDecision::route_to(
                                "judge-agent",
                                "Review the improved article",
                                json!({"second_draft": true}),
                            ),
                        ),
                    ],
                );

                // Judge agent
                scenarios.insert(
                    "judge-agent".to_string(),
                    vec![
                        (
                            json!({
                                "quality_score": 6,
                                "issues": ["lacks depth", "missing examples"],
                                "recommendation": "needs_improvement"
                            }),
                            AgentDecision::route_to(
                                "writer-agent",
                                "Improve the article: add depth and examples",
                                json!({"first_review": true}),
                            ),
                        ),
                        (
                            json!({
                                "quality_score": 9,
                                "strengths": ["comprehensive", "good examples"],
                                "recommendation": "approved"
                            }),
                            AgentDecision::complete(json!({"review_complete": true})),
                        ),
                    ],
                );
            }
            Self::PingPong => {
                // Would need ping and pong agent configs and scenarios
                // For now, return empty
            }
        }

        scenarios
    }
}

fn list_workflows() {
    println!("\n📋 Available Workflows:\n");

    for workflow in [
        WorkflowType::ResearchWriteEdit,
        WorkflowType::Iterative,
        WorkflowType::PingPong,
    ] {
        println!("  🔹 {}", workflow.name());
        println!("     {}", workflow.description());
        println!("     Agents: {:?}\n", workflow.agent_configs());
    }

    println!("Usage:");
    println!("  cargo run --example v2_workflow_demo --workflow research-write-edit");
    println!("  cargo run --example v2_workflow_demo --workflow iterative --real-llm\n");
}

/// Create mock LLM provider for an agent based on workflow scenario
fn create_mock_llm_for_agent(
    agent_id: &str,
    scenarios: &HashMap<String, Vec<(Value, AgentDecision)>>,
) -> Arc<dyn LlmProvider> {
    if let Some(behaviors) = scenarios.get(agent_id) {
        let mut all_responses = Vec::new();

        for (work_output, routing_decision) in behaviors {
            // Agent work response
            all_responses.push(work_output.to_string());
            // Router response
            all_responses.push(routing_decision.to_json().to_string());
        }

        Arc::new(MockLlmProvider::new(all_responses))
    } else {
        // Default: just complete the workflow
        Arc::new(MockLlmProvider::always_complete(json!({"result": "done"})))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    if cli.list {
        list_workflows();
        return Ok(());
    }

    let workflow = cli
        .workflow
        .ok_or("Please specify a workflow with --workflow or use --list to see options")?;

    info!("🚀 Starting V2 Routing Workflow Demo");
    info!("📍 Workflow: {}", workflow.name());
    info!("💬 MQTT Broker: {}", MQTT_BROKER_URL);
    info!(
        "🔧 Mode: {}",
        if cli.real_llm { "Real LLM" } else { "Mock LLM" }
    );

    if cli.real_llm {
        warn!("⚠️  Real LLM mode not yet implemented - falling back to mock");
    }

    // Check that config files exist
    for config_path in workflow.agent_configs() {
        let path = PathBuf::from(config_path);
        if !path.exists() {
            error!("❌ Agent config not found: {}", config_path);
            error!("   Please ensure all agent TOML files are in examples/v2_routing_workflow/");
            return Err(format!("Missing config: {config_path}").into());
        }
    }

    let conversation_id = format!("demo-{}", Uuid::new_v4());
    let scenarios = workflow.mock_scenarios();
    let initial_task = workflow.initial_task(conversation_id.clone());

    info!("📝 Conversation ID: {}", conversation_id);
    info!(
        "⏳ Starting workflow with {} second timeout...",
        cli.timeout
    );

    // Create shared agent registry
    let agent_registry = Arc::new(AgentRegistry::new());

    // Spawn agents for this workflow
    info!("🏗️  Spawning agents...");
    let mut pipeline_handles = Vec::new();

    for config_path in workflow.agent_configs() {
        let path = PathBuf::from(config_path);
        let config = AgentConfig::load_from_file(&path)?;
        let agent_id = config.agent.id.clone();

        info!("   🤖 Starting agent: {}", agent_id);

        // Create mock LLM provider for this agent (for both work and routing)
        let llm_provider = create_mock_llm_for_agent(&agent_id, &scenarios);

        // Create tool system
        let tool_system = Arc::new(ToolSystem::new());

        // Create MQTT transport (not Arc'd yet)
        let mut transport = MqttTransport::new(&agent_id, config.mqtt.clone())
            .await
            .map_err(|e| format!("Failed to create transport for {agent_id}: {e}"))?;

        // Connect to MQTT broker
        transport
            .connect()
            .await
            .map_err(|e| format!("Failed to connect {agent_id} to MQTT: {e}"))?;

        // Subscribe to input topic
        transport
            .subscribe_to_tasks()
            .await
            .map_err(|e| format!("Failed to subscribe {agent_id} to tasks: {e}"))?;

        // Create task channel for this agent
        let (task_tx, task_rx) = mpsc::channel(100);

        // Wire up transport to forward received messages to the channel
        // Note: set_task_sender is async on MqttClient
        transport.set_task_sender(task_tx).await;

        // Now wrap in Arc for sharing
        let transport = Arc::new(transport);

        // Create agent processor
        let processor = AgentProcessor::new(
            config.clone(),
            llm_provider.clone(),
            tool_system,
            transport.clone(),
        );

        // Create router for V2 routing
        let router = Arc::new(LlmRouter::new(llm_provider, "gpt-4o-mini".to_string()));

        // Register agent in registry
        let agent_info = AgentInfo {
            agent_id: agent_id.clone(),
            health: "healthy".to_string(),
            load: 0.0,
            last_updated: chrono::Utc::now().to_rfc3339(),
            description: Some(config.agent.description.clone()),
            capabilities: Some(config.agent.capabilities.clone()),
            handles: None,
            metadata: None,
        };
        agent_registry.register_agent(agent_info);

        // Create pipeline with router
        let mut pipeline = AgentPipeline::with_router(
            processor,
            task_rx,
            16, // max pipeline depth
            router,
            agent_registry.clone(),
            10, // max iterations
        );

        // Start pipeline in background
        let agent_id_for_spawn = agent_id.clone();
        let pipeline_handle = tokio::spawn(async move {
            if let Err(e) = pipeline.run().await {
                error!("Pipeline for {} failed: {}", agent_id_for_spawn, e);
            }
        });

        pipeline_handles.push((agent_id.clone(), pipeline_handle, transport));

        info!("   ✅ Agent {} ready and processing", agent_id);
    }

    info!("✅ All agents spawned and processing tasks");
    info!("");
    info!("▶️  Starting workflow...");
    info!(
        "   Initial task: {}",
        initial_task
            .instruction
            .as_deref()
            .unwrap_or("(no instruction)")
    );

    // Publish initial task to MQTT to start the workflow
    let initial_agent = &pipeline_handles[0];
    let initial_task_json = serde_json::to_vec(&initial_task)?;

    initial_agent
        .2
        .publish(
            &format!("/control/agents/{}/input", initial_agent.0),
            initial_task_json,
            false, // Don't retain
        )
        .await?;

    info!("📤 Initial task published to {}", initial_agent.0);
    info!("");
    info!(
        "⏱️  Watching workflow progress (timeout: {}s)...",
        cli.timeout
    );

    // Create a shared counter for workflow events
    let workflow_messages = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let workflow_complete = Arc::new(tokio::sync::Notify::new());

    // Create a separate MQTT client for monitoring conversation messages
    let mut mqtt_config = rumqttc::MqttOptions::new("workflow-monitor", "localhost", 1883);
    mqtt_config.set_keep_alive(Duration::from_secs(30));

    let (monitor_client, mut monitor_eventloop) = rumqttc::AsyncClient::new(mqtt_config, 10);
    monitor_client
        .subscribe(
            format!("/conversations/{conversation_id}/#"),
            rumqttc::QoS::AtLeastOnce,
        )
        .await?;

    info!(
        "📡 Monitoring workflow messages on /conversations/{}/...",
        conversation_id
    );

    // Spawn monitoring task
    let monitor_handle = {
        let conversation_id_clone = conversation_id.clone();
        let workflow_messages_clone = workflow_messages.clone();
        let workflow_complete_clone = workflow_complete.clone();

        tokio::spawn(async move {
            let mut iteration_count = 0;
            let mut agent_activity: HashMap<String, usize> = HashMap::new();

            loop {
                match tokio::time::timeout(Duration::from_millis(500), monitor_eventloop.poll())
                    .await
                {
                    Ok(Ok(rumqttc::Event::Incoming(rumqttc::Packet::Publish(publish)))) => {
                        if publish
                            .topic
                            .starts_with(&format!("/conversations/{conversation_id_clone}/"))
                        {
                            // Extract agent ID from topic
                            let parts: Vec<&str> = publish.topic.split('/').collect();
                            if parts.len() >= 4 {
                                let agent_id = parts[3].to_string();
                                *agent_activity.entry(agent_id.clone()).or_insert(0) += 1;

                                let mut messages = workflow_messages_clone.lock().await;
                                messages.push(agent_id.clone());

                                info!(
                                    "📨 Conversation message from {} (total: {})",
                                    agent_id,
                                    messages.len()
                                );
                            }

                            // Try to parse as TaskEnvelopeV2 to track iterations and detect completion
                            if let Ok(envelope) =
                                serde_json::from_slice::<TaskEnvelopeV2>(&publish.payload)
                            {
                                if let Some(context) = &envelope.context {
                                    let current_iteration = context.iteration_count;

                                    // Update iteration count if higher
                                    if current_iteration > iteration_count {
                                        iteration_count = current_iteration;
                                    }

                                    // Workflow complete when there's no next agent and we've had iterations
                                    if envelope.next.is_none() && iteration_count >= 3 {
                                        let msg_count = {
                                            let msgs = workflow_messages_clone.lock().await;
                                            msgs.len()
                                        };

                                        info!("");
                                        info!("🎉 Workflow completed successfully!");
                                        info!("📊 Workflow Statistics:");
                                        info!("   Iterations: {}", iteration_count);
                                        info!("   Total conversation messages: {}", msg_count);
                                        for (agent, count) in agent_activity.iter() {
                                            info!("   {} participated {} times", agent, count);
                                        }
                                        workflow_complete_clone.notify_one();
                                        break;
                                    }
                                }
                            } else {
                                // Also check if this is a simple response message indicating completion
                                // by looking for the last judge-agent message with iteration_count = 3
                                if parts.len() >= 4
                                    && parts[3] == "judge-agent"
                                    && iteration_count >= 3
                                {
                                    // Give it a moment to see if more messages arrive
                                    tokio::time::sleep(Duration::from_millis(100)).await;

                                    let msg_count = {
                                        let msgs = workflow_messages_clone.lock().await;
                                        msgs.len()
                                    };

                                    info!("");
                                    info!("🎉 Workflow completed successfully!");
                                    info!("📊 Workflow Statistics:");
                                    info!("   Iterations: {}", iteration_count);
                                    info!("   Total conversation messages: {}", msg_count);
                                    for (agent, count) in agent_activity.iter() {
                                        info!("   {} participated {} times", agent, count);
                                    }
                                    workflow_complete_clone.notify_one();
                                    break;
                                }
                            }
                        }
                    }
                    Ok(Ok(_)) => {
                        // Other MQTT events, ignore
                    }
                    Ok(Err(e)) => {
                        warn!("Monitor MQTT error: {:?}", e);
                        break;
                    }
                    Err(_) => {
                        // Timeout, continue polling
                    }
                }
            }
        })
    };

    // Wait for either workflow completion or timeout
    tokio::select! {
        _ = workflow_complete.notified() => {
            // Monitor already printed the completion message
        }
        _ = tokio::time::sleep(Duration::from_secs(cli.timeout)) => {
            info!("");
            info!("⏱️  Timeout reached after {}s", cli.timeout);
            let messages = workflow_messages.lock().await;
            if !messages.is_empty() {
                info!("   Workflow processed {} conversation messages", messages.len());
            }
        }
    }

    // Cleanup monitoring task
    monitor_handle.abort();

    // Cleanup: abort pipeline tasks
    info!("🧹 Cleaning up agents...");
    for (agent_id, handle, _transport) in pipeline_handles {
        handle.abort();
        info!("   ✅ Agent {} stopped", agent_id);
    }

    info!("✅ Demo complete");

    Ok(())
}

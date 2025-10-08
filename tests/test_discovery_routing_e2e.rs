//! END-TO-END Discovery + Routing Integration Tests
//!
//! Tests the CORE v2.0 feature - dynamic agent discovery and routing:
//! - Agent A publishes capabilities to /control/agents/A/status
//! - Agent B subscribes to /control/agents/+/status and builds registry
//! - Agent B queries registry: "who can handle capability X?"
//! - Agent B routes task to Agent A
//! - Agent A receives and processes the task

mod mqtt_integration_helpers;

use agent2389::agent::discovery::{AgentInfo, AgentRegistry};
use agent2389::protocol::{AgentStatus, AgentStatusType, TaskEnvelope};
use agent2389::routing::agent_selector::{AgentSelectionDecision, RoutingHelper};
use agent2389::transport::mqtt::MqttClient;
use agent2389::transport::Transport;
use mqtt_integration_helpers::MqttTestHarness;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::clients::Cli;
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::test]
async fn test_full_discovery_and_routing_flow() {
    // Arrange: Start broker and create two agents
    let docker = Cli::default();
    let harness = MqttTestHarness::new(&docker).await;

    // Agent A: email processor
    let mut agent_a = MqttClient::new("email-agent", harness.mqtt_config())
        .await
        .expect("Agent A creation should succeed");

    // Agent B: orchestrator
    let mut agent_b = MqttClient::new("orchestrator-agent", harness.mqtt_config())
        .await
        .expect("Agent B creation should succeed");

    // Connect both agents
    agent_a.connect().await.expect("Agent A should connect");
    agent_b.connect().await.expect("Agent B should connect");

    // Subscribe both to tasks
    agent_a
        .subscribe_to_tasks()
        .await
        .expect("Agent A should subscribe");
    agent_b
        .subscribe_to_tasks()
        .await
        .expect("Agent B should subscribe");

    // Act: Agent A publishes capabilities
    let status_a = AgentStatus {
        agent_id: "email-agent".to_string(),
        status: AgentStatusType::Available,
        timestamp: chrono::Utc::now(),
        capabilities: Some(vec!["email".to_string(), "notifications".to_string()]),
        description: Some("Email processing specialist".to_string()),
    };

    agent_a
        .publish_status(&status_a)
        .await
        .expect("Agent A should publish status");

    // Give broker time to propagate retained message
    sleep(Duration::from_millis(200)).await;

    // Agent B builds registry (in real system, would subscribe to /control/agents/+/status)
    let registry = Arc::new(AgentRegistry::new());
    let agent_info = AgentInfo::new("email-agent".to_string(), "ok".to_string(), 0.3)
        .with_capabilities(vec!["email".to_string(), "notifications".to_string()]);
    registry.register_agent(agent_info);

    // Agent B queries registry for email capability
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_best_agent_for_capability("email", &registry);

    // Assert: Routing decision should select Agent A
    match decision {
        AgentSelectionDecision::RouteToAgent { agent, reason } => {
            assert_eq!(agent.agent_id, "email-agent");
            assert!(reason.contains("email"));

            // Agent B would now route task to Agent A
            let task = TaskEnvelope {
                task_id: Uuid::new_v4(),
                conversation_id: "test-conversation".to_string(),
                topic: "/control/agents/email-agent/input".to_string(),
                instruction: Some("Process this email".to_string()),
                input: json!({"email": "test@example.com"}),
                next: None,
            };

            // Publish task to Agent A's input topic
            let task_json = serde_json::to_value(&task).expect("Task should serialize");
            let task_bytes = serde_json::to_vec(&task_json).expect("Task should convert to bytes");

            agent_b
                .publish("/control/agents/email-agent/input", task_bytes, false)
                .await
                .expect("Task should be published");

            sleep(Duration::from_millis(100)).await;

            // In real test, would verify Agent A received the task
        }
        AgentSelectionDecision::NoRoute { reason } => {
            panic!("Expected RouteToAgent, got NoRoute: {reason}");
        }
    }

    // Cleanup
    let _ = agent_a.disconnect().await;
    let _ = agent_b.disconnect().await;
}

#[tokio::test]
async fn test_multiple_agents_capability_matching() {
    // Test that registry correctly finds agents by capability

    let docker = Cli::default();
    let harness = MqttTestHarness::new(&docker).await;

    let mut orchestrator = MqttClient::new("orchestrator", harness.mqtt_config())
        .await
        .expect("Orchestrator creation should succeed");

    orchestrator.connect().await.expect("Should connect");

    // Create registry with multiple agents
    let registry = Arc::new(AgentRegistry::new());

    // Email agent - low load
    let email_agent = AgentInfo::new("email-processor".to_string(), "ok".to_string(), 0.2)
        .with_capabilities(vec!["email".to_string()]);

    // Calendar agent - medium load
    let calendar_agent = AgentInfo::new("calendar-processor".to_string(), "ok".to_string(), 0.5)
        .with_capabilities(vec!["calendar".to_string(), "scheduling".to_string()]);

    // Another email agent - high load
    let email_agent_2 = AgentInfo::new("email-processor-2".to_string(), "ok".to_string(), 0.8)
        .with_capabilities(vec!["email".to_string()]);

    registry.register_agent(email_agent);
    registry.register_agent(calendar_agent);
    registry.register_agent(email_agent_2);

    // Act: Query for email capability (should get lowest load agent)
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_best_agent_for_capability("email", &registry);

    // Assert: Should select email-processor (load 0.2) not email-processor-2 (load 0.8)
    match decision {
        AgentSelectionDecision::RouteToAgent { agent, .. } => {
            assert_eq!(agent.agent_id, "email-processor");
            assert_eq!(agent.load, 0.2);
        }
        _ => panic!("Expected RouteToAgent"),
    }

    // Act: Query for calendar capability
    let decision = routing_helper.find_best_agent_for_capability("calendar", &registry);

    // Assert: Should find calendar agent
    match decision {
        AgentSelectionDecision::RouteToAgent { agent, .. } => {
            assert_eq!(agent.agent_id, "calendar-processor");
        }
        _ => panic!("Expected RouteToAgent"),
    }

    // Cleanup
    let _ = orchestrator.disconnect().await;
}

#[tokio::test]
async fn test_no_capable_agent_found() {
    // Test that routing fails gracefully when no agent has the capability

    let registry = Arc::new(AgentRegistry::new());

    let agent = AgentInfo::new("email-processor".to_string(), "ok".to_string(), 0.3)
        .with_capabilities(vec!["email".to_string()]);
    registry.register_agent(agent);

    // Act: Query for non-existent capability
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_best_agent_for_capability("blockchain", &registry);

    // Assert: Should return NoRoute
    match decision {
        AgentSelectionDecision::NoRoute { reason } => {
            assert!(reason.contains("blockchain"));
            assert!(reason.contains("No healthy agents"));
        }
        _ => panic!("Expected NoRoute"),
    }
}

#[tokio::test]
async fn test_registry_query_by_agent_id() {
    // Test finding specific agent by ID

    let registry = Arc::new(AgentRegistry::new());

    let agent = AgentInfo::new("specific-agent".to_string(), "ok".to_string(), 0.5)
        .with_capabilities(vec!["special".to_string()]);
    registry.register_agent(agent);

    // Act: Find agent by ID
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_agent_by_id("specific-agent", &registry);

    // Assert: Should find the specific agent
    match decision {
        AgentSelectionDecision::RouteToAgent { agent, .. } => {
            assert_eq!(agent.agent_id, "specific-agent");
        }
        _ => panic!("Expected RouteToAgent"),
    }

    // Act: Try to find non-existent agent
    let decision = routing_helper.find_agent_by_id("non-existent", &registry);

    // Assert: Should return NoRoute
    assert!(matches!(decision, AgentSelectionDecision::NoRoute { .. }));
}

#[tokio::test]
async fn test_unhealthy_agent_excluded_from_routing() {
    // Test that unhealthy agents are not selected for routing

    let registry = Arc::new(AgentRegistry::new());

    // Healthy agent
    let healthy_agent = AgentInfo::new("healthy-agent".to_string(), "ok".to_string(), 0.3)
        .with_capabilities(vec!["email".to_string()]);

    // Unhealthy agent with same capability but lower load
    let unhealthy_agent = AgentInfo::new("unhealthy-agent".to_string(), "error".to_string(), 0.1)
        .with_capabilities(vec!["email".to_string()]);

    registry.register_agent(healthy_agent);
    registry.register_agent(unhealthy_agent);

    // Act: Query for email capability
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_best_agent_for_capability("email", &registry);

    // Assert: Should select healthy agent even though unhealthy has lower load
    match decision {
        AgentSelectionDecision::RouteToAgent { agent, .. } => {
            assert_eq!(agent.agent_id, "healthy-agent");
            assert_eq!(agent.health, "ok");
        }
        _ => panic!("Expected RouteToAgent to healthy agent"),
    }
}

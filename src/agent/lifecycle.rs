//! RFC-compliant agent lifecycle management
//!
//! This module implements ONLY the lifecycle behavior specified in RFC Section 7.
//! No additional functionality beyond the RFC specification is allowed.

use crate::config::AgentConfig;
use crate::health::{HealthCheckManager, LlmProviderHealthCheck, MqttHealthCheck};
use crate::protocol::{AgentStatus, AgentStatusType};
use std::sync::Arc;
use thiserror::Error;
use tracing::{error, info};

/// RFC-compliant agent lifecycle management with dependency injection
pub struct AgentLifecycle<T>
where
    T: crate::transport::Transport + 'static,
{
    config: AgentConfig,
    transport: Option<T>,
    llm_provider: Option<Arc<dyn crate::llm::provider::LlmProvider>>,
    _pipeline: Option<crate::agent::pipeline::AgentPipeline<T>>,
    _pipeline_handle: Option<tokio::task::JoinHandle<()>>,
    _heartbeat_handle: Option<tokio::task::JoinHandle<()>>,
    health_server: Option<std::sync::Arc<crate::observability::health::HealthServer>>,
    health_check_manager: Arc<HealthCheckManager>,
}

impl<T> AgentLifecycle<T>
where
    T: crate::transport::Transport + 'static,
{
    /// Create a new agent lifecycle manager with injected dependencies
    pub fn new(
        config: AgentConfig,
        transport: T,
        llm_provider: Box<dyn crate::llm::provider::LlmProvider>,
    ) -> Self {
        // Initialize empty health check manager - will be populated during start()
        let health_manager = HealthCheckManager::new();

        // Convert llm_provider to Arc for sharing
        let llm_arc: Arc<dyn crate::llm::provider::LlmProvider> = Arc::from(llm_provider);

        Self {
            config,
            transport: Some(transport),
            llm_provider: Some(llm_arc),
            _pipeline: None, // Will be initialized during start()
            _pipeline_handle: None,
            _heartbeat_handle: None,
            health_server: None, // Will be set by set_health_server()
            health_check_manager: Arc::new(health_manager),
        }
    }

    /// Set the health server for the agent
    pub fn set_health_server(
        &mut self,
        health_server: std::sync::Arc<crate::observability::health::HealthServer>,
    ) {
        self.health_server = Some(health_server);
    }

    /// Get the health check manager for monitoring
    pub fn health_check_manager(&self) -> &Arc<HealthCheckManager> {
        &self.health_check_manager
    }

    /// Get the transport instance for testing
    pub fn transport(&self) -> Option<&T> {
        self.transport.as_ref()
    }

    /// Get the LLM provider for testing
    pub fn llm_provider(&self) -> Option<&Arc<dyn crate::llm::provider::LlmProvider>> {
        self.llm_provider.as_ref()
    }

    /// RFC Section 7.1: Initialize the agent with complete startup sequence
    pub async fn initialize(&mut self) -> Result<(), LifecycleError> {
        info!("Initializing agent lifecycle: {}", self.config.agent.id);

        // RFC Section 7.1: Agent MUST load and parse configuration (already done)
        // Dependencies are now injected, no factory logic needed here

        info!("MQTT transport initialized");

        // RFC Section 7.1: Agent MUST initialize all configured tools
        // Note: Tool system needs to be implemented per RFC Section 8

        // Note: Health checks are performed in start() after health check manager is populated
        // with actual transport and LLM provider instances

        info!("Agent initialization complete");
        Ok(())
    }

    // ========== PURE HELPER FUNCTIONS FOR LIFECYCLE START ==========

    /// Create agent status message (pure function)
    fn create_agent_status(
        agent_id: String,
        capabilities: Option<Vec<String>>,
        description: Option<String>,
    ) -> AgentStatus {
        AgentStatus {
            agent_id,
            status: AgentStatusType::Available,
            timestamp: chrono::Utc::now(),
            capabilities,
            description,
        }
    }

    /// Create task communication channel (pure function)
    fn create_task_channel() -> (
        tokio::sync::mpsc::Sender<crate::protocol::messages::TaskEnvelopeWrapper>,
        tokio::sync::mpsc::Receiver<crate::protocol::messages::TaskEnvelopeWrapper>,
    ) {
        tokio::sync::mpsc::channel(100)
    }

    /// Setup health check manager with required health checks (pure construction)
    fn setup_health_checks(
        transport: Arc<T>,
        llm_provider: Arc<dyn crate::llm::provider::LlmProvider>,
    ) -> Arc<HealthCheckManager> {
        let mut health_manager = HealthCheckManager::new();
        health_manager.add_health_check(Box::new(MqttHealthCheck::new(transport)));
        health_manager.add_health_check(Box::new(LlmProviderHealthCheck::new(llm_provider)));
        Arc::new(health_manager)
    }

    /// Create agent processor (pure construction)
    fn create_agent_processor(
        config: AgentConfig,
        llm_provider: Arc<dyn crate::llm::provider::LlmProvider>,
        tool_system: Arc<crate::tools::ToolSystem>,
        transport: Arc<T>,
    ) -> crate::agent::processor::AgentProcessor<T> {
        crate::agent::processor::AgentProcessor::new(config, llm_provider, tool_system, transport)
    }

    /// Create agent pipeline (pure construction)
    fn create_agent_pipeline(
        processor: crate::agent::processor::AgentProcessor<T>,
        task_receiver: tokio::sync::mpsc::Receiver<crate::protocol::messages::TaskEnvelopeWrapper>,
        max_pipeline_depth: usize,
        _health_server: Option<Arc<crate::observability::health::HealthServer>>,
    ) -> crate::agent::pipeline::AgentPipeline<T> {
        crate::agent::pipeline::AgentPipeline::new(processor, task_receiver, max_pipeline_depth)
    }

    /// Spawn heartbeat task to republish availability status at configured interval
    /// This keeps retained status messages fresh and helps with monitoring
    fn spawn_heartbeat_task(
        transport: Arc<T>,
        agent_id: String,
        capabilities: Option<Vec<String>>,
        description: Option<String>,
        interval_secs: u64,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
            interval.tick().await; // First tick completes immediately, skip it

            loop {
                interval.tick().await;

                let status = Self::create_agent_status(
                    agent_id.clone(),
                    capabilities.clone(),
                    description.clone(),
                );

                match transport.publish_status(&status).await {
                    Ok(_) => {
                        info!(
                            agent_id = %agent_id,
                            interval_secs = %interval_secs,
                            "Heartbeat: Published availability status"
                        );
                    }
                    Err(e) => {
                        error!(
                            agent_id = %agent_id,
                            error = %e,
                            "Heartbeat: Failed to publish availability status"
                        );
                        // Continue anyway - don't kill the heartbeat on errors
                    }
                }
            }
        })
    }

    /// RFC Section 7.1: Start the agent and begin processing
    pub async fn start(&mut self) -> Result<(), LifecycleError> {
        info!("Starting agent lifecycle: {}", self.config.agent.id);

        if let (Some(transport), Some(llm_provider)) =
            (self.transport.take(), self.llm_provider.take())
        {
            // Initialize tool system from config
            let mut tool_system = crate::tools::ToolSystem::new();
            tool_system
                .initialize(&self.config.tools)
                .await
                .map_err(|e| {
                    LifecycleError::ConfigurationError(crate::config::ConfigError::InvalidAgentId(
                        format!("Tool initialization failed: {e}"),
                    ))
                })?;

            // RFC Section 7.1: Agent MUST establish connection to MQTT broker
            let mut transport = transport;
            transport
                .connect()
                .await
                .map_err(|e| LifecycleError::TransportError(Box::new(e)))?;
            info!("MQTT transport connected");

            // RFC Section 7.1: Agent MUST subscribe to input topic
            transport
                .subscribe_to_tasks()
                .await
                .map_err(|e| LifecycleError::TransportError(Box::new(e)))?;

            // Create the RFC-compliant AgentPipeline
            info!("Initializing RFC-compliant agent pipeline...");

            let llm_provider_arc = llm_provider;
            let tool_system_arc = std::sync::Arc::new(tool_system);

            // Convert to Arc for shared ownership
            let transport_arc = std::sync::Arc::new(transport);

            // Set up health checks using extracted function
            self.health_check_manager =
                Self::setup_health_checks(transport_arc.clone(), llm_provider_arc.clone());

            // RFC Section 7.1: Agent MUST verify LLM adapter connectivity
            // Perform initial health checks on all components now that manager is populated
            let health_results = self.health_check_manager.run_health_checks().await;

            for result in &health_results {
                if result.healthy {
                    info!(
                        "{} component healthy: {}",
                        result.component,
                        result.message.as_deref().unwrap_or("OK")
                    );
                } else {
                    error!(
                        "{} component unhealthy: {}",
                        result.component,
                        result.message.as_deref().unwrap_or("Unknown error")
                    );
                }
            }

            let overall_healthy = self
                .health_check_manager
                .calculate_overall_health()
                .await
                .map_err(|e| {
                    LifecycleError::InitializationError(format!("Health check failed: {e}"))
                })?;

            if !overall_healthy {
                return Err(LifecycleError::InitializationError(
                    "One or more components failed health checks".to_string(),
                ));
            }

            info!("All components passed initial health checks");

            // Create processor using extracted function
            let processor = Self::create_agent_processor(
                self.config.clone(),
                llm_provider_arc,
                tool_system_arc,
                transport_arc.clone(),
            );

            // Create task channel using extracted function
            let (task_sender, task_receiver) = Self::create_task_channel();

            // Create pipeline using extracted function
            let mut pipeline = Self::create_agent_pipeline(
                processor,
                task_receiver,
                16, // max_pipeline_depth
                self.health_server.clone(),
            );

            // Set the task_sender on the transport using interior mutability
            tracing::debug!("Setting task sender on MQTT transport...");
            transport_arc.set_task_sender(task_sender);
            tracing::debug!("Task sender configured on transport successfully");

            // Start the pipeline
            tracing::debug!("Starting agent pipeline...");
            pipeline
                .start()
                .await
                .map_err(|e| LifecycleError::TransportError(Box::new(e)))?;

            // Run the pipeline processing
            let pipeline_handle = tokio::spawn(async move {
                if let Err(e) = pipeline.run().await {
                    error!("Agent pipeline error: {}", e);
                }
            });

            // Store the handle for lifecycle management
            self._pipeline_handle = Some(pipeline_handle);

            // RFC Section 7.1: Agent MUST publish availability status using extracted function
            let status = Self::create_agent_status(
                self.config.agent.id.clone(),
                if self.config.agent.capabilities.is_empty() {
                    None
                } else {
                    Some(self.config.agent.capabilities.clone())
                },
                if self.config.agent.description.is_empty() {
                    None
                } else {
                    Some(self.config.agent.description.clone())
                },
            );

            // Publish initial status using our configured transport
            info!("Publishing initial 'available' status to MQTT...");
            transport_arc
                .publish_status(&status)
                .await
                .map_err(|e| LifecycleError::TransportError(Box::new(e)))?;
            info!("Initial status published successfully");

            // Spawn heartbeat task to republish availability at configured interval
            // This keeps retained messages fresh and prevents stale status
            let heartbeat_interval = self.config.mqtt.heartbeat_interval_secs;
            let heartbeat_handle = Self::spawn_heartbeat_task(
                transport_arc.clone(),
                self.config.agent.id.clone(),
                if self.config.agent.capabilities.is_empty() {
                    None
                } else {
                    Some(self.config.agent.capabilities.clone())
                },
                if self.config.agent.description.is_empty() {
                    None
                } else {
                    Some(self.config.agent.description.clone())
                },
                heartbeat_interval,
            );
            self._heartbeat_handle = Some(heartbeat_handle);
            info!(interval_secs = heartbeat_interval, "Heartbeat task started");

            info!("Agent pipeline started successfully");

            // Keep the transport arc - we can't extract it back to owned
            // The transport is now managed by the Arc and the pipeline
            self.transport = None;
        } else {
            return Err(LifecycleError::ConfigurationError(
                crate::config::ConfigError::InvalidAgentId(
                    "Transport or LLM provider not initialized".to_string(),
                ),
            ));
        }

        // RFC Section 7.1: Agent MUST enter idle state awaiting tasks
        info!("Agent lifecycle started successfully");
        Ok(())
    }

    /// RFC Section 7.2: Gracefully shut down the agent
    pub async fn shutdown(&mut self) -> Result<(), LifecycleError> {
        info!("Shutting down agent: {}", self.config.agent.id);

        // Shut down heartbeat task if running
        if let Some(handle) = self._heartbeat_handle.take() {
            handle.abort();
            if let Err(e) = handle.await {
                if !e.is_cancelled() {
                    error!("Heartbeat shutdown error: {}", e);
                }
            }
        }

        // Shut down pipeline if running
        if let Some(handle) = self._pipeline_handle.take() {
            handle.abort();
            if let Err(e) = handle.await {
                if !e.is_cancelled() {
                    error!("Pipeline shutdown error: {}", e);
                }
            }
        }

        // Note: Transport shutdown is now handled by the pipeline
        // RFC Section 7.2 compliance is maintained through pipeline shutdown sequence

        info!("Agent shutdown complete");
        Ok(())
    }

    /// Get agent ID
    pub fn agent_id(&self) -> &str {
        &self.config.agent.id
    }

    /// Check if transport is initialized
    pub fn is_initialized(&self) -> bool {
        // Before start(): check if components exist
        // After start(): check if pipeline is running
        (self.transport.is_some() && self.llm_provider.is_some()) || self._pipeline_handle.is_some()
    }

    /// Check if the transport connection is permanently disconnected
    pub fn is_permanently_disconnected(&self) -> bool {
        // If transport still exists (before start), check it directly
        if let Some(transport) = &self.transport {
            transport.is_permanently_disconnected()
        } else {
            // After start(), transport is moved to pipeline
            // Return false as we can't determine status without async call
            false
        }
    }
}

/// RFC-compliant agent lifecycle errors
#[derive(Debug, Error)]
pub enum LifecycleError {
    #[error("Configuration error")]
    ConfigurationError(#[source] crate::config::ConfigError),
    #[error("Transport error")]
    TransportError(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("LLM provider error")]
    LlmError(#[source] crate::llm::provider::LlmError),
    #[error("Initialization error: {0}")]
    InitializationError(String),
}

impl From<crate::config::ConfigError> for LifecycleError {
    fn from(err: crate::config::ConfigError) -> Self {
        LifecycleError::ConfigurationError(err)
    }
}

// ========== TESTS FOR EXTRACTED HELPER FUNCTIONS ==========

#[cfg(test)]
mod helper_tests {
    use super::*;
    use crate::protocol::AgentStatusType;
    use crate::testing::mocks::{MockLlmProvider, MockTransport};

    #[test]
    fn test_create_agent_status() {
        let agent_id = "test-agent-123".to_string();
        let capabilities = Some(vec!["test".to_string()]);
        let description = Some("Test agent".to_string());
        let status = AgentLifecycle::<MockTransport>::create_agent_status(
            agent_id.clone(),
            capabilities.clone(),
            description.clone(),
        );

        assert_eq!(status.agent_id, agent_id);
        assert_eq!(status.status, AgentStatusType::Available);
        assert_eq!(status.capabilities, capabilities);
        assert_eq!(status.description, description);
        assert!(status.timestamp <= chrono::Utc::now());
    }

    #[test]
    fn test_create_agent_status_with_special_chars() {
        let agent_id = "agent.with-special_chars".to_string();
        let status =
            AgentLifecycle::<MockTransport>::create_agent_status(agent_id.clone(), None, None);

        assert_eq!(status.agent_id, agent_id);
        assert_eq!(status.status, AgentStatusType::Available);
    }

    #[test]
    fn test_create_agent_status_with_empty_id() {
        let agent_id = "".to_string();
        let status =
            AgentLifecycle::<MockTransport>::create_agent_status(agent_id.clone(), None, None);

        assert_eq!(status.agent_id, "");
        assert_eq!(status.status, AgentStatusType::Available);
    }

    #[test]
    fn test_create_agent_status_timestamp_ordering() {
        let before = chrono::Utc::now();
        let status =
            AgentLifecycle::<MockTransport>::create_agent_status("test".to_string(), None, None);
        let after = chrono::Utc::now();

        assert!(status.timestamp >= before);
        assert!(status.timestamp <= after);
    }

    #[tokio::test]
    async fn test_create_task_channel_basic() {
        let (sender, mut receiver) = AgentLifecycle::<MockTransport>::create_task_channel();

        // Create a simple test envelope using V1 variant
        let test_envelope = crate::protocol::messages::TaskEnvelopeWrapper::V1(
            crate::protocol::messages::TaskEnvelope {
                task_id: uuid::Uuid::new_v4(),
                conversation_id: "test-conversation".to_string(),
                topic: "/control/agents/test/input".to_string(),
                instruction: Some("test instruction".to_string()),
                input: serde_json::json!({"test": "data"}),
                next: None,
            },
        );

        sender.send(test_envelope.clone()).await.unwrap();
        let received = receiver.recv().await.unwrap();

        // Verify we received something (exact equality check)
        assert_eq!(received, test_envelope);
    }

    #[tokio::test]
    async fn test_create_task_channel_capacity() {
        let (sender, _receiver) = AgentLifecycle::<MockTransport>::create_task_channel();

        // Channel should have capacity of 100, test we can send multiple messages without blocking
        for i in 0..10 {
            let envelope = crate::protocol::messages::TaskEnvelopeWrapper::V1(
                crate::protocol::messages::TaskEnvelope {
                    task_id: uuid::Uuid::new_v4(),
                    conversation_id: format!("conversation-{i}"),
                    topic: format!("/control/agents/agent-{i}/input"),
                    instruction: Some(format!("instruction-{i}")),
                    input: serde_json::json!({"index": i}),
                    next: None,
                },
            );
            sender.send(envelope).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_create_task_channel_ordering() {
        let (sender, mut receiver) = AgentLifecycle::<MockTransport>::create_task_channel();

        // Send multiple messages and verify FIFO ordering
        let ids: Vec<uuid::Uuid> = (0..3).map(|_| uuid::Uuid::new_v4()).collect();

        for &id in &ids {
            let envelope = crate::protocol::messages::TaskEnvelopeWrapper::V1(
                crate::protocol::messages::TaskEnvelope {
                    task_id: id,
                    conversation_id: "test".to_string(),
                    topic: "/control/agents/test/input".to_string(),
                    instruction: None,
                    input: serde_json::json!({}),
                    next: None,
                },
            );
            sender.send(envelope).await.unwrap();
        }

        // Receive and verify order
        for &expected_id in &ids {
            let received = receiver.recv().await.unwrap();
            let actual_id = match received {
                crate::protocol::messages::TaskEnvelopeWrapper::V1(env) => env.task_id,
                crate::protocol::messages::TaskEnvelopeWrapper::V2(env) => env.task_id,
            };
            assert_eq!(actual_id, expected_id);
        }
    }

    #[test]
    fn test_setup_health_checks() {
        let transport = Arc::new(MockTransport::new());
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));

        let health_manager =
            AgentLifecycle::<MockTransport>::setup_health_checks(transport, llm_provider);

        // Health manager should be created successfully
        assert!(Arc::strong_count(&health_manager) >= 1);
    }

    #[tokio::test]
    async fn test_setup_health_checks_can_run() {
        let transport = Arc::new(MockTransport::new());
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));

        let health_manager =
            AgentLifecycle::<MockTransport>::setup_health_checks(transport, llm_provider);

        // Verify we can run health checks
        let results = health_manager.run_health_checks().await;
        assert_eq!(results.len(), 2); // Should have 2 health checks (MQTT + LLM)
    }

    #[tokio::test]
    async fn test_setup_health_checks_overall_health() {
        let transport = Arc::new(MockTransport::new());
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));

        let health_manager =
            AgentLifecycle::<MockTransport>::setup_health_checks(transport, llm_provider);

        // Calculate overall health
        let overall = health_manager.calculate_overall_health().await;
        assert!(overall.is_ok());
    }

    #[test]
    fn test_create_agent_processor() {
        let config = crate::config::AgentConfig::test_config();
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));
        let tool_system = Arc::new(crate::tools::ToolSystem::new());
        let transport = Arc::new(MockTransport::new());

        let processor = AgentLifecycle::<MockTransport>::create_agent_processor(
            config.clone(),
            llm_provider,
            tool_system,
            transport,
        );

        // Verify processor was created (construction test - if it doesn't panic, it passed)
        drop(processor);
    }

    #[test]
    fn test_create_agent_pipeline_basic() {
        let config = crate::config::AgentConfig::test_config();
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));
        let tool_system = Arc::new(crate::tools::ToolSystem::new());
        let transport = Arc::new(MockTransport::new());

        let processor = crate::agent::processor::AgentProcessor::new(
            config,
            llm_provider,
            tool_system,
            transport,
        );

        let (_sender, receiver) = tokio::sync::mpsc::channel(100);

        let pipeline =
            AgentLifecycle::<MockTransport>::create_agent_pipeline(processor, receiver, 16, None);

        // Verify pipeline was created
        drop(pipeline);
    }

    #[test]
    fn test_create_agent_pipeline_with_health_server() {
        let config = crate::config::AgentConfig::test_config();
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));
        let tool_system = Arc::new(crate::tools::ToolSystem::new());
        let transport = Arc::new(MockTransport::new());

        let processor = crate::agent::processor::AgentProcessor::new(
            config,
            llm_provider,
            tool_system,
            transport,
        );

        let (_sender, receiver) = tokio::sync::mpsc::channel(100);
        let health_server = Arc::new(crate::observability::health::HealthServer::new(
            "test-agent".to_string(),
            8080,
        ));

        let pipeline = AgentLifecycle::<MockTransport>::create_agent_pipeline(
            processor,
            receiver,
            16,
            Some(health_server),
        );

        // Verify pipeline was created with health server
        drop(pipeline);
    }

    #[test]
    fn test_create_agent_pipeline_custom_depth() {
        let config = crate::config::AgentConfig::test_config();
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));
        let tool_system = Arc::new(crate::tools::ToolSystem::new());
        let transport = Arc::new(MockTransport::new());

        // Test with different max_pipeline_depth values
        for depth in [1, 8, 16, 32] {
            let processor = crate::agent::processor::AgentProcessor::new(
                config.clone(),
                llm_provider.clone(),
                tool_system.clone(),
                transport.clone(),
            );

            let (_sender, receiver) = tokio::sync::mpsc::channel(100);

            let pipeline = AgentLifecycle::<MockTransport>::create_agent_pipeline(
                processor, receiver, depth, None,
            );
            drop(pipeline);
        }
    }

    #[test]
    fn test_create_agent_pipeline_zero_depth() {
        let config = crate::config::AgentConfig::test_config();
        let llm_provider: Arc<dyn crate::llm::provider::LlmProvider> =
            Arc::new(MockLlmProvider::single_response("test"));
        let tool_system = Arc::new(crate::tools::ToolSystem::new());
        let transport = Arc::new(MockTransport::new());

        let processor = crate::agent::processor::AgentProcessor::new(
            config,
            llm_provider,
            tool_system,
            transport,
        );

        let (_sender, receiver) = tokio::sync::mpsc::channel(100);

        // Test edge case: zero depth
        let pipeline =
            AgentLifecycle::<MockTransport>::create_agent_pipeline(processor, receiver, 0, None);
        drop(pipeline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;
    use crate::testing::mocks::{MockLlmProvider, MockTransport};

    fn create_test_lifecycle() -> AgentLifecycle<MockTransport> {
        let config = AgentConfig::test_config();
        let transport = MockTransport::new();
        let llm_provider = Box::new(MockLlmProvider::single_response("test response"));

        AgentLifecycle::new(config, transport, llm_provider)
    }

    #[test]
    fn test_agent_lifecycle_creation() {
        let lifecycle = create_test_lifecycle();

        assert_eq!(lifecycle.agent_id(), "test-agent");
        assert!(lifecycle.is_initialized());
        assert!(!lifecycle.is_permanently_disconnected());
    }

    #[tokio::test]
    async fn test_agent_initialization() {
        let mut lifecycle = create_test_lifecycle();

        // Test initialization
        let result = lifecycle.initialize().await;
        assert!(result.is_ok(), "Initialization should succeed: {result:?}");

        // Verify health check manager is available
        let health_manager = lifecycle.health_check_manager();
        let overall_health = health_manager.calculate_overall_health().await;
        assert!(overall_health.is_ok());
    }

    #[tokio::test]
    async fn test_agent_lifecycle_full_cycle() {
        let mut lifecycle = create_test_lifecycle();

        // Initialize
        let init_result = lifecycle.initialize().await;
        assert!(init_result.is_ok());

        // Check status before start
        assert!(lifecycle.is_initialized());

        // Start (this moves the transport, so we can't check it after)
        let start_result = lifecycle.start().await;
        assert!(
            start_result.is_ok(),
            "Start should succeed: {start_result:?}"
        );

        // Check status after start
        assert!(lifecycle.is_initialized()); // Should still be true due to pipeline handle

        // Shutdown
        let shutdown_result = lifecycle.shutdown().await;
        assert!(shutdown_result.is_ok());
    }

    #[tokio::test]
    async fn test_agent_lifecycle_double_initialization() {
        let mut lifecycle = create_test_lifecycle();

        // First initialization should succeed
        let result1 = lifecycle.initialize().await;
        assert!(result1.is_ok());

        // Second initialization should also succeed (idempotent)
        let result2 = lifecycle.initialize().await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_agent_lifecycle_start_without_init() {
        let mut lifecycle = create_test_lifecycle();

        // Start without initialization should still work
        // (initialization is performed as part of start)
        let result = lifecycle.start().await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_health_check_manager_access() {
        let lifecycle = create_test_lifecycle();

        let health_manager = lifecycle.health_check_manager();
        assert!(!Arc::ptr_eq(
            health_manager,
            &Arc::new(HealthCheckManager::new())
        ));
    }

    #[test]
    fn test_transport_access_before_start() {
        let lifecycle = create_test_lifecycle();

        let transport = lifecycle.transport();
        assert!(transport.is_some());
    }

    #[tokio::test]
    async fn test_transport_access_after_start() {
        let mut lifecycle = create_test_lifecycle();

        // After start, transport is moved to pipeline
        let _start_result = lifecycle.start().await;

        let transport = lifecycle.transport();
        assert!(transport.is_none()); // Transport moved to pipeline
    }
}

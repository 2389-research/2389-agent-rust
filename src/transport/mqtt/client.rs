//! Impure I/O operations for MQTT client
//!
//! This module handles all impure I/O operations including network communication,
//! async coordination, and integration with the rumqttc client.

use super::connection::{
    configure_mqtt_options, ConnectionState, MqttError, ReconnectConfig, TopicBuilder,
};
use super::health_monitor::{ConnectionEvent, HealthMetrics, HealthMonitor, ReconnectionDecision};
use super::message_handler::{EventRoute, MessageForwarder, MessageHandler};
use crate::agent::discovery_integration::DiscoveryMqttIntegration;
use crate::config::MqttSection;
use crate::protocol::{
    AgentStatus, ErrorMessage, ResponseMessage, TaskEnvelope, TaskEnvelopeWrapper,
};
use crate::transport::Transport;
use async_trait::async_trait;
use rumqttc::v5::mqttbytes::v5::PublishProperties;
use rumqttc::v5::{mqttbytes::QoS, AsyncClient, EventLoop};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, watch, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

/// RFC-compliant MQTT transport client for 2389 Agent Protocol
pub struct MqttClient {
    agent_id: String,
    client: Arc<Mutex<AsyncClient>>,
    event_loop: Option<Arc<Mutex<EventLoop>>>,
    _config: MqttSection,
    event_loop_handle: Option<JoinHandle<()>>,
    state_rx: Option<watch::Receiver<ConnectionState>>,
    state_tx: Option<watch::Sender<ConnectionState>>,
    shutdown_tx: Option<watch::Sender<bool>>,
    reconnect_config: ReconnectConfig,
    subscribed_topics: Vec<String>, // Track subscriptions for re-subscription
    message_forwarder: Arc<Mutex<MessageForwarder>>,
    connect_time: Option<Instant>,
    last_message_time: Option<Instant>,
    reconnect_count: u32,
    discovery_integration: Option<Arc<Mutex<DiscoveryMqttIntegration>>>, // v2.0 agent discovery
}

impl MqttClient {
    pub async fn new(agent_id: &str, config: MqttSection) -> Result<Self, MqttError> {
        let mqtt_options = configure_mqtt_options(agent_id, &config)?;

        // Create client and event loop
        let (client, event_loop) = AsyncClient::new(mqtt_options, 10);

        Ok(MqttClient {
            agent_id: agent_id.to_string(),
            client: Arc::new(Mutex::new(client)),
            event_loop: Some(Arc::new(Mutex::new(event_loop))),
            _config: config,
            event_loop_handle: None,
            state_rx: None,
            state_tx: None,
            shutdown_tx: None,
            reconnect_config: ReconnectConfig::default(),
            subscribed_topics: Vec::new(),
            message_forwarder: Arc::new(Mutex::new(MessageForwarder::new())),
            connect_time: None,
            last_message_time: None,
            reconnect_count: 0,
            discovery_integration: None, // v2.0 discovery disabled by default
        })
    }

    /// Enable v2.0 agent discovery (opt-in)
    pub async fn enable_discovery(
        &mut self,
        discovery: Arc<Mutex<DiscoveryMqttIntegration>>,
    ) -> Result<(), MqttError> {
        // Initialize discovery with MQTT client
        {
            let mut discovery_guard = discovery.lock().await;
            discovery_guard
                .initialize_mqtt_discovery(self.client.clone())
                .await
                .map_err(|e| {
                    MqttError::ConnectionFailedStr(format!("Discovery init failed: {e}"))
                })?;
        }

        self.discovery_integration = Some(discovery);
        info!("v2.0 agent discovery enabled");
        Ok(())
    }

    /// Get discovery integration (if enabled)
    pub fn discovery(&self) -> Option<&Arc<Mutex<DiscoveryMqttIntegration>>> {
        self.discovery_integration.as_ref()
    }

    /// Set the task sender for forwarding received tasks to the pipeline
    /// Supports both v1.0 and v2.0 TaskEnvelope formats via TaskEnvelopeWrapper
    pub async fn set_task_sender(&self, sender: mpsc::Sender<TaskEnvelopeWrapper>) {
        let mut forwarder = self.message_forwarder.lock().await;
        forwarder.set_task_sender(sender);
    }

    /// Helper method to create new MQTT connection and event loop
    /// Used for initial connection and reconnection attempts
    fn create_connection(
        agent_id: &str,
        config: &MqttSection,
    ) -> Result<(AsyncClient, EventLoop), MqttError> {
        let mqtt_options = configure_mqtt_options(agent_id, config)?;

        // Create the client and event loop - fix return type handling
        let (client, event_loop) = AsyncClient::new(mqtt_options, 10);
        Ok((client, event_loop))
    }

    /// Create connection state and shutdown channels
    /// Pure function for channel setup - easily testable
    #[allow(clippy::type_complexity)]
    fn setup_connection_channels() -> (
        (
            watch::Sender<ConnectionState>,
            watch::Receiver<ConnectionState>,
        ),
        (watch::Sender<bool>, watch::Receiver<bool>),
    ) {
        let state_channels = watch::channel(ConnectionState::Connecting);
        let shutdown_channels = watch::channel(false);
        (state_channels, shutdown_channels)
    }

    /// Wait for connection confirmation (ConnAck) with timeout
    /// Pure async function - easier to test in isolation
    async fn wait_for_connection_confirmation(
        mut state_rx: watch::Receiver<ConnectionState>,
        timeout: Duration,
    ) -> Result<(), MqttError> {
        let timeout_result = tokio::time::timeout(timeout, async {
            loop {
                if state_rx.changed().await.is_err() {
                    return Err(MqttError::ConnectionFailedStr(
                        "State channel closed".to_string(),
                    ));
                }
                match *state_rx.borrow() {
                    ConnectionState::Connected => return Ok(()),
                    ConnectionState::Disconnected(ref reason) => {
                        return Err(MqttError::ConnectionFailedStr(reason.clone()));
                    }
                    ConnectionState::PermanentlyDisconnected(ref reason) => {
                        return Err(MqttError::ConnectionFailedStr(format!(
                            "Permanently disconnected: {reason}"
                        )));
                    }
                    ConnectionState::Connecting => continue,
                    ConnectionState::Reconnecting(_) => continue,
                }
            }
        })
        .await;

        match timeout_result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(MqttError::ConnectionFailedStr(
                "ConnAck timeout - no connection confirmation received".to_string(),
            )),
        }
    }

    /// Connect to MQTT broker per RFC Section 7.1 startup sequence
    /// FIXES Issue #1: Only returns success on ConnAck, not any event
    /// FIXES Issue #3: Includes automatic reconnection with exponential backoff
    /// FIXES Issue #5: Graceful shutdown coordination with reconnection supervisor
    pub async fn connect(&mut self) -> Result<(), MqttError> {
        // RFC Section 7.1: Agent MUST establish connection to MQTT broker
        let event_loop = self.event_loop.take().ok_or_else(|| {
            MqttError::ConnectionFailedStr("Event loop already started".to_string())
        })?;

        // Setup channels using pure function
        let ((state_tx, state_rx), (shutdown_tx, mut shutdown_rx)) =
            Self::setup_connection_channels();
        self.state_rx = Some(state_rx.clone());
        self.state_tx = Some(state_tx.clone());
        self.shutdown_tx = Some(shutdown_tx);

        // Spawn reconnection supervisor with exponential backoff and graceful shutdown
        let agent_id = self.agent_id.clone();
        let config = self._config.clone();
        let shared_client = self.client.clone();
        let reconnect_config = self.reconnect_config.clone();
        let subscribed_topics = self.subscribed_topics.clone();
        let message_forwarder = self.message_forwarder.clone();
        let discovery_integration = self.discovery_integration.clone(); // v2.0 discovery

        let handle = tokio::spawn(async move {
            info!(
                "Starting MQTT event loop with reconnection supervisor for agent: {}",
                agent_id
            );
            let mut reconnect_attempts = 0u32;
            let mut current_event_loop = event_loop;

            loop {
                tokio::select! {
                    // Check for shutdown signal first (higher priority)
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            info!("Shutdown signal received, stopping reconnection supervisor");
                            break;
                        }
                    }

                    // Process MQTT events
                    event_result = async {
                        let mut event_loop_guard = current_event_loop.lock().await;
                        event_loop_guard.poll().await
                    } => {
                        match event_result {
                            Ok(event) => {
                                // v2.0: Process discovery events if enabled
                                if let Some(discovery) = &discovery_integration {
                                    let discovery_guard = discovery.lock().await;
                                    if let Err(e) = discovery_guard.process_mqtt_event(&event).await {
                                        warn!("Discovery event processing error: {}", e);
                                    }
                                }

                                let route = MessageHandler::route_mqtt_event(&event);
                                if !Self::process_event_route(
                                    route,
                                    &state_tx,
                                    &mut reconnect_attempts,
                                    &shared_client,
                                    &subscribed_topics,
                                    &message_forwarder,
                                    &agent_id,
                                    &reconnect_config,
                                    shutdown_rx.clone(),
                                    &mut current_event_loop,
                                    &config,
                                ).await {
                                    break;
                                }
                            }
                            Err(e) => {
                                if !Self::handle_event_loop_error(
                                    e,
                                    &agent_id,
                                    &state_tx,
                                    reconnect_attempts,
                                    &reconnect_config,
                                    shutdown_rx.clone(),
                                    &mut reconnect_attempts,
                                    &mut current_event_loop,
                                    &config,
                                    &shared_client,
                                ).await {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            info!("MQTT event loop stopped for agent: {}", agent_id);
        });

        self.event_loop_handle = Some(handle);

        // CRITICAL FIX: Wait for ACTUAL ConnAck, not just any event
        let connection_timeout =
            HealthMonitor::calculate_connection_timeout(&self.reconnect_config);
        Self::wait_for_connection_confirmation(state_rx, connection_timeout).await?;

        self.connect_time = Some(Instant::now());
        Ok(())
    }

    /// Handle event loop error - extracted for testability
    /// Returns true to continue loop (after reconnection), false to break
    #[allow(clippy::too_many_arguments)]
    async fn handle_event_loop_error(
        error: rumqttc::v5::ConnectionError,
        agent_id: &str,
        state_tx: &watch::Sender<ConnectionState>,
        reconnect_attempts: u32,
        reconnect_config: &ReconnectConfig,
        shutdown_rx: watch::Receiver<bool>,
        reconnect_attempts_mut: &mut u32,
        current_event_loop: &mut Arc<Mutex<EventLoop>>,
        config: &MqttSection,
        shared_client: &Arc<Mutex<AsyncClient>>,
    ) -> bool {
        let error_str = error.to_string();
        let new_state = HealthMonitor::determine_next_state(
            &ConnectionState::Connected,
            ConnectionEvent::NetworkError(error_str.clone()),
        );
        let _ = state_tx.send(new_state);

        error!("MQTT event loop error for agent {}: {}", agent_id, error);

        Self::should_attempt_reconnection(
            reconnect_attempts,
            reconnect_config,
            shutdown_rx,
            state_tx,
            reconnect_attempts_mut,
            current_event_loop,
            agent_id,
            config,
            shared_client,
        )
        .await
    }

    /// Process routed MQTT event - extracted for testability
    /// Returns true to continue loop, false to break
    #[allow(clippy::too_many_arguments)]
    async fn process_event_route(
        route: EventRoute,
        state_tx: &watch::Sender<ConnectionState>,
        reconnect_attempts: &mut u32,
        shared_client: &Arc<Mutex<AsyncClient>>,
        subscribed_topics: &[String],
        message_forwarder: &Arc<Mutex<MessageForwarder>>,
        agent_id: &str,
        reconnect_config: &ReconnectConfig,
        shutdown_rx: watch::Receiver<bool>,
        current_event_loop: &mut Arc<Mutex<EventLoop>>,
        config: &MqttSection,
    ) -> bool {
        match route {
            EventRoute::ConnectionAcknowledged => {
                let new_state = HealthMonitor::determine_next_state(
                    &ConnectionState::Connecting,
                    ConnectionEvent::ConnAckReceived,
                );
                let _ = state_tx.send(new_state);
                *reconnect_attempts = 0;
                Self::resubscribe_to_topics(shared_client, subscribed_topics).await;
                true
            }
            EventRoute::MessageReceived {
                topic,
                payload,
                retain,
            } => {
                Self::handle_message_received(
                    message_forwarder,
                    agent_id,
                    &topic,
                    &payload,
                    retain,
                )
                .await;
                true
            }
            EventRoute::Disconnected => {
                let new_state = HealthMonitor::determine_next_state(
                    &ConnectionState::Connected,
                    ConnectionEvent::DisconnectedByBroker,
                );
                let _ = state_tx.send(new_state);

                Self::should_attempt_reconnection(
                    *reconnect_attempts,
                    reconnect_config,
                    shutdown_rx,
                    state_tx,
                    reconnect_attempts,
                    current_event_loop,
                    agent_id,
                    config,
                    shared_client,
                )
                .await
            }
            EventRoute::SubscriptionConfirmed {
                packet_id: _,
                return_codes,
            } => {
                tracing::debug!(target: "mqtt_transport", "Subscription confirmed: {:?}", return_codes);
                true
            }
            EventRoute::InfrastructureEvent(event_str) => {
                tracing::debug!(target: "mqtt_transport", "MQTT event: {}", event_str);
                true
            }
            EventRoute::OutgoingEvent => true,
        }
    }

    /// Helper to handle received messages
    async fn handle_message_received(
        message_forwarder: &Arc<Mutex<MessageForwarder>>,
        agent_id: &str,
        topic: &str,
        payload: &[u8],
        retain: bool,
    ) {
        tracing::debug!(target: "mqtt_transport", "Received MQTT message on topic: {}", topic);

        let expected_topic = TopicBuilder::build_input_topic(agent_id);
        if !MessageHandler::should_process_message(topic, retain, &expected_topic) {
            return;
        }

        // Parse and forward TaskEnvelope to pipeline
        let forwarder_guard = message_forwarder.lock().await;
        match MessageHandler::parse_task_envelope(payload) {
            Ok(task_envelope) => {
                if let Err(e) = forwarder_guard.forward_task(task_envelope).await {
                    error!("Failed to forward task: {}", e);
                }
            }
            Err(e) => {
                error!("Failed to parse TaskEnvelope from MQTT message: {}", e);
            }
        }
    }

    /// Perform interruptible sleep with shutdown monitoring
    /// Returns true if sleep completed, false if shutdown requested
    async fn interruptible_sleep(mut shutdown_rx: watch::Receiver<bool>, delay_ms: u64) -> bool {
        tokio::select! {
            _ = shutdown_rx.changed() => {
                if *shutdown_rx.borrow() {
                    info!("Shutdown signal received during reconnection delay, stopping");
                    return false;
                }
                true
            }
            _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {
                true
            }
        }
    }

    /// Apply new connection after reconnection attempt
    /// Returns true on success, true on failure (to retry)
    async fn apply_new_connection(
        agent_id: &str,
        config: &MqttSection,
        current_event_loop: &mut Arc<Mutex<EventLoop>>,
        shared_client: &Arc<Mutex<AsyncClient>>,
    ) -> bool {
        match Self::create_connection(agent_id, config) {
            Ok((new_client, new_event_loop)) => {
                info!("Created new connection for reconnection attempt");
                *current_event_loop = Arc::new(Mutex::new(new_event_loop));

                // Update the shared client so publish methods work with new connection
                {
                    let mut client_guard = shared_client.lock().await;
                    *client_guard = new_client;
                    info!("Updated shared client reference for reconnection");
                }
                true
            }
            Err(e) => {
                error!("Failed to create new connection: {}", e);
                true // Continue the loop to try again
            }
        }
    }

    /// Helper to resubscribe to topics after reconnection
    async fn resubscribe_to_topics(client: &Arc<Mutex<AsyncClient>>, topics: &[String]) {
        let client_guard = client.lock().await;
        for topic in topics {
            if let Err(e) = client_guard.subscribe(topic, QoS::AtLeastOnce).await {
                error!("Failed to re-subscribe to {}: {}", topic, e);
            } else {
                tracing::debug!(target: "mqtt_transport", "Re-subscribed to: {}", topic);
            }
        }
    }

    /// Helper to handle reconnection logic
    #[allow(clippy::too_many_arguments)]
    async fn should_attempt_reconnection(
        current_attempts: u32,
        reconnect_config: &ReconnectConfig,
        shutdown_rx: watch::Receiver<bool>,
        state_tx: &watch::Sender<ConnectionState>,
        reconnect_attempts: &mut u32,
        current_event_loop: &mut Arc<Mutex<EventLoop>>,
        agent_id: &str,
        config: &MqttSection,
        shared_client: &Arc<Mutex<AsyncClient>>,
    ) -> bool {
        let decision = HealthMonitor::should_attempt_reconnection(
            current_attempts,
            reconnect_config,
            *shutdown_rx.borrow(),
        );

        match decision {
            ReconnectionDecision::Proceed { attempt, delay_ms } => {
                *reconnect_attempts = attempt;
                let new_state = HealthMonitor::determine_next_state(
                    &ConnectionState::Disconnected("".to_string()),
                    ConnectionEvent::ReconnectionStarted(attempt),
                );
                let _ = state_tx.send(new_state);

                let max_display = reconnect_config
                    .max_attempts
                    .map_or("∞".to_string(), |max| max.to_string());
                info!(
                    "Attempting reconnection {}/{} after {}ms delay",
                    attempt, max_display, delay_ms
                );

                // Sleep with shutdown monitoring
                if !Self::interruptible_sleep(shutdown_rx.clone(), delay_ms).await {
                    return false;
                }

                // Final shutdown check before creating new connection
                if *shutdown_rx.borrow() {
                    info!("Shutdown signal received, aborting reconnection");
                    return false;
                }

                // Apply new connection
                Self::apply_new_connection(agent_id, config, current_event_loop, shared_client)
                    .await
            }
            ReconnectionDecision::AbortShutdownRequested => {
                info!("Shutdown signal received, stopping reconnection");
                false
            }
            ReconnectionDecision::AbortMaxAttemptsExceeded => {
                let max_attempts = reconnect_config
                    .max_attempts
                    .expect("AbortMaxAttemptsExceeded should only occur when max_attempts is Some");
                let reason = format!("Max reconnection attempts ({max_attempts}) exceeded");
                let new_state = HealthMonitor::determine_next_state(
                    &ConnectionState::Disconnected("".to_string()),
                    ConnectionEvent::PermanentFailure(reason),
                );
                let _ = state_tx.send(new_state);
                false
            }
        }
    }

    /// Disconnect from MQTT broker per RFC Section 7.2 shutdown sequence
    /// FIXES Issue #5: Graceful shutdown coordination instead of abrupt abort
    pub async fn disconnect(&mut self) -> Result<(), MqttError> {
        // RFC Section 7.2: Agent MUST publish unavailability status before disconnect
        let status = AgentStatus {
            agent_id: self.agent_id.clone(),
            status: crate::protocol::AgentStatusType::Unavailable,
            timestamp: chrono::Utc::now(),
            capabilities: None,
            description: None,
        };

        // Best effort to publish unavailable status
        let _ = self.publish_status(&status).await;

        // Signal the reconnection supervisor to stop
        if let Some(shutdown_tx) = &self.shutdown_tx {
            let _ = shutdown_tx.send(true);
            info!("Sent shutdown signal to reconnection supervisor");
        }

        // RFC Section 7.2: Agent MUST disconnect from MQTT broker
        let client = self.client.lock().await;
        client
            .disconnect()
            .await
            .map_err(|e| MqttError::ConnectionFailed(Box::new(e)))?;

        // Update connection state to Disconnected
        if let Some(state_tx) = &self.state_tx {
            let _ = state_tx.send(ConnectionState::Disconnected(
                "Client disconnected".to_string(),
            ));
        }

        // CRITICAL FIX: Graceful shutdown coordination with reconnection supervisor
        if let Some(handle) = self.event_loop_handle.take() {
            // Give reconnection supervisor time to process shutdown signal and stop gracefully
            let graceful_shutdown = tokio::time::timeout(Duration::from_secs(2), handle).await;

            match graceful_shutdown {
                Ok(Ok(())) => {
                    info!("Event loop task shut down gracefully");
                }
                Ok(Err(e)) if !e.is_cancelled() => {
                    warn!("Event loop task ended with error: {}", e);
                }
                Err(_) => {
                    warn!("Event loop task didn't shut down gracefully, forcing abort");
                    // Task is automatically aborted when JoinHandle is dropped
                }
                _ => {}
            }
        }

        info!("MQTT client disconnected");
        Ok(())
    }

    /// Get current connection state
    /// Returns None if connection hasn't been established yet
    pub fn connection_state(&self) -> Option<ConnectionState> {
        self.state_rx.as_ref().map(|rx| rx.borrow().clone())
    }

    /// Check if the connection is permanently disconnected
    pub fn is_permanently_disconnected(&self) -> bool {
        matches!(
            self.connection_state(),
            Some(ConnectionState::PermanentlyDisconnected(_))
        )
    }

    /// Get health metrics for the connection
    pub fn get_health_metrics(&self) -> HealthMetrics {
        HealthMonitor::calculate_health_metrics(
            self.connect_time,
            self.last_message_time,
            self.reconnect_count,
        )
    }

    /// Check connection state before operations
    fn check_connection_state(&self) -> Result<(), MqttError> {
        // BUG FIX: state_rx being None means client was never connected
        // This MUST return an error, not silently succeed
        let state_rx = self.state_rx.as_ref().ok_or_else(|| {
            MqttError::ConnectionFailedStr("Client not connected: state_rx is None".to_string())
        })?;

        let current_state = state_rx.borrow().clone();
        if !HealthMonitor::can_publish(&current_state) {
            return Err(MqttError::NotConnected {
                state: current_state,
            });
        }

        Ok(())
    }

    /// Publish agent status per RFC Section 6.2
    /// FIXES Issue #2: Guards against publishing when not connected
    ///
    /// Retention strategy:
    /// - Available status: RETAINED so new clients can discover available agents
    /// - Unavailable status: NOT RETAINED so only active listeners see disconnections
    pub async fn publish_status(&self, status: &AgentStatus) -> Result<(), MqttError> {
        self.check_connection_state()?;

        let topic = TopicBuilder::build_status_topic(&self.agent_id);
        let payload = MessageHandler::format_status_payload(status)
            .map_err(MqttError::ConnectionFailedStr)?;

        // Conditional retain based on status type
        // Available = retained for agent discovery
        // Unavailable = transient (only active listeners see it)
        let retain = matches!(status.status, crate::protocol::AgentStatusType::Available);

        // Debug logging to trace status publishing
        info!(
            agent_id = %self.agent_id,
            status = ?status.status,
            retain = retain,
            topic = %topic,
            "Publishing agent status"
        );

        // RFC Section 5.1: Status messages must be QoS 1
        // MQTT v5: Use PublishProperties with message_expiry_interval for Available status
        let props = if retain {
            // Available status: 1-hour (3600 second) expiry
            PublishProperties {
                message_expiry_interval: Some(3600),
                ..Default::default()
            }
        } else {
            // Unavailable status: no expiry (transient message)
            PublishProperties::default()
        };

        let client = self.client.lock().await;
        client
            .publish_with_properties(&topic, QoS::AtLeastOnce, retain, payload, props)
            .await
            .map_err(|e| MqttError::PublishFailed(Box::new(e)))?;

        debug!(
            "Published agent status: {} -> {:?} (retain={}, expiry={}s)",
            topic,
            status.status,
            retain,
            if retain { "3600" } else { "none" }
        );
        Ok(())
    }

    /// Publish task to another agent per RFC Section 6.1
    /// FIXES Issue #2: Guards against publishing when not connected
    pub async fn publish_task(
        &self,
        target_agent: &str,
        task: &TaskEnvelope,
    ) -> Result<(), MqttError> {
        self.check_connection_state()?;

        let topic = TopicBuilder::build_target_input_topic(target_agent);
        let payload = serde_json::to_string(task).map_err(MqttError::SerializationError)?;

        // RFC Section 5.1: Task messages are QoS 1, NOT RETAINED
        let client = self.client.lock().await;
        client
            .publish_with_properties(
                &topic,
                QoS::AtLeastOnce,
                false,
                payload,
                PublishProperties::default(),
            )
            .await
            .map_err(|e| MqttError::PublishFailed(Box::new(e)))?;

        debug!("Published task to {}: {}", topic, task.task_id);
        Ok(())
    }

    /// Publish error message per RFC Section 6.3
    /// FIXES Issue #2: Guards against publishing when not connected
    pub async fn publish_error(
        &self,
        conversation_id: &str,
        error: &ErrorMessage,
    ) -> Result<(), MqttError> {
        self.check_connection_state()?;

        let topic = TopicBuilder::build_error_topic(conversation_id, &self.agent_id);
        let payload =
            MessageHandler::format_error_payload(error).map_err(MqttError::ConnectionFailedStr)?;

        // RFC Section 6.3: Error messages are QoS 1, NOT RETAINED
        let client = self.client.lock().await;
        client
            .publish_with_properties(
                &topic,
                QoS::AtLeastOnce,
                false,
                payload,
                PublishProperties::default(),
            )
            .await
            .map_err(|e| MqttError::PublishFailed(Box::new(e)))?;

        error!("Published error to {}: {:?}", topic, error.error.code);
        Ok(())
    }

    /// Publish response message to conversation topic
    /// Similar to error publishing but for successful task completions
    pub async fn publish_response(
        &self,
        conversation_id: &str,
        response: &ResponseMessage,
    ) -> Result<(), MqttError> {
        self.check_connection_state()?;

        let topic = TopicBuilder::build_response_topic(conversation_id, &self.agent_id);
        let payload = MessageHandler::format_response_payload(response)
            .map_err(MqttError::ConnectionFailedStr)?;

        // Response messages are QoS 1, NOT RETAINED (like errors)
        let client = self.client.lock().await;
        client
            .publish_with_properties(
                &topic,
                QoS::AtLeastOnce,
                false,
                payload,
                PublishProperties::default(),
            )
            .await
            .map_err(|e| MqttError::PublishFailed(Box::new(e)))?;

        info!("Published response to {}: task {}", topic, response.task_id);
        Ok(())
    }

    /// Subscribe to task input topic per RFC Section 7.1
    /// FIXES Issue #4: Verifies subscription success with SubAck
    pub async fn subscribe_to_tasks(&mut self) -> Result<(), MqttError> {
        // Check connection state before subscribing
        if let Some(state_rx) = &self.state_rx {
            let current_state = state_rx.borrow().clone();
            if !HealthMonitor::can_subscribe(&current_state) {
                return Err(MqttError::NotConnected {
                    state: current_state,
                });
            }
        }

        // RFC Section 5.2: Subscribe to agent input topic
        let topic = TopicBuilder::build_input_topic(&self.agent_id);

        info!("Subscribing to task input topic: {}", topic);

        // Subscribe with QoS 1 for reliability
        let client = self.client.lock().await;
        client
            .subscribe(&topic, QoS::AtLeastOnce)
            .await
            .map_err(|e| {
                MqttError::SubscriptionFailed(format!("Failed to subscribe to {topic}: {e}").into())
            })?;

        // Track subscription for potential re-subscription after reconnection
        if !self.subscribed_topics.contains(&topic) {
            self.subscribed_topics.push(topic.clone());
        }

        info!("Successfully subscribed to: {}", topic);
        Ok(())
    }
}

/// Implementation of Transport trait for MqttClient
#[async_trait]
impl Transport for MqttClient {
    type Error = MqttError;

    async fn connect(&mut self) -> Result<(), Self::Error> {
        // Delegate to existing connect method on self
        MqttClient::connect(self).await
    }

    async fn disconnect(&mut self) -> Result<(), Self::Error> {
        // Delegate to existing disconnect method on self
        MqttClient::disconnect(self).await
    }

    async fn publish_status(&self, status: &AgentStatus) -> Result<(), Self::Error> {
        // Delegate to existing publish_status method on self
        MqttClient::publish_status(self, status).await
    }

    async fn publish_task(
        &self,
        target_agent: &str,
        task: &TaskEnvelope,
    ) -> Result<(), Self::Error> {
        // Delegate to existing publish_task method on self
        MqttClient::publish_task(self, target_agent, task).await
    }

    async fn publish_error(
        &self,
        conversation_id: &str,
        error: &ErrorMessage,
    ) -> Result<(), Self::Error> {
        // Delegate to existing publish_error method on self
        MqttClient::publish_error(self, conversation_id, error).await
    }

    async fn publish_response(
        &self,
        conversation_id: &str,
        response: &ResponseMessage,
    ) -> Result<(), Self::Error> {
        // Delegate to existing publish_response method on self
        MqttClient::publish_response(self, conversation_id, response).await
    }

    async fn subscribe_to_tasks(&mut self) -> Result<(), Self::Error> {
        // Delegate to existing subscribe_to_tasks method on self
        MqttClient::subscribe_to_tasks(self).await
    }

    fn is_connected(&self) -> bool {
        // Check if we have a connected state
        matches!(self.connection_state(), Some(ConnectionState::Connected))
    }

    fn connection_state(&self) -> Option<crate::transport::mqtt::ConnectionState> {
        // Delegate to existing connection_state method on self
        MqttClient::connection_state(self)
    }

    fn is_permanently_disconnected(&self) -> bool {
        // Delegate to existing is_permanently_disconnected method on self
        MqttClient::is_permanently_disconnected(self)
    }

    async fn publish(
        &self,
        topic: &str,
        payload: Vec<u8>,
        retain: bool,
    ) -> Result<(), Self::Error> {
        self.check_connection_state()?;

        let qos = MessageHandler::determine_qos_level(retain);
        let client = self.client.lock().await;
        client
            .publish_with_properties(topic, qos, retain, payload, PublishProperties::default())
            .await
            .map_err(|e| MqttError::PublishFailed(Box::new(e)))?;

        Ok(())
    }

    fn set_task_sender(&self, sender: mpsc::Sender<TaskEnvelopeWrapper>) {
        // Use async runtime to handle the async method call
        let message_forwarder = self.message_forwarder.clone();
        tokio::spawn(async move {
            let mut forwarder = message_forwarder.lock().await;
            forwarder.set_task_sender(sender);
        });
    }
}
impl Drop for MqttClient {
    fn drop(&mut self) {
        // Signal shutdown to background tasks if they're still running
        if let Some(shutdown_tx) = &self.shutdown_tx {
            let _ = shutdown_tx.send(true);
        }

        // Abort the event loop task if it's still running
        if let Some(handle) = self.event_loop_handle.take() {
            handle.abort();
        }

        // Note: We can't do async operations in Drop, so we can't call disconnect()
        // Users should call disconnect() explicitly for graceful shutdown
        // This Drop implementation only ensures background tasks are cleaned up
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::Duration;

    #[test]
    fn test_setup_connection_channels() {
        // Act: Create channels using pure function
        let ((state_tx, state_rx), (shutdown_tx, shutdown_rx)) =
            MqttClient::setup_connection_channels();

        // Assert: Verify initial states
        assert_eq!(*state_rx.borrow(), ConnectionState::Connecting);
        assert!(!(*shutdown_rx.borrow()));

        // Test that channels work
        state_tx.send(ConnectionState::Connected).unwrap();
        assert_eq!(*state_rx.borrow(), ConnectionState::Connected);

        shutdown_tx.send(true).unwrap();
        assert!(*shutdown_rx.borrow());
    }

    #[tokio::test]
    async fn test_wait_for_connection_confirmation_success() {
        // Arrange: Create channels and spawn task to signal connected
        let ((state_tx, state_rx), (_, _)) = MqttClient::setup_connection_channels();

        // Spawn task to signal connection after delay
        let state_tx_clone = state_tx.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = state_tx_clone.send(ConnectionState::Connected);
        });

        // Act: Wait for connection with timeout
        let result =
            MqttClient::wait_for_connection_confirmation(state_rx, Duration::from_millis(100))
                .await;

        // Assert: Should succeed
        assert!(result.is_ok(), "Should successfully wait for connection");
    }

    #[tokio::test]
    async fn test_wait_for_connection_confirmation_timeout() {
        // Arrange: Create channels but don't signal connection
        // CRITICAL: Keep state_tx alive so channel doesn't close
        let ((state_tx, state_rx), (_, _)) = MqttClient::setup_connection_channels();

        // Spawn task that keeps the channel open but never signals
        let _handle = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            drop(state_tx); // Keep sender alive during test
        });

        // Act: Wait for connection with short timeout
        let result =
            MqttClient::wait_for_connection_confirmation(state_rx, Duration::from_millis(10)).await;

        // Assert: Should timeout
        assert!(result.is_err(), "Should timeout when no connection signal");
        // Error format: "Connection failed: ConnAck timeout - no connection confirmation received"
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("ConnAck") || err_msg.contains("timeout"),
            "Error should mention timeout or ConnAck, got: {err_msg}"
        );
    }

    #[tokio::test]
    async fn test_wait_for_connection_confirmation_disconnected() {
        // Arrange: Create channels and signal disconnection
        let ((state_tx, state_rx), (_, _)) = MqttClient::setup_connection_channels();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = state_tx.send(ConnectionState::Disconnected("Test disconnect".to_string()));
        });

        // Act: Wait for connection
        let result =
            MqttClient::wait_for_connection_confirmation(state_rx, Duration::from_millis(100))
                .await;

        // Assert: Should return error
        assert!(result.is_err(), "Should fail when disconnected");
        assert!(result.unwrap_err().to_string().contains("Test disconnect"));
    }

    #[tokio::test]
    async fn test_interruptible_sleep_completes() {
        // Arrange: Create shutdown channel
        let ((_, _), (_, shutdown_rx)) = MqttClient::setup_connection_channels();

        // Act: Sleep without interruption
        let result = MqttClient::interruptible_sleep(shutdown_rx, 10).await;

        // Assert: Should complete normally
        assert!(result, "Sleep should complete without interruption");
    }

    #[tokio::test]
    async fn test_interruptible_sleep_interrupted() {
        // Arrange: Create shutdown channel and signal shutdown
        let ((_, _), (shutdown_tx, shutdown_rx)) = MqttClient::setup_connection_channels();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(5)).await;
            let _ = shutdown_tx.send(true);
        });

        // Act: Sleep with interruption
        let result = MqttClient::interruptible_sleep(shutdown_rx, 100).await;

        // Assert: Should be interrupted
        assert!(!result, "Sleep should be interrupted by shutdown signal");
    }

    #[tokio::test]
    async fn test_connection_state_before_connect() {
        // Arrange: Create client without connecting
        let config = crate::config::MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        };
        let client = MqttClient::new("test-agent-state", config).await.unwrap();

        // Act: Query state
        let state = client.connection_state();

        // Assert: Should return None (not connected)
        assert!(state.is_none(), "State should be None before connect()");
    }

    #[tokio::test]
    async fn test_is_permanently_disconnected_initial_state() {
        // Arrange: Create disconnected client
        let config = crate::config::MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        };
        let client = MqttClient::new("test-agent-perm", config).await.unwrap();

        // Act: Check if permanently disconnected
        let is_perm_disconnected = client.is_permanently_disconnected();

        // Assert: Should not be permanently disconnected initially
        assert!(
            !is_perm_disconnected,
            "Should not be permanently disconnected on creation"
        );
    }

    #[tokio::test]
    async fn test_get_health_metrics_initial_state() {
        // Arrange: Create new client
        let config = crate::config::MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        };
        let client = MqttClient::new("test-agent-health", config).await.unwrap();

        // Act: Get health metrics
        let metrics = client.get_health_metrics();

        // Assert: Initial values
        assert_eq!(
            metrics.uptime, None,
            "Uptime should be None before connection"
        );
        assert_eq!(
            metrics.time_since_last_message, None,
            "Message time should be None initially"
        );
        assert_eq!(metrics.reconnect_count, 0, "Reconnect count should be 0");
    }

    #[tokio::test]
    async fn test_publish_operations_fail_without_connection() {
        // Arrange: Create client without connecting
        let config = crate::config::MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        };
        let client = MqttClient::new("test-agent-publish-fail", config)
            .await
            .unwrap();

        // Create test messages
        let status = crate::protocol::AgentStatus {
            agent_id: "test-agent".to_string(),
            status: crate::protocol::AgentStatusType::Available,
            timestamp: chrono::Utc::now(),
            capabilities: None,
            description: None,
        };

        let task = crate::protocol::TaskEnvelope {
            task_id: uuid::Uuid::new_v4(),
            conversation_id: "test-conv".to_string(),
            topic: "/test/topic".to_string(),
            instruction: Some("test".to_string()),
            input: serde_json::json!({}),
            next: None,
        };

        let error_msg = crate::protocol::ErrorMessage {
            task_id: uuid::Uuid::new_v4(),
            error: crate::protocol::ErrorDetails {
                code: crate::protocol::ErrorCode::InternalError,
                message: "test error".to_string(),
            },
        };

        // Act & Assert: All publish operations should fail
        assert!(
            client.publish_status(&status).await.is_err(),
            "publish_status should fail without connection"
        );
        assert!(
            client.publish_task("target-agent", &task).await.is_err(),
            "publish_task should fail without connection"
        );
        assert!(
            client
                .publish_error("/test/topic", &error_msg)
                .await
                .is_err(),
            "publish_error should fail without connection"
        );
    }

    #[tokio::test]
    async fn test_disconnect_without_connection() {
        // Arrange: Create client that was never connected
        let config = crate::config::MqttSection {
            broker_url: "mqtt://localhost:1883".to_string(),
            username_env: None,
            password_env: None,
            heartbeat_interval_secs: 900,
        };
        let mut client = MqttClient::new("test-agent-disc", config).await.unwrap();

        // Act: Attempt to disconnect
        let result = client.disconnect().await;

        // Assert: Should succeed (no-op is acceptable)
        assert!(
            result.is_ok(),
            "Disconnect should not fail even if not connected"
        );
    }
}

//! Transport layer for agent communication
//!
//! This module provides transport abstraction and MQTT implementation
//! for agent-to-agent communication and control messaging.

use crate::protocol::{
    AgentStatus, ErrorMessage, ResponseMessage, TaskEnvelope, TaskEnvelopeWrapper,
};

pub mod mqtt;

/// Transport trait for agent communication
///
/// This trait provides an abstraction over different transport mechanisms
/// (primarily MQTT) to enable dependency injection and testing.
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Connect to the transport broker/server
    async fn connect(&mut self) -> Result<(), Self::Error>;

    /// Disconnect from the transport broker/server
    async fn disconnect(&mut self) -> Result<(), Self::Error>;

    /// Publish agent status message
    async fn publish_status(&self, status: &AgentStatus) -> Result<(), Self::Error>;

    /// Publish task to another agent
    async fn publish_task(
        &self,
        target_agent: &str,
        envelope: &TaskEnvelope,
    ) -> Result<(), Self::Error>;

    /// Publish error message to conversation topic
    async fn publish_error(
        &self,
        conversation_id: &str,
        error: &ErrorMessage,
    ) -> Result<(), Self::Error>;

    /// Publish response message to conversation topic
    async fn publish_response(
        &self,
        conversation_id: &str,
        response: &ResponseMessage,
    ) -> Result<(), Self::Error>;

    /// Subscribe to task input messages for this agent
    async fn subscribe_to_tasks(&mut self) -> Result<(), Self::Error>;

    /// Publish arbitrary message to specified topic (for progress reporting and other generic use cases)
    async fn publish(&self, topic: &str, payload: Vec<u8>, retain: bool)
    -> Result<(), Self::Error>;

    /// Check if transport is currently connected
    fn is_connected(&self) -> bool;

    /// Get current connection state
    fn connection_state(&self) -> Option<crate::transport::mqtt::ConnectionState>;

    /// Check if the connection is permanently disconnected
    fn is_permanently_disconnected(&self) -> bool;

    /// Set the task sender for forwarding received tasks to the pipeline
    /// Supports both v1.0 and v2.0 TaskEnvelope formats via TaskEnvelopeWrapper
    fn set_task_sender(&self, sender: tokio::sync::mpsc::Sender<TaskEnvelopeWrapper>);
}

/// Type alias for MQTT transport
pub type MqttTransport = mqtt::MqttClient;

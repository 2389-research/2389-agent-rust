//! Pure message routing and processing logic for MQTT events
//!
//! This module contains pure functions for handling MQTT events,
//! message parsing, and routing decisions.

#[cfg(test)]
use crate::protocol::TaskEnvelope;
use crate::protocol::{AgentStatus, ErrorMessage, ResponseMessage, TaskEnvelopeWrapper};
use rumqttc::v5::{mqttbytes::QoS, Event};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Pure message routing decisions based on MQTT events
pub struct MessageHandler;

impl MessageHandler {
    /// Extract task envelope from MQTT publish message (pure function)
    /// Supports both v1.0 and v2.0 TaskEnvelope formats via auto-detection
    pub fn parse_task_envelope(payload: &[u8]) -> Result<TaskEnvelopeWrapper, String> {
        serde_json::from_slice::<TaskEnvelopeWrapper>(payload)
            .map_err(|e| format!("Failed to parse TaskEnvelope: {e}"))
    }

    /// Determine if message should be processed based on topic and retain flag (pure function)
    pub fn should_process_message(topic: &str, retain: bool, expected_topic: &str) -> bool {
        // RFC requirement: Ignore retained messages to prevent reprocessing
        if retain {
            debug!("Ignoring retained message on topic: {}", topic);
            return false;
        }

        // Check if topic matches expected input topic
        if topic != expected_topic {
            debug!("Topic mismatch: expected {}, got {}", expected_topic, topic);
            return false;
        }

        true
    }

    /// Route MQTT event to appropriate handler (pure routing decision)
    /// Updated for MQTT v5 Event types
    pub fn route_mqtt_event(event: &Event) -> EventRoute {
        match event {
            Event::Incoming(incoming) => {
                use rumqttc::v5::mqttbytes::v5::Packet;
                match incoming {
                    Packet::ConnAck(_) => EventRoute::ConnectionAcknowledged,
                    Packet::Publish(publish) => EventRoute::MessageReceived {
                        topic: String::from_utf8_lossy(&publish.topic).to_string(),
                        payload: publish.payload.to_vec(),
                        retain: publish.retain,
                    },
                    Packet::Disconnect(_) => EventRoute::Disconnected,
                    Packet::SubAck(suback) => EventRoute::SubscriptionConfirmed {
                        packet_id: suback.pkid,
                        return_codes: suback.return_codes.iter().map(|_c| 0x01).collect(), // QoS 1 success for now
                    },
                    other => EventRoute::InfrastructureEvent(format!("{other:?}")),
                }
            }
            Event::Outgoing(_) => EventRoute::OutgoingEvent,
        }
    }

    /// Format response into JSON payload (pure function)
    pub fn format_response_payload(response: &ResponseMessage) -> Result<String, String> {
        serde_json::to_string(response).map_err(|e| format!("Serialization error: {e}"))
    }

    /// Format error into JSON payload (pure function)
    pub fn format_error_payload(error: &ErrorMessage) -> Result<String, String> {
        serde_json::to_string(error).map_err(|e| format!("Serialization error: {e}"))
    }

    /// Format status into JSON payload (pure function)
    pub fn format_status_payload(status: &AgentStatus) -> Result<String, String> {
        serde_json::to_string(status).map_err(|e| format!("Serialization error: {e}"))
    }

    /// Determine QoS level based on message type (pure function)
    pub fn determine_qos_level(retain: bool) -> QoS {
        match retain {
            true => QoS::AtLeastOnce, // Retained messages should use QoS 1 for reliability
            false => QoS::AtMostOnce, // Progress messages can use QoS 0 for performance
        }
    }

    /// Build subscription topics for agent (pure function)
    pub fn build_subscription_topics(agent_id: &str) -> Vec<String> {
        vec![format!("/control/agents/{}/input", agent_id)]
    }

    /// Validate subscription success from SubAck (pure function)
    pub fn validate_subscription_success(return_codes: &[u8]) -> Result<(), String> {
        if return_codes.iter().any(|&code| code >= 0x80) {
            Err(format!(
                "Subscription failed with return codes: {return_codes:?}"
            ))
        } else {
            Ok(())
        }
    }
}

/// Routing decisions for MQTT events
#[derive(Debug, Clone)]
pub enum EventRoute {
    /// Connection acknowledged - ready to publish/subscribe
    ConnectionAcknowledged,
    /// Message received on subscribed topic
    MessageReceived {
        topic: String,
        payload: Vec<u8>,
        retain: bool,
    },
    /// MQTT broker disconnected
    Disconnected,
    /// Subscription confirmed with return codes
    SubscriptionConfirmed {
        packet_id: u16,
        return_codes: Vec<u8>,
    },
    /// Infrastructure event (PingResp, etc.)
    InfrastructureEvent(String),
    /// Outgoing event (handled automatically)
    OutgoingEvent,
}

/// Message forwarding operations (impure I/O)
pub struct MessageForwarder {
    task_sender: Option<mpsc::Sender<TaskEnvelopeWrapper>>,
}

impl MessageForwarder {
    pub fn new() -> Self {
        Self { task_sender: None }
    }

    pub fn set_task_sender(&mut self, sender: mpsc::Sender<TaskEnvelopeWrapper>) {
        self.task_sender = Some(sender);
    }

    /// Forward parsed task envelope to pipeline (impure I/O)
    /// Accepts both v1.0 and v2.0 envelopes and forwards them as-is
    pub async fn forward_task(
        &self,
        task_envelope_wrapper: TaskEnvelopeWrapper,
    ) -> Result<(), String> {
        if let Some(ref sender) = self.task_sender {
            let task_id = task_envelope_wrapper.task_id();
            info!("Forwarding task {} to pipeline", task_id);

            sender
                .send(task_envelope_wrapper)
                .await
                .map_err(|e| format!("Failed to forward task to pipeline: {e}"))?;
            Ok(())
        } else {
            warn!("Received MQTT message but no task sender configured - message dropped");
            Err("No task sender configured".to_string())
        }
    }
}

impl Default for MessageForwarder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AgentStatusType, ErrorCode, ErrorDetails};
    use bytes::Bytes;
    use chrono::Utc;
    use rumqttc::v5::mqttbytes::v5::Publish;
    use serde_json::Value;
    use uuid::Uuid;

    #[test]
    fn test_parse_task_envelope() {
        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test-conversation".to_string(),
            topic: "/control/agents/target/input".to_string(),
            instruction: Some("Test task".to_string()),
            input: serde_json::json!({"test": "data"}),
            next: None,
        };

        let json = serde_json::to_vec(&task).unwrap();
        let parsed = MessageHandler::parse_task_envelope(&json);
        assert!(parsed.is_ok());

        let parsed_task = parsed.unwrap();
        assert_eq!(parsed_task.task_id(), task.task_id);
        assert_eq!(parsed_task.conversation_id(), task.conversation_id);
    }

    #[test]
    fn test_parse_invalid_task_envelope() {
        let invalid_json = b"invalid json";
        let result = MessageHandler::parse_task_envelope(invalid_json);
        assert!(result.is_err());
    }

    #[test]
    fn test_should_process_message() {
        let topic = "/control/agents/test/input";

        // Should process non-retained messages on correct topic
        assert!(MessageHandler::should_process_message(topic, false, topic));

        // Should not process retained messages
        assert!(!MessageHandler::should_process_message(topic, true, topic));

        // Should not process messages on wrong topic
        assert!(!MessageHandler::should_process_message(
            "/wrong/topic",
            false,
            topic
        ));
    }

    #[test]
    fn test_route_mqtt_event() {
        use rumqttc::v5::mqttbytes::v5::{ConnAck, ConnectReturnCode, Disconnect, Packet};

        // Test ConnAck routing
        let connack = Event::Incoming(Packet::ConnAck(ConnAck {
            session_present: false,
            code: ConnectReturnCode::Success,
            properties: None,
        }));
        assert!(matches!(
            MessageHandler::route_mqtt_event(&connack),
            EventRoute::ConnectionAcknowledged
        ));

        // Test Disconnect routing
        let disconnect = Event::Incoming(Packet::Disconnect(Disconnect {
            reason_code: rumqttc::v5::mqttbytes::v5::DisconnectReasonCode::NormalDisconnection,
            properties: None,
        }));
        assert!(matches!(
            MessageHandler::route_mqtt_event(&disconnect),
            EventRoute::Disconnected
        ));

        // Test Publish routing
        let publish = Event::Incoming(Packet::Publish(Publish {
            dup: false,
            qos: QoS::AtLeastOnce,
            retain: false,
            topic: Bytes::from("test/topic"),
            pkid: 1,
            payload: Bytes::from("test payload"),
            properties: None,
        }));

        if let EventRoute::MessageReceived {
            topic,
            payload,
            retain,
        } = MessageHandler::route_mqtt_event(&publish)
        {
            assert_eq!(topic, "test/topic");
            assert_eq!(payload, b"test payload");
            assert!(!retain);
        } else {
            panic!("Expected MessageReceived route");
        }
    }

    #[test]
    fn test_format_payloads() {
        // Test response payload formatting
        let response = ResponseMessage {
            task_id: Uuid::new_v4(),
            response: serde_json::json!({"success": true}).to_string(),
        };
        let payload = MessageHandler::format_response_payload(&response);
        assert!(payload.is_ok());
        assert!(payload.unwrap().contains("task_id"));

        // Test error payload formatting
        let error = ErrorMessage {
            error: ErrorDetails {
                code: ErrorCode::InternalError,
                message: "Test error".to_string(),
            },
            task_id: Uuid::new_v4(),
        };
        let payload = MessageHandler::format_error_payload(&error);
        assert!(payload.is_ok());
        let payload_str = payload.unwrap();
        assert!(payload_str.contains("internal_error")); // Check for snake_case serialization

        // Test status payload formatting
        let status = AgentStatus {
            agent_id: "test-agent".to_string(),
            status: AgentStatusType::Available,
            timestamp: Utc::now(),
            capabilities: None,
            description: None,
        };
        let payload = MessageHandler::format_status_payload(&status);
        assert!(payload.is_ok());
        let payload_str = payload.unwrap();
        assert!(payload_str.contains("available")); // Check for snake_case serialization
    }

    #[test]
    fn test_determine_qos_level() {
        // Retained messages should use QoS 1
        assert_eq!(MessageHandler::determine_qos_level(true), QoS::AtLeastOnce);

        // Non-retained messages can use QoS 0
        assert_eq!(MessageHandler::determine_qos_level(false), QoS::AtMostOnce);
    }

    #[test]
    fn test_build_subscription_topics() {
        let topics = MessageHandler::build_subscription_topics("test-agent");
        assert_eq!(topics, vec!["/control/agents/test-agent/input"]);
    }

    #[test]
    fn test_validate_subscription_success() {
        // Success codes (< 0x80)
        let success_codes = vec![0x00, 0x01, 0x02];
        assert!(MessageHandler::validate_subscription_success(&success_codes).is_ok());

        // Failure codes (>= 0x80)
        let failure_codes = vec![0x80, 0x81];
        assert!(MessageHandler::validate_subscription_success(&failure_codes).is_err());

        // Mixed codes - should fail
        let mixed_codes = vec![0x00, 0x80];
        assert!(MessageHandler::validate_subscription_success(&mixed_codes).is_err());
    }

    #[tokio::test]
    async fn test_message_forwarder() {
        let mut forwarder = MessageForwarder::new();

        let task = TaskEnvelope {
            task_id: Uuid::new_v4(),
            conversation_id: "test".to_string(),
            topic: "/test".to_string(),
            instruction: None,
            input: Value::Null,
            next: None,
        };

        // Should fail without sender
        let result = forwarder
            .forward_task(TaskEnvelopeWrapper::V1(task.clone()))
            .await;
        assert!(result.is_err());

        // Set up sender
        let (tx, mut rx) = mpsc::channel(1);
        forwarder.set_task_sender(tx);

        // Should succeed with sender
        let result = forwarder
            .forward_task(TaskEnvelopeWrapper::V1(task.clone()))
            .await;
        assert!(result.is_ok());

        // Verify task was forwarded
        let received = rx.recv().await;
        assert!(received.is_some());
        let received_wrapper = received.unwrap();
        assert_eq!(received_wrapper.task_id(), task.task_id);
    }
}
